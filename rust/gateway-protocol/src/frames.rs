use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: u32 = 1;

fn default_protocol_version() -> u32 {
    PROTOCOL_VERSION
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Frame {
    Hello(ClientHello),
    Welcome(ServerWelcome),
    Command(GatewayCommand),
    Ack(GatewayAck),
    Event(GatewayEvent),
    Ping(PingFrame),
    Pong(PongFrame),
    ResyncRequired(ResyncRequired),
    Error(GatewayProtocolError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ResumeRequest {
    pub session_id: Option<String>,
    pub last_event_sequence: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ClientHello {
    #[serde(rename = "v", default = "default_protocol_version")]
    pub protocol_version: u32,
    pub client_id: String,
    #[serde(default = "default_client_name")]
    pub client_name: String,
    #[serde(default = "default_client_version")]
    pub client_version: String,
    pub device_id: Option<String>,
    pub credential: Option<String>,
    pub resume: Option<ResumeRequest>,
}

fn default_client_name() -> String {
    "unknown".to_string()
}

fn default_client_version() -> String {
    "0".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ServerCapabilities {
    pub supports_replay: bool,
    pub max_replay_events: Option<usize>,
    pub max_event_journal: Option<usize>,
    pub supports_raw_acp: bool,
    pub permission_options: bool,
    pub host_admin: bool,
}

impl Default for ServerCapabilities {
    fn default() -> Self {
        Self {
            supports_replay: true,
            max_replay_events: None,
            max_event_journal: None,
            supports_raw_acp: false,
            permission_options: true,
            host_admin: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ServerWelcome {
    #[serde(rename = "v", default = "default_protocol_version")]
    pub protocol_version: u32,
    pub connection_id: String,
    pub server_name: String,
    pub server_version: String,
    pub host_id: Option<String>,
    #[serde(default)]
    pub capabilities: ServerCapabilities,
    pub heartbeat_interval_ms: Option<u64>,
    #[serde(default)]
    pub resync_required: bool,
    pub resync_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GatewayCommand {
    #[serde(rename = "v", default = "default_protocol_version")]
    pub protocol_version: u32,
    pub request_id: String,
    pub command: String,
    pub session_id: Option<String>,
    pub params: Option<Value>,
}

impl GatewayCommand {
    pub fn new(request_id: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.into(),
            command: command.into(),
            session_id: None,
            params: None,
        }
    }

    pub fn for_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    pub fn with_params<T: Serialize>(mut self, params: &T) -> Self {
        self.params = serde_json::to_value(params).ok();
        self
    }

    pub fn params_as<T: serde::de::DeserializeOwned>(&self) -> Result<T, crate::ProtocolError> {
        let value = self.params.clone().unwrap_or(Value::Null);
        serde_json::from_value(value)
            .map_err(|error| crate::ProtocolError::InvalidParams(error.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GatewayAck {
    #[serde(rename = "v", default = "default_protocol_version")]
    pub protocol_version: u32,
    pub request_id: String,
    pub ok: bool,
    pub result: Option<Value>,
    pub error: Option<super::GatewayErrorBody>,
}

impl GatewayAck {
    pub fn ok<T: Serialize>(request_id: impl Into<String>, result: &T) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.into(),
            ok: true,
            result: serde_json::to_value(result).ok(),
            error: None,
        }
    }

    pub fn unit(request_id: impl Into<String>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.into(),
            ok: true,
            result: Some(Value::Null),
            error: None,
        }
    }

    pub fn error(request_id: impl Into<String>, body: super::GatewayErrorBody) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.into(),
            ok: false,
            result: None,
            error: Some(body),
        }
    }

    pub fn result_as<T: serde::de::DeserializeOwned>(&self) -> Result<T, crate::ProtocolError> {
        let value = self.result.clone().unwrap_or(Value::Null);
        serde_json::from_value(value)
            .map_err(|error| crate::ProtocolError::MalformedFrame(error.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GatewayEvent {
    #[serde(rename = "v", default = "default_protocol_version")]
    pub protocol_version: u32,
    pub session_id: String,
    pub sequence: u64,
    pub event: String,
    pub at: Option<String>,
    pub payload: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PingFrame {
    #[serde(rename = "v", default = "default_protocol_version")]
    pub protocol_version: u32,
    pub nonce: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PongFrame {
    #[serde(rename = "v", default = "default_protocol_version")]
    pub protocol_version: u32,
    pub nonce: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ResyncRequired {
    #[serde(rename = "v", default = "default_protocol_version")]
    pub protocol_version: u32,
    pub reason: String,
    pub session_id: Option<String>,
    pub oldest_available_sequence: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GatewayProtocolError {
    #[serde(rename = "v", default = "default_protocol_version")]
    pub protocol_version: u32,
    pub code: String,
    pub message: String,
    pub request_id: Option<String>,
    #[serde(default)]
    pub retryable: bool,
}

impl Frame {
    pub fn frame_type(&self) -> &str {
        match self {
            Frame::Hello(_) => "hello",
            Frame::Welcome(_) => "welcome",
            Frame::Command(_) => "command",
            Frame::Ack(_) => "ack",
            Frame::Event(_) => "event",
            Frame::Ping(_) => "ping",
            Frame::Pong(_) => "pong",
            Frame::ResyncRequired(_) => "resync_required",
            Frame::Error(_) => "error",
        }
    }
}
