//! Normalized ACP capability and metadata summaries.
//!
//! The wire types in [`crate::wire`] are conformant but agent-shaped: they carry
//! `_meta`, unstable feature-gated fields, and opaque tagged payloads that the
//! gateway contract must not expose. This module flattens the parts of the ACP
//! surface that clients actually consume — negotiated capabilities, auth
//! methods, session modes, plans, and available commands — into small,
//! versionable types that `session-core` and `gateway-protocol` can depend on
//! without ever seeing a raw ACP envelope.
//!
//! Nothing here is agent-specific. Every conversion is total: unknown or future
//! enum variants collapse to a documented default rather than being dropped.

use crate::wire::{
    AgentCapabilities, AuthMethod, AvailableCommand as WireAvailableCommand, AvailableCommandInput,
    Cost as WireCost, Plan as WirePlan, SessionConfigKind, SessionConfigOption,
    SessionConfigOptionCategory as WireConfigCategory, SessionConfigSelectOption,
    SessionConfigSelectOptions, SessionInfo as WireSessionInfo,
    SessionInfoUpdate as WireSessionInfoUpdate, SessionMode, SessionModeState,
    UsageUpdate as WireUsageUpdate,
};
use serde::{Deserialize, Serialize};

/// The kind of authentication an agent advertises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethodKind {
    /// The agent authenticates itself through an `authenticate` call.
    Agent,
    /// The host runs an interactive terminal flow for the user.
    Terminal,
}

/// A normalized authentication method advertised by an agent during
/// `initialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AuthMethodSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub kind: AuthMethodKind,
}

impl From<&AuthMethod> for AuthMethodSummary {
    fn from(method: &AuthMethod) -> Self {
        let kind = match method {
            AuthMethod::Terminal(_) => AuthMethodKind::Terminal,
            _ => AuthMethodKind::Agent,
        };
        Self {
            id: method.id().0.to_string(),
            name: method.name().to_string(),
            description: method.description().map(str::to_string),
            kind,
        }
    }
}

/// Flattened agent capabilities negotiated during `initialize`.
///
/// Only the booleans Slight can act on are surfaced. Unstable or
/// product-specific capability groups stay out of the contract until a client
/// feature requires them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", default)]
pub struct AgentCapabilitiesSummary {
    pub load_session: bool,
    pub prompt_image: bool,
    pub prompt_audio: bool,
    pub prompt_embedded_context: bool,
    pub mcp_http: bool,
    pub mcp_sse: bool,
    pub session_list: bool,
    pub session_resume: bool,
    pub session_close: bool,
    pub session_delete: bool,
    pub session_additional_directories: bool,
}

impl From<&AgentCapabilities> for AgentCapabilitiesSummary {
    fn from(capabilities: &AgentCapabilities) -> Self {
        let prompt = &capabilities.prompt_capabilities;
        let mcp = &capabilities.mcp_capabilities;
        let session = &capabilities.session_capabilities;
        Self {
            load_session: capabilities.load_session,
            prompt_image: prompt.image,
            prompt_audio: prompt.audio,
            prompt_embedded_context: prompt.embedded_context,
            mcp_http: mcp.http,
            mcp_sse: mcp.sse,
            session_list: session.list.is_some(),
            session_resume: session.resume.is_some(),
            session_close: session.close.is_some(),
            session_delete: session.delete.is_some(),
            session_additional_directories: session.additional_directories.is_some(),
        }
    }
}

/// A single session mode the agent can operate in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AcpMode {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

impl From<&SessionMode> for AcpMode {
    fn from(mode: &SessionMode) -> Self {
        Self {
            id: mode.id.0.to_string(),
            name: mode.name.clone(),
            description: mode.description.clone(),
        }
    }
}

/// The agent's mode state for a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AcpModeState {
    pub current_mode_id: String,
    pub available_modes: Vec<AcpMode>,
}

