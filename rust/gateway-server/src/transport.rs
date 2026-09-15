//! WebSocket transport helpers shared by the gateway server and client.
//!
//! The gateway-v1 frame union is transport-independent: each frame is encoded
//! as JSON. `URLSessionWebSocketTask` sends `.data`, which arrives as a binary
//! WebSocket message, so both text and binary messages are accepted as long as
//! the payload is UTF-8 JSON. Responses are always sent as text.

use gateway_protocol::{decode_text, encode_text, Frame, ProtocolError};
use std::io;
use tungstenite::Message;

pub fn encode_message(frame: &Frame) -> Result<Message, ProtocolError> {
    Ok(Message::text(encode_text(frame)?))
}

/// Decode a frame from a text or binary WebSocket payload. Binary payloads are
/// accepted because Apple clients send JSON as `.data` rather than `.string`.
pub fn decode_payload(payload: &[u8]) -> Result<Frame, ProtocolError> {
    let text = std::str::from_utf8(payload).map_err(|error| {
        ProtocolError::MalformedFrame(format!("websocket payload is not valid utf-8: {error}"))
    })?;
    decode_text(text)
}

/// A non-fatal read/write timeout. The server polls with a short socket read
/// timeout so it can also service event subscriptions and heartbeats on the
/// same thread.
pub fn is_would_block(error: &tungstenite::Error) -> bool {
    match error {
        tungstenite::Error::Io(error) => matches!(
            error.kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted
        ),
        _ => false,
    }
}

pub fn is_closed(error: &tungstenite::Error) -> bool {
    matches!(
        error,
        tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed
    )
}
