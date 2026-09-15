//! Integration tests for the real OpenCode ACP adapter.
//!
//! These launch `opencode acp` when it is available on the host and skip with a
//! clear message otherwise. Point `SLIGHT_OPENCODE_BIN` at a specific
//! executable to test a non-`PATH` install. Set `SLIGHT_OPENCODE_E2E=1` to opt
//! into a real prompt turn, which additionally requires `opencode auth login`
//! and network access.

use acp_adapters::opencode::{OpencodeAdapter, OpencodeConfig};
use acp_adapters::{AgentAdapter, AgentLaunchConfig};
use acp_types::session::{AcpSession, AgentEvent};
use acp_types::wire::Implementation;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn opencode_adapter() -> OpencodeAdapter {
    OpencodeAdapter::from_environment()
}

fn skip(reason: &str) {
    eprintln!("skipping OpenCode integration test: {reason}");
}

fn launch_config() -> AgentLaunchConfig {
    AgentLaunchConfig {
        working_directory: Some(std::env::temp_dir()),
        ..AgentLaunchConfig::default()
    }
}

fn open_session(adapter: &OpencodeAdapter) -> acp_adapters::opencode::OpencodeSession {
    let mut session = adapter
        .spawn_session_typed(&launch_config())
        .expect("opencode acp should start");
    let init = session
        .initialize(Implementation::new("slight-test", "0.1.0"))
        .expect("initialize should succeed");
    assert_eq!(
        init.agent_name.as_deref(),
        Some("OpenCode"),
        "unexpected agent info: {init:?}"
    );
    let session_metadata = session
        .new_session(&std::env::temp_dir())
        .expect("session/new should succeed");
    assert!(!session_metadata.agent_session_id.is_empty());
    session
}

#[test]
fn handshake_against_real_opencode_when_available() {
    let adapter = opencode_adapter();
    if !adapter.is_available() {
        skip("`opencode` executable not found; set SLIGHT_OPENCODE_BIN or add it to PATH");
        return;
    }

    let mut session = open_session(&adapter);
    assert!(session.is_running());
    // Bootstrapping OpenCode reads config and starts its server; there should be
    // no fatal diagnostics buffered by this point.
    let drain = session.drain();
    assert!(
        !drain
            .iter()
            .any(|event| matches!(event, AgentEvent::Failed { .. })),
        "unexpected failure during handshake: {drain:?}"
    );
}

#[test]
fn missing_executable_is_reported_clearly() {
    let adapter = OpencodeAdapter::new(OpencodeConfig {
        program: Some(PathBuf::from("/definitely/not/a/real/opencode")),
        ..OpencodeConfig::default()
    });
    let error = match adapter.spawn_session_typed(&launch_config()) {
        Ok(_) => panic!("spawn should fail for a missing executable"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("does not exist"),
        "unhelpful error: {error}"
    );
}

#[test]
fn prompt_round_trip_when_explicitly_enabled() {
    if std::env::var("SLIGHT_OPENCODE_E2E").as_deref() != Ok("1") {
        skip("set SLIGHT_OPENCODE_E2E=1 to run a real prompt turn");
        return;
    }
    let adapter = opencode_adapter();
    if !adapter.is_available() {
        skip("`opencode` executable not found; set SLIGHT_OPENCODE_BIN or add it to PATH");
        return;
    }

    let mut session = open_session(&adapter);
    session
        .prompt("Reply with exactly one word: pong")
        .expect("prompt is accepted");

    let deadline = Instant::now() + Duration::from_secs(120);
    let mut saw_message = false;
    let mut ended = false;
    let mut observed = Vec::new();
    while Instant::now() < deadline && !ended {
        for event in session.drain() {
            match &event {
                AgentEvent::Message { .. } | AgentEvent::Thought { .. } => saw_message = true,
                AgentEvent::TurnEnded { .. } => ended = true,
                _ => {}
            }
            observed.push(event);
        }
        if !ended {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    assert!(ended, "prompt turn did not end; saw: {observed:?}");
    assert!(
        saw_message,
        "expected at least one agent message; saw: {observed:?}"
    );
}
