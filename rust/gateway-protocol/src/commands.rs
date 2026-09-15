use crate::dto::{AgentDescriptorDto, AgentSessionSummaryDto, SessionSummaryDto};
use crate::frames::GatewayEvent;
use serde::{Deserialize, Serialize};
use session_core::{SessionAcpMetadata, SessionRecovery};

pub mod command_name {
    pub const SESSION_RESUME: &str = "session.resume";
    pub const SESSION_LIST: &str = "session.list";
    pub const SESSION_WORKING_DIRECTORIES: &str = "session.working_directories";
    pub const SESSION_CREATE: &str = "session.create";
    pub const SESSION_ATTACH: &str = "session.attach";
    pub const SESSION_DETACH: &str = "session.detach";
    pub const SESSION_INPUT: &str = "session.input";
    pub const SESSION_CANCEL: &str = "session.cancel";
    pub const SESSION_RENAME: &str = "session.rename";
    pub const SESSION_ARCHIVE: &str = "session.archive";
    pub const SESSION_SET_MODE: &str = "session.set_mode";
    pub const SESSION_SET_CONFIG_OPTION: &str = "session.set_config_option";
    pub const SESSION_PERMISSION_RESPOND: &str = "session.permission.respond";
    pub const SESSION_INSPECT: &str = "session.inspect";
    pub const SESSION_HISTORY: &str = "session.history";
    pub const EVENTS_REPLAY: &str = "events.replay";

    /// Lists native sessions the connected agent already owns.
    pub const AGENT_SESSIONS_LIST: &str = "agent.sessions.list";
    /// Imports one native agent session into Slight as a product session.
    pub const AGENT_SESSION_IMPORT: &str = "agent.sessions.import";

    pub const HOST_STATUS: &str = "host.status";
    pub const HOST_START: &str = "host.start";
    pub const HOST_STOP: &str = "host.stop";
    pub const HOST_RESTART: &str = "host.restart";
    pub const HOST_CONFIGURATION: &str = "host.configuration";
    pub const HOST_DIAGNOSTICS: &str = "host.diagnostics";
    pub const HOST_SHUTDOWN: &str = "host.shutdown";

    pub const DEVICE_LIST: &str = "device.list";
    pub const DEVICE_REVOKE: &str = "device.revoke";
    pub const PAIRING_CREATE: &str = "pairing.create";
    pub const PAIRING_LIST: &str = "pairing.list";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateSessionParams {
    pub agent: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub working_directory_label: String,
    pub initial_prompt: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct AttachSessionParams {
    pub after_sequence: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct SessionHistoryParams {
    pub before_sequence: Option<u64>,
    pub limit: Option<usize>,
}

/// Params for `agent.sessions.list`.
///
/// Discovery is scoped by agent and, optionally, a working-directory label
/// (`~`-relative or absolute). The agent may ignore the filter or apply it
/// loosely; clients must treat the returned `cwd` as authoritative.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AgentSessionsListParams {
    pub agent: String,
    pub working_directory_label: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
}

/// Params for `agent.sessions.import`.
///
/// `agent_session_id` is the native identity returned by `agent.sessions.list`.
/// `recovery` selects whether the host replays agent-side history (`load`) or
/// only reconnects (`resume`); neither is Slight's local journal replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ImportAgentSessionParams {
    pub agent: String,
    pub agent_session_id: String,
    pub working_directory_label: String,
    pub recovery: SessionRecovery,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SendInputParams {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RenameSessionParams {
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ArchiveSessionParams {
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SetSessionModeParams {
    pub mode_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SetSessionConfigOptionParams {
    pub config_id: String,
    pub value_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RespondPermissionParams {
    pub permission_id: String,
    pub option_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BeginPairingParams {
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RevokeDeviceParams {
    pub device_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SessionListResult {
    pub sessions: Vec<SessionSummaryDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SessionWorkingDirectoriesResult {
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AgentSessionsListResult {
    pub sessions: Vec<AgentSessionSummaryDto>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ImportAgentSessionResult {
    pub session: SessionSummaryDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SessionCreateResult {
    pub session: SessionSummaryDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SessionRenameResult {
    pub session: SessionSummaryDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SessionArchiveResult {
    pub session: SessionSummaryDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SessionAttachResult {
    pub session: SessionSummaryDto,
    pub replayed: Vec<GatewayEvent>,
    pub latest_sequence: u64,
    pub oldest_available_sequence: Option<u64>,
    pub resync_required: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SessionHistoryResult {
    pub session: SessionSummaryDto,
    pub events: Vec<GatewayEvent>,
    pub latest_sequence: u64,
    pub oldest_available_sequence: Option<u64>,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SessionInspectResult {
    pub session: SessionSummaryDto,
    pub agent: AgentDescriptorDto,
    /// Normalized ACP capabilities and session metadata negotiated with the
    /// agent. Omitted by older hosts; clients must default it.
    #[serde(default)]
    pub acp: SessionAcpMetadata,
    pub running: bool,
    pub pending_permission: Option<serde_json::Value>,
    pub recent_events: Vec<GatewayEvent>,
}
