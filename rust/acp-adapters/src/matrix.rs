//! Adapter capability matrix and reporting.
//!
//! The registry knows which adapters exist and whether their executables are
//! present. This module turns that into an inspectable report: static launch and
//! authentication facts for every adapter, plus an optional runtime probe that
//! negotiates ACP `initialize` and records exactly which capabilities the real
//! agent advertised.
//!
//! The report is deliberately explicit about what is *not* supported. A missing
//! executable is reported as [`ProbeOutcome::Unavailable`], a broker that does
//! not complete the ACP handshake as [`ProbeOutcome::Failed`], and a negotiated
//! gap as [`ProbeSummary::unsupported_capabilities`], rather than claiming
//! uniform support across agents.

use crate::AgentLaunchConfig;
use acp_types::metadata::{AcpConfigOption, AgentCapabilitiesSummary, AuthMethodSummary};
use acp_types::wire::Implementation;
use acp_types::{AcpError, AgentKind};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Whether an adapter's executable can be launched on this host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AdapterAvailability {
    /// The executable was located.
    Available { program: Option<PathBuf> },
    /// No executable was found on the host.
    MissingExecutable {
        expected: String,
        env_var: String,
        hint: String,
    },
    /// The adapter exists but is intentionally not launchable in this build.
    Unsupported { reason: String },
}

impl AdapterAvailability {
    pub fn is_available(&self) -> bool {
        matches!(self, AdapterAvailability::Available { .. })
    }
}

/// How a user authenticates this agent.
///
/// ACP `authenticate` is a live agent call the host does not exercise in this
/// slice, so this describes the out-of-band login path and the environment
/// variables the adapter will pass through when configured.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthRequirement {
    /// Interactive command the user runs once, outside the gateway.
    pub login_command: Option<String>,
    /// Environment variables the host can forward as an alternative to login.
    pub env_vars: Vec<String>,
    /// Human-readable note about how authentication works for this agent.
    pub note: Option<String>,
}

/// The normalized ACP boundary every adapter in this crate is expected to drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BoundarySupport {
    pub initialize: bool,
    pub new_session: bool,
    pub prompt: bool,
    pub permission: bool,
    pub cancel: bool,
}

impl Default for BoundarySupport {
    fn default() -> Self {
        Self {
            initialize: true,
            new_session: true,
            prompt: true,
            permission: true,
            cancel: true,
        }
    }
}

/// The negotiated summary captured from a successful `initialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeSummary {
    pub protocol_version: u16,
    pub agent_name: Option<String>,
    pub agent_title: Option<String>,
    pub agent_version: Option<String>,
    pub auth_methods: Vec<AuthMethodSummary>,
    pub capabilities: AgentCapabilitiesSummary,
    pub config_options: Vec<AcpConfigOption>,
}

impl ProbeSummary {
    /// Agent capabilities the agent did not advertise, named in Slight's terms.
    ///
    /// This is the explicit "unsupported" half of the matrix: clients must not
    /// assume these work just because the agent completed the handshake.
    pub fn unsupported_capabilities(&self) -> Vec<&'static str> {
        let mut unsupported = Vec::new();
        let capabilities = &self.capabilities;
        if !capabilities.load_session {
            unsupported.push("session/load");
        }
        if !capabilities.prompt_image {
            unsupported.push("prompt image");
        }
        if !capabilities.prompt_audio {
            unsupported.push("prompt audio");
        }
        if !capabilities.prompt_embedded_context {
            unsupported.push("prompt embedded context");
        }
        if !capabilities.mcp_http {
            unsupported.push("mcp http");
        }
        if !capabilities.mcp_sse {
            unsupported.push("mcp sse");
        }
        if !capabilities.session_list {
            unsupported.push("session/list");
        }
        if !capabilities.session_resume {
            unsupported.push("session/resume");
        }
        if !capabilities.session_close {
            unsupported.push("session/close");
        }
        if !capabilities.session_delete {
            unsupported.push("session/delete");
        }
        if !capabilities.session_additional_directories {
            unsupported.push("session additional directories");
        }
        unsupported
    }
}

/// The result of probing one adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProbeOutcome {
    /// The adapter was not probed. Static reporting only.
    NotProbed,
    /// The executable is not installed on this host.
    Unavailable { reason: String },
    /// The adapter intentionally does not implement the normalized boundary.
    Unsupported { reason: String },
    /// The agent completed `initialize` and advertised its capabilities.
    Compatible(ProbeSummary),
    /// The executable exists but the ACP handshake failed.
    Failed { message: String },
}

impl ProbeOutcome {
    pub fn is_compatible(&self) -> bool {
        matches!(self, ProbeOutcome::Compatible(_))
    }
}

