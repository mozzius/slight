use crate::types::{
    AttachResult, CommandsPayload, ConfigOptionsPayload, ContentBlock, ContentKind,
    CreateSessionRequest, DiagnosticsPayload, ExitPayload, HistoryResult, ImportSessionRequest,
    MessagePayload, ModePayload, PermissionRequestPayload, PermissionResolvedPayload, PlanPayload,
    SequencedEvent, SessionAcpMetadata, SessionDetail, SessionError, SessionEvent,
    SessionInfoPayload, SessionRecovery, SessionRecoveryState, SessionStatus, SessionSummary,
    SnapshotPayload, StatusPayload, ToolCallContentPayload, ToolCallLocationPayload,
    ToolCallPayload, TurnEndedPayload, UsagePayload,
};
use acp_adapters::{AdapterRegistry, AgentAdapter, AgentLaunchConfig};
use acp_types::metadata::{
    AcpConfigCategory, AcpConfigKind, AcpConfigOption, AcpModeState, AgentMetadataSummary,
    SessionMetadata,
};
use acp_types::session::{AcpSession, AgentEvent, InitializeSummary, ListedSessionPage};
use acp_types::wire::{Implementation, StopReason};
use acp_types::{AgentDescriptor, AgentKind, DiagnosticLevel, MessageRole};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use session_store::{SessionStore, StoredEvent, StoredSession};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

struct Session {
    summary: SessionSummary,
    descriptor: AgentDescriptor,
    initialize: InitializeSummary,
    connection: Box<dyn AcpSession>,
    agent_session_id: Option<String>,
    origin: SessionOrigin,
    resolved_working_directory: PathBuf,
    modes: Option<AcpModeState>,
    config_options: Vec<AcpConfigOption>,
    journal: VecDeque<SequencedEvent>,
    pending_permission: Option<PermissionRequestPayload>,
    running: bool,
    exit_reported: bool,
    reasoning_started_at_ms: Option<i64>,
    reasoning_ended_at_ms: Option<i64>,
}

struct Inner {
    adapters: Arc<AdapterRegistry>,
    store: Box<dyn SessionStore>,
    sessions: HashMap<String, Session>,
    subscribers: HashMap<String, Vec<Sender<SequencedEvent>>>,
    journal_limit: usize,
    recovered: HashMap<String, RecoveredSession>,
}

struct RecoveredSession {
    summary: SessionSummary,
    descriptor: AgentDescriptor,
    agent_session_id: Option<String>,
    journal: VecDeque<SequencedEvent>,
}

/// How a product session was originally bound to its native agent session.
///
/// Persisted so recovery can report what kind of session it is restoring; it
/// does not change the resume path, which always prefers `session/resume`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SessionOrigin {
    #[default]
    New,
    Load,
    Resume,
}

impl SessionOrigin {
    fn as_str(self) -> &'static str {
        match self {
            SessionOrigin::New => "new",
            SessionOrigin::Load => "loaded",
            SessionOrigin::Resume => "resumed",
        }
    }
}

/// The durable record for one product session.
///
/// This is the JSON stored in [`StoredSession::metadata`]. It carries the
/// native agent session identity, the resolved working directory, and the
/// launch/recovery origin so the host can rehydrate after a restart. Fields are
/// defaulted so a bare [`SessionSummary`] still deserializes (it simply has no
/// native session to resume).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedSession {
    summary: SessionSummary,
    #[serde(default)]
    origin: SessionOrigin,
    #[serde(default)]
    agent_session_id: Option<String>,
    #[serde(default)]
    resolved_working_directory: String,
}

/// How the shared open path binds a product session to an agent session.
enum OpenKind {
    /// Brand-new `session/new`.
    New,
    /// `session/load` for a native agent session, replaying its history.
    Load(String),
    /// `session/resume` for a native agent session, without replay.
    Resume(String),
}

pub struct SessionManager {
    inner: Mutex<Inner>,
}

impl SessionManager {
    pub fn new(
        adapters: Arc<AdapterRegistry>,
        store: Box<dyn SessionStore>,
        journal_limit: usize,
    ) -> Self {
        let mut inner = Inner {
            adapters,
            store,
            sessions: HashMap::new(),
            subscribers: HashMap::new(),
            journal_limit,
            recovered: HashMap::new(),
        };
        // Rehydrate every persisted session before the host serves clients.
        // Native resume may fail; each failure is recorded as an explicit
        // stale/unavailable recovery state rather than spawning a new session.
        let stored = inner.store.list_sessions().unwrap_or_default();
        for record in stored {
            inner.recover_stored_session(record);
        }
        Self {
            inner: Mutex::new(inner),
        }
    }

    pub fn create_session(
        &self,
        request: CreateSessionRequest,
    ) -> Result<SessionSummary, SessionError> {
        let mut inner = self.lock();
        inner.create_session(request)
    }

    pub fn import_session(
        &self,
        request: ImportSessionRequest,
    ) -> Result<SessionSummary, SessionError> {
        let mut inner = self.lock();
        inner.import_session(request)
    }

    /// Lists the existing sessions a connected agent advertises through
    /// `session/list`, optionally scoped to one working directory.
    pub fn list_agent_sessions(
        &self,
        agent: &AgentKind,
        cwd: Option<&Path>,
        cursor: Option<&str>,
    ) -> Result<ListedSessionPage, SessionError> {
        let mut inner = self.lock();
        inner.list_agent_sessions(agent, cwd, cursor)
    }

    pub fn list_sessions(&self) -> Vec<SessionSummary> {
        let inner = self.lock();
        let mut sessions: Vec<SessionSummary> = inner
            .sessions
            .values()
            .map(|session| session.summary.clone())
            .collect();
        sessions.extend(
            inner
                .recovered
                .values()
                .map(|session| session.summary.clone()),
        );
        sessions.sort_by_key(|session| std::cmp::Reverse(session.last_activity_at_ms));
        sessions
    }