impl From<&SessionModeState> for AcpModeState {
    fn from(state: &SessionModeState) -> Self {
        Self {
            current_mode_id: state.current_mode_id.0.to_string(),
            available_modes: state.available_modes.iter().map(AcpMode::from).collect(),
        }
    }
}

/// A normalized semantic category for a session configuration option.
///
/// Categories are a UX hint only; clients must render unknown categories
/// gracefully. Category names beginning with `_` are reserved for custom use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcpConfigCategory {
    /// Session mode selector.
    Mode,
    /// Model selector.
    Model,
    /// Model-related configuration parameter.
    ModelConfig,
    /// Thought/reasoning level selector.
    ThoughtLevel,
    /// Unknown or custom category, preserved as its wire string.
    Other(String),
}

impl Serialize for AcpConfigCategory {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let value = match self {
            Self::Mode => "mode",
            Self::Model => "model",
            Self::ModelConfig => "model_config",
            Self::ThoughtLevel => "thought_level",
            Self::Other(value) => value,
        };
        serializer.serialize_str(value)
    }
}

impl<'de> Deserialize<'de> for AcpConfigCategory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "mode" => Self::Mode,
            "model" => Self::Model,
            "model_config" => Self::ModelConfig,
            "thought_level" => Self::ThoughtLevel,
            _ => Self::Other(value),
        })
    }
}

impl From<&WireConfigCategory> for AcpConfigCategory {
    fn from(category: &WireConfigCategory) -> Self {
        match category {
            WireConfigCategory::Mode => Self::Mode,
            WireConfigCategory::Model => Self::Model,
            WireConfigCategory::ModelConfig => Self::ModelConfig,
            WireConfigCategory::ThoughtLevel => Self::ThoughtLevel,
            other => Self::Other(enum_string(other)),
        }
    }
}

/// One selectable value within a session configuration option.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AcpConfigChoice {
    pub value_id: String,
    pub name: String,
    pub description: Option<String>,
}

impl From<&SessionConfigSelectOption> for AcpConfigChoice {
    fn from(choice: &SessionConfigSelectOption) -> Self {
        Self {
            value_id: choice.value.0.to_string(),
            name: choice.name.clone(),
            description: choice.description.clone(),
        }
    }
}

/// A named group of selectable values.
///
/// An ungrouped option is represented as a single group with no id or name so
/// consumers always walk one shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AcpConfigGroup {
    pub id: Option<String>,
    pub name: Option<String>,
    pub options: Vec<AcpConfigChoice>,
}

/// The type and current value of a session configuration option.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AcpConfigKind {
    /// A single-value selector backed by a list of choices.
    Select {
        current_value_id: String,
        groups: Vec<AcpConfigGroup>,
    },
    /// An on/off toggle.
    Boolean { current_value: bool },
}

impl From<&SessionConfigKind> for AcpConfigKind {
    fn from(kind: &SessionConfigKind) -> Self {
        match kind {
            SessionConfigKind::Boolean(boolean) => Self::Boolean {
                current_value: boolean.current_value,
            },
            SessionConfigKind::Select(select) => Self::Select {
                current_value_id: select.current_value.0.to_string(),
                groups: config_groups(&select.options),
            },
            // Future option kinds are preserved by id/name but collapse to an
            // inert boolean so the contract stays forward-compatible.
            _ => Self::Boolean {
                current_value: false,
            },
        }
    }
}

/// A normalized session configuration option advertised by the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AcpConfigOption {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<AcpConfigCategory>,
    pub kind: AcpConfigKind,
}

impl From<&SessionConfigOption> for AcpConfigOption {
    fn from(option: &SessionConfigOption) -> Self {
        Self {
            id: option.id.0.to_string(),
            name: option.name.clone(),
            description: option.description.clone(),
            category: option.category.as_ref().map(AcpConfigCategory::from),
            kind: AcpConfigKind::from(&option.kind),
        }
    }
}

