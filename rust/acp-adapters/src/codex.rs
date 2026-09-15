//! Real Codex ACP adapter.
//!
//! Codex exposes ACP through the `codex-acp` broker process, which wraps the
//! Codex app-server and speaks ACP over stdio. This module locates that broker,
//! launches it with a sanitized environment, and returns the normalized
//! [`AcpSession`] boundary.
//!
//! Authentication is handled out of band: the broker reuses Codex's own
//! credentials (`codex login`) or the standard OpenAI environment variables.
//! See [`CodexConfig::environment`] and the adapter's
//! [`AgentAdapter::capability`] entry for what is forwarded.
//!
//! # Launch resolution
//!
//! A shipped host must not depend on a globally installed npm/Bun/codex-acp, so
//! the adapter resolves a self-contained bundle first. In order:
//!
//! 1. an explicit [`CodexConfig::program`] or `SLIGHT_CODEX_BIN` (development
//!    and tests; missing explicit paths are an error, never a silent fallback);
//! 2. a bundle directory from [`CodexConfig::bundle_dir`] or
//!    `SLIGHT_CODEX_BUNDLE_DIR`, or one discovered next to the running host
//!    executable (`codex-acp/<triple>`, `../Resources/codex-acp/<triple>`, or
//!    `../codex-acp/<triple>`), where the bundle supplies Node, the codex-acp
//!    entry point, and the native Codex binary;
//! 3. `codex-acp` on `PATH`, only when
//!    [`CodexConfig::allow_global_fallback`] or `SLIGHT_CODEX_ALLOW_GLOBAL=1`
//!    is set.
//!
//! The bundle layout and packaging decision are documented in
//! `docs/architecture/adr-0004-codex-acp-packaging.md` and produced by
//! `scripts/bundle-codex-acp.sh`.

use crate::matrix::{AdapterAvailability, AdapterCapability, AuthRequirement, BoundarySupport};
use crate::process::ChildSpec;
use crate::stdio;
use crate::{AgentAdapter, AgentLaunchConfig};
use acp_types::session::AcpSession;
use acp_types::{AcpConnection, AcpError, AgentDescriptor, AgentKind};
use std::path::{Path, PathBuf};

/// Environment variable used to pin the Codex ACP broker explicitly.
pub const CODEX_BIN_ENV: &str = "SLIGHT_CODEX_BIN";

/// Environment variable pointing at a self-contained Codex ACP bundle root.
/// The value may be the bundle root (containing `<triple>/`) or the target
/// directory itself (containing `bin/`).
pub const CODEX_BUNDLE_ENV: &str = "SLIGHT_CODEX_BUNDLE_DIR";

/// Environment variable that opts into launching a globally installed
/// `codex-acp`. Disabled by default so shipped hosts never silently depend on a
/// developer machine's global install.
pub const CODEX_ALLOW_GLOBAL_ENV: &str = "SLIGHT_CODEX_ALLOW_GLOBAL";

/// The executable name searched on `PATH` for the development fallback.
pub const CODEX_EXECUTABLE: &str = "codex-acp";

const RESOLUTION_HINT: &str =
    "Build the bundle with scripts/bundle-codex-acp.sh and set SLIGHT_CODEX_BUNDLE_DIR, \
     set SLIGHT_CODEX_BIN to a codex-acp executable, or opt into the development fallback \
     with SLIGHT_CODEX_ALLOW_GLOBAL=1.";

/// Launch configuration for the Codex adapter.
#[derive(Debug, Clone, Default)]
pub struct CodexConfig {
    /// Explicit path to a `codex-acp` executable. When `None`, the
    /// `SLIGHT_CODEX_BIN` environment variable is consulted, then the bundle,
    /// then the opt-in global fallback.
    pub program: Option<PathBuf>,
    /// Explicit path to a self-contained Codex ACP bundle root or target
    /// directory. When `None`, `SLIGHT_CODEX_BUNDLE_DIR` and bundle locations
    /// next to the host executable are searched.
    pub bundle_dir: Option<PathBuf>,
    /// When true, a globally installed `codex-acp` on `PATH` may be launched.
    /// Intended for development only; shipped hosts leave this false.
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

/// Where a resolved Codex launch came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexOrigin {
    /// An explicit [`CodexConfig::program`] or `SLIGHT_CODEX_BIN` value.
    Override { program: PathBuf },
    /// A self-contained bundle directory.
    Bundled { directory: PathBuf },
    /// A `codex-acp` executable on `PATH` (development fallback).
    GlobalFallback { program: PathBuf },
}

