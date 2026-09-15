//! Durable restart recovery.
//!
//! These tests persist sessions into a SQLite store, drop the `SessionManager`
//! (simulating a host shutdown), and open a new manager over the same store.
//! Recovery must resume the native agent session when it still exists, and must
//! report an explicit stale/unavailable state instead of silently creating a
//! fresh session when it does not.

use acp_adapters::{AdapterRegistry, AgentAdapter, AgentLaunchConfig};
use acp_types::metadata::SessionMetadata;
use acp_types::session::{AcpSession, AgentEvent, InitializeSummary, ListedSessionPage};
use acp_types::wire::{
    AgentCapabilities, Implementation, SessionCapabilities, SessionResumeCapabilities,
};
use acp_types::{AcpConnection, AcpError, AgentDescriptor, AgentKind};
use session_core::{
    CreateSessionRequest, ImportSessionRequest, SessionError, SessionEvent, SessionManager,
    SessionRecovery, SessionRecoveryState, SessionStatus,
};
use session_store::SqliteStore;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn store_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "slight-restart-recovery-{}-{}.sqlite",
        std::process::id(),
        uuid::Uuid::new_v4()
    ))
}

fn manager(adapters: Arc<AdapterRegistry>, path: &Path) -> Arc<SessionManager> {
    Arc::new(SessionManager::new(
        adapters,
        Box::new(SqliteStore::open(path, 256).expect("store opens")),
        256,
    ))
}

fn create(manager: &SessionManager, prompt: Option<&str>) -> session_core::SessionSummary {
    manager
        .create_session(CreateSessionRequest {
            agent: AgentKind::Fake,
            model: None,
            effort: None,
            working_directory_label: "~/work/slight".to_string(),
            initial_prompt: prompt.map(str::to_string),
        })
        .expect("session creates")
}

fn import_load(manager: &SessionManager, native_id: &str) -> session_core::SessionSummary {
    manager
        .import_session(ImportSessionRequest {
            agent: AgentKind::Fake,
            agent_session_id: native_id.to_string(),
            working_directory_label: "~/work/slight".to_string(),
            recovery: SessionRecovery::Load,
            title: Some("Imported session".to_string()),
        })
        .expect("session imports")
}

