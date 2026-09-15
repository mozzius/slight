//! Real Claude Code ACP adapter.
//!
//! Claude Code is not itself an ACP server. An ACP broker process (the
//! `claude-code-acp` adapter published by Zed) wraps Claude Code and speaks ACP
//! over stdio. This module locates that broker, launches it with a sanitized
//! environment, and returns the normalized [`AcpSession`] boundary.
//!
//! Authentication is handled out of band: the broker reuses Claude Code's own
//! credentials (OAuth/Keychain) or the standard Anthropic environment
//! variables. See [`ClaudeCodeConfig::environment`] and the adapter's
//! [`AgentAdapter::capability`] entry for what is forwarded.
//!
//! A shipped host resolves a self-contained bundle first. A global
//! `claude-agent-acp` install is only used when explicitly enabled for
//! development.

use crate::matrix::{AdapterAvailability, AdapterCapability, AuthRequirement, BoundarySupport};
use crate::process::ChildSpec;
use crate::stdio;
use crate::{AgentAdapter, AgentLaunchConfig};
use acp_types::session::AcpSession;
use acp_types::{AcpConnection, AcpError, AgentDescriptor, AgentKind};
use std::path::{Path, PathBuf};

/// Environment variable used to pin the Claude Code ACP broker.
pub const CLAUDE_CODE_BIN_ENV: &str = "SLIGHT_CLAUDE_CODE_BIN";
pub const CLAUDE_CODE_BUNDLE_ENV: &str = "SLIGHT_CLAUDE_CODE_BUNDLE_DIR";
pub const CLAUDE_CODE_ALLOW_GLOBAL_ENV: &str = "SLIGHT_CLAUDE_CODE_ALLOW_GLOBAL";

/// The executable name searched on `PATH`.
pub const CLAUDE_CODE_EXECUTABLE: &str = "claude-agent-acp";

/// Launch configuration for the Claude Code adapter.
#[derive(Debug, Clone, Default)]
pub struct ClaudeCodeConfig {
    /// Explicit path to the `claude-code-acp` executable. When `None`, the
    /// `SLIGHT_CLAUDE_CODE_BIN` environment variable and then `PATH` are
    /// consulted.
    pub program: Option<PathBuf>,
    /// Explicit path to a self-contained Claude ACP bundle root or target
    /// directory.
    pub bundle_dir: Option<PathBuf>,
    /// Permit a globally installed broker on PATH. Intended for development.
    pub allow_global_fallback: bool,
    /// Default working directory for the agent process when the launch config
    /// does not provide one.
    pub working_directory: Option<PathBuf>,
    /// Extra environment variables applied after the safe allowlist. These are
    /// the only way to pass provider credentials to the agent.
    pub environment: Vec<(String, String)>,
    /// Extra arguments appended after the adapter's base arguments.
    pub extra_args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeCodeOrigin {
    Override { program: PathBuf },
    Bundled { directory: PathBuf },
    GlobalFallback { program: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeCodeLaunch {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub origin: ClaudeCodeOrigin,
}

pub fn bundled_target_triple() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        _ => None,
    }
}

pub fn is_claude_code_bundle_dir(dir: &Path) -> bool {
    let native_runtime = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "node_modules/@anthropic-ai/claude-agent-sdk-darwin-arm64/claude",
        ("macos", "x86_64") => "node_modules/@anthropic-ai/claude-agent-sdk-darwin-x64/claude",
        _ => return false,
    };
    dir.join("bin/node").is_file()
        && dir.join("lib/claude-agent-acp/dist/index.js").is_file()
        && dir
            .join("node_modules/@anthropic-ai/claude-agent-sdk/sdk.mjs")
            .is_file()
        && dir.join(native_runtime).is_file()
}

/// The Claude Code ACP adapter.
#[derive(Debug, Clone, Default)]
pub struct ClaudeCodeAdapter {
    config: ClaudeCodeConfig,
}

impl ClaudeCodeAdapter {
    pub fn new(config: ClaudeCodeConfig) -> Self {
        Self { config }
    }

    /// Adapter configured from the ambient environment (`SLIGHT_CLAUDE_CODE_BIN`
    /// and `PATH`).
    pub fn from_environment() -> Self {
        Self::default()
    }

    pub fn config(&self) -> &ClaudeCodeConfig {
        &self.config
    }

