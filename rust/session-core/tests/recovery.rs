//! `session-core` discovery and import through the normalized ACP boundary.
//!
//! These tests use the in-memory fake adapter to prove that `session/list`,
//! `session/load`, and `session/resume` are reachable from the session manager
//! and that load replay is preserved as normalized [`SessionEvent`]s in the
//! bounded journal.

use acp_adapters::fake::FakeAgentConnection;
use acp_adapters::{AdapterRegistry, AgentAdapter, AgentLaunchConfig};
use acp_types::{AcpConnection, AcpError, AgentDescriptor, AgentKind, MessageRole};
use session_core::{
    ImportSessionRequest, SessionError, SessionEvent, SessionManager, SessionRecovery,
    SessionStatus,
};
use session_store::InMemoryStore;
use std::sync::Arc;

fn manager() -> Arc<SessionManager> {
    Arc::new(SessionManager::new(
        Arc::new(AdapterRegistry::with_builtins()),
        Box::new(InMemoryStore::new()),
        128,
    ))
}

/// An adapter that never overrides `spawn_session`, so the default wraps a
/// legacy connection in `LegacyAcpSession`, which advertises no session
/// discovery capabilities.
struct LegacyOnlyAdapter;

const LEGACY_ONLY: &str = "legacy-only";

impl AgentAdapter for LegacyOnlyAdapter {
    fn kind(&self) -> AgentKind {
        AgentKind::Custom(LEGACY_ONLY.to_string())
    }

    fn descriptor(&self) -> AgentDescriptor {
        AgentDescriptor::new(self.kind(), "Legacy only", None)
    }

    fn spawn(&self, _config: &AgentLaunchConfig) -> Result<Box<dyn AcpConnection>, AcpError> {
        Ok(Box::new(FakeAgentConnection::new(Default::default())))
    }
}

fn manager_with(adapter: Arc<dyn AgentAdapter>) -> Arc<SessionManager> {
    let mut registry = AdapterRegistry::new();
    registry.register(adapter);
    Arc::new(SessionManager::new(
        Arc::new(registry),
        Box::new(InMemoryStore::new()),
        128,
    ))
}

fn import(
    manager: &SessionManager,
    agent_session_id: &str,
    recovery: SessionRecovery,
) -> session_core::SessionSummary {
    manager
        .import_session(ImportSessionRequest {
            agent: AgentKind::Fake,
            agent_session_id: agent_session_id.to_string(),
            working_directory_label: "~/work/slight".to_string(),
            recovery,
            title: Some("Imported session".to_string()),
        })
        .expect("session imports")
}

#[test]
fn list_agent_sessions_preserves_native_identity() {
    let manager = manager();
    let sessions = manager
        .list_agent_sessions(&AgentKind::Fake, None, None)
        .expect("session list succeeds");
    assert_eq!(sessions.sessions.len(), 2);
    assert_eq!(sessions.sessions[0].agent_session_id, "fake-listed-1");
    assert_eq!(sessions.sessions[0].cwd, "/tmp/slight/fake-project");
    assert_eq!(
        sessions.sessions[0].title.as_deref(),
        Some("Prior fake session")
    );
}

#[test]
fn load_import_replays_history_as_normalized_events() {
    let manager = manager();
    let summary = import(&manager, "fake-listed-1", SessionRecovery::Load);
    assert_eq!(summary.title, "Imported session");
    assert_eq!(summary.status, SessionStatus::Idle);

    let detail = manager.inspect_session(&summary.id).unwrap();
    assert_eq!(detail.agent_session_id.as_deref(), Some("fake-listed-1"));
    assert!(detail.recent_events.iter().any(|sequenced| matches!(
        &sequenced.event,
        SessionEvent::Message(message)
            if message.role == MessageRole::User && message.text == "replayed question"
    )));
    assert!(detail.recent_events.iter().any(|sequenced| matches!(
        &sequenced.event,
        SessionEvent::Message(message)
            if message.role == MessageRole::Assistant && message.text == "replayed answer"
    )));
}

#[test]
fn resume_import_connects_without_replaying_history() {
    let manager = manager();
    let summary = import(&manager, "fake-listed-2", SessionRecovery::Resume);
    let detail = manager.inspect_session(&summary.id).unwrap();
    assert_eq!(detail.agent_session_id.as_deref(), Some("fake-listed-2"));
    assert!(!detail.recent_events.iter().any(|sequenced| matches!(
        &sequenced.event,
        SessionEvent::Message(message) if message.text.starts_with("replayed")
    )));
}

#[test]
fn discovery_reports_unsupported_capability_for_legacy_agents() {
    let manager = manager_with(Arc::new(LegacyOnlyAdapter));
    let error = manager
        .list_agent_sessions(&AgentKind::Custom(LEGACY_ONLY.to_string()), None, None)
        .expect_err("legacy agents cannot list sessions");
    assert!(matches!(
        error,
        SessionError::UnsupportedCapability("session/list")
    ));
}

#[test]
fn import_reports_unsupported_capability_for_legacy_agents() {
    let manager = manager_with(Arc::new(LegacyOnlyAdapter));
    for recovery in [SessionRecovery::Load, SessionRecovery::Resume] {
        let error = manager
            .import_session(ImportSessionRequest {
                agent: AgentKind::Custom(LEGACY_ONLY.to_string()),
                agent_session_id: "native-1".to_string(),
                working_directory_label: "~/work/slight".to_string(),
                recovery,
                title: None,
            })
            .expect_err("legacy agents cannot import sessions");
        let capability = match recovery {
            SessionRecovery::Load => "session/load",
            SessionRecovery::Resume => "session/resume",
        };
        assert!(matches!(
            error,
            SessionError::UnsupportedCapability(reported) if reported == capability
        ));
    }
}