    /// Returns native agent identities already owned by Slight. Discovery uses
    /// this to avoid offering an import that would open the same agent session
    /// a second time, including when the existing Slight session is archived.
    pub fn imported_agent_session_ids(&self, agent: &AgentKind) -> HashSet<String> {
        let inner = self.lock();
        let mut ids: HashSet<String> = inner
            .sessions
            .values()
            .filter(|session| session.summary.agent == agent.as_str())
            .filter_map(|session| session.agent_session_id.clone())
            .collect();
        ids.extend(
            inner
                .recovered
                .values()
                .filter(|session| session.summary.agent == agent.as_str())
                .filter_map(|session| session.agent_session_id.clone()),
        );
        ids
    }

    pub fn recent_working_directories(&self, limit: usize) -> Vec<String> {
        let inner = self.lock();
        let mut summaries: Vec<&SessionSummary> = inner
            .sessions
            .values()
            .map(|session| &session.summary)
            .chain(inner.recovered.values().map(|session| &session.summary))
            .collect();
        summaries.sort_by_key(|summary| std::cmp::Reverse(summary.last_activity_at_ms));

        let mut paths = Vec::new();
        for summary in summaries {
            if !paths.contains(&summary.working_directory_label) {
                paths.push(summary.working_directory_label.clone());
                if paths.len() >= limit {
                    break;
                }
            }
        }
        paths
    }

    /// Returns paths used by both Slight-owned sessions and native sessions
    /// advertised by the configured ACP agents. Discovery failures are ignored
    /// here because one unavailable harness should not hide other workspaces.
    pub fn workspace_paths(&self, limit: usize) -> Vec<String> {
        let mut paths = self.recent_working_directories(limit);
        let mut inner = self.lock();
        for agent in inner.adapters.available_kinds() {
            let Ok(page) = inner.list_agent_sessions(&agent, None, None) else {
                continue;
            };
            for session in page.sessions {
                if !paths.contains(&session.cwd) {
                    paths.push(session.cwd);
                    if paths.len() >= limit {
                        return paths;
                    }
                }
            }
        }
        paths
    }

    pub fn inspect_session(&self, session_id: &str) -> Result<SessionDetail, SessionError> {
        let inner = self.lock();
        let Some(session) = inner.sessions.get(session_id) else {
            let recovered = inner
                .recovered
                .get(session_id)
                .ok_or_else(|| SessionError::UnknownSession(session_id.to_string()))?;
            return Ok(SessionDetail {
                summary: recovered.summary.clone(),
                agent: recovered.descriptor.clone(),
                agent_session_id: recovered.agent_session_id.clone(),
                acp: SessionAcpMetadata::default(),
                running: false,
                pending_permission: None,
                recent_events: recovered.journal.iter().cloned().collect(),
            });
        };
        let acp = SessionAcpMetadata {
            protocol_version: session.initialize.protocol_version,
            agent: AgentMetadataSummary {
                name: session.initialize.agent_name.clone(),
                title: session.initialize.agent_title.clone(),
                version: session.initialize.agent_version.clone(),
            },
            auth_methods: session.initialize.auth_methods.clone(),
            capabilities: session.initialize.capability_summary(),
            modes: session.modes.clone(),
            config_options: session.config_options.clone(),
        };
        Ok(SessionDetail {
            summary: session.summary.clone(),
            agent: session.descriptor.clone(),
            agent_session_id: session.agent_session_id.clone(),
            acp,
            running: session.running,
            pending_permission: session.pending_permission.clone(),
            recent_events: session.journal.iter().cloned().collect(),
        })
    }

    pub fn send_input(&self, session_id: &str, text: &str) -> Result<(), SessionError> {
        let mut inner = self.lock();
        inner.send_prompt(session_id, text)?;
        inner.pump_all();
        Ok(())
    }

    pub fn cancel(&self, session_id: &str) -> Result<(), SessionError> {
        let mut inner = self.lock();
        inner.cancel(session_id)?;
        inner.pump_all();
        Ok(())
    }

    pub fn respond_permission(
        &self,
        session_id: &str,
        permission_id: &str,
        option_id: &str,
    ) -> Result<(), SessionError> {
        let mut inner = self.lock();
        inner.respond_permission(session_id, permission_id, option_id)?;
        inner.pump_all();
        Ok(())
    }

    pub fn rename_session(
        &self,
        session_id: &str,
        title: &str,
    ) -> Result<SessionSummary, SessionError> {
        let mut inner = self.lock();
        inner.rename_session(session_id, title)
    }

    pub fn archive_session(
        &self,
        session_id: &str,
        archived: bool,
    ) -> Result<SessionSummary, SessionError> {
        let mut inner = self.lock();
        inner.archive_session(session_id, archived)
    }

    pub fn set_mode(&self, session_id: &str, mode_id: &str) -> Result<(), SessionError> {
        let mut inner = self.lock();
        inner.set_mode(session_id, mode_id)
    }

    pub fn set_config_option(
        &self,
        session_id: &str,
        config_id: &str,
        value_id: &str,
    ) -> Result<(), SessionError> {
        let mut inner = self.lock();
        inner.set_config_option(session_id, config_id, value_id)
    }

    pub fn attach(
        &self,
        session_id: &str,
        after_sequence: Option<u64>,
    ) -> Result<AttachResult, SessionError> {
        let inner = self.lock();
        inner.attach(session_id, after_sequence)
    }

    pub fn history(
        &self,
        session_id: &str,
        before_sequence: Option<u64>,
        limit: usize,
    ) -> Result<HistoryResult, SessionError> {
        let inner = self.lock();
        inner.history(session_id, before_sequence, limit)
    }

    pub fn subscribe(&self, session_id: &str) -> Result<Receiver<SequencedEvent>, SessionError> {
        let mut inner = self.lock();
        inner.subscribe(session_id)
    }

