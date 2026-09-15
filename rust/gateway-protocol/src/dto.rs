use acp_types::metadata::ListedSessionSummary;
use acp_types::AgentDescriptor;
use serde::{Deserialize, Serialize};
use session_core::{SessionRecoveryState, SessionStatus, SessionSummary};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SessionSummaryDto {
    pub id: String,
    pub title: String,
    pub agent: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub working_directory_label: String,
    pub git_branch: Option<String>,
    pub status: SessionStatus,
    pub created_at: Option<String>,
    pub last_activity_at: Option<String>,
    pub last_sequence: u64,
    /// How the session relates to its native agent session after a host
    /// restart. Omitted by hosts that predate recovery; clients must default it
    /// to `live`.
    #[serde(default)]
    pub recovery: SessionRecoveryState,
    #[serde(default)]
    pub archived: bool,
}

impl From<&SessionSummary> for SessionSummaryDto {
    fn from(summary: &SessionSummary) -> Self {
        Self {
            id: summary.id.clone(),
            title: summary.title.clone(),
            agent: summary.agent.clone(),
            model: summary.model.clone(),
            effort: summary.effort.clone(),
            working_directory_label: summary.working_directory_label.clone(),
            git_branch: summary.git_branch.clone(),
            status: summary.status,
            created_at: Some(crate::time::rfc3339_from_ms(summary.created_at_ms)),
            last_activity_at: Some(crate::time::rfc3339_from_ms(summary.last_activity_at_ms)),
            last_sequence: summary.last_sequence,
            recovery: summary.recovery.clone(),
            archived: summary.archived,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AgentDescriptorDto {
    pub kind: String,
    pub display_name: String,
    pub version: Option<String>,
}

impl From<&AgentDescriptor> for AgentDescriptorDto {
    fn from(descriptor: &AgentDescriptor) -> Self {
        Self {
            kind: descriptor.kind.as_str().to_string(),
            display_name: descriptor.display_name.clone(),
            version: descriptor.version.clone(),
        }
    }
}

/// A native, agent-owned session that Slight has not imported.
///
/// `agent_session_id` is the agent's own session identifier and is exposed
/// deliberately: the discovery/import contract is the one place a client may
/// see native identity so it can ask the host to import a specific session.
/// `cwd` is the absolute working directory the agent associates with the
/// session and must be supplied back on import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AgentSessionSummaryDto {
    pub agent: String,
    pub agent_session_id: String,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_directories: Vec<String>,
    pub title: Option<String>,
    pub updated_at: Option<String>,
}

impl AgentSessionSummaryDto {
    pub fn from_listed(agent: &str, listed: &ListedSessionSummary) -> Self {
        Self {
            agent: agent.to_string(),
            agent_session_id: listed.agent_session_id.clone(),
            cwd: listed.cwd.clone(),
            additional_directories: listed.additional_directories.clone(),
            title: listed.title.clone(),
            updated_at: listed.updated_at.clone(),
        }
    }
}