/// Static launch, auth, and support facts about one adapter, plus its probe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterCapability {
    pub kind: AgentKind,
    pub display_name: String,
    pub availability: AdapterAvailability,
    /// Base arguments the adapter always passes (for example `acp`).
    pub args: Vec<String>,
    pub auth: AuthRequirement,
    pub boundary: BoundarySupport,
    pub notes: Vec<String>,
    pub probe: ProbeOutcome,
}

impl AdapterCapability {
    /// A conservative default derived only from the adapter's kind/descriptor.
    pub fn placeholder(kind: AgentKind, display_name: String, available: bool) -> Self {
        let availability = if available {
            AdapterAvailability::Available { program: None }
        } else {
            AdapterAvailability::MissingExecutable {
                expected: kind.as_str().to_string(),
                env_var: String::new(),
                hint: String::new(),
            }
        };
        Self {
            kind,
            display_name,
            availability,
            args: Vec::new(),
            auth: AuthRequirement::default(),
            boundary: BoundarySupport::default(),
            notes: Vec::new(),
            probe: ProbeOutcome::NotProbed,
        }
    }
}

/// The full capability report across every registered adapter.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityMatrix {
    pub generated_at_ms: i64,
    pub entries: Vec<AdapterCapability>,
}

impl CapabilityMatrix {
    /// Every adapter kind in the matrix, including unavailable ones.
    pub fn kinds(&self) -> Vec<AgentKind> {
        self.entries
            .iter()
            .map(|entry| entry.kind.clone())
            .collect()
    }

    /// Entries whose executable was found on this host.
    pub fn available(&self) -> Vec<&AdapterCapability> {
        self.entries
            .iter()
            .filter(|entry| entry.availability.is_available())
            .collect()
    }

    /// Renders the matrix as pretty JSON for logs and test reports.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Builds a probe report for one adapter.
///
/// This never panics on a broken agent: a missing executable, a process that
/// exits before responding, or a malformed handshake all become explicit
/// outcomes instead of a claimed support level.
pub fn probe_adapter(
    adapter: &dyn crate::AgentAdapter,
    launch: &AgentLaunchConfig,
) -> ProbeOutcome {
    if !adapter.is_available() {
        return ProbeOutcome::Unavailable {
            reason: format!("{} executable not found on this host", adapter.kind()),
        };
    }

    let mut session = match adapter.spawn_session(launch) {
        Ok(session) => session,
        Err(error) => {
            return ProbeOutcome::Failed {
                message: error.to_string(),
            }
        }
    };

    let client = Implementation::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    match session.initialize(client) {
        Ok(summary) => {
            let capabilities = summary.capability_summary();
            let cwd = launch
                .working_directory
                .clone()
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_default();
            let config_options = match session.new_session(&cwd) {
                Ok(metadata) => metadata.config_options,
                Err(error) => {
                    return ProbeOutcome::Failed {
                        message: error.to_string(),
                    }
                }
            };
            ProbeOutcome::Compatible(ProbeSummary {
                protocol_version: summary.protocol_version,
                agent_name: summary.agent_name,
                agent_title: summary.agent_title,
                agent_version: summary.agent_version,
                auth_methods: summary.auth_methods,
                capabilities,
                config_options,
            })
        }
        Err(error) => ProbeOutcome::Failed {
            message: error.to_string(),
        },
    }
}

impl From<AcpError> for ProbeOutcome {
    fn from(error: AcpError) -> Self {
        ProbeOutcome::Failed {
            message: error.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_capabilities_names_negotiated_gaps() {
        let summary = ProbeSummary {
            protocol_version: 1,
            agent_name: Some("agent".to_string()),
            agent_title: None,
            agent_version: None,
            auth_methods: Vec::new(),
            capabilities: AgentCapabilitiesSummary {
                load_session: true,
                session_list: true,
                ..AgentCapabilitiesSummary::default()
            },
            config_options: Vec::new(),
        };
        let unsupported = summary.unsupported_capabilities();
        assert!(!unsupported.contains(&"session/load"));
        assert!(!unsupported.contains(&"session/list"));
        assert!(unsupported.contains(&"session/resume"));
        assert!(unsupported.contains(&"prompt image"));
    }

    #[test]
    fn matrix_available_filters_missing_executables() {
        let matrix = CapabilityMatrix {
            generated_at_ms: 0,
            entries: vec![
                AdapterCapability::placeholder(AgentKind::Fake, "Fake".to_string(), true),
                AdapterCapability::placeholder(AgentKind::Codex, "Codex".to_string(), false),
            ],
        };
        assert_eq!(matrix.available().len(), 1);
        assert_eq!(matrix.available()[0].kind, AgentKind::Fake);
    }
}