/// The fully resolved command line for launching the Codex broker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexLaunch {
    /// Executable to spawn. For a bundle this is the bundled Node runtime.
    pub program: PathBuf,
    /// Arguments passed to `program`. For a bundle this is the codex-acp entry.
    pub args: Vec<String>,
    /// Bundle-provided environment (currently just `CODEX_PATH`).
    pub environment: Vec<(String, String)>,
    /// How this launch was chosen.
    pub origin: CodexOrigin,
}

/// Maps the host OS/architecture to the Rust-style target triple used as the
/// bundle directory name.
pub fn bundled_target_triple() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-musl"),
        ("linux", "x86_64") => Some("x86_64-unknown-linux-musl"),
        ("windows", "x86_64") => Some("x86_64-pc-windows-msvc"),
        ("windows", "aarch64") => Some("aarch64-pc-windows-msvc"),
        _ => None,
    }
}

/// Whether `dir` has the full layout produced by `scripts/bundle-codex-acp.sh`.
pub fn is_codex_bundle_dir(dir: &Path) -> bool {
    dir.join("bin/node").is_file()
        && dir.join("lib/codex-acp/index.js").is_file()
        && dir.join("bin/codex").is_file()
}

/// The Codex ACP adapter.
#[derive(Debug, Clone, Default)]
pub struct CodexAdapter {
    config: CodexConfig,
}

impl CodexAdapter {
    pub fn new(config: CodexConfig) -> Self {
        Self { config }
    }

    /// Adapter configured from the ambient environment
    /// (`SLIGHT_CODEX_BIN`, `SLIGHT_CODEX_BUNDLE_DIR`, and `PATH`).
    pub fn from_environment() -> Self {
        Self::default()
    }

    pub fn config(&self) -> &CodexConfig {
        &self.config
    }

    /// Resolves the full Codex broker launch, including bundle-supplied
    /// arguments and environment.
    pub fn resolve_launch(&self) -> Result<CodexLaunch, AcpError> {
        // 1. Explicit overrides win and never silently fall through.
        let explicit = self
            .config
            .program
            .clone()
            .or_else(|| std::env::var_os(CODEX_BIN_ENV).map(PathBuf::from));
        if let Some(program) = explicit {
            let program = stdio::resolve_candidate(&program, RESOLUTION_HINT)?;
            return Ok(CodexLaunch {
                origin: CodexOrigin::Override {
                    program: program.clone(),
                },
                program,
                args: Vec::new(),
                environment: Vec::new(),
            });
        }

        // 2. A self-contained bundle.
        if let Some(directory) = self.resolve_bundle_dir() {
            let program = directory.join("bin/node");
            let entry = directory.join("lib/codex-acp/index.js");
            let codex = directory.join("bin/codex");
            return Ok(CodexLaunch {
                program,
                args: vec![entry.to_string_lossy().into_owned()],
                environment: vec![(
                    "CODEX_PATH".to_string(),
                    codex.to_string_lossy().into_owned(),
                )],
                origin: CodexOrigin::Bundled { directory },
            });
        }

        // 3. Explicit development fallback.
        if self.allow_global_fallback() {
            if let Ok(program) =
                stdio::resolve_program(None, CODEX_BIN_ENV, CODEX_EXECUTABLE, RESOLUTION_HINT)
            {
                return Ok(CodexLaunch {
                    origin: CodexOrigin::GlobalFallback {
                        program: program.clone(),
                    },
                    program,
                    args: Vec::new(),
                    environment: Vec::new(),
                });
            }
        }

        let target = bundled_target_triple().unwrap_or("unknown");
        Err(AcpError::new(format!(
            "no Codex ACP broker found: no explicit program or bundle for {target}, and the \
             global fallback is disabled. {RESOLUTION_HINT}"
        )))
    }

    /// Resolves the ACP broker executable that the adapter will launch.
    pub fn resolve_program(&self) -> Result<PathBuf, AcpError> {
        self.resolve_launch().map(|launch| launch.program)
    }

    /// Spawns the Codex ACP broker and returns the concrete session.
    pub fn spawn_session_typed(
        &self,
        launch: &AgentLaunchConfig,
    ) -> Result<CodexSession, AcpError> {
        let cwd = launch
            .working_directory
            .clone()
            .or_else(|| self.config.working_directory.clone())
            .or_else(|| std::env::current_dir().ok());
        let spec = self.build_spec(launch, cwd.as_deref())?;
        stdio::spawn_session(spec)
    }