    pub fn resolve_launch(&self) -> Result<ClaudeCodeLaunch, AcpError> {
        let explicit = self
            .config
            .program
            .clone()
            .or_else(|| std::env::var_os(CLAUDE_CODE_BIN_ENV).map(PathBuf::from));
        if let Some(program) = explicit {
            let program = stdio::resolve_candidate(
                &program,
                "Build the Claude ACP bundle, set SLIGHT_CLAUDE_CODE_BUNDLE_DIR, or opt into the development fallback.",
            )?;
            let program = std::fs::canonicalize(&program).map_err(|error| {
                AcpError::new(format!(
                    "failed to resolve Claude ACP broker `{}`: {error}",
                    program.display()
                ))
            })?;
            return Ok(ClaudeCodeLaunch {
                origin: ClaudeCodeOrigin::Override {
                    program: program.clone(),
                },
                program,
                args: Vec::new(),
            });
        }

        if let Some(directory) = self.resolve_bundle_dir() {
            return Ok(ClaudeCodeLaunch {
                program: directory.join("bin/node"),
                args: vec![directory
                    .join("lib/claude-agent-acp/dist/index.js")
                    .to_string_lossy()
                    .into_owned()],
                origin: ClaudeCodeOrigin::Bundled { directory },
            });
        }

        if self.allow_global_fallback() {
            if let Ok(program) = stdio::resolve_program(
                None,
                CLAUDE_CODE_BIN_ENV,
                CLAUDE_CODE_EXECUTABLE,
                "Install @agentclientprotocol/claude-agent-acp and add it to PATH.",
            ) {
                return Ok(ClaudeCodeLaunch {
                    origin: ClaudeCodeOrigin::GlobalFallback {
                        program: program.clone(),
                    },
                    program,
                    args: Vec::new(),
                });
            }
        }

        Err(AcpError::new(
            "no Claude ACP broker found: no explicit program or self-contained bundle, and the global fallback is disabled",
        ))
    }

    /// Resolves the ACP broker executable that the adapter will launch.
    pub fn resolve_program(&self) -> Result<PathBuf, AcpError> {
        self.resolve_launch().map(|launch| launch.program)
    }

    /// Spawns the Claude Code ACP broker and returns the concrete session.
    pub fn spawn_session_typed(
        &self,
        launch: &AgentLaunchConfig,
    ) -> Result<ClaudeCodeSession, AcpError> {
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
        let resolved = self.resolve_launch()?;
        let mut spec = ChildSpec::new(resolved.program)
            .args(
                resolved
                    .args
                    .into_iter()
                    .chain(self.config.extra_args.iter().cloned()),
            )
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

    fn allow_global_fallback(&self) -> bool {
        self.config.allow_global_fallback
            || std::env::var(CLAUDE_CODE_ALLOW_GLOBAL_ENV).as_deref() == Ok("1")
    }

    fn resolve_bundle_dir(&self) -> Option<PathBuf> {
        let target = bundled_target_triple()?;
        let mut explicit = Vec::new();
        if let Some(dir) = &self.config.bundle_dir {
            explicit.push(dir.clone());
        }
        if let Some(dir) = std::env::var_os(CLAUDE_CODE_BUNDLE_ENV) {
            explicit.push(PathBuf::from(dir));
        }
        for candidate in explicit {
            let candidate = std::fs::canonicalize(&candidate).unwrap_or(candidate);
            if is_claude_code_bundle_dir(&candidate) {
                return Some(candidate);
            }
            let nested = candidate.join(target);
            if let Ok(nested) = std::fs::canonicalize(nested) {
                if is_claude_code_bundle_dir(&nested) {
                    return Some(nested);
                }
            }
        }
        let executable = std::env::current_exe().ok()?;
        let directory = executable.parent()?;
        for base in [
            directory.join("claude-agent-acp"),
            directory.join("../Resources/claude-agent-acp"),
            directory.join("../claude-agent-acp"),
        ] {
            let candidate = base.join(target);
            if is_claude_code_bundle_dir(&candidate) {
                return Some(candidate);
            }
        }
        None
    }
}

impl AgentAdapter for ClaudeCodeAdapter {
    fn kind(&self) -> AgentKind {
        AgentKind::ClaudeCode
    }

    fn descriptor(&self) -> AgentDescriptor {
        AgentDescriptor::new(self.kind(), "Claude Code", None)
    }

    fn is_available(&self) -> bool {
        self.resolve_program().is_ok()
    }

