use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod admin;
pub mod commands;
pub mod dto;
pub mod events;
pub mod frames;
pub mod framing;
pub mod time;

pub use admin::*;
pub use commands::*;
pub use dto::*;
pub use events::*;
pub use frames::*;
pub use framing::{decode_line, decode_text, encode_line, encode_text, write_frame};
pub use session_core::{SessionRecovery, SessionRecoveryState, SessionStatus};

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("malformed frame: {0}")]
    MalformedFrame(String),
    #[error("unsupported protocol version {requested}; supported version is {supported}")]
    InvalidVersion { requested: u32, supported: u32 },
    #[error("authentication failed: {0}")]
    Unauthenticated(String),
    #[error("not authorized: {0}")]
    Unauthorized(String),
    #[error("unknown session: {0}")]
    UnknownSession(String),
    #[error("agent does not support {0}")]
    UnsupportedCapability(String),
    #[error("resync required: {0}")]
    ResyncRequired(String),
    #[error("unsupported command: {0}")]
    UnsupportedCommand(String),
    #[error("invalid params: {0}")]
    InvalidParams(String),
    #[error("invalid frame: {0}")]
    InvalidFrame(String),
    #[error("internal error: {0}")]
    Internal(String),
}

pub mod error_code {
    pub const MALFORMED_FRAME: &str = "malformed_frame";
    pub const INVALID_VERSION: &str = "invalid_version";
    pub const UNAUTHENTICATED: &str = "unauthenticated";
    pub const UNAUTHORIZED: &str = "unauthorized";
    pub const UNKNOWN_SESSION: &str = "unknown_session";
    pub const UNSUPPORTED_CAPABILITY: &str = "unsupported_capability";
    pub const RESYNC_REQUIRED: &str = "resync_required";
    pub const INVALID_COMMAND: &str = "invalid_command";
    pub const UNSUPPORTED: &str = "unsupported";
    pub const INVALID_PARAMS: &str = "invalid_params";
    pub const INTERNAL: &str = "internal";
}

impl ProtocolError {
    pub fn code(&self) -> &'static str {
        match self {
            ProtocolError::MalformedFrame(_) => error_code::MALFORMED_FRAME,
            ProtocolError::InvalidVersion { .. } => error_code::INVALID_VERSION,
            ProtocolError::Unauthenticated(_) => error_code::UNAUTHENTICATED,
            ProtocolError::Unauthorized(_) => error_code::UNAUTHORIZED,
            ProtocolError::UnknownSession(_) => error_code::UNKNOWN_SESSION,
            ProtocolError::UnsupportedCapability(_) => error_code::UNSUPPORTED_CAPABILITY,
            ProtocolError::ResyncRequired(_) => error_code::RESYNC_REQUIRED,
            ProtocolError::UnsupportedCommand(_) => error_code::UNSUPPORTED,
            ProtocolError::InvalidParams(_) => error_code::INVALID_PARAMS,
            ProtocolError::InvalidFrame(_) => error_code::INVALID_COMMAND,
            ProtocolError::Internal(_) => error_code::INTERNAL,
        }
    }

    pub fn retryable(&self) -> bool {
        matches!(self, ProtocolError::Internal(_))
    }

    pub fn body(&self) -> GatewayErrorBody {
        GatewayErrorBody {
            code: self.code().to_string(),
            message: self.to_string(),
            retryable: self.retryable(),
        }
    }

    pub fn to_wire(&self, request_id: Option<String>) -> GatewayProtocolError {
        GatewayProtocolError {
            protocol_version: frames::PROTOCOL_VERSION,
            code: self.code().to_string(),
            message: self.to_string(),
            request_id,
            retryable: self.retryable(),
        }
    }
}

impl From<serde_json::Error> for ProtocolError {
    fn from(error: serde_json::Error) -> Self {
        ProtocolError::MalformedFrame(error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GatewayErrorBody {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub retryable: bool,
}
