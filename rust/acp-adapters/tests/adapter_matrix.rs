//! Opt-in integration coverage for the real adapter matrix.
//!
//! The static assertions always run: every real adapter is registered, reports
//! its launch/auth configuration, and serializes into the capability report.
//!
//! The probes run only when the relevant executable is installed. A missing
//! binary is reported as `unavailable`, never as support, and a broker that
//! cannot complete the ACP handshake is reported as `failed`. The full matrix is
//! printed as JSON so a failing run explains exactly what the host found.
//!
//! Set `SLIGHT_ADAPTER_E2E=1` to opt into real prompt turns. That additionally
//! requires each agent to be authenticated (`opencode auth login`,
//! `claude` sign-in, `codex login`) and network access.

use acp_adapters::claude_code::{ClaudeCodeAdapter, ClaudeCodeConfig};
use acp_adapters::codex::{CodexAdapter, CodexConfig};
use acp_adapters::matrix::{CapabilityMatrix, ProbeOutcome};
use acp_adapters::opencode::{OpencodeAdapter, OpencodeConfig};
use acp_adapters::{AdapterRegistry, AgentAdapter, AgentLaunchConfig, DefaultAgentConfig};
use acp_types::session::AgentEvent;
use acp_types::wire::Implementation;
use acp_types::AgentKind;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn launch_config() -> AgentLaunchConfig {
    AgentLaunchConfig {
        working_directory: Some(std::env::temp_dir()),
        ..AgentLaunchConfig::default()
    }
}

fn print_matrix(matrix: &CapabilityMatrix) {
    match matrix.to_json_pretty() {
        Ok(json) => eprintln!("adapter capability matrix:\n{json}"),
        Err(error) => eprintln!("failed to render capability matrix: {error}"),
    }
}

#[test]
fn static_matrix_covers_every_real_adapter() {
    let registry = AdapterRegistry::with_default_agents();
    let matrix = registry.capability_matrix();

    for kind in [
        AgentKind::Custom("opencode".to_string()),
        AgentKind::ClaudeCode,
        AgentKind::Codex,
    ] {
        assert!(
            matrix.kinds().contains(&kind),
            "capability matrix is missing {kind}"
        );
    }

    for entry in &matrix.entries {
        assert!(
            entry.boundary.initialize && entry.boundary.new_session && entry.boundary.prompt,
            "{} must implement the normalized boundary",
            entry.kind
        );
        assert_eq!(
            entry.probe,
            ProbeOutcome::NotProbed,
            "static matrix must not claim probe results"
        );
    }

    // Every real adapter must name how the user authenticates.
    for kind in [
        AgentKind::Custom("opencode".to_string()),
        AgentKind::ClaudeCode,
        AgentKind::Codex,
    ] {
        let entry = matrix
            .entries
            .iter()
            .find(|entry| entry.kind == kind)
            .expect("real adapter entry");
        assert!(
            !entry.auth.env_vars.is_empty() || entry.auth.login_command.is_some(),
            "{} must document an auth path",
            entry.kind
        );
    }

    print_matrix(&matrix);
}

#[test]
fn missing_binaries_are_reported_as_unavailable_not_supported() {
    // Force every real adapter at a path that cannot exist so the test is
    // deterministic regardless of what is installed on the host.
    let registry = AdapterRegistry::with_configured_agents(DefaultAgentConfig {
        opencode: OpencodeConfig {
            program: Some(PathBuf::from("/definitely/not/opencode")),
            ..OpencodeConfig::default()
        },
        claude_code: ClaudeCodeConfig {
            program: Some(PathBuf::from("/definitely/not/claude-code-acp")),
            ..ClaudeCodeConfig::default()
        },
        codex: CodexConfig {
            program: Some(PathBuf::from("/definitely/not/codex-acp")),
            ..CodexConfig::default()
        },
    });
    let matrix = registry.probe_capability_matrix(&launch_config());
    print_matrix(&matrix);

    for entry in &matrix.entries {
        assert!(
            matches!(entry.probe, ProbeOutcome::Unavailable { .. }),
            "{} should be unavailable, got {:?}",
            entry.kind,
            entry.probe
        );
    }
}

