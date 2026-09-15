use crate::ProtocolError;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::{self, Write};

pub fn encode_line<T: Serialize>(frame: &T) -> Result<Vec<u8>, ProtocolError> {
    let mut bytes = serde_json::to_vec(frame)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub fn decode_line<T: DeserializeOwned>(line: &str) -> Result<T, ProtocolError> {
    serde_json::from_str(line).map_err(|error| ProtocolError::MalformedFrame(error.to_string()))
}

/// Encode a frame as a single JSON text payload. This is the framing used by
/// WebSocket transports, where the message boundary replaces the newline
/// terminator of the legacy line transport.
pub fn encode_text<T: Serialize>(frame: &T) -> Result<String, ProtocolError> {
    serde_json::to_string(frame).map_err(ProtocolError::from)
}

/// Decode a frame from a JSON text payload (for example one WebSocket text
/// message). Trailing whitespace is tolerated, but the payload must be a single
/// JSON object matching a known frame.
pub fn decode_text<T: DeserializeOwned>(text: &str) -> Result<T, ProtocolError> {
    serde_json::from_str(text).map_err(|error| ProtocolError::MalformedFrame(error.to_string()))
}

pub fn write_frame<W: Write, T: Serialize>(writer: &mut W, frame: &T) -> io::Result<()> {
    let bytes = encode_line(frame)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    writer.write_all(&bytes)
}