#[test]
fn restart_resumes_created_session_and_preserves_journal() {
    let path = store_path();
    let first = manager(Arc::new(AdapterRegistry::with_builtins()), &path);
    let created = create(&first, Some("hello"));
    assert_eq!(created.recovery, SessionRecoveryState::Live);
    let native_id = first
        .inspect_session(&created.id)
        .expect("session exists")
        .agent_session_id
        .expect("native session id bound");
    let before = first.inspect_session(&created.id).expect("session exists");
    let last_sequence = before.summary.last_sequence;
    let event_count = before.recent_events.len();
    drop(first);

    let restarted = manager(Arc::new(AdapterRegistry::with_builtins()), &path);
    let sessions = restarted.list_sessions();
    let recovered = sessions
        .iter()
        .find(|session| session.id == created.id)
        .expect("persisted session is listed after restart");
    assert_eq!(recovered.recovery, SessionRecoveryState::Recovered);
    assert_eq!(recovered.status, SessionStatus::Idle);
    assert_eq!(recovered.last_sequence, last_sequence);

    let detail = restarted
        .inspect_session(&created.id)
        .expect("recovered session inspects");
    assert!(detail.running);
    assert_eq!(detail.agent_session_id.as_deref(), Some(native_id.as_str()));
    assert_eq!(detail.recent_events.len(), event_count);

    // The recovered session is live: input reaches the resumed agent.
    restarted
        .send_input(&created.id, "continue")
        .expect("recovered session accepts input");
    let detail = restarted.inspect_session(&created.id).unwrap();
    assert!(detail.recent_events.iter().any(|sequenced| matches!(
        &sequenced.event,
        SessionEvent::Message(message) if message.text.starts_with("echo: continue")
    )));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn restart_resumes_imported_session_and_keeps_replayed_journal() {
    let path = store_path();
    let first = manager(Arc::new(AdapterRegistry::with_builtins()), &path);
    let imported = import_load(&first, "fake-listed-1");
    let before = first.inspect_session(&imported.id).expect("session exists");
    assert!(before.recent_events.iter().any(|sequenced| matches!(
        &sequenced.event,
        SessionEvent::Message(message) if message.text == "replayed answer"
    )));
    let last_sequence = before.summary.last_sequence;
    drop(first);

    let restarted = manager(Arc::new(AdapterRegistry::with_builtins()), &path);
    let recovered = restarted
        .list_sessions()
        .into_iter()
        .find(|session| session.id == imported.id)
        .expect("imported session is listed after restart");
    assert_eq!(recovered.recovery, SessionRecoveryState::Recovered);

    let detail = restarted.inspect_session(&imported.id).unwrap();
    assert!(detail.running);
    assert_eq!(detail.summary.last_sequence, last_sequence);
    assert!(detail.recent_events.iter().any(|sequenced| matches!(
        &sequenced.event,
        SessionEvent::Message(message) if message.text == "replayed answer"
    )));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn missing_agent_reports_unavailable_without_replacement_session() {
    let path = store_path();
    let first = manager(Arc::new(AdapterRegistry::with_builtins()), &path);
    let created = create(&first, None);
    drop(first);

    // A host that does not know this agent cannot recover the session, but it
    // must still surface it rather than dropping or replacing it.
    let restarted = manager(Arc::new(AdapterRegistry::new()), &path);
    let sessions = restarted.list_sessions();
    assert_eq!(sessions.len(), 1);
    let recovered = &sessions[0];
    assert_eq!(recovered.id, created.id);
    assert!(matches!(
        recovered.recovery,
        SessionRecoveryState::Unavailable { .. }
    ));
    assert_eq!(recovered.status, SessionStatus::Exited);
    assert_eq!(restarted.session_count(), 1);
    assert_eq!(restarted.active_session_count(), 0);

    let _ = std::fs::remove_file(&path);
}

// ---------------------------------------------------------------------------
// An adapter whose agent can be told to reject native sessions. It models a
// host restart against an agent that has since forgotten (or deleted) the
// native session, which is exactly when recovery must report `stale`.
// ---------------------------------------------------------------------------

struct FlakyResumeAdapter {
    kind: AgentKind,
    fail_resume: Arc<AtomicBool>,
}

impl AgentAdapter for FlakyResumeAdapter {
    fn kind(&self) -> AgentKind {
        self.kind.clone()
    }

    fn descriptor(&self) -> AgentDescriptor {
        AgentDescriptor::new(
            self.kind.clone(),
            "Flaky resume agent",
            Some("0.1.0".to_string()),
        )
    }

    fn spawn(&self, _config: &AgentLaunchConfig) -> Result<Box<dyn AcpConnection>, AcpError> {
        Err(AcpError::new(
            "flaky adapter only implements the normalized boundary",
        ))
    }

    fn spawn_session(&self, _config: &AgentLaunchConfig) -> Result<Box<dyn AcpSession>, AcpError> {
        Ok(Box::new(FlakyResumeSession {
            fail_resume: Arc::clone(&self.fail_resume),
            agent_session_id: None,
        }))
    }
}

struct FlakyResumeSession {
    fail_resume: Arc<AtomicBool>,
    agent_session_id: Option<String>,
}

impl FlakyResumeSession {
    fn metadata(&self, agent_session_id: String) -> SessionMetadata {
        SessionMetadata {
            agent_session_id,
            modes: None,
            config_options: Vec::new(),
        }
    }
}

impl AcpSession for FlakyResumeSession {
    fn initialize(&mut self, _client: Implementation) -> Result<InitializeSummary, AcpError> {
        Ok(InitializeSummary {
            protocol_version: 1,
            agent_name: Some("Flaky resume agent".to_string()),
            agent_title: None,
            agent_version: Some("0.1.0".to_string()),
            agent_capabilities: AgentCapabilities::new()
                .load_session(true)
                .session_capabilities(
                    SessionCapabilities::new().resume(SessionResumeCapabilities::new()),
                ),
            auth_methods: Vec::new(),
        })
    }

    fn new_session(&mut self, _cwd: &Path) -> Result<SessionMetadata, AcpError> {
        self.agent_session_id = Some("flaky-new".to_string());
        Ok(self.metadata("flaky-new".to_string()))
    }

    fn list_sessions(
        &mut self,
        _cwd: Option<&Path>,
        _cursor: Option<&str>,
    ) -> Result<ListedSessionPage, AcpError> {
        Ok(ListedSessionPage {
            sessions: Vec::new(),
            next_cursor: None,
        })
    }

    fn load_session(
        &mut self,
        agent_session_id: &str,
        cwd: &Path,
    ) -> Result<SessionMetadata, AcpError> {
        self.resume_session(agent_session_id, cwd)
    }

    fn resume_session(
        &mut self,
        agent_session_id: &str,
        _cwd: &Path,
    ) -> Result<SessionMetadata, AcpError> {
        if self.fail_resume.load(Ordering::SeqCst) {
            return Err(AcpError::new(format!(
                "unknown session: {agent_session_id}"
            )));
        }
        self.agent_session_id = Some(agent_session_id.to_string());
        Ok(self.metadata(agent_session_id.to_string()))
    }

    fn prompt(&mut self, _text: &str) -> Result<(), AcpError> {
        Ok(())
    }

    fn cancel(&mut self) -> Result<(), AcpError> {
        Ok(())
    }

    fn respond_permission(&mut self, _option_id: &str) -> Result<(), AcpError> {
        Ok(())
    }

    fn set_mode(&mut self, _mode_id: &str) -> Result<(), AcpError> {
        Ok(())
    }

    fn drain(&mut self) -> Vec<AgentEvent> {
        Vec::new()
    }

    fn is_running(&mut self) -> bool {
        true
    }
}

#[test]
fn missing_native_session_reports_stale_without_new_session() {
    let path = store_path();
    let flaky = AgentKind::Custom("flaky".to_string());
    let fail_resume = Arc::new(AtomicBool::new(false));

    let build = |fail: bool| {
        fail_resume.store(fail, Ordering::SeqCst);
        let mut registry = AdapterRegistry::new();
        registry.register(Arc::new(FlakyResumeAdapter {
            kind: flaky.clone(),
            fail_resume: Arc::clone(&fail_resume),
        }));
        manager(Arc::new(registry), &path)
    };

    let first = build(false);
    let imported = first
        .import_session(ImportSessionRequest {
            agent: flaky.clone(),
            agent_session_id: "native-1".to_string(),
            working_directory_label: "~/work/slight".to_string(),
            recovery: SessionRecovery::Resume,
            title: Some("Flaky session".to_string()),
        })
        .expect("import succeeds while the native session exists");
    assert_eq!(imported.recovery, SessionRecoveryState::Live);
    drop(first);

    // Restart against an agent that no longer knows the native session.
    let restarted = build(true);
    let sessions = restarted.list_sessions();
    assert_eq!(sessions.len(), 1, "no replacement session is created");
    let stale = &sessions[0];
    assert_eq!(stale.id, imported.id);
    assert!(matches!(stale.recovery, SessionRecoveryState::Stale { .. }));
    assert_eq!(stale.status, SessionStatus::Exited);
    assert_eq!(restarted.session_count(), 1);
    assert_eq!(restarted.active_session_count(), 0);

    let error = restarted
        .send_input(&imported.id, "hello")
        .expect_err("a stale session cannot accept input");
    assert!(matches!(
        error,
        SessionError::UnknownSession(_) | SessionError::NotRunning(_)
    ));

    let _ = std::fs::remove_file(&path);
}
