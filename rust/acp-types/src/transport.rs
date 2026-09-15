//! Newline-delimited stdio framing for the ACP transport.
//!
//! ACP is JSON-RPC 2.0 carried over a byte stream where every message is a
//! single JSON object on one line terminated by `\n`. This module owns that
//! framing only: it never inspects method names and never translates between
//! ACP and the Slight `gateway-v1` contract. Both the client side (the host
//! driving an agent) and the agent side (the fake agent in tests) build on it.

use crate::wire::{self, AcpMessage, RawMessage, WireError};
use serde::Serialize;
use std::io::{BufRead, Write};

/// Upper bound on a single JSON frame, to keep a misbehaving peer from forcing
/// unbounded buffering. ACP messages are small; this is far above any real one.
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

/// Reads newline-delimited JSON frames from an underlying buffered reader.
#[derive(Debug)]
pub struct FrameReader<R> {
    inner: R,
}

impl<R: BufRead> FrameReader<R> {
    pub fn new(inner: R) -> Self {
        Self { inner }
    }

    /// Reads the next non-empty frame, or `None` at end of stream.
    ///
    /// The trailing newline (and an optional `\r`) is stripped so the returned
    /// string is the bare JSON object.
    pub fn read_frame(&mut self) -> Result<Option<String>, WireError> {
        loop {
            let mut line = String::new();
            let read = self.inner.read_line(&mut line)?;
            if read == 0 {
                return Ok(None);
            }
            if line.len() > MAX_FRAME_BYTES {
                return Err(WireError::Malformed(format!(
                    "frame exceeds {MAX_FRAME_BYTES} bytes"
                )));
            }
            let trimmed = line.trim_end_matches(['\n', '\r']);
            if trimmed.is_empty() {
                continue;
            }
            return Ok(Some(trimmed.to_string()));
        }
    }

    pub fn into_inner(self) -> R {
        self.inner
    }
}

/// Writes newline-terminated JSON frames to an underlying writer.
#[derive(Debug)]
pub struct FrameWriter<W> {
    inner: W,
}

impl<W: Write> FrameWriter<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }

    /// Writes an already-encoded JSON object as one frame. Exactly one trailing
    /// newline is emitted even if `frame` already carries one.
    ///
    /// The body and its terminating newline are written in a single
    /// `write_all`, so a writer shared between threads (see
    /// [`crate::session::SharedWriter`]) cannot interleave two frames.
    pub fn write_encoded(&mut self, frame: &[u8]) -> Result<(), WireError> {
        let mut end = frame.len();
        while end > 0 && matches!(frame[end - 1], b'\n' | b'\r') {
            end -= 1;
        }
        let mut framed = Vec::with_capacity(end + 1);
        framed.extend_from_slice(&frame[..end]);
        framed.push(b'\n');
        self.inner.write_all(&framed)?;
        self.inner.flush()?;
        Ok(())
    }

    /// Serializes and writes a single frame.
    pub fn write_message<T: Serialize>(&mut self, message: &T) -> Result<(), WireError> {
        self.write_encoded(&serde_json::to_vec(message)?)
    }

    pub fn into_inner(self) -> W {
        self.inner
    }
}

/// A bidirectional newline-delimited JSON transport.
///
/// This is intentionally transport-agnostic: it works over an in-process pipe
/// pair in unit tests, over a child process's `stdin`/`stdout` in production,
/// and (later) over any other byte stream without changing the protocol code.
#[derive(Debug)]
pub struct Transport<R, W> {
    reader: FrameReader<R>,
    writer: FrameWriter<W>,
}

impl<R: BufRead, W: Write> Transport<R, W> {
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader: FrameReader::new(reader),
            writer: FrameWriter::new(writer),
        }
    }

    pub fn split(self) -> (FrameReader<R>, FrameWriter<W>) {
        (self.reader, self.writer)
    }

    /// Writes raw encoded bytes as one frame.
    pub fn send_raw(&mut self, frame: &[u8]) -> Result<(), WireError> {
        self.writer.write_encoded(frame)
    }

    /// Serializes and writes a single frame.
    pub fn send<T: Serialize>(&mut self, message: &T) -> Result<(), WireError> {
        self.writer.write_message(message)
    }

    /// Reads the next frame as a string, or `None` at end of stream.
    pub fn recv_line(&mut self) -> Result<Option<String>, WireError> {
        self.reader.read_frame()
    }

    /// Reads and structurally classifies the next frame without direction bias.
    pub fn recv_raw(&mut self) -> Result<Option<RawMessage>, WireError> {
        match self.recv_line()? {
            Some(line) => Ok(Some(wire::classify(&line)?)),
            None => Ok(None),
        }
    }

    /// Reads and routes the next frame as seen by the host (the ACP client).
    pub fn recv_inbound(&mut self) -> Result<Option<AcpMessage>, WireError> {
        match self.recv_line()? {
            Some(line) => Ok(Some(wire::classify_inbound(&line)?)),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{RequestId, AGENT_METHOD_NAMES};

    #[test]
    fn frames_round_trip_over_a_pipe() {
        let (client_read, mut agent_write) = std::io::pipe().unwrap();
        let (agent_read, client_write) = std::io::pipe().unwrap();

        let mut writer = FrameWriter::new(client_write);
        writer
            .write_encoded(br#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#)
            .unwrap();

        let mut reader = FrameReader::new(std::io::BufReader::new(agent_read));
        let line = reader.read_frame().unwrap().unwrap();
        assert_eq!(line, r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#);

        // The agent replies on the other pipe.
        agent_write
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n")
            .unwrap();
        agent_write.flush().unwrap();
        let mut client_reader = FrameReader::new(std::io::BufReader::new(client_read));
        assert_eq!(
            client_reader.read_frame().unwrap().unwrap(),
            r#"{"jsonrpc":"2.0","id":1,"result":{}}"#
        );
    }

    #[test]
    fn skips_blank_lines_and_handles_crlf() {
        let input: &[u8] = b"\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\r\n";
        let mut reader = FrameReader::new(std::io::BufReader::new(input));
        assert_eq!(
            reader.read_frame().unwrap().unwrap(),
            r#"{"jsonrpc":"2.0","id":1,"result":{}}"#
        );
        assert!(reader.read_frame().unwrap().is_none());
    }

    #[test]
    fn write_normalizes_double_newline() {
        let mut buffer = Vec::new();
        let mut writer = FrameWriter::new(&mut buffer);
        writer.write_encoded(b"{\"a\":1}\n\n").unwrap();
        assert_eq!(buffer, b"{\"a\":1}\n");
    }

    #[test]
    fn recv_raw_and_recv_inbound_route_messages() {
        let lines = concat!(
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"session/prompt\",\"params\":null}\n",
            "{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"s\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"hi\"}}}}\n",
        );
        let mut transport = Transport::new(std::io::BufReader::new(lines.as_bytes()), Vec::new());

        match transport.recv_raw().unwrap().unwrap() {
            RawMessage::Request { id, method, .. } => {
                assert_eq!(id, RequestId::Number(1));
                assert_eq!(method, AGENT_METHOD_NAMES.session_prompt);
            }
            other => panic!("expected request, got {other:?}"),
        }

        match transport.recv_inbound().unwrap().unwrap() {
            AcpMessage::Notification(_) => {}
            other => panic!("expected notification, got {other:?}"),
        }
        assert!(transport.recv_line().unwrap().is_none());
    }
}
