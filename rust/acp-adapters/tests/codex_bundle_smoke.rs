//! Opt-in smoke test for a built self-contained Codex ACP bundle.
//!
//! Unlike the adapter-matrix probes, this test requires a bundle produced by
//! `scripts/bundle-codex-acp.sh` and never falls back to a global install. It
//! launches the bundled broker and completes the ACP `initialize` handshake,
//! which needs no authentication, so it can run in release CI before any Codex
//! session or credentials exist.
//!
//! Enable it with:
//!
//! ```text
//! SLIGHT_CODEX_ACP_SMOKE=1 \
//! SLIGHT_CODEX_BUNDLE_DIR=<repo>/vendor/codex-acp/<triple> \
//! cargo test -p acp-adapters --test codex_bundle_smoke -- --nocapture
//! ```

use acp_adapters::codex::{CodexAdapter, CodexConfig};
use acp_types::session::AcpSession;
use acp_types::wire::Implementation;
use std::path::PathBuf;

fn bundle_dir_from_env() -> Option<PathBuf> {
    std::env::var_os("SLIGHT_CODEX_BUNDLE_DIR").map(PathBuf::from)
}

#[test]
fn bundled_broker_completes_initialize() {
    if std::env::var("SLIGHT_CODEX_ACP_SMOKE").as_deref() != Ok("1") {
        eprintln!("skipping bundle smoke test: set SLIGHT_CODEX_ACP_SMOKE=1 to enable");
        return;
    }

    let Some(bundle_dir) = bundle_dir_from_env() else {
        eprintln!(
            "skipping bundle smoke test: set SLIGHT_CODEX_BUNDLE_DIR to a bundle <triple> directory"
        );
        return;
    };

    let adapter = CodexAdapter::new(CodexConfig {
        bundle_dir: Some(bundle_dir.clone()),
        allow_global_fallback: false,
        ..CodexConfig::default()
    });

    let launch = adapter.resolve_launch().unwrap_or_else(|error| {
        panic!(
            "bundle at {} did not resolve: {error}",
            bundle_dir.display()
        )
    });
    eprintln!("resolved bundled launch: {launch:?}");

    let mut session = adapter
        .spawn_session_typed(&acp_adapters::AgentLaunchConfig {
            working_directory: Some(std::env::temp_dir()),
            ..Default::default()
        })
        .expect("bundled broker should spawn");

    let summary = session
        .initialize(Implementation::new("slight-bundle-smoke", "0.1.0"))
        .expect("bundled broker should complete ACP initialize");

    eprintln!(
        "bundle negotiated protocol v{} as {:?} with auth methods {:?}",
        summary.protocol_version,
        summary.agent_name,
        summary
            .auth_methods
            .iter()
            .map(|method| method.id.clone())
            .collect::<Vec<_>>()
    );

    assert_eq!(summary.protocol_version, 1, "expected ACP v1");
    let agent_name = summary.agent_name.unwrap_or_default().to_lowercase();
    assert!(
        agent_name.contains("codex"),
        "expected a Codex agent, got {agent_name:?}"
    );
    assert!(
        !summary.auth_methods.is_empty(),
        "bundled Codex broker should advertise auth methods"
    );
}