fn config_groups(options: &SessionConfigSelectOptions) -> Vec<AcpConfigGroup> {
    match options {
        SessionConfigSelectOptions::Ungrouped(options) => vec![AcpConfigGroup {
            id: None,
            name: None,
            options: options.iter().map(AcpConfigChoice::from).collect(),
        }],
        SessionConfigSelectOptions::Grouped(groups) => groups
            .iter()
            .map(|group| AcpConfigGroup {
                id: Some(group.group.0.to_string()),
                name: Some(group.name.clone()),
                options: group.options.iter().map(AcpConfigChoice::from).collect(),
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// A normalized `session/update` metadata change.
///
/// `None` means the field was absent (unchanged), `Some(None)` means the agent
/// explicitly cleared it, and `Some(Some(value))` means it was set. This
/// preserves the ACP "undefined vs null vs value" distinction without leaking
/// `MaybeUndefined` into the contract.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AcpSessionInfoUpdate {
    pub title: Option<Option<String>>,
    pub updated_at: Option<Option<String>>,
}

impl From<&WireSessionInfoUpdate> for AcpSessionInfoUpdate {
    fn from(update: &WireSessionInfoUpdate) -> Self {
        Self {
            title: update.title.clone().into(),
            updated_at: update.updated_at.clone().into(),
        }
    }
}

/// Cumulative cost reported alongside context-window usage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AcpCost {
    pub amount: f64,
    /// ISO 4217 currency code, for example `USD`.
    pub currency: String,
}

impl From<&WireCost> for AcpCost {
    fn from(cost: &WireCost) -> Self {
        Self {
            amount: cost.amount,
            currency: cost.currency.clone(),
        }
    }
}

/// Context-window and cost usage for a session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AcpUsage {
    /// Tokens currently in context.
    pub used: u64,
    /// Total context-window size in tokens.
    pub size: u64,
    pub cost: Option<AcpCost>,
}

impl From<&WireUsageUpdate> for AcpUsage {
    fn from(update: &WireUsageUpdate) -> Self {
        Self {
            used: update.used,
            size: update.size,
            cost: update.cost.as_ref().map(AcpCost::from),
        }
    }
}

/// Normalized identity metadata for the negotiated agent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentMetadataSummary {
    /// Programmatic name.
    pub name: Option<String>,
    /// Human-readable title, when the agent advertises one.
    pub title: Option<String>,
    pub version: Option<String>,
}

/// Metadata returned by `session/new`, normalized for the host and clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SessionMetadata {
    /// The agent-side session id this product session is bound to.
    pub agent_session_id: String,
    /// Initial mode state, when the agent supports session modes.
    pub modes: Option<AcpModeState>,
    /// Initial session configuration options, when the agent advertises them.
    #[serde(default)]
    pub config_options: Vec<AcpConfigOption>,
}

/// A normalized reference to an existing agent-side session.
///
/// This is what `session/list` returns: enough to identify and import a native
/// agent session without exposing agent-private storage. `cwd` is the absolute
/// working directory the agent associates with the session, which `session/load`
/// and `session/resume` require.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ListedSessionSummary {
    /// The agent-side session id, preserved verbatim as native identity.
    pub agent_session_id: String,
    /// Absolute working directory the agent associates with the session.
    pub cwd: String,
    /// Additional workspace roots reported for the session.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_directories: Vec<String>,
    /// Human-readable title, when the agent advertises one.
    pub title: Option<String>,
    /// ISO 8601 timestamp of the agent's last activity.
    pub updated_at: Option<String>,
}

impl From<&WireSessionInfo> for ListedSessionSummary {
    fn from(info: &WireSessionInfo) -> Self {
        Self {
            agent_session_id: info.session_id.to_string(),
            cwd: info.cwd.display().to_string(),
            additional_directories: info
                .additional_directories
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            title: info.title.clone(),
            updated_at: info.updated_at.clone(),
        }
    }
}

/// A single entry in an agent execution plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PlanEntrySummary {
    pub content: String,
    pub priority: String,
    pub status: String,
}

/// A normalized agent execution plan. Every update replaces the whole plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PlanSummary {
    pub entries: Vec<PlanEntrySummary>,
}