    fn allow_global_fallback(&self) -> bool {
        self.config.allow_global_fallback
            || std::env::var(CODEX_ALLOW_GLOBAL_ENV).as_deref() == Ok("1")
    }

    fn resolve_bundle_dir(&self) -> Option<PathBuf> {
        let target = bundled_target_triple()?;

        let mut explicit = Vec::new();
        if let Some(dir) = &self.config.bundle_dir {
            explicit.push(dir.clone());
        }
        if let Some(dir) = std::env::var_os(CODEX_BUNDLE_ENV) {
            explicit.push(PathBuf::from(dir));
        }
        for candidate in explicit {
            if is_codex_bundle_dir(&candidate) {
                return Some(candidate);
            }
            let nested = candidate.join(target);
            if is_codex_bundle_dir(&nested) {
                return Some(nested);
            }
        }

        let executable = std::env::current_exe().ok()?;
        let directory = executable.parent()?;
        for base in [
            directory.join("codex-acp"),
            directory.join("../Resources/codex-acp"),
            directory.join("../codex-acp"),
        ] {
            let candidate = base.join(target);
            if is_codex_bundle_dir(&candidate) {
                return Some(candidate);
            }
        }
        None
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
                    .iter()
                    .cloned()
                    .chain(self.config.extra_args.iter().cloned()),
            )
            .env_clear()
            .capture_stderr();
        if let Some(cwd) = cwd {
            spec = spec.working_directory(cwd);
        }
        // Bundle defaults first, then the sanitized allowlist plus explicit
        // config/launch overrides, so a caller can still point CODEX_PATH at a
        // different Codex binary if it needs to.
        for (key, value) in resolved.environment {
            spec = spec.environment(key, value);
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

impl AgentAdapter for CodexAdapter {
    fn kind(&self) -> AgentKind {
        AgentKind::Codex
    }

    fn descriptor(&self) -> AgentDescriptor {
        AgentDescriptor::new(self.kind(), "Codex", None)
    }

    fn is_available(&self) -> bool {
        self.resolve_launch().is_ok()
    }

    /// The ACP broker is driven through the normalized [`AcpSession`] boundary;
    /// the legacy [`AcpConnection`] surface cannot represent a real agent.
    fn spawn(&self, _config: &AgentLaunchConfig) -> Result<Box<dyn AcpConnection>, AcpError> {
        Err(AcpError::new(
            "codex requires the AcpSession boundary; use AgentAdapter::spawn_session",
        ))
    }

    fn spawn_session(&self, config: &AgentLaunchConfig) -> Result<Box<dyn AcpSession>, AcpError> {
        Ok(Box::new(self.spawn_session_typed(config)?))
    }

    fn capability(&self) -> AdapterCapability {
        let (availability, mut notes) = match self.resolve_launch() {
            Ok(launch) => {
                let source = match &launch.origin {
                    CodexOrigin::Override { .. } => "explicitly configured codex-acp",
                    CodexOrigin::Bundled { .. } => "self-contained codex-acp bundle",
                    CodexOrigin::GlobalFallback { .. } => {
                        "development fallback: globally installed codex-acp"
                    }
                };
                (
                    AdapterAvailability::Available {
                        program: Some(launch.program),
                    },
                    vec![format!("Launching {source}.")],
                )
            }
            Err(_) => (
                AdapterAvailability::MissingExecutable {
                    expected: CODEX_EXECUTABLE.to_string(),
                    env_var: CODEX_BIN_ENV.to_string(),
                    hint: RESOLUTION_HINT.to_string(),
                },
                Vec::new(),
            ),
        };
        notes.push(
            "Codex is not an ACP server; this adapter launches the `codex-acp` broker.".to_string(),
        );
        AdapterCapability {
            kind: self.kind(),
            display_name: self.descriptor().display_name,
            availability,
            args: self.config.extra_args.clone(),
            auth: AuthRequirement {
                login_command: Some("codex login".to_string()),
                env_vars: vec!["OPENAI_API_KEY".to_string()],
                note: Some(
                    "The Codex ACP broker reuses Codex's own credentials; sign in with `codex login` first or forward OPENAI_API_KEY."
                        .to_string(),
                ),
            },
            boundary: BoundarySupport::default(),
            notes,
            probe: crate::matrix::ProbeOutcome::NotProbed,
        }
    }
}

/// A live Codex broker process plus its normalized ACP session.
pub type CodexSession = stdio::StdioSession;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_explicit_program_reports_clear_error() {
        let adapter = CodexAdapter::new(CodexConfig {
            program: Some(PathBuf::from("/definitely/not/codex-acp")),
            ..CodexConfig::default()
        });
        let error = adapter.resolve_program().unwrap_err();
        assert!(error.to_string().contains("does not exist"));
    }

    #[test]
    fn legacy_spawn_points_callers_at_spawn_session() {
        let adapter = CodexAdapter::default();
        let error = match adapter.spawn(&AgentLaunchConfig::default()) {
            Ok(_) => panic!("legacy spawn should not launch the broker"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("spawn_session"));
    }

    #[test]
    fn capability_reports_broker_and_auth() {
        let capability = CodexAdapter::default().capability();
        assert_eq!(capability.kind, AgentKind::Codex);
        assert_eq!(
            capability.auth.login_command.as_deref(),
            Some("codex login")
        );
        assert!(capability
            .notes
            .iter()
            .any(|note| note.contains("codex-acp")));
    }

    #[test]
    fn default_agents_include_codex() {
        let registry = crate::AdapterRegistry::with_default_agents();
        assert!(registry.kinds().contains(&AgentKind::Codex));
    }

    #[test]
    fn global_fallback_is_off_unless_enabled() {
        // A missing bundle plus no explicit program must not resolve through an
        // ambient PATH install unless the fallback is explicitly enabled.
        let adapter = CodexAdapter::new(CodexConfig {
            bundle_dir: Some(PathBuf::from("/definitely/not/a/bundle")),
            ..CodexConfig::default()
        });
        assert!(!adapter.allow_global_fallback());
        let error = adapter.resolve_launch().unwrap_err();
        assert!(error.to_string().contains("global fallback is disabled"));
    }

    #[test]
    fn bundle_layout_detection_requires_every_part() {
        let dir =
            std::env::temp_dir().join(format!("slight-codex-bundle-detect-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::create_dir_all(dir.join("lib/codex-acp")).unwrap();
        assert!(!is_codex_bundle_dir(&dir));
        std::fs::write(dir.join("bin/node"), b"node").unwrap();
        std::fs::write(dir.join("lib/codex-acp/index.js"), b"js").unwrap();
        assert!(!is_codex_bundle_dir(&dir));
        std::fs::write(dir.join("bin/codex"), b"codex").unwrap();
        assert!(is_codex_bundle_dir(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bundled_launch_sets_node_entry_and_codex_path() {
        let dir =
            std::env::temp_dir().join(format!("slight-codex-bundle-launch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::create_dir_all(dir.join("lib/codex-acp")).unwrap();
        std::fs::write(dir.join("bin/node"), b"node").unwrap();
        std::fs::write(dir.join("bin/codex"), b"codex").unwrap();
        std::fs::write(dir.join("lib/codex-acp/index.js"), b"js").unwrap();

        let adapter = CodexAdapter::new(CodexConfig {
            bundle_dir: Some(dir.clone()),
            ..CodexConfig::default()
        });
        let launch = adapter.resolve_launch().unwrap();
        assert_eq!(launch.program, dir.join("bin/node"));
        assert_eq!(
            launch.args,
            vec![dir
                .join("lib/codex-acp/index.js")
                .to_string_lossy()
                .to_string()]
        );
        assert_eq!(
            launch.environment,
            vec![(
                "CODEX_PATH".to_string(),
                dir.join("bin/codex").to_string_lossy().to_string()
            )]
        );
        assert!(matches!(launch.origin, CodexOrigin::Bundled { .. }));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn explicit_program_wins_over_bundle() {
        let dir = std::env::temp_dir().join(format!(
            "slight-codex-bundle-precedence-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::create_dir_all(dir.join("lib/codex-acp")).unwrap();
        std::fs::write(dir.join("bin/node"), b"node").unwrap();
        std::fs::write(dir.join("bin/codex"), b"codex").unwrap();
        std::fs::write(dir.join("lib/codex-acp/index.js"), b"js").unwrap();

        let adapter = CodexAdapter::new(CodexConfig {
            program: Some(PathBuf::from("/bin/echo")),
            bundle_dir: Some(dir.clone()),
            ..CodexConfig::default()
        });
        let launch = adapter.resolve_launch().unwrap();
        assert_eq!(launch.program, PathBuf::from("/bin/echo"));
        assert!(matches!(launch.origin, CodexOrigin::Override { .. }));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
