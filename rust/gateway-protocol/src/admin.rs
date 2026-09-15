use crate::frames::ServerCapabilities;
use acp_types::metadata::AcpConfigOption;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostRunState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct HostStatusResult {
    pub state: HostRunState,
    pub host_name: String,
    pub server_id: String,
    pub server_version: String,
    pub protocol_version: u32,
    pub listener: Option<String>,
    pub session_count: usize,
    pub active_session_count: usize,
    pub paired_device_count: usize,
    pub uptime_ms: u64,
    pub supported_agents: Vec<String>,
    pub agent_catalog: Vec<AgentCatalogEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AgentCatalogEntry {
    pub id: String,
    pub display_name: String,
    pub version: Option<String>,
    pub available: bool,
    pub config_options: Vec<AcpConfigOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct HostLifecycleResult {
    pub state: HostRunState,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SessionDiagnosticsDto {
    pub session_id: String,
    pub status: String,
    pub agent: String,
    pub last_sequence: u64,
    pub event_count: usize,
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LogEntryDto {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct HostDiagnosticsResult {
    pub server_version: String,
    pub protocol_version: u32,
    pub log_level: String,
    pub sessions: Vec<SessionDiagnosticsDto>,
    pub recent_logs: Vec<LogEntryDto>,
    pub capabilities: ServerCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PairingCreateResult {
    pub pairing_id: String,
    pub code: String,
    pub label: String,
    pub expires_at: String,
    pub qr_payload: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PairingSummaryDto {
    pub pairing_id: String,
    pub code: String,
    pub label: String,
    pub expires_at: String,
    pub consumed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PairingListResult {
    pub pairings: Vec<PairingSummaryDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DeviceSummaryDto {
    pub device_id: String,
    pub label: String,
    pub created_at: String,
    pub last_seen_at: String,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DeviceListResult {
    pub devices: Vec<DeviceSummaryDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DeviceRevokeResult {
    pub device: DeviceSummaryDto,
}