#[test]
fn probe_reports_installed_agents_and_their_gaps() {
    let registry = AdapterRegistry::with_default_agents();
    let matrix = registry.probe_capability_matrix(&launch_config());
    print_matrix(&matrix);

    for entry in &matrix.entries {
        match (&entry.availability, &entry.probe) {
            (acp_adapters::matrix::AdapterAvailability::Available { .. }, probe) => {
                assert!(
                    !matches!(probe, ProbeOutcome::Unavailable { .. }),
                    "{} is installed but probed unavailable",
                    entry.kind
                );
                if let ProbeOutcome::Compatible(summary) = probe {
                    eprintln!(
                        "{} negotiated protocol v{} (agent={:?}); unsupported: {:?}",
                        entry.kind,
                        summary.protocol_version,
                        summary.agent_name,
                        summary.unsupported_capabilities()
                    );
                } else {
                    eprintln!(
                        "{} is installed but did not complete the ACP handshake: {probe:?}",
                        entry.kind
                    );
                }
            }
            (_, ProbeOutcome::Unavailable { reason }) => {
                eprintln!("{} unavailable: {reason}", entry.kind);
            }
            _ => {}
        }
    }
}

#[test]
fn real_prompt_turn_when_explicitly_enabled() {
    if std::env::var("SLIGHT_ADAPTER_E2E").as_deref() != Ok("1") {
        eprintln!("skipping real prompt turn: set SLIGHT_ADAPTER_E2E=1 to enable");
        return;
    }

    let registry = AdapterRegistry::with_default_agents();
    let mut tested = 0;
    for kind in [
        AgentKind::Custom("opencode".to_string()),
        AgentKind::ClaudeCode,
        AgentKind::Codex,
    ] {
        let Some(adapter) = registry.get(&kind) else {
            continue;
        };
        if !adapter.is_available() {
            eprintln!("skipping {kind}: executable not installed");
            continue;
        }

        let launch = launch_config();
        let mut session = match adapter.spawn_session(&launch) {
            Ok(session) => session,
            Err(error) => {
                eprintln!("skipping {kind}: spawn failed: {error}");
                continue;
            }
        };
        if let Err(error) = session.initialize(Implementation::new("slight-e2e", "0.1.0")) {
            eprintln!("skipping {kind}: initialize failed (auth?): {error}");
            continue;
        }
        let cwd = std::env::temp_dir();
        if let Err(error) = session.new_session(&cwd) {
            eprintln!("skipping {kind}: session/new failed (auth?): {error}");
            continue;
        }
        if let Err(error) = session.prompt("Reply with exactly one word: pong") {
            eprintln!("skipping {kind}: prompt rejected: {error}");
            continue;
        }

        let deadline = Instant::now() + Duration::from_secs(180);
        let mut ended = false;
        let mut saw_message = false;
        while Instant::now() < deadline && !ended {
            for event in session.drain() {
                match &event {
                    AgentEvent::Message { .. } | AgentEvent::Thought { .. } => saw_message = true,
                    AgentEvent::TurnEnded { .. } => ended = true,
                    AgentEvent::Failed { message } => {
                        eprintln!("{kind} reported failure: {message}");
                    }
                    _ => {}
                }
            }
            if !ended {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        assert!(ended, "{kind} prompt turn did not finish");
        assert!(saw_message, "{kind} produced no message or thought");
        tested += 1;
    }

    if tested == 0 {
        eprintln!("no authenticated agents available for a real prompt turn");
    }
}

#[test]
fn adapter_configs_expose_their_launch_shape() {
    let opencode = OpencodeAdapter::default().capability();
    assert_eq!(opencode.args, vec!["acp".to_string()]);

    let claude = ClaudeCodeAdapter::default().capability();
    assert_eq!(claude.kind, AgentKind::ClaudeCode);
    assert!(claude.args.is_empty());

    let codex = CodexAdapter::default().capability();
    assert_eq!(codex.kind, AgentKind::Codex);
    assert!(codex.args.is_empty());
}
