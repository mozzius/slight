//! `session-core` driven by the real ACP client over the wire-level fake agent.
//!
//! These tests register a small [`AgentAdapter`] that hands `session-core` a
//! [`ClientSession`] connected to `acp-adapters::wire_agent` over an in-process
//! pipe pair. That exercises the full path — JSON-RPC framing, initialize and
//! `session/new` correlation, streaming `session/update` normalization, the
//! deferred permission exchange, cancellation, and structured handshake
//! failures — without a real agent installed.

use acp_adapters::wire_agent::{spawn_in_process_agent, FakeWireAgentConfig};
use acp_adapters::{AdapterRegistry, AgentAdapter, AgentLaunchConfig};
use acp_types::session::{AcpSession, ClientSession};
use acp_types::{AcpError, AgentDescriptor, AgentKind, MessageRole};
use session_core::{
    CreateSessionRequest, ImportSessionRequest, SessionDetail, SessionError, SessionEvent,
    SessionManager, SessionRecovery, SessionStatus,
};
use session_store::InMemoryStore;
use std::sync::Arc;
use std::time::{Duration, Instant};

struct WireFakeAdapter {
    config: FakeWireAgentConfig,
    kind: AgentKind,
}

impl AgentAdapter for WireFakeAdapter {
    fn kind(&self) -> AgentKind {
        self.kind.clone()
    }

    fn descriptor(&self) -> AgentDescriptor {
        AgentDescriptor::new(
            self.kind.clone(),
            "Wire fake agent",
            Some("0.1.0".to_string()),
        )
    }

    fn spawn(
        &self,
        _config: &AgentLaunchConfig,
    ) -> Result<Box<dyn acp_types::AcpConnection>, AcpError> {
        Err(AcpError::new(
            "wire adapter only implements the normalized boundary",
        ))
    }

    fn spawn_session(&self, _config: &AgentLaunchConfig) -> Result<Box<dyn AcpSession>, AcpError> {
        let (transport, handle) = spawn_in_process_agent(self.config.clone())
            .map_err(|error| AcpError::new(error.to_string()))?;
        // Keep the in-process agent alive until the client transport closes.
        std::thread::spawn(move || {
            let _ = handle.join();
        });
        Ok(Box::new(ClientSession::new(transport)))
    }
}

fn kind() -> AgentKind {
    AgentKind::Custom("wire-fake".to_string())
}

fn manager(config: FakeWireAgentConfig) -> Arc<SessionManager> {
    let mut registry = AdapterRegistry::new();
    registry.register(Arc::new(WireFakeAdapter {
        config,
        kind: kind(),
    }));
    Arc::new(SessionManager::new(
        Arc::new(registry),
        Box::new(InMemoryStore::new()),
        128,
    ))
}

fn create(manager: &SessionManager, prompt: &str) -> String {
    manager
        .create_session(CreateSessionRequest {
            agent: kind(),
            model: None,
            effort: None,
            working_directory_label: "~/work/slight".to_string(),
            initial_prompt: Some(prompt.to_string()),
        })
        .expect("wire session creates")
        .id
}

