//! Opt-in smoke test for a self-contained Claude Agent ACP bundle.

use acp_adapters::claude_code::{ClaudeCodeAdapter, ClaudeCodeConfig};
use acp_types::session::AcpSession;
use acp_types::wire::Implementation;
use std::path::PathBuf;

#[test]
fn bundled_broker_completes_initialize() {
    if std::env::var("SLIGHT_CLAUDE_ACP_SMOKE").as_deref() != Ok("1") {
        eprintln!("skipping Claude bundle smoke test: set SLIGHT_CLAUDE_ACP_SMOKE=1");
        return;
    }
    let bundle_dir = PathBuf::from(
        std::env::var_os("SLIGHT_CLAUDE_CODE_BUNDLE_DIR")
            .expect("SLIGHT_CLAUDE_CODE_BUNDLE_DIR must point at a bundle"),
    );
    let adapter = ClaudeCodeAdapter::new(ClaudeCodeConfig {
        bundle_dir: Some(bundle_dir),
        allow_global_fallback: false,
        ..ClaudeCodeConfig::default()
    });
    let mut session = adapter
        .spawn_session_typed(&acp_adapters::AgentLaunchConfig {
            working_directory: Some(std::env::temp_dir()),
            ..Default::default()
        })
        .expect("bundled broker should spawn");
    let summary = session
        .initialize(Implementation::new("slight-bundle-smoke", "0.1.0"))
        .expect("bundled broker should complete ACP initialize");
    assert_eq!(summary.protocol_version, 1);
}