    pub fn pump(&self) -> usize {
        let mut inner = self.lock();
        inner.pump_all()
    }

    pub fn session_count(&self) -> usize {
        let inner = self.lock();
        inner.sessions.len() + inner.recovered.len()
    }

    pub fn active_session_count(&self) -> usize {
        self.lock()
            .sessions
            .values()
            .filter(|session| {
                matches!(
                    session.summary.status,
                    SessionStatus::Idle | SessionStatus::Working | SessionStatus::WaitingPermission
                )
            })
            .count()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Inner {
    fn create_session(
        &mut self,
        request: CreateSessionRequest,
    ) -> Result<SessionSummary, SessionError> {
        let title = default_title(&request);
        let agent = request.agent.clone();
        let working_directory_label = request.working_directory_label.clone();
        let initial_prompt = request.initial_prompt.clone();
        let model = request.model.clone();
        let effort = request.effort.clone();
        self.open_session(
            agent,
            working_directory_label,
            title,
            model,
            effort,
            initial_prompt,
            OpenKind::New,
            SessionOrigin::New,
        )
    }

    fn import_session(
        &mut self,
        request: ImportSessionRequest,
    ) -> Result<SessionSummary, SessionError> {
        let title = request
            .title
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| format!("{} session", request.agent));
        let agent = request.agent.clone();
        let working_directory_label = request.working_directory_label.clone();
        let agent_session_id = request.agent_session_id.clone();
        let (kind, origin) = match request.recovery {
            SessionRecovery::Load => (OpenKind::Load(agent_session_id), SessionOrigin::Load),
            SessionRecovery::Resume => (OpenKind::Resume(agent_session_id), SessionOrigin::Resume),
        };
        let summary = self.open_session(
            agent,
            working_directory_label,
            title,
            None,
            None,
            None,
            kind,
            origin,
        )?;
        // `session/load` replays history as normalized updates; drain them into
        // the bounded journal before returning the imported session.
        self.pump_all();
        Ok(self
            .sessions
            .get(&summary.id)
            .map(|session| session.summary.clone())
            .unwrap_or(summary))
    }

    fn list_agent_sessions(
        &mut self,
        agent: &AgentKind,
        cwd: Option<&Path>,
        cursor: Option<&str>,
    ) -> Result<ListedSessionPage, SessionError> {
        let adapter = self
            .adapters
            .get(agent)
            .ok_or_else(|| SessionError::UnknownAgent(agent.to_string()))?;
        let launch = AgentLaunchConfig {
            working_directory: cwd.map(Path::to_path_buf),
            ..AgentLaunchConfig::default()
        };
        let mut connection = adapter.spawn_session(&launch)?;
        let initialize = connection.initialize(Implementation::new(
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
        ))?;
        if initialize
            .agent_capabilities
            .session_capabilities
            .list
            .is_none()
        {
            return Err(SessionError::UnsupportedCapability("session/list"));
        }
        let sessions = connection.list_sessions(cwd, cursor)?;
        Ok(sessions)
    }

    /// Shared open path for `session/new`, `session/load`, and `session/resume`.
    ///
    /// The caller supplies the [`OpenKind`] that binds the ACP session; everything
    /// else — adapter lookup, launch, `initialize`, descriptor negotiation,
    /// product-session creation, persistence, and the initial status event — is
    /// identical so imported and freshly created sessions share one lifecycle.
    fn open_session(
        &mut self,
        agent: AgentKind,
        working_directory_label: String,
        title: String,
        model: Option<String>,
        effort: Option<String>,
        initial_prompt: Option<String>,
        kind: OpenKind,
        origin: SessionOrigin,
    ) -> Result<SessionSummary, SessionError> {
        let adapter = self
            .adapters
            .get(&agent)
            .ok_or_else(|| SessionError::UnknownAgent(agent.to_string()))?;
        let working_directory_label = normalized_working_directory_label(&working_directory_label);
        let working_directory = resolve_working_directory(&working_directory_label);
        let launch = AgentLaunchConfig {
            working_directory: Some(working_directory.clone()),
            initial_prompt: initial_prompt.clone(),
            ..AgentLaunchConfig::default()
        };
        let mut connection = adapter.spawn_session(&launch)?;
        let id = uuid::Uuid::new_v4().to_string();

        // Handshake synchronously so opening fails fast on a broken agent, and
        // so the product session id is correlated with the agent-side session
        // id before any prompt is accepted.
        let initialize = connection.initialize(Implementation::new(
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
        ))?;
        let mut descriptor = adapter.descriptor();
        if let Some(name) = initialize.agent_name.clone() {
            descriptor.display_name = name;
        }
        if let Some(version) = initialize.agent_version.clone() {
            descriptor.version = Some(version);
        }
        let session_metadata = match kind {
            OpenKind::New => connection.new_session(&working_directory)?,
            OpenKind::Load(agent_session_id) => {
                if !initialize.agent_capabilities.load_session {
                    return Err(SessionError::UnsupportedCapability("session/load"));
                }
                connection.load_session(&agent_session_id, &working_directory)?
            }
            OpenKind::Resume(agent_session_id) => {
                if initialize
                    .agent_capabilities
                    .session_capabilities
                    .resume
                    .is_none()
                {
                    return Err(SessionError::UnsupportedCapability("session/resume"));
                }
                connection.resume_session(&agent_session_id, &working_directory)?
            }
        };
        let agent_session_id = session_metadata.agent_session_id.clone();
        let modes = session_metadata.modes;
        let config_options = session_metadata.config_options;
        let model =
            model.or_else(|| current_config_value(&config_options, AcpConfigCategory::Model));

        let now = now_ms();
        let summary = SessionSummary {
            id: id.clone(),
            title,
            agent: agent.as_str().to_string(),
            model,
            effort,
            working_directory_label,
            git_branch: detect_git_branch(&working_directory),
            status: SessionStatus::Idle,
            created_at_ms: now,
            last_activity_at_ms: now,
            last_sequence: 0,
            recovery: SessionRecoveryState::Live,
            archived: false,
        };
        self.sessions.insert(
            id.clone(),
            Session {
                summary: summary.clone(),
                descriptor,
                initialize,
                connection,
                agent_session_id: Some(agent_session_id),
                origin,
                resolved_working_directory: working_directory,
                modes,
                config_options,
                journal: VecDeque::new(),
                pending_permission: None,
                running: true,
                exit_reported: false,
                reasoning_started_at_ms: None,
                reasoning_ended_at_ms: None,
            },
        );
        self.persist_session(&id);
        self.append(
            &id,
            SessionEvent::Status(StatusPayload {
                status: SessionStatus::Idle,
            }),
        )?;
        if let Some(prompt) = initial_prompt {
            self.send_prompt(&id, &prompt)?;
            self.pump_all();
        }
        Ok(self
            .sessions
            .get(&id)
            .map(|session| session.summary.clone())
            .unwrap_or(summary))
    }

    fn send_prompt(&mut self, session_id: &str, text: &str) -> Result<(), SessionError> {
        {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SessionError::UnknownSession(session_id.to_string()))?;
            if !session.running {
                return Err(SessionError::NotRunning(session_id.to_string()));
            }
            session
                .connection
                .prompt(text)
                .map_err(SessionError::Agent)?;
            session.reasoning_started_at_ms = None;
            session.reasoning_ended_at_ms = None;
        }
        self.append(
            session_id,
            SessionEvent::Message(MessagePayload {
                role: MessageRole::User,
                text: text.to_string(),
                blocks: vec![ContentBlock::text(text)],
            }),
        )?;
        self.append(
            session_id,
            SessionEvent::Status(StatusPayload {
                status: SessionStatus::Working,
            }),
        )?;
        Ok(())
    }