    /// The ACP broker is driven through the normalized [`AcpSession`] boundary;
    /// the legacy [`AcpConnection`] surface cannot represent a real agent.
    fn spawn(&self, _config: &AgentLaunchConfig) -> Result<Box<dyn AcpConnection>, AcpError> {
        Err(AcpError::new(
            "claude_code requires the AcpSession boundary; use AgentAdapter::spawn_session",
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
                expected: CLAUDE_CODE_EXECUTABLE.to_string(),
                env_var: CLAUDE_CODE_BIN_ENV.to_string(),
                hint: "Install the `claude-code-acp` adapter and add it to PATH.".to_string(),
            },
        };
        AdapterCapability {
            kind: self.kind(),
            display_name: self.descriptor().display_name,
            availability,
            args: self.config.extra_args.clone(),
            auth: AuthRequirement {
                login_command: Some("claude".to_string()),
                env_vars: vec![
                    "ANTHROPIC_API_KEY".to_string(),
                    "ANTHROPIC_AUTH_TOKEN".to_string(),
                    "ANTHROPIC_BASE_URL".to_string(),
                ],
                note: Some(
            "The bundled Claude Agent SDK reuses Claude's own credentials; sign in with the `claude` CLI first or forward ANTHROPIC_* environment variables."
                        .to_string(),
                ),
            },
            boundary: BoundarySupport::default(),
            notes: vec![
                "Claude Code is driven through the bundled `claude-agent-acp` broker."
                    .to_string(),
            ],
            probe: crate::matrix::ProbeOutcome::NotProbed,
        }
    }
}

/// A live Claude Code broker process plus its normalized ACP session.
pub type ClaudeCodeSession = stdio::StdioSession;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_explicit_program_reports_clear_error() {
        let adapter = ClaudeCodeAdapter::new(ClaudeCodeConfig {
            program: Some(PathBuf::from("/definitely/not/claude-code-acp")),
            ..ClaudeCodeConfig::default()
        });
        let error = adapter.resolve_program().unwrap_err();
        assert!(error.to_string().contains("does not exist"));
    }

    #[test]
    fn legacy_spawn_points_callers_at_spawn_session() {
        let adapter = ClaudeCodeAdapter::default();
        let error = match adapter.spawn(&AgentLaunchConfig::default()) {
            Ok(_) => panic!("legacy spawn should not launch the broker"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("spawn_session"));
    }

    #[test]
    fn capability_reports_broker_and_auth() {
        let capability = ClaudeCodeAdapter::default().capability();
        assert_eq!(capability.kind, AgentKind::ClaudeCode);
        assert_eq!(capability.auth.login_command.as_deref(), Some("claude"));
        assert!(capability
            .notes
            .iter()
            .any(|note| note.contains("claude-agent-acp")));
    }

    #[test]
    fn bundled_launch_uses_node_and_acp_entrypoint() {
        let dir = std::env::temp_dir().join(format!(
            "slight-claude-bundle-launch-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::create_dir_all(dir.join("lib/claude-agent-acp/dist")).unwrap();
        std::fs::create_dir_all(dir.join("node_modules/@anthropic-ai/claude-agent-sdk")).unwrap();
        std::fs::create_dir_all(
            dir.join("node_modules/@anthropic-ai/claude-agent-sdk-darwin-arm64"),
        )
        .unwrap();
        std::fs::write(dir.join("bin/node"), b"node").unwrap();
        std::fs::write(dir.join("lib/claude-agent-acp/dist/index.js"), b"js").unwrap();
        std::fs::write(
            dir.join("node_modules/@anthropic-ai/claude-agent-sdk/sdk.mjs"),
            b"js",
        )
        .unwrap();
        std::fs::write(
            dir.join("node_modules/@anthropic-ai/claude-agent-sdk-darwin-arm64/claude"),
            b"claude",
        )
        .unwrap();

        let adapter = ClaudeCodeAdapter::new(ClaudeCodeConfig {
            bundle_dir: Some(dir.clone()),
            ..ClaudeCodeConfig::default()
        });
        let launch = adapter.resolve_launch().unwrap();
        let dir = std::fs::canonicalize(dir).unwrap();
        assert_eq!(launch.program, dir.join("bin/node"));
        assert_eq!(
            launch.args,
            vec![dir
                .join("lib/claude-agent-acp/dist/index.js")
                .to_string_lossy()
                .to_string()]
        );
        assert!(matches!(launch.origin, ClaudeCodeOrigin::Bundled { .. }));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn default_agents_include_claude_code() {
        let registry = crate::AdapterRegistry::with_default_agents();
        assert!(registry.kinds().contains(&AgentKind::ClaudeCode));
    }
}
