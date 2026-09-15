//! Real OpenCode ACP adapter.
//!
//! This module locates and launches `opencode acp` as an ACP v1 stdio agent and
//! hands the connection to the agent-neutral [`ClientSession`] from
//! `acp-types::session`. All OpenCode-specific behavior — executable discovery,
//! sanitized environment, working directory, the mandatory `acp` argument, and
//! stderr capture — lives here; `session-core` and the native clients never see
//! it.
//!
//! The adapter emits the normalized [`AcpSession`] boundary, so `session-core`
//! drives `initialize`, `session/new`, prompts, permissions, and cancellation
//! through the same code path as every other agent.
//!
//! # Prerequisites
//!
//! The host must have the `opencode` executable on `PATH`, or the adapter must
//! be configured with an explicit path. Set `SLIGHT_OPENCODE_BIN` to inject a
//! specific binary (used by integration tests). A prompt turn may additionally
//! require `opencode auth login` and network access; the handshake itself does
//! not.

use crate::matrix::{AdapterAvailability, AdapterCapability, AuthRequirement, BoundarySupport};
use crate::process::ChildSpec;
use crate::stdio;
use crate::{AgentAdapter, AgentLaunchConfig};
use acp_types::session::AcpSession;
use acp_types::{AcpConnection, AcpError, AgentDescriptor, AgentKind};
use std::path::{Path, PathBuf};

/// Environment variable used to pin the OpenCode executable.
pub const OPENCODE_BIN_ENV: &str = "SLIGHT_OPENCODE_BIN";

/// The executable name searched on `PATH`.
pub const OPENCODE_EXECUTABLE: &str = "opencode";

/// Launch configuration for the OpenCode adapter.
#[derive(Debug, Clone, Default)]
pub struct OpencodeConfig {
    /// Explicit path to the `opencode` executable. When `None`, the
    /// `SLIGHT_OPENCODE_BIN` environment variable and then `PATH` are consulted.
    pub program: Option<PathBuf>,
    /// Default working directory for the agent process when the launch config
    /// does not provide one.
    pub working_directory: Option<PathBuf>,
    /// Extra environment variables applied after the safe allowlist. These are
    /// the only way to pass provider credentials to the agent.
    pub environment: Vec<(String, String)>,
    /// Extra arguments appended after the mandatory `acp` argument.
    pub extra_args: Vec<String>,
}

/// The OpenCode ACP adapter.
#[derive(Debug, Clone, Default)]
pub struct OpencodeAdapter {
    config: OpencodeConfig,
}

impl OpencodeAdapter {
    pub fn new(config: OpencodeConfig) -> Self {
        Self { config }
    }

    /// Adapter configured from the ambient environment (`SLIGHT_OPENCODE_BIN`
    /// and `PATH`).
    pub fn from_environment() -> Self {
        Self::default()
    }

    pub fn config(&self) -> &OpencodeConfig {
        &self.config
    }

    /// Resolves the executable that the adapter will launch.
    pub fn resolve_program(&self) -> Result<PathBuf, AcpError> {
        stdio::resolve_program(
            self.config.program.as_deref(),
            OPENCODE_BIN_ENV,
            OPENCODE_EXECUTABLE,
            "Install OpenCode from https://opencode.ai and add `opencode` to PATH.",
        )
    }

    /// Spawns `opencode acp` and returns the concrete session handle.
    ///
    /// The caller is expected to drive `initialize` and `session/new`, exactly
    /// as `session-core` does through the [`AcpSession`] trait.
    pub fn spawn_session_typed(
        &self,
        launch: &AgentLaunchConfig,
    ) -> Result<OpencodeSession, AcpError> {
        let cwd = launch
            .working_directory
            .clone()
            .or_else(|| self.config.working_directory.clone())
            .or_else(|| std::env::current_dir().ok());
        let spec = self.build_spec(launch, cwd.as_deref())?;
        stdio::spawn_session(spec)
    }

    fn build_spec(
        &self,
        launch: &AgentLaunchConfig,
        cwd: Option<&Path>,
    ) -> Result<ChildSpec, AcpError> {
        let program = self.resolve_program()?;
        let mut args = vec!["acp".to_string()];
        args.extend(self.config.extra_args.iter().cloned());

        let mut spec = ChildSpec::new(program)
            .args(args)
            .env_clear()
            .capture_stderr();
        if let Some(cwd) = cwd {
            spec = spec.working_directory(cwd);
        }
        for (key, value) in stdio::sanitized_environment(
            stdio::DEFAULT_ENV_ALLOWLIST,
            &self.config.environment,
            &launch.environment,
        ) {
            spec = spec.environment(key, value);
        }
        Ok(spec)
    }
}

impl AgentAdapter for OpencodeAdapter {
    fn kind(&self) -> AgentKind {
        AgentKind::Custom("opencode".to_string())
    }

    fn descriptor(&self) -> AgentDescriptor {
        AgentDescriptor::new(self.kind(), "OpenCode", None)
    }