    fn set_mode(&mut self, session_id: &str, mode_id: &str) -> Result<(), SessionError> {
        {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SessionError::UnknownSession(session_id.to_string()))?;
            session
                .connection
                .set_mode(mode_id)
                .map_err(SessionError::Agent)?;
            if let Some(modes) = session.modes.as_mut() {
                modes.current_mode_id = mode_id.to_string();
            }
        }
        self.append(
            session_id,
            SessionEvent::Mode(ModePayload {
                current_mode_id: mode_id.to_string(),
            }),
        )?;
        Ok(())
    }

    fn set_config_option(
        &mut self,
        session_id: &str,
        config_id: &str,
        value_id: &str,
    ) -> Result<(), SessionError> {
        let options = {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SessionError::UnknownSession(session_id.to_string()))?;
            session
                .connection
                .set_config_option(config_id, value_id)
                .map_err(SessionError::Agent)?
        };
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.config_options = options.clone();
        }
        self.append(
            session_id,
            SessionEvent::ConfigOptions(ConfigOptionsPayload { options }),
        )?;
        Ok(())
    }

    fn cancel(&mut self, session_id: &str) -> Result<(), SessionError> {
        {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SessionError::UnknownSession(session_id.to_string()))?;
            session.connection.cancel().map_err(SessionError::Agent)?;
            session.pending_permission = None;
        }
        self.append(
            session_id,
            SessionEvent::Diagnostics(DiagnosticsPayload {
                level: DiagnosticLevel::Info,
                message: "cancellation requested".to_string(),
            }),
        )?;
        self.append(
            session_id,
            SessionEvent::Status(StatusPayload {
                status: SessionStatus::Idle,
            }),
        )?;
        Ok(())
    }

    fn respond_permission(
        &mut self,
        session_id: &str,
        permission_id: &str,
        option_id: &str,
    ) -> Result<(), SessionError> {
        let pending_id = {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SessionError::UnknownSession(session_id.to_string()))?;
            let pending = session
                .pending_permission
                .as_ref()
                .ok_or_else(|| SessionError::NoPendingPermission(session_id.to_string()))?;
            if pending.id != permission_id {
                let expected = pending.id.clone();
                return Err(SessionError::PermissionMismatch {
                    session_id: session_id.to_string(),
                    expected,
                    actual: permission_id.to_string(),
                });
            }
            let pending_id = pending.id.clone();
            session
                .connection
                .respond_permission(option_id)
                .map_err(SessionError::Agent)?;
            session.pending_permission = None;
            pending_id
        };
        self.append(
            session_id,
            SessionEvent::PermissionResolved(PermissionResolvedPayload {
                id: pending_id,
                option_id: option_id.to_string(),
            }),
        )?;
        self.append(
            session_id,
            SessionEvent::Status(StatusPayload {
                status: SessionStatus::Working,
            }),
        )?;
        Ok(())
    }

    fn rename_session(
        &mut self,
        session_id: &str,
        title: &str,
    ) -> Result<SessionSummary, SessionError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(SessionError::EmptyTitle);
        }
        let (summary, pending_permission) = {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SessionError::UnknownSession(session_id.to_string()))?;
            session.summary.title = title.to_string();
            let summary = session.summary.clone();
            (summary, session.pending_permission.clone())
        };
        self.append(
            session_id,
            SessionEvent::Snapshot(SnapshotPayload {
                summary,
                pending_permission,
            }),
        )?;
        self.sessions
            .get(session_id)
            .map(|session| session.summary.clone())
            .ok_or_else(|| SessionError::UnknownSession(session_id.to_string()))
    }

    fn archive_session(
        &mut self,
        session_id: &str,
        archived: bool,
    ) -> Result<SessionSummary, SessionError> {
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.summary.archived = archived;
            let summary = session.summary.clone();
            self.persist_session(session_id);
            return Ok(summary);
        }
        if let Some(session) = self.recovered.get_mut(session_id) {
            session.summary.archived = archived;
            let summary = session.summary.clone();
            if let Ok(Some(mut stored)) = self.store.get_session(session_id) {
                if let Some(object) = stored.metadata.as_object_mut() {
                    if let Some(summary_object) =
                        object.get_mut("summary").and_then(Value::as_object_mut)
                    {
                        summary_object.insert("archived".to_string(), Value::Bool(archived));
                    }
                }
                let _ = self.store.upsert_session(stored);
            }
            return Ok(summary);
        }
        Err(SessionError::UnknownSession(session_id.to_string()))
    }

    fn attach(
        &self,
        session_id: &str,
        after_sequence: Option<u64>,
    ) -> Result<AttachResult, SessionError> {
        let Some(session) = self.sessions.get(session_id) else {
            let recovered = self
                .recovered
                .get(session_id)
                .ok_or_else(|| SessionError::UnknownSession(session_id.to_string()))?;
            let oldest = recovered.journal.front().map(|event| event.sequence);
            let from = after_sequence.unwrap_or(0);
            let replayed = recovered
                .journal
                .iter()
                .filter(|event| event.sequence > from)
                .cloned()
                .collect();
            return Ok(AttachResult {
                summary: recovered.summary.clone(),
                replayed,
                latest_sequence: recovered.summary.last_sequence,
                oldest_available_sequence: oldest,
                resync_required: matches!((after_sequence, oldest), (Some(after), Some(oldest)) if after.saturating_add(1) < oldest),
            });
        };
        let oldest = session.journal.front().map(|event| event.sequence);
        let from = after_sequence.unwrap_or(0);
        let replayed: Vec<SequencedEvent> = session
            .journal
            .iter()
            .filter(|event| event.sequence > from)
            .cloned()
            .collect();
        let resync_required = matches!(
            (after_sequence, oldest),
            (Some(after), Some(oldest)) if after.saturating_add(1) < oldest
        );
        Ok(AttachResult {
            summary: session.summary.clone(),
            replayed,
            latest_sequence: session.summary.last_sequence,
            oldest_available_sequence: oldest,
            resync_required,
        })
    }

    fn history(
        &self,
        session_id: &str,
        before_sequence: Option<u64>,
        limit: usize,
    ) -> Result<HistoryResult, SessionError> {
        let Some(session) = self.sessions.get(session_id) else {
            let recovered = self
                .recovered
                .get(session_id)
                .ok_or_else(|| SessionError::UnknownSession(session_id.to_string()))?;
            let oldest = recovered.journal.front().map(|event| event.sequence);
            let boundary = before_sequence.unwrap_or(u64::MAX);
            let page_limit = limit.clamp(1, 100);
            let eligible: Vec<SequencedEvent> = recovered
                .journal
                .iter()
                .filter(|event| event.sequence < boundary)
                .cloned()
                .collect();
            let start = eligible.len().saturating_sub(page_limit);
            return Ok(HistoryResult {
                summary: recovered.summary.clone(),
                events: eligible[start..].to_vec(),
                latest_sequence: recovered.summary.last_sequence,
                oldest_available_sequence: oldest,
                has_more: start > 0,
            });
        };
        let oldest = session.journal.front().map(|event| event.sequence);
        let boundary = before_sequence.unwrap_or(u64::MAX);
        let page_limit = limit.clamp(1, 200);
        let eligible: Vec<SequencedEvent> = session
            .journal
            .iter()
            .filter(|event| event.sequence < boundary)
            .cloned()
            .collect();
        let start = eligible.len().saturating_sub(page_limit);
        let events = eligible[start..].to_vec();
        let has_more = start > 0;
        Ok(HistoryResult {
            summary: session.summary.clone(),
            events,
            latest_sequence: session.summary.last_sequence,
            oldest_available_sequence: oldest,
            has_more,
        })
    }

    fn subscribe(&mut self, session_id: &str) -> Result<Receiver<SequencedEvent>, SessionError> {
        if !self.sessions.contains_key(session_id) {
            return Err(SessionError::UnknownSession(session_id.to_string()));
        }
        let (sender, receiver) = mpsc::channel();
        self.subscribers
            .entry(session_id.to_string())
            .or_default()
            .push(sender);
        Ok(receiver)
    }

    fn pump_all(&mut self) -> usize {
        let collected: Vec<(String, Vec<AgentEvent>, bool)> = self
            .sessions
            .iter_mut()
            .filter_map(|(id, session)| {
                let events = session.connection.drain();
                let running = session.connection.is_running();
                if events.is_empty() && running {
                    None
                } else {
                    Some((id.clone(), events, running))
                }
            })
            .collect();

        let mut applied = 0;
        for (session_id, events, running) in collected {
            for event in events {
                if self.apply_agent_event(&session_id, event).is_ok() {
                    applied += 1;
                }
            }
            if !running {
                let _ = self.ensure_exited(&session_id);
            }
        }
        applied
    }

    fn apply_agent_event(
        &mut self,
        session_id: &str,
        event: AgentEvent,
    ) -> Result<(), SessionError> {
        match event {
            AgentEvent::Message { role, content } => {
                let payload = MessagePayload {
                    role,
                    text: content.text_fallback(),
                    blocks: vec![ContentBlock::from_normalized(&content)],
                };
                self.append(session_id, SessionEvent::Message(payload))?;
            }
            AgentEvent::Thought { content } => {
                if let Some(session) = self.sessions.get_mut(session_id) {
                    let now = now_ms();
                    if session.reasoning_started_at_ms.is_none() {
                        session.reasoning_started_at_ms = Some(now);
                    }
                    session.reasoning_ended_at_ms = Some(now);
                }
                let mut block = ContentBlock::from_normalized(&content);
                if block.kind == ContentKind::Text {
                    block.kind = ContentKind::Reasoning;
                }
                let payload = MessagePayload {
                    role: MessageRole::Assistant,
                    text: content.text_fallback(),
                    blocks: vec![block],
                };
                self.append(session_id, SessionEvent::Message(payload))?;
            }
            AgentEvent::ToolCall(update) => {
                let payload = ToolCallPayload {
                    id: update.tool_call_id,
                    title: update.title,
                    kind: update.kind,
                    status: update.status,
                    detail: update.detail,
                    content: update
                        .content
                        .iter()
                        .map(ToolCallContentPayload::from_normalized)
                        .collect(),
                    locations: update
                        .locations
                        .iter()
                        .map(ToolCallLocationPayload::from)
                        .collect(),
                    raw_input: update.raw_input,
                    raw_output: update.raw_output,
                };
                self.append(session_id, SessionEvent::ToolCall(payload))?;
            }
            AgentEvent::Plan(plan) => {
                self.append(
                    session_id,
                    SessionEvent::Plan(PlanPayload {
                        entries: plan.entries,
                    }),
                )?;
            }
            AgentEvent::AvailableCommands(commands) => {
                self.append(
                    session_id,
                    SessionEvent::Commands(CommandsPayload { commands }),
                )?;
            }
            AgentEvent::ConfigOptions(options) => {
                if let Some(session) = self.sessions.get_mut(session_id) {
                    session.config_options = options.clone();
                }
                self.append(
                    session_id,
                    SessionEvent::ConfigOptions(ConfigOptionsPayload { options }),
                )?;
            }
            AgentEvent::SessionInfo(update) => {
                let title = update.title.flatten();
                let updated_at = update.updated_at.flatten();
                if let Some(title) = &title {
                    if let Some(session) = self.sessions.get_mut(session_id) {
                        if !title.trim().is_empty() {
                            session.summary.title = title.clone();
                        }
                    }
                }
                self.append(
                    session_id,
                    SessionEvent::SessionInfo(SessionInfoPayload { title, updated_at }),
                )?;
            }
            AgentEvent::Usage(usage) => {
                self.append(
                    session_id,
                    SessionEvent::Usage(UsagePayload {
                        used: usage.used,
                        size: usage.size,
                        cost: usage.cost,
                    }),
                )?;
            }
            AgentEvent::Mode { current_mode_id } => {
                if let Some(session) = self.sessions.get_mut(session_id) {
                    if let Some(modes) = session.modes.as_mut() {
                        modes.current_mode_id = current_mode_id.clone();
                    }
                }
                self.append(
                    session_id,
                    SessionEvent::Mode(ModePayload { current_mode_id }),
                )?;
            }
            AgentEvent::PermissionRequest(request) => {
                let payload = PermissionRequestPayload {
                    id: request.permission_id,
                    title: request.title,
                    detail: request.detail,
                    tool_call_id: request.tool_call_id,
                    options: request.options,
                };
                if let Some(session) = self.sessions.get_mut(session_id) {
                    session.pending_permission = Some(payload.clone());
                }
                self.append(session_id, SessionEvent::PermissionRequest(payload))?;
                self.append(
                    session_id,
                    SessionEvent::Status(StatusPayload {
                        status: SessionStatus::WaitingPermission,
                    }),
                )?;
            }
            AgentEvent::Diagnostics { level, message } => {
                self.append(
                    session_id,
                    SessionEvent::Diagnostics(DiagnosticsPayload { level, message }),
                )?;
            }
            AgentEvent::TurnEnded { stop_reason } => {
                let (reasoning_started_at_ms, reasoning_ended_at_ms) = self
                    .sessions
                    .get_mut(session_id)
                    .map(|session| {
                        let timing = (
                            session.reasoning_started_at_ms,
                            session.reasoning_ended_at_ms,
                        );
                        session.reasoning_started_at_ms = None;
                        session.reasoning_ended_at_ms = None;
                        timing
                    })
                    .unwrap_or((None, None));
                self.append(
                    session_id,
                    SessionEvent::Status(StatusPayload {
                        status: SessionStatus::Idle,
                    }),
                )?;
                self.append(
                    session_id,
                    SessionEvent::TurnEnded(TurnEndedPayload {
                        stop_reason: stop_reason_string(stop_reason),
                        reasoning_started_at_ms,
                        reasoning_ended_at_ms,
                    }),
                )?;
                if stop_reason != StopReason::EndTurn {
                    self.append(
                        session_id,
                        SessionEvent::Diagnostics(DiagnosticsPayload {
                            level: DiagnosticLevel::Info,
                            message: format!("turn ended: {stop_reason:?}"),
                        }),
                    )?;
                }
            }
            AgentEvent::Exited { code, reason } => {
                if let Some(session) = self.sessions.get_mut(session_id) {
                    session.running = false;
                    session.exit_reported = true;
                }
                self.append(session_id, SessionEvent::Exit(ExitPayload { code, reason }))?;
                self.append(
                    session_id,
                    SessionEvent::Status(StatusPayload {
                        status: SessionStatus::Exited,
                    }),
                )?;
            }
            AgentEvent::Failed { message } => {
                self.append(
                    session_id,
                    SessionEvent::Diagnostics(DiagnosticsPayload {
                        level: DiagnosticLevel::Error,
                        message,
                    }),
                )?;
                self.append(
                    session_id,
                    SessionEvent::Status(StatusPayload {
                        status: SessionStatus::Failed,
                    }),
                )?;
            }
        }
        Ok(())
    }

    fn ensure_exited(&mut self, session_id: &str) -> Result<(), SessionError> {
        let should_report = self
            .sessions
            .get(session_id)
            .map(|session| !session.exit_reported)
            .unwrap_or(false);
        if !should_report {
            return Ok(());
        }
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.running = false;
            session.exit_reported = true;
        }
        self.append(
            session_id,
            SessionEvent::Exit(ExitPayload {
                code: None,
                reason: "agent_exited".to_string(),
            }),
        )?;
        self.append(
            session_id,
            SessionEvent::Status(StatusPayload {
                status: SessionStatus::Exited,
            }),
        )?;
        Ok(())
    }

    fn append(
        &mut self,
        session_id: &str,
        event: SessionEvent,
    ) -> Result<SequencedEvent, SessionError> {
        let limit = self.journal_limit;
        let sequenced = {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SessionError::UnknownSession(session_id.to_string()))?;
            session.summary.last_sequence += 1;
            let timestamp_ms = now_ms();
            session.summary.last_activity_at_ms = timestamp_ms;
            if let SessionEvent::Status(payload) = &event {
                session.summary.status = payload.status;
            }
            let sequenced = SequencedEvent {
                sequence: session.summary.last_sequence,
                timestamp_ms,
                event,
            };
            session.journal.push_back(sequenced.clone());
            while session.journal.len() > limit {
                session.journal.pop_front();
            }
            sequenced
        };

        let stored = StoredEvent {
            sequence: sequenced.sequence,
            event: serde_json::to_value(&sequenced).unwrap_or(Value::Null),
        };
        let _ = self.store.append_event(session_id, stored);
        self.persist_session(session_id);
        if let Some(subscribers) = self.subscribers.get(session_id) {
            for subscriber in subscribers {
                let _ = subscriber.send(sequenced.clone());
            }
        }
        Ok(sequenced)
    }

    fn persist_session(&mut self, session_id: &str) {
        let Some(session) = self.sessions.get(session_id) else {
            return;
        };
        let record = PersistedSession {
            summary: session.summary.clone(),
            origin: session.origin,
            agent_session_id: session.agent_session_id.clone(),
            resolved_working_directory: session
                .resolved_working_directory
                .to_string_lossy()
                .to_string(),
        };
        let stored = StoredSession {
            session_id: record.summary.id.clone(),
            metadata: serde_json::to_value(&record).unwrap_or(Value::Null),
            last_sequence: record.summary.last_sequence,
            created_at_ms: record.summary.created_at_ms,
            updated_at_ms: record.summary.last_activity_at_ms,
        };
        let _ = self.store.upsert_session(stored);
    }

    /// Loads one persisted session and either rehydrates it or records why it
    /// could not be recovered.
    fn recover_stored_session(&mut self, stored: StoredSession) {
        let session_id = stored.session_id.clone();
        let journal: VecDeque<SequencedEvent> = self
            .store
            .events_after(&session_id, None, self.journal_limit)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|event| serde_json::from_value(event.event).ok())
            .collect();

        let record = match serde_json::from_value::<PersistedSession>(stored.metadata.clone()) {
            Ok(record) => record,
            Err(error) => {
                let summary = unreadable_summary(&stored, &error.to_string());
                let descriptor =
                    AgentDescriptor::new(AgentKind::Custom("unknown".to_string()), "Unknown", None);
                self.recovered.insert(
                    session_id,
                    RecoveredSession {
                        summary,
                        descriptor,
                        agent_session_id: None,
                        journal,
                    },
                );
                return;
            }
        };

        let agent = agent_kind_from_str(&record.summary.agent);
        let descriptor = self
            .adapters
            .get(&agent)
            .map(|adapter| adapter.descriptor())
            .unwrap_or_else(|| {
                AgentDescriptor::new(agent.clone(), record.summary.agent.clone(), None)
            });

        let mut summary = record.summary.clone();
        summary.last_sequence = stored.last_sequence;
        let native_id = record.agent_session_id.clone();

        let rehydration = match (self.adapters.get(&agent), native_id.clone()) {
            (None, _) => Err(SessionRecoveryState::Unavailable {
                reason: format!(
                    "agent {} is not registered on this host",
                    record.summary.agent
                ),
            }),
            (Some(_), None) => Err(SessionRecoveryState::Unavailable {
                reason: "session has no native agent session id to resume".to_string(),
            }),
            (Some(adapter), Some(native_id)) => {
                match rehydrate_session(adapter.as_ref(), &record, &native_id) {
                    Ok(rehydrated) => {
                        self.install_rehydrated_session(record, rehydrated, journal, summary);
                        return;
                    }
                    Err(state) => Err(state),
                }
            }
        };

        // Recovery failed: keep the session visible with its local journal and
        // an explicit stale/unavailable state. Never spawn a replacement.
        summary.status = SessionStatus::Exited;
        summary.recovery = match rehydration {
            Err(state) => state,
            Ok(()) => unreachable!("successful rehydration returns above"),
        };
        self.recovered.insert(
            session_id,
            RecoveredSession {
                summary,
                descriptor,
                agent_session_id: native_id,
                journal,
            },
        );
    }

    fn install_rehydrated_session(
        &mut self,
        record: PersistedSession,
        rehydrated: RehydratedSession,
        journal: VecDeque<SequencedEvent>,
        mut summary: SessionSummary,
    ) {
        let RehydratedSession {
            descriptor,
            initialize,
            connection,
            session_metadata,
        } = rehydrated;
        summary.status = SessionStatus::Idle;
        summary.recovery = SessionRecoveryState::Recovered;
        summary.last_activity_at_ms = now_ms();
        let agent_session_id = session_metadata.agent_session_id.clone();
        let modes = session_metadata.modes;
        let config_options = session_metadata.config_options;
        let id = summary.id.clone();
        self.sessions.insert(
            id,
            Session {
                summary,
                descriptor,
                initialize,
                connection,
                agent_session_id: Some(agent_session_id),
                origin: record.origin,
                resolved_working_directory: PathBuf::from(record.resolved_working_directory),
                modes,
                config_options,
                journal,
                pending_permission: None,
                running: true,
                exit_reported: false,
                reasoning_started_at_ms: None,
                reasoning_ended_at_ms: None,
            },
        );
    }
}

