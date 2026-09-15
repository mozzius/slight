use acp_types::content::{NormalizedContent, NormalizedToolContent, ResourceContent, ToolLocation};
use acp_types::metadata::{
    AcpConfigOption, AcpCost, AcpModeState, AgentCapabilitiesSummary, AgentMetadataSummary,
    AuthMethodSummary, AvailableCommandSummary, PlanEntrySummary,
};
use acp_types::{
    AgentDescriptor, AgentKind, DiagnosticLevel, MessageRole, PermissionOption, ToolCallStatus,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub type SessionId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Idle,
    Working,
    WaitingPermission,
    Exited,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    Text,
    Code,
    Reasoning,
    ToolResult,
    Image,
    Audio,
    ResourceLink,
    ResourceText,
    ResourceBlob,
}

/// A normalized content block safe to send to a client.
///
/// Media and resource blocks carry base64 payloads and always label them with a
/// MIME type so clients can decide whether they understand the format. Resource
/// links carry a URI but the host never resolves or fetches it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentBlock {
    pub id: String,
    pub kind: ContentKind,
    pub text: String,
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
}

impl ContentBlock {
    fn new(kind: ContentKind, text: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            text: text.into(),
            language: None,
            mime_type: None,
            data_base64: None,
            uri: None,
            name: None,
            title: None,
            description: None,
            size: None,
        }
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self::new(ContentKind::Text, text)
    }

    pub fn reasoning(text: impl Into<String>) -> Self {
        Self::new(ContentKind::Reasoning, text)
    }

    /// Flattens a normalized agent content block into the gateway payload.
    pub fn from_normalized(content: &NormalizedContent) -> Self {
        match content {
            NormalizedContent::Text { text } => Self::new(ContentKind::Text, text.clone()),
            NormalizedContent::Image {
                data_base64,
                mime_type,
                uri,
            } => {
                let mut block = Self::new(ContentKind::Image, "");
                block.mime_type = Some(mime_type.clone());
                block.data_base64 = Some(data_base64.clone());
                block.uri = uri.clone();
                block
            }
            NormalizedContent::Audio {
                data_base64,
                mime_type,
            } => {
                let mut block = Self::new(ContentKind::Audio, "");
                block.mime_type = Some(mime_type.clone());
                block.data_base64 = Some(data_base64.clone());
                block
            }
            NormalizedContent::ResourceLink(link) => {
                let mut block = Self::new(ContentKind::ResourceLink, "");
                block.uri = Some(link.uri.clone());
                block.name = Some(link.name.clone());
                block.mime_type = link.mime_type.clone();
                block.title = link.title.clone();
                block.description = link.description.clone();
                block.size = link.size;
                block
            }
            NormalizedContent::Resource(ResourceContent::Text {
                uri,
                text,
                mime_type,
            }) => {
                let mut block = Self::new(ContentKind::ResourceText, text.clone());
                block.uri = Some(uri.clone());
                block.mime_type = mime_type.clone();
                block
            }
            NormalizedContent::Resource(ResourceContent::Blob {
                uri,
                data_base64,
                mime_type,
            }) => {
                let mut block = Self::new(ContentKind::ResourceBlob, "");
                block.uri = Some(uri.clone());
                block.mime_type = mime_type.clone();
                block.data_base64 = Some(data_base64.clone());
                block
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatusPayload {
    pub status: SessionStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessagePayload {
    pub role: MessageRole,
    pub text: String,
    pub blocks: Vec<ContentBlock>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallPayload {
    pub id: String,
    pub title: String,
    pub kind: Option<String>,
    pub status: ToolCallStatus,
    pub detail: Option<String>,
    /// Rich tool output: content blocks, diffs, and terminal references.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<ToolCallContentPayload>,
    /// File locations the tool touched, for follow-along UIs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<ToolCallLocationPayload>,
    /// Raw, untyped tool input, kept for an explicit debug/structured view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_input: Option<Value>,
    /// Raw, untyped tool result, kept for an explicit debug/structured view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_output: Option<Value>,
}

/// One piece of rich tool-call output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolCallContentPayload {
    /// A standard content block (text, image, audio, or resource).
    Content(Box<ContentBlock>),
    /// A file modification shown as a diff.
    Diff {
        path: String,
        old_text: Option<String>,
        new_text: String,
    },
    /// A reference to a terminal that produced output.
    Terminal { terminal_id: String },
}

impl ToolCallContentPayload {
    pub fn from_normalized(content: &NormalizedToolContent) -> Self {
        match content {
            NormalizedToolContent::Content(block) => {
                Self::Content(Box::new(ContentBlock::from_normalized(block)))
            }
            NormalizedToolContent::Diff(diff) => Self::Diff {
                path: diff.path.clone(),
                old_text: diff.old_text.clone(),
                new_text: diff.new_text.clone(),
            },
            NormalizedToolContent::Terminal(terminal) => Self::Terminal {
                terminal_id: terminal.terminal_id.clone(),
            },
        }
    }
}

/// A file location touched by a tool call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ToolCallLocationPayload {
    pub path: String,
    pub line: Option<u32>,
}

impl From<&ToolLocation> for ToolCallLocationPayload {
    fn from(location: &ToolLocation) -> Self {
        Self {
            path: location.path.clone(),
            line: location.line,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionRequestPayload {
    pub id: String,
    pub title: String,
    pub detail: Option<String>,
    pub tool_call_id: Option<String>,
    pub options: Vec<PermissionOption>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionResolvedPayload {
    pub id: String,
    pub option_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExitPayload {
    pub code: Option<i32>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticsPayload {
    pub level: DiagnosticLevel,
    pub message: String,
}

/// Final outcome of a prompt turn. `stop_reason` is the conformant ACP stop
/// reason string (`end_turn`, `max_tokens`, `max_turn_requests`, `refusal`,
/// `cancelled`, or a future value).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnEndedPayload {
    pub stop_reason: String,
    pub reasoning_started_at_ms: Option<i64>,
    pub reasoning_ended_at_ms: Option<i64>,
}

/// A full execution-plan replacement from the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanPayload {
    pub entries: Vec<PlanEntrySummary>,
}

/// The agent switched its current session mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModePayload {
    pub current_mode_id: String,
}

/// The agent's available slash commands changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandsPayload {
    pub commands: Vec<AvailableCommandSummary>,
}

/// A full replacement of the session's configuration options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigOptionsPayload {
    pub options: Vec<AcpConfigOption>,
}

/// A partial session metadata update from the agent.
///
/// `title` is the effective title only when the agent set one; a cleared or
/// absent title leaves the existing product title in place. `updated_at` is the
/// agent-reported ISO 8601 last-activity timestamp when present.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionInfoPayload {
    pub title: Option<String>,
    pub updated_at: Option<String>,
}

/// Context-window and cost usage for the session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsagePayload {
    pub used: u64,
    pub size: u64,
    pub cost: Option<AcpCost>,
}

/// Negotiated ACP capability and session metadata for one session.
///
/// This is the normalized, agent-neutral view clients receive: protocol
/// version, agent identity, advertised capabilities and auth methods, and the
/// session's initial mode state. It never contains raw ACP envelopes or `_meta`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SessionAcpMetadata {
    pub protocol_version: u16,
    /// Negotiated agent identity (programmatic name, display title, version).
    pub agent: AgentMetadataSummary,
    pub auth_methods: Vec<AuthMethodSummary>,
    pub capabilities: AgentCapabilitiesSummary,
    pub modes: Option<AcpModeState>,
    /// Session configuration options the agent advertised, updated as the agent
    /// reports changes.
    pub config_options: Vec<AcpConfigOption>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotPayload {
    pub summary: SessionSummary,
    pub pending_permission: Option<PermissionRequestPayload>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    Status(StatusPayload),
    Message(MessagePayload),
    ToolCall(ToolCallPayload),
    Plan(PlanPayload),
    Mode(ModePayload),
    Commands(CommandsPayload),
    ConfigOptions(ConfigOptionsPayload),
    SessionInfo(SessionInfoPayload),
    Usage(UsagePayload),
    TurnEnded(TurnEndedPayload),
    PermissionRequest(PermissionRequestPayload),
    PermissionResolved(PermissionResolvedPayload),
    Exit(ExitPayload),
    Diagnostics(DiagnosticsPayload),
    Snapshot(SnapshotPayload),
}

impl SessionEvent {
    pub fn name(&self) -> &'static str {
        match self {
            SessionEvent::Status(_) => "session.status",
            SessionEvent::Message(_) => "session.message",
            SessionEvent::ToolCall(_) => "session.tool_call",
            SessionEvent::Plan(_) => "session.plan",
            SessionEvent::Mode(_) => "session.mode",
            SessionEvent::Commands(_) => "session.commands",
            SessionEvent::ConfigOptions(_) => "session.config_options",
            SessionEvent::SessionInfo(_) => "session.info",
            SessionEvent::Usage(_) => "session.usage",
            SessionEvent::TurnEnded(_) => "session.turn_ended",
            SessionEvent::PermissionRequest(_) => "session.permission_request",
            SessionEvent::PermissionResolved(_) => "session.permission_resolved",
            SessionEvent::Exit(_) => "session.exit",
            SessionEvent::Diagnostics(_) => "session.diagnostics",
            SessionEvent::Snapshot(_) => "session.snapshot",
        }
    }

    pub fn payload(&self) -> Value {
        let value = match self {
            SessionEvent::Status(payload) => serde_json::to_value(payload),
            SessionEvent::Message(payload) => serde_json::to_value(payload),
            SessionEvent::ToolCall(payload) => serde_json::to_value(payload),
            SessionEvent::Plan(payload) => serde_json::to_value(payload),
            SessionEvent::Mode(payload) => serde_json::to_value(payload),
            SessionEvent::Commands(payload) => serde_json::to_value(payload),
            SessionEvent::ConfigOptions(payload) => serde_json::to_value(payload),
            SessionEvent::SessionInfo(payload) => serde_json::to_value(payload),
            SessionEvent::Usage(payload) => serde_json::to_value(payload),
            SessionEvent::TurnEnded(payload) => serde_json::to_value(payload),
            SessionEvent::PermissionRequest(payload) => serde_json::to_value(payload),
            SessionEvent::PermissionResolved(payload) => serde_json::to_value(payload),
            SessionEvent::Exit(payload) => serde_json::to_value(payload),
            SessionEvent::Diagnostics(payload) => serde_json::to_value(payload),
            SessionEvent::Snapshot(payload) => serde_json::to_value(payload),
        };
        value.unwrap_or(Value::Null)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SequencedEvent {
    pub sequence: u64,
    pub timestamp_ms: i64,
    pub event: SessionEvent,
}

/// Durable recovery outcome for a session after a host restart.
///
/// ACP agents are child processes that cannot survive the host, so a persisted
/// session is either rehydrated by reconnecting to its native agent session
/// ([`SessionRecoveryState::Recovered`]) or is reported as explicitly
/// stale/unavailable. It is never silently replaced by a new session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SessionRecoveryState {
    /// The session is live on this host and was not restored from disk.
    #[default]
    Live,
    /// The session was rehydrated by resuming its native agent session.
    Recovered,
    /// The native agent session no longer exists; the local journal is retained
    /// so clients can still review and replay it.
    Stale { reason: String },
    /// The agent could not be launched, or cannot resume its sessions. The
    /// local journal is retained.
    Unavailable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: SessionId,
    pub title: String,
    pub agent: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub working_directory_label: String,
    pub git_branch: Option<String>,
    pub status: SessionStatus,
    pub created_at_ms: i64,
    pub last_activity_at_ms: i64,
    pub last_sequence: u64,
    /// How this session relates to its native agent session. Defaults to
    /// [`SessionRecoveryState::Live`] for sessions that did not come from disk.
    #[serde(default)]
    pub recovery: SessionRecoveryState,
    #[serde(default)]
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionDetail {
    pub summary: SessionSummary,
    pub agent: AgentDescriptor,
    /// The ACP agent-side session id this product session is bound to.
    pub agent_session_id: Option<String>,
    /// Normalized ACP capabilities and session metadata negotiated for this
    /// session.
    pub acp: SessionAcpMetadata,
    pub running: bool,
    pub pending_permission: Option<PermissionRequestPayload>,
    pub recent_events: Vec<SequencedEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateSessionRequest {
    pub agent: AgentKind,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub working_directory_label: String,
    pub initial_prompt: Option<String>,
}

/// How an existing agent session is brought into Slight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionRecovery {
    /// `session/load`: replay the agent's retained history into the journal.
    Load,
    /// `session/resume`: reconnect without replaying agent-side history.
    Resume,
}

/// Imports an existing agent-side session into Slight under a new product
/// session. The native agent session id is preserved verbatim.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportSessionRequest {
    pub agent: AgentKind,
    /// The agent-side session id returned by `session/list`.
    pub agent_session_id: String,
    pub working_directory_label: String,
    /// Replay agent history (`load`) or reconnect only (`resume`).
    pub recovery: SessionRecovery,
    /// Optional title for the imported product session.
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttachResult {
    pub summary: SessionSummary,
    pub replayed: Vec<SequencedEvent>,
    pub latest_sequence: u64,
    pub oldest_available_sequence: Option<u64>,
    pub resync_required: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryResult {
    pub summary: SessionSummary,
    pub events: Vec<SequencedEvent>,
    pub latest_sequence: u64,
    pub oldest_available_sequence: Option<u64>,
    pub has_more: bool,
}

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("unknown agent: {0}")]
    UnknownAgent(String),
    #[error("unknown session: {0}")]
    UnknownSession(String),
    #[error("session is not running: {0}")]
    NotRunning(String),
    #[error("session title must not be empty")]
    EmptyTitle,
    #[error("agent does not support {0}")]
    UnsupportedCapability(&'static str),
    #[error("session {0} has no pending permission request")]
    NoPendingPermission(String),
    #[error("permission id mismatch for session {session_id}: expected {expected}, got {actual}")]
    PermissionMismatch {
        session_id: String,
        expected: String,
        actual: String,
    },
    #[error("agent error: {0}")]
    Agent(#[from] acp_types::AcpError),
    #[error("store error: {0}")]
    Store(#[from] session_store::StoreError),
}