    fn is_available(&self) -> bool {
        self.resolve_program().is_ok()
    }

    /// OpenCode is driven through the normalized [`AcpSession`] boundary;
    /// `session-core` calls [`AgentAdapter::spawn_session`]. The legacy
    /// [`AcpConnection`] surface has no `initialize`/`session/new` and so
    /// cannot represent a real agent.
    fn spawn(&self, _config: &AgentLaunchConfig) -> Result<Box<dyn AcpConnection>, AcpError> {
        Err(AcpError::new(
            "opencode requires the AcpSession boundary; use AgentAdapter::spawn_session",
        ))
    }

    fn spawn_session(&self, config: &AgentLaunchConfig) -> Result<Box<dyn AcpSession>, AcpError> {
        Ok(Box::new(self.spawn_session_typed(config)?))
    }

    fn capability(&self) -> AdapterCapability {
        let availability = match self.resolve_program() {
            Ok(program) => AdapterAvailability::Available {
                program: Some(program),
            },
            Err(_) => AdapterAvailability::MissingExecutable {
                expected: OPENCODE_EXECUTABLE.to_string(),
                env_var: OPENCODE_BIN_ENV.to_string(),
                hint: "Install OpenCode from https://opencode.ai and add `opencode` to PATH."
                    .to_string(),
            },
        };
        let mut args = vec!["acp".to_string()];
        args.extend(self.config.extra_args.iter().cloned());
        AdapterCapability {
            kind: self.kind(),
            display_name: self.descriptor().display_name,
            availability,
            args,
            auth: AuthRequirement {
                login_command: Some("opencode auth login".to_string()),
                env_vars: vec![
                    "ANTHROPIC_API_KEY".to_string(),
                    "OPENAI_API_KEY".to_string(),
                    "GEMINI_API_KEY".to_string(),
                ],
                note: Some(
                    "OpenCode reads provider credentials from its own auth store or provider env vars."
                        .to_string(),
                ),
            },
            boundary: BoundarySupport::default(),
            notes: vec![
                "Standalone `opencode acp` server; no separate adapter package is required."
                    .to_string(),
            ],
            probe: crate::matrix::ProbeOutcome::NotProbed,
        }
    }
}

/// A live OpenCode agent process plus its normalized ACP session.
pub type OpencodeSession = stdio::StdioSession;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitized_environment_applies_overrides_in_order() {
        let config = OpencodeConfig {
            environment: vec![("OPENCODE_TEST".to_string(), "from-config".to_string())],
            ..OpencodeConfig::default()
        };
        let launch = AgentLaunchConfig {
            environment: vec![
                ("OPENCODE_TEST".to_string(), "from-launch".to_string()),
                ("ANTHROPIC_API_KEY".to_string(), "secret".to_string()),
            ],
            ..AgentLaunchConfig::default()
        };
        let environment = stdio::sanitized_environment(
            stdio::DEFAULT_ENV_ALLOWLIST,
            &config.environment,
            &launch.environment,
        );
        let find = |key: &str| {
            environment
                .iter()
                .find(|(candidate, _)| candidate == key)
                .map(|(_, value)| value.clone())
        };
        assert_eq!(find("OPENCODE_TEST").as_deref(), Some("from-launch"));
        assert_eq!(find("ANTHROPIC_API_KEY").as_deref(), Some("secret"));
        assert_eq!(find("SLIGHT_ENSURE_ABSENT"), None);
    }

    #[test]
    fn missing_explicit_program_reports_clear_error() {
        let adapter = OpencodeAdapter::new(OpencodeConfig {
            program: Some(PathBuf::from("/definitely/not/opencode")),
            ..OpencodeConfig::default()
        });
        let error = adapter.resolve_program().unwrap_err();
        assert!(error.to_string().contains("does not exist"));
    }

    #[test]
    fn legacy_spawn_points_callers_at_spawn_session() {
        let adapter = OpencodeAdapter::default();
        let error = match adapter.spawn(&AgentLaunchConfig::default()) {
            Ok(_) => panic!("legacy spawn should not launch opencode"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("spawn_session"));
    }

    #[test]
    fn default_agents_include_opencode() {
        let registry = crate::AdapterRegistry::with_default_agents();
        let kinds = registry.kinds();
        assert!(kinds.contains(&AgentKind::Custom("opencode".to_string())));
        assert!(!kinds.contains(&AgentKind::Fake));
        let adapter = registry
            .get(&AgentKind::Custom("opencode".to_string()))
            .expect("opencode adapter is registered");
        assert_eq!(adapter.descriptor().display_name, "OpenCode");
    }

    #[test]
    fn capability_reports_launch_and_auth() {
        let capability = OpencodeAdapter::default().capability();
        assert_eq!(capability.args, vec!["acp".to_string()]);
        assert_eq!(
            capability.auth.login_command.as_deref(),
            Some("opencode auth login")
        );
    }
}