/// A successfully resumed native session, ready to be installed as a live
/// product session.
struct RehydratedSession {
    descriptor: AgentDescriptor,
    initialize: InitializeSummary,
    connection: Box<dyn AcpSession>,
    session_metadata: SessionMetadata,
}

/// Spawns the owning adapter and reconnects to the persisted native session.
///
/// Recovery deliberately prefers `session/resume`; it never replays
/// `session/load` history, so the local journal is not duplicated. Failures are
/// classified so the caller can report an explicit state:
/// - launch or handshake failure, or a missing resume capability, is
///   [`SessionRecoveryState::Unavailable`];
/// - a resume the agent rejects (for example a deleted native session) is
///   [`SessionRecoveryState::Stale`].
fn rehydrate_session(
    adapter: &dyn AgentAdapter,
    record: &PersistedSession,
    native_id: &str,
) -> Result<RehydratedSession, SessionRecoveryState> {
    let cwd = PathBuf::from(&record.resolved_working_directory);
    let launch = AgentLaunchConfig {
        working_directory: Some(cwd.clone()),
        ..AgentLaunchConfig::default()
    };
    let mut connection =
        adapter
            .spawn_session(&launch)
            .map_err(|error| SessionRecoveryState::Unavailable {
                reason: format!("could not launch agent: {error}"),
            })?;
    let initialize = connection
        .initialize(Implementation::new(
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
        ))
        .map_err(|error| SessionRecoveryState::Unavailable {
            reason: format!("agent handshake failed: {error}"),
        })?;
    if initialize
        .agent_capabilities
        .session_capabilities
        .resume
        .is_none()
    {
        return Err(SessionRecoveryState::Unavailable {
            reason: format!(
                "agent {} does not support session/resume",
                record.summary.agent
            ),
        });
    }
    let session_metadata = connection
        .resume_session(native_id, &cwd)
        .map_err(|error| SessionRecoveryState::Stale {
            reason: format!(
                "{} session {native_id} is no longer available: {error}",
                record.origin.as_str()
            ),
        })?;
    let mut descriptor = adapter.descriptor();
    if let Some(name) = initialize.agent_name.clone() {
        descriptor.display_name = name;
    }
    if let Some(version) = initialize.agent_version.clone() {
        descriptor.version = Some(version);
    }
    Ok(RehydratedSession {
        descriptor,
        initialize,
        connection,
        session_metadata,
    })
}

