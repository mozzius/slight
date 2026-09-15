pub mod client;
pub mod content;
pub mod metadata;
pub mod session;
pub mod transport;
pub mod wire;

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    ClaudeCode,
    Codex,
    Fake,
    Custom(String),
}

impl AgentKind {
    pub fn as_str(&self) -> &str {
        match self {
            AgentKind::ClaudeCode => "claude_code",
            AgentKind::Codex => "codex",
            AgentKind::Fake => "fake",
            AgentKind::Custom(name) => name.as_str(),
        }
    }
}

impl fmt::Display for AgentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDescriptor {
    pub kind: AgentKind,
    pub display_name: String,
    pub version: Option<String>,
}

impl AgentDescriptor {
    pub fn new(kind: AgentKind, display_name: impl Into<String>, version: Option<String>) -> Self {
        Self {
            kind,
            display_name: display_name.into(),
            version,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ToolCallUpdate {
    pub tool_call_id: String,
    pub title: String,
    pub kind: Option<String>,
    pub status: ToolCallStatus,
    pub detail: Option<String>,
    /// Rich content produced by the tool call (text, media, diffs, terminals).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<content::NormalizedToolContent>,
    /// File locations touched by the tool call.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<content::ToolLocation>,
    /// Raw, untyped input sent to the tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_input: Option<serde_json::Value>,
    /// Raw, untyped output returned by the tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_output: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionOptionKind {
    Allow,
    AllowAlways,
    Deny,
    DenyAlways,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PermissionOption {
    pub option_id: String,
    pub label: String,
    pub kind: PermissionOptionKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PermissionRequest {
    pub permission_id: String,
    pub title: String,
    pub detail: Option<String>,
    pub tool_call_id: Option<String>,
    pub options: Vec<PermissionOption>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticLevel {
    Debug,
    Info,
    Warning,
    Error,
}

/// The legacy normalized event model. It predates `session::AgentEvent` and is
/// still produced by the in-tree adapter surface; prefer `session::AgentEvent`
/// for new code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AcpEvent {
    Message {
        role: MessageRole,
        text: String,
    },
    ToolCall(ToolCallUpdate),
    PermissionRequest(PermissionRequest),
    Diagnostics {
        level: DiagnosticLevel,
        message: String,
    },
    /// The agent finished a prompt turn. `stop_reason` is the ACP stop reason
    /// (for example `end_turn` or `cancelled`).
    TurnEnded {
        stop_reason: String,
    },
    Exited {
        code: Option<i32>,
        reason: String,
    },
}

/// The legacy command model. Prefer the typed methods on
/// `session::AcpSession` for new code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AcpCommand {
    Prompt {
        text: String,
    },
    Cancel,
    RespondPermission {
        permission_id: String,
        option_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpError(pub String);

impl AcpError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for AcpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AcpError {}

impl From<wire::WireError> for AcpError {
    fn from(error: wire::WireError) -> Self {
        AcpError(error.to_string())
    }
}

impl From<std::io::Error> for AcpError {
    fn from(error: std::io::Error) -> Self {
        AcpError(error.to_string())
    }
}

/// The legacy, non-blocking connection surface still used by the adapter
/// layer. New session code should depend on `session::AcpSession` instead.
pub trait AcpConnection: Send {
    fn send(&mut self, command: AcpCommand) -> Result<(), AcpError>;
    fn try_recv(&mut self) -> Result<Vec<AcpEvent>, AcpError>;
    fn is_running(&self) -> bool;
}