fn pump_until(
    manager: &SessionManager,
    id: &str,
    predicate: impl Fn(&SessionDetail) -> bool,
) -> SessionDetail {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        manager.pump();
        if let Ok(detail) = manager.inspect_session(id) {
            if predicate(&detail) {
                return detail;
            }
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for the session to reach the expected state"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn wire_session_exposes_initial_config_options() {
    let manager = manager(FakeWireAgentConfig::default());
    let id = manager
        .create_session(CreateSessionRequest {
            agent: kind(),
            model: None,
            effort: None,
            working_directory_label: "~/work/slight".to_string(),
            initial_prompt: None,
        })
        .expect("wire session creates")
        .id;

    let detail = manager.inspect_session(&id).expect("session exists");
    assert_eq!(detail.acp.config_options.len(), 2);
    assert_eq!(detail.acp.config_options[0].id, "model");
    assert_eq!(detail.acp.config_options[1].id, "brave_mode");
}

#[test]
fn wire_session_streams_updates_and_permission() {
    let manager = manager(FakeWireAgentConfig::default());
    let id = create(&manager, "hello");

    let detail = pump_until(&manager, &id, |detail| {
        detail.summary.status == SessionStatus::WaitingPermission
    });
    assert_eq!(detail.agent_session_id.as_deref(), Some("session-1"));
    assert_eq!(detail.acp.protocol_version, 1);
    assert!(detail.acp.modes.is_none());
    assert!(detail.acp.capabilities.load_session);
    assert!(detail.acp.capabilities.session_list);
    assert!(detail.acp.capabilities.session_resume);
    assert!(detail.pending_permission.is_some());
    assert!(detail.running);
    assert!(detail
        .recent_events
        .iter()
        .any(|event| matches!(event.event, SessionEvent::Message(_))));
    assert!(detail
        .recent_events
        .iter()
        .any(|event| matches!(event.event, SessionEvent::ToolCall(_))));
    // The agent's `session/new` options are replaced by the streamed
    // `config_option_update`, so only the updated option remains.
    assert_eq!(detail.acp.config_options.len(), 1);
    assert_eq!(detail.acp.config_options[0].id, "brave_mode");
    assert_eq!(
        detail.acp.config_options[0].kind,
        acp_types::metadata::AcpConfigKind::Boolean {
            current_value: true
        }
    );
    assert!(detail.recent_events.iter().any(|sequenced| matches!(
        &sequenced.event,
        SessionEvent::ConfigOptions(payload)
            if payload.options.iter().any(|option| option.id == "brave_mode")
    )));
    assert!(detail.recent_events.iter().any(|sequenced| matches!(
        &sequenced.event,
        SessionEvent::SessionInfo(payload) if payload.title.as_deref() == Some("Wire session")
    )));
    assert!(detail.recent_events.iter().any(|sequenced| matches!(
        &sequenced.event,
        SessionEvent::Usage(payload) if payload.used == 1_024
    )));
    assert_eq!(detail.summary.title, "Wire session");

    let pending = detail.pending_permission.unwrap();
    manager
        .respond_permission(&id, &pending.id, "allow")
        .expect("permission response is accepted by the wire session");

    let after = pump_until(&manager, &id, |detail| {
        detail.summary.status == SessionStatus::Idle && detail.pending_permission.is_none()
    });
    assert!(after
        .recent_events
        .iter()
        .any(|sequenced| match &sequenced.event {
            SessionEvent::Message(message) => message.text.contains("permission:allow"),
            _ => false,
        }));
    assert!(after.recent_events.iter().any(|sequenced| matches!(
        &sequenced.event,
        SessionEvent::TurnEnded(payload) if payload.stop_reason == "end_turn"
    )));
}

#[test]
fn wire_session_import_replays_agent_history() {
    let manager = manager(FakeWireAgentConfig::default());

    let listed = manager
        .list_agent_sessions(&kind(), None, None)
        .expect("wire session list succeeds");
    assert_eq!(listed.sessions.len(), 1);
    assert_eq!(listed.sessions[0].agent_session_id, "session-1");

    let summary = manager
        .import_session(ImportSessionRequest {
            agent: kind(),
            agent_session_id: "session-1".to_string(),
            working_directory_label: "~/work/slight".to_string(),
            recovery: SessionRecovery::Load,
            title: Some("Imported wire session".to_string()),
        })
        .expect("import succeeds");
    assert_eq!(summary.title, "Imported wire session");

    let detail = manager
        .inspect_session(&summary.id)
        .expect("session exists");
    assert_eq!(detail.agent_session_id.as_deref(), Some("session-1"));
    assert!(detail.acp.modes.is_some());
    assert!(detail.recent_events.iter().any(|sequenced| matches!(
        &sequenced.event,
        SessionEvent::Message(message)
            if message.role == MessageRole::User && message.text == "replayed user"
    )));
    assert!(detail.recent_events.iter().any(|sequenced| matches!(
        &sequenced.event,
        SessionEvent::Message(message)
            if message.role == MessageRole::Assistant && message.text == "replayed assistant"
    )));
}

#[test]
fn wire_session_cancel_surfaces_cancelled_stop_reason() {
    let manager = manager(FakeWireAgentConfig::default());
    let id = create(&manager, "hello");
    pump_until(&manager, &id, |detail| {
        detail.summary.status == SessionStatus::WaitingPermission
    });

    manager.cancel(&id).expect("cancel is accepted");

    let detail = pump_until(&manager, &id, |detail| {
        detail.recent_events.iter().any(|sequenced| {
            matches!(
                &sequenced.event,
                SessionEvent::Diagnostics(diagnostics) if diagnostics.message.contains("Cancelled")
            )
        })
    });
    assert!(detail.pending_permission.is_none());
    assert_eq!(detail.summary.status, SessionStatus::Idle);
}

struct ClosedAdapter {
    kind: AgentKind,
}

impl AgentAdapter for ClosedAdapter {
    fn kind(&self) -> AgentKind {
        self.kind.clone()
    }

    fn descriptor(&self) -> AgentDescriptor {
        AgentDescriptor::new(self.kind.clone(), "Closed agent", None)
    }

    fn spawn(
        &self,
        _config: &AgentLaunchConfig,
    ) -> Result<Box<dyn acp_types::AcpConnection>, AcpError> {
        Err(AcpError::new(
            "closed adapter only implements the normalized boundary",
        ))
    }

    fn spawn_session(&self, _config: &AgentLaunchConfig) -> Result<Box<dyn AcpSession>, AcpError> {
        // An immediately-closed stream models an agent that dies during the
        // handshake.
        let transport = acp_types::transport::Transport::new(
            std::io::BufReader::new(std::io::Cursor::new(Vec::new())),
            Vec::new(),
        );
        Ok(Box::new(ClientSession::new(transport)))
    }
}

#[test]
fn wire_session_handshake_failure_is_reported() {
    let closed = AgentKind::Custom("closed".to_string());
    let mut registry = AdapterRegistry::new();
    registry.register(Arc::new(ClosedAdapter {
        kind: closed.clone(),
    }));
    let manager = SessionManager::new(Arc::new(registry), Box::new(InMemoryStore::new()), 16);

    let error = manager
        .create_session(CreateSessionRequest {
            agent: closed,
            model: Some("auto".to_string()),
            effort: Some("medium".to_string()),
            working_directory_label: "~".to_string(),
            initial_prompt: None,
        })
        .unwrap_err();
    assert!(matches!(error, SessionError::Agent(_)));
}