/// Reconstructs an [`AgentKind`] from the persisted string form. Unknown names
/// round-trip through [`AgentKind::Custom`].
fn agent_kind_from_str(agent: &str) -> AgentKind {
    match agent {
        "claude_code" => AgentKind::ClaudeCode,
        "codex" => AgentKind::Codex,
        "fake" => AgentKind::Fake,
        other => AgentKind::Custom(other.to_string()),
    }
}

/// A summary for a persisted row whose metadata could not be decoded at all.
fn unreadable_summary(stored: &StoredSession, reason: &str) -> SessionSummary {
    SessionSummary {
        id: stored.session_id.clone(),
        title: "Unreadable session".to_string(),
        agent: "unknown".to_string(),
        model: None,
        effort: None,
        working_directory_label: "~".to_string(),
        git_branch: None,
        status: SessionStatus::Exited,
        created_at_ms: stored.created_at_ms,
        last_activity_at_ms: stored.updated_at_ms,
        last_sequence: stored.last_sequence,
        recovery: SessionRecoveryState::Unavailable {
            reason: format!("session metadata could not be read: {reason}"),
        },
        archived: false,
    }
}

fn normalized_working_directory_label(label: &str) -> String {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        "~".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Resolves a client-supplied working-directory label to an absolute path.
///
/// `~` and `~/…` are expanded against `HOME`; anything else (including an
/// absolute path) is used verbatim. Callers that need the label for display
/// keep it separately; this is only for filesystem work.
pub fn resolve_working_directory(label: &str) -> PathBuf {
    let home = || {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."))
    };
    if label == "~" {
        return home();
    }
    if let Some(relative) = label.strip_prefix("~/") {
        return home().join(relative);
    }
    PathBuf::from(label)
}

fn detect_git_branch(directory: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["-C", directory.to_str()?, "branch", "--show-current"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let branch = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!branch.is_empty()).then_some(branch)
}

/// Serializes an ACP stop reason to its wire string, defaulting to `end_turn`
/// for future variants so the contract stays forward-compatible.
fn stop_reason_string(reason: StopReason) -> String {
    serde_json::to_value(reason)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "end_turn".to_string())
}