impl From<&WirePlan> for PlanSummary {
    fn from(plan: &WirePlan) -> Self {
        Self {
            entries: plan
                .entries
                .iter()
                .map(|entry| PlanEntrySummary {
                    content: entry.content.clone(),
                    priority: enum_string(&entry.priority),
                    status: enum_string(&entry.status),
                })
                .collect(),
        }
    }
}

/// A normalized slash command advertised by the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AvailableCommandSummary {
    pub name: String,
    pub description: String,
    /// Hint text for commands that accept unstructured input.
    pub input_hint: Option<String>,
}

impl From<&WireAvailableCommand> for AvailableCommandSummary {
    fn from(command: &WireAvailableCommand) -> Self {
        let input_hint = command.input.as_ref().and_then(|input| match input {
            AvailableCommandInput::Unstructured(unstructured) => Some(unstructured.hint.clone()),
            _ => None,
        });
        Self {
            name: command.name.clone(),
            description: command.description.clone(),
            input_hint,
        }
    }
}

/// Serializes a conformant string enum (for example plan priority or status) to
/// its wire string, defaulting to the empty string for future variants.
fn enum_string<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{
        AgentCapabilities, AvailableCommand, Cost, Plan, PlanEntry, PlanEntryPriority,
        PlanEntryStatus, PromptCapabilities, SessionCapabilities, SessionConfigOption,
        SessionConfigSelectGroup, SessionConfigSelectOption, SessionInfo, SessionListCapabilities,
        SessionMode, SessionModeState, SessionResumeCapabilities, UsageUpdate,
    };

    #[test]
    fn capabilities_summary_flattens_capability_groups() {
        let capabilities = AgentCapabilities::new()
            .load_session(true)
            .prompt_capabilities(PromptCapabilities::new().image(true).audio(true))
            .mcp_capabilities(crate::wire::McpCapabilities::new())
            .session_capabilities(
                SessionCapabilities::new()
                    .list(SessionListCapabilities::new())
                    .resume(SessionResumeCapabilities::new()),
            );
        let summary = AgentCapabilitiesSummary::from(&capabilities);
        assert!(summary.load_session);
        assert!(summary.prompt_image);
        assert!(summary.prompt_audio);
        assert!(!summary.prompt_embedded_context);
        assert!(summary.session_list);
        assert!(summary.session_resume);
        assert!(!summary.session_close);
    }

    #[test]
    fn plan_summary_uses_wire_strings() {
        let plan = Plan::new(vec![PlanEntry::new(
            "ship it",
            PlanEntryPriority::High,
            PlanEntryStatus::InProgress,
        )]);
        let summary = PlanSummary::from(&plan);
        assert_eq!(summary.entries.len(), 1);
        assert_eq!(summary.entries[0].priority, "high");
        assert_eq!(summary.entries[0].status, "in_progress");
    }

    #[test]
    fn available_command_keeps_unstructured_hint() {
        let mut command = AvailableCommand::new("create_plan", "Create a plan");
        command.input = Some(AvailableCommandInput::Unstructured(
            crate::wire::UnstructuredCommandInput::new("describe the task"),
        ));
        let summary = AvailableCommandSummary::from(&command);
        assert_eq!(summary.input_hint.as_deref(), Some("describe the task"));
    }

    #[test]
    fn mode_state_maps_ids_and_labels() {
        let state = SessionModeState::new(
            "plan",
            vec![
                SessionMode::new("plan", "Plan"),
                SessionMode::new("build", "Build"),
            ],
        );
        let summary = AcpModeState::from(&state);
        assert_eq!(summary.current_mode_id, "plan");
        assert_eq!(summary.available_modes.len(), 2);
    }

    #[test]
    fn select_config_option_keeps_choices_and_category() {
        let option = SessionConfigOption::select(
            "model",
            "Model",
            "fast",
            vec![
                SessionConfigSelectOption::new("fast", "Fast"),
                SessionConfigSelectOption::new("smart", "Smart").description("slower"),
            ],
        )
        .description("Pick a model")
        .category(crate::wire::SessionConfigOptionCategory::Model);
        let normalized = AcpConfigOption::from(&option);
        assert_eq!(normalized.id, "model");
        assert_eq!(normalized.category, Some(AcpConfigCategory::Model));
        assert_eq!(normalized.description.as_deref(), Some("Pick a model"));
        match normalized.kind {
            AcpConfigKind::Select {
                current_value_id,
                groups,
            } => {
                assert_eq!(current_value_id, "fast");
                assert_eq!(groups.len(), 1);
                assert!(groups[0].id.is_none());
                assert_eq!(groups[0].options.len(), 2);
                assert_eq!(groups[0].options[1].description.as_deref(), Some("slower"));
            }
            other => panic!("expected select option, got {other:?}"),
        }
    }

    #[test]
    fn grouped_select_config_option_preserves_group_labels() {
        let option = SessionConfigOption::select(
            "model",
            "Model",
            "fast",
            vec![SessionConfigSelectGroup::new(
                "cheap",
                "Cheap",
                vec![SessionConfigSelectOption::new("fast", "Fast")],
            )],
        );
        let normalized = AcpConfigOption::from(&option);
        match normalized.kind {
            AcpConfigKind::Select { groups, .. } => {
                assert_eq!(groups.len(), 1);
                assert_eq!(groups[0].id.as_deref(), Some("cheap"));
                assert_eq!(groups[0].name.as_deref(), Some("Cheap"));
            }
            other => panic!("expected select option, got {other:?}"),
        }
    }

    #[test]
    fn boolean_config_option_maps_current_value() {
        let option = SessionConfigOption::boolean("brave_mode", "Brave Mode", true);
        let normalized = AcpConfigOption::from(&option);
        assert_eq!(
            normalized.kind,
            AcpConfigKind::Boolean {
                current_value: true
            }
        );
        assert!(normalized.category.is_none());
    }

    #[test]
    fn usage_update_maps_context_and_cost() {
        let update = UsageUpdate::new(53_000, 200_000).cost(Cost::new(0.045, "USD"));
        let normalized = AcpUsage::from(&update);
        assert_eq!(normalized.used, 53_000);
        assert_eq!(normalized.size, 200_000);
        let cost = normalized.cost.expect("cost present");
        assert_eq!(cost.amount, 0.045);
        assert_eq!(cost.currency, "USD");
    }

    #[test]
    fn session_info_update_preserves_undefined_and_null() {
        let update = crate::wire::SessionInfoUpdate::new().title("Renamed");
        let normalized = AcpSessionInfoUpdate::from(&update);
        assert_eq!(normalized.title, Some(Some("Renamed".to_string())));
        assert_eq!(normalized.updated_at, None);
    }

    #[test]
    fn listed_session_maps_native_identity_and_cwd() {
        let info = SessionInfo::new("native-7", "/work/project")
            .title("Prior work")
            .updated_at("2026-02-02T00:00:00Z")
            .additional_directories(vec![std::path::PathBuf::from("/work/extra")]);
        let summary = ListedSessionSummary::from(&info);
        assert_eq!(summary.agent_session_id, "native-7");
        assert_eq!(summary.cwd, "/work/project");
        assert_eq!(summary.title.as_deref(), Some("Prior work"));
        assert_eq!(summary.updated_at.as_deref(), Some("2026-02-02T00:00:00Z"));
        assert_eq!(summary.additional_directories, vec!["/work/extra"]);
    }

    #[test]
    fn session_metadata_carries_config_options() {
        let metadata = SessionMetadata {
            agent_session_id: "s-1".to_string(),
            modes: None,
            config_options: vec![AcpConfigOption::from(&SessionConfigOption::boolean(
                "brave_mode",
                "Brave Mode",
                false,
            ))],
        };
        assert_eq!(metadata.config_options.len(), 1);
        assert_eq!(metadata.config_options[0].id, "brave_mode");
    }
}