fn current_config_value(
    options: &[AcpConfigOption],
    category: AcpConfigCategory,
) -> Option<String> {
    options.iter().find_map(|option| {
        if option.category.as_ref() != Some(&category) {
            return None;
        }
        match &option.kind {
            AcpConfigKind::Select {
                current_value_id, ..
            } => Some(current_value_id.clone()),
            AcpConfigKind::Boolean { .. } => None,
        }
    })
}

fn default_title(request: &CreateSessionRequest) -> String {
    if let Some(prompt) = &request.initial_prompt {
        let trimmed = prompt.trim();
        if !trimmed.is_empty() {
            let prefix: String = trimmed.chars().take(40).collect();
            return prefix;
        }
    }
    format!("{} session", request.agent)
}

#[cfg(test)]
mod tests {
    use super::{normalized_working_directory_label, resolve_working_directory};
    use std::path::PathBuf;

    #[test]
    fn empty_working_directory_defaults_to_home_label() {
        assert_eq!(normalized_working_directory_label("  "), "~");
    }

    #[test]
    fn tilde_working_directory_resolves_to_home() {
        let home = PathBuf::from(std::env::var_os("HOME").expect("HOME is set in tests"));
        assert_eq!(
            resolve_working_directory("~/projects"),
            home.join("projects")
        );
    }

    #[test]
    fn explicit_working_directory_is_preserved() {
        assert_eq!(
            resolve_working_directory("/tmp/slight-session"),
            PathBuf::from("/tmp/slight-session")
        );
    }
}
