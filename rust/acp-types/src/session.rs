//! The normalized agent-session boundary consumed by `session-core`.
//!
//! ACP is bidirectional: while the host waits for the response to a
//! `session/prompt`, the agent keeps streaming `session/update` notifications
//! and may call back with `session/request_permission`. [`AcpClient`] models
//! that correctly but is synchronous — a single thread blocks inside
//! `prompt` until the turn ends.
//!
//! [`ClientSession`] bridges the synchronous client into the poll-based,
//! object-safe boundary `session-core` needs:
//!
//! - one worker thread per session owns the [`AcpClient`];
//! - the [`ClientHandler`] normalizes every inbound update into an
//!   [`AgentEvent`] and forwards it over a channel;
//! - a deferred permission request blocks inside the handler until
//!   [`AcpSession::respond_permission`] answers it, which is exactly what the
//!   agent is waiting on;
//! - `session/cancel` is written through a [`SharedWriter`] so it can be sent
//!   even while the worker thread is blocked inside a prompt turn.
//!
//! This type is deliberately agent-neutral: it maps conformant ACP wire types
//! into Slight's normalized [`AgentEvent`] vocabulary and knows nothing about
//! `gateway-v1`, subprocess launch, or the native clients.

use crate::client::{
    permission_outcome_value, AcpClient, ClientError, ClientHandler, HandlerOutcome,
};
use crate::content::{normalized_raw, NormalizedContent, NormalizedToolContent, ToolLocation};
use crate::metadata::{
    AcpConfigOption, AcpSessionInfoUpdate, AcpUsage, AgentCapabilitiesSummary, AuthMethodSummary,
    AvailableCommandSummary, ListedSessionSummary, PlanSummary, SessionMetadata,
};
use crate::transport::Transport;
use crate::wire::{
    AgentCapabilities, AgentToClientNotification, AgentToClientRequest,
    ContentBlock as WireContentBlock, Error as WireError, Implementation, ListSessionsRequest,
    PermissionOption as WirePermissionOption, PermissionOptionKind as WirePermissionOptionKind,
    RequestId, RequestPermissionOutcome, RequestPermissionRequest, SelectedPermissionOutcome,
    SessionUpdate, SetSessionModeRequest, SetSessionModeResponse, StopReason,
    ToolCall as WireToolCall, ToolCallStatus as WireToolCallStatus,
    ToolCallUpdate as WireToolCallUpdate, ToolKind as WireToolKind, AGENT_METHOD_NAMES,
};
use crate::{
    AcpCommand, AcpConnection, AcpError, AcpEvent, DiagnosticLevel, MessageRole, PermissionOption,
    PermissionOptionKind, PermissionRequest, ToolCallStatus, ToolCallUpdate,
};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;

/// A normalized, agent-neutral event emitted by an ACP session.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentEvent {
    /// A streamed user or assistant message chunk.
    Message {
        role: MessageRole,
        content: NormalizedContent,
    },
    /// A streamed agent reasoning/thought chunk.
    Thought { content: NormalizedContent },
    /// A tool-call creation or progress update.
    ToolCall(ToolCallUpdate),
    /// A full execution-plan replacement from the agent.
    Plan(PlanSummary),
    /// A replacement list of slash commands the agent makes available.
    AvailableCommands(Vec<AvailableCommandSummary>),
    /// A replacement set of session configuration options.
    ConfigOptions(Vec<AcpConfigOption>),
    /// A partial session metadata update (title and last-activity timestamp).
    SessionInfo(AcpSessionInfoUpdate),
    /// Context-window and cost usage for the session.
    Usage(AcpUsage),
    /// The agent switched its current session mode.
    Mode { current_mode_id: String },
    /// A permission request awaiting a client decision.
    PermissionRequest(PermissionRequest),
    /// Diagnostics from the agent or the transport.
    Diagnostics {
        level: DiagnosticLevel,
        message: String,
    },
    /// The prompt turn finished, with the agent's stop reason.
    TurnEnded { stop_reason: StopReason },
    /// The agent process or connection exited.
    Exited { code: Option<i32>, reason: String },
    /// A structured protocol, decode, or transport failure.
    Failed { message: String },
}

/// The subset of the `initialize` response the host consumes.
#[derive(Debug, Clone)]
pub struct InitializeSummary {
    pub protocol_version: u16,
    pub agent_name: Option<String>,
    /// Human-readable agent title, when the agent advertises one.
    pub agent_title: Option<String>,
    pub agent_version: Option<String>,
    pub agent_capabilities: AgentCapabilities,
    /// Normalized authentication methods the agent advertises.
    pub auth_methods: Vec<AuthMethodSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedSessionPage {
    pub sessions: Vec<ListedSessionSummary>,
    pub next_cursor: Option<String>,
}

impl InitializeSummary {
    /// The flattened capability view safe to expose through the gateway.
    pub fn capability_summary(&self) -> AgentCapabilitiesSummary {
        AgentCapabilitiesSummary::from(&self.agent_capabilities)
    }
}

/// The object-safe, normalized view of one ACP agent session.
///
/// Implementations must be usable from `session-core`'s synchronous,
/// mutex-guarded state machine: `drain` must never block, and the command
/// methods must not require the caller to hold a lock on the connection.
pub trait AcpSession: Send {
    /// Sends `initialize` and returns the negotiated agent summary.
    fn initialize(&mut self, client: Implementation) -> Result<InitializeSummary, AcpError>;
    /// Sends `session/new` and returns the agent-side session id and metadata.
    fn new_session(&mut self, cwd: &Path) -> Result<SessionMetadata, AcpError>;
    /// Lists existing agent sessions, filtered by `cwd` when one is supplied.
    ///
    /// Requires the agent to advertise `sessionCapabilities.list`.
    fn list_sessions(
        &mut self,
        cwd: Option<&Path>,
        cursor: Option<&str>,
    ) -> Result<ListedSessionPage, AcpError>;
    /// Sends `session/load` for an existing agent session, replaying its
    /// history. The replayed `session/update` notifications are normalized into
    /// [`AgentEvent`]s and delivered through [`Self::drain`] exactly like live
    /// updates; this call returns once the response arrives.
    ///
    /// Requires the agent to advertise the top-level `loadSession` capability.
    fn load_session(
        &mut self,
        agent_session_id: &str,
        cwd: &Path,
    ) -> Result<SessionMetadata, AcpError>;
    /// Sends `session/resume` for an existing agent session without replaying
    /// history.
    ///
    /// Requires the agent to advertise `sessionCapabilities.resume`.
    fn resume_session(
        &mut self,
        agent_session_id: &str,
        cwd: &Path,
    ) -> Result<SessionMetadata, AcpError>;
    /// Starts a prompt turn. Progress arrives asynchronously via [`Self::drain`].
    fn prompt(&mut self, text: &str) -> Result<(), AcpError>;
    /// Requests cancellation of the in-flight turn.
    fn cancel(&mut self) -> Result<(), AcpError>;
    /// Answers the outstanding permission request with `option_id`.
    fn respond_permission(&mut self, option_id: &str) -> Result<(), AcpError>;
    /// Changes the ACP session mode advertised by the agent.
    fn set_mode(&mut self, mode_id: &str) -> Result<(), AcpError>;
    /// Sets a selectable session configuration option and returns its updated options.
    fn set_config_option(
        &mut self,
        config_id: &str,
        value_id: &str,
    ) -> Result<Vec<AcpConfigOption>, AcpError> {
        let _ = (config_id, value_id);
        Err(AcpError::new("session configuration is not supported"))
    }
    /// Drains buffered normalized events without blocking.
    fn drain(&mut self) -> Vec<AgentEvent>;
    /// Whether the agent connection is still alive.
    fn is_running(&mut self) -> bool;
}

/// A clonable writer that serializes whole frames across threads.
///
/// `session-core` holds one clone to send `session/cancel` while the worker
/// thread owns another clone inside its [`AcpClient`]. `write_all` and `flush`
/// are each performed under the same lock, and [`crate::transport::FrameWriter`]
/// always writes a frame body plus its newline in a single `write_all`, so two
/// frames can never interleave.
pub struct SharedWriter {
    inner: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl SharedWriter {
    pub fn new(inner: Box<dyn Write + Send>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Box<dyn Write + Send>> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Clone for SharedWriter {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.lock().write(buf)
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.lock().write_all(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.lock().flush()
    }
}

/// An erased `BufRead` so one `ClientSession` type can own any transport
/// reader, including the test pipe pair and a child process's stdout.
struct BoxedRead(Box<dyn BufRead + Send>);

impl BoxedRead {
    fn new(reader: impl BufRead + Send + 'static) -> Self {
        Self(Box::new(reader))
    }
}

impl io::Read for BoxedRead {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl BufRead for BoxedRead {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.0.fill_buf()
    }

    fn consume(&mut self, amount: usize) {
        self.0.consume(amount)
    }

    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.0.read_until(byte, buf)
    }

    fn read_line(&mut self, buf: &mut String) -> io::Result<usize> {
        self.0.read_line(buf)
    }
}

#[derive(Debug, Default)]
struct PermissionGateState {
    pending: bool,
    decision: Option<RequestPermissionOutcome>,
    cancelled: bool,
    closed: bool,
}

/// Coordinates a deferred permission request between the worker thread (which
/// is blocked inside the handler) and `session-core` (which answers it).
#[derive(Debug, Default)]
struct PermissionGate {
    state: Mutex<PermissionGateState>,
    signal: Condvar,
}

impl PermissionGate {
    fn lock(&self) -> MutexGuard<'_, PermissionGateState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn begin(&self) {
        self.lock().pending = true;
    }

    fn finish(&self) {
        let mut state = self.lock();
        state.pending = false;
        state.decision = None;
        state.cancelled = false;
    }

    fn decide(&self, outcome: RequestPermissionOutcome) -> Result<(), AcpError> {
        let mut state = self.lock();
        if !state.pending {
            return Err(AcpError::new("no permission request is pending"));
        }
        state.decision = Some(outcome);
        self.signal.notify_all();
        Ok(())
    }

    fn cancel(&self) {
        let mut state = self.lock();
        if state.pending {
            state.cancelled = true;
            self.signal.notify_all();
        }
    }

    /// Permanently releases the gate so a worker thread blocked on a
    /// permission request can exit during shutdown.
    fn close(&self) {
        let mut state = self.lock();
        state.closed = true;
        self.signal.notify_all();
    }

    fn wait(&self) -> RequestPermissionOutcome {
        let mut state = self.lock();
        loop {
            if let Some(outcome) = state.decision.take() {
                return outcome;
            }
            if state.cancelled || state.closed {
                return RequestPermissionOutcome::Cancelled;
            }
            state = self
                .signal
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }
}

/// Normalizes every agent-to-client message and defers permission requests
/// until `session-core` answers them.
struct NormalizingHandler {
    events: Sender<AgentEvent>,
    gate: Arc<PermissionGate>,
}

impl ClientHandler for NormalizingHandler {
    fn handle_request(&mut self, id: RequestId, request: AgentToClientRequest) -> HandlerOutcome {
        match request {
            AgentToClientRequest::RequestPermission(request) => {
                let permission = normalize_permission(id, &request);
                self.gate.begin();
                let _ = self.events.send(AgentEvent::PermissionRequest(permission));
                let outcome = self.gate.wait();
                self.gate.finish();
                HandlerOutcome::Respond(Ok(permission_outcome_value(outcome)))
            }
            AgentToClientRequest::Other { .. } => {
                HandlerOutcome::Respond(Err(WireError::method_not_found()))
            }
        }
    }

    fn handle_notification(&mut self, notification: AgentToClientNotification) {
        for event in normalize_notification(notification) {
            let _ = self.events.send(event);
        }
    }
}

enum WorkerCommand {
    Initialize {
        client: Implementation,
        reply: Sender<Result<InitializeSummary, AcpError>>,
    },
    NewSession {
        cwd: PathBuf,
        reply: Sender<Result<SessionMetadata, AcpError>>,
    },
    ListSessions {
        cwd: Option<PathBuf>,
        cursor: Option<String>,
        reply: Sender<Result<ListedSessionPage, AcpError>>,
    },
    LoadSession {
        agent_session_id: String,
        cwd: PathBuf,
        reply: Sender<Result<SessionMetadata, AcpError>>,
    },
    ResumeSession {
        agent_session_id: String,
        cwd: PathBuf,
        reply: Sender<Result<SessionMetadata, AcpError>>,
    },
    Prompt {
        text: String,
    },
    SetMode {
        mode_id: String,
        reply: Sender<Result<(), AcpError>>,
    },
    SetConfigOption {
        config_id: String,
        value_id: String,
        reply: Sender<Result<Vec<AcpConfigOption>, AcpError>>,
    },
    Shutdown,
}

type BoxedReader = BoxedRead;
type BoxedWriter = SharedWriter;
type Client = AcpClient<BoxedReader, BoxedWriter>;

/// A [`AcpSession`] backed by the conformant [`AcpClient`] on a worker thread.
pub struct ClientSession {
    commands: Sender<WorkerCommand>,
    events: Receiver<AgentEvent>,
    gate: Arc<PermissionGate>,
    writer: BoxedWriter,
    running: Arc<AtomicBool>,
    agent_session_id: Option<String>,
    handle: Option<JoinHandle<()>>,
}

impl ClientSession {
    /// Takes ownership of an already-connected ACP transport.
    pub fn new<R, W>(transport: Transport<R, W>) -> Self
    where
        R: BufRead + Send + 'static,
        W: Write + Send + 'static,
    {
        let (reader, writer) = transport.split();
        let shared_writer = SharedWriter::new(Box::new(writer.into_inner()));
        let client_transport =
            Transport::new(BoxedRead::new(reader.into_inner()), shared_writer.clone());

        let (events_tx, events_rx) = mpsc::channel();
        let (commands_tx, commands_rx) = mpsc::channel();
        let gate = Arc::new(PermissionGate::default());
        let handler = NormalizingHandler {
            events: events_tx.clone(),
            gate: Arc::clone(&gate),
        };
        let client = AcpClient::new(client_transport, Box::new(handler));

        let running = Arc::new(AtomicBool::new(true));
        let worker_running = Arc::clone(&running);
        let handle = std::thread::spawn(move || {
            run_worker(client, commands_rx, events_tx, worker_running);
        });

        Self {
            commands: commands_tx,
            events: events_rx,
            gate,
            writer: shared_writer,
            running,
            agent_session_id: None,
            handle: Some(handle),
        }
    }
}

impl AcpSession for ClientSession {
    fn initialize(&mut self, client: Implementation) -> Result<InitializeSummary, AcpError> {
        let (reply, response) = mpsc::channel();
        self.commands
            .send(WorkerCommand::Initialize { client, reply })
            .map_err(|_| AcpError::new("agent session worker stopped"))?;
        response
            .recv()
            .map_err(|_| AcpError::new("agent session worker stopped"))?
    }

    fn new_session(&mut self, cwd: &Path) -> Result<SessionMetadata, AcpError> {
        let (reply, response) = mpsc::channel();
        self.commands
            .send(WorkerCommand::NewSession {
                cwd: cwd.to_path_buf(),
                reply,
            })
            .map_err(|_| AcpError::new("agent session worker stopped"))?;
        let metadata = response
            .recv()
            .map_err(|_| AcpError::new("agent session worker stopped"))??;
        self.agent_session_id = Some(metadata.agent_session_id.clone());
        Ok(metadata)
    }

    fn list_sessions(
        &mut self,
        cwd: Option<&Path>,
        cursor: Option<&str>,
    ) -> Result<ListedSessionPage, AcpError> {
        let (reply, response) = mpsc::channel();
        self.commands
            .send(WorkerCommand::ListSessions {
                cwd: cwd.map(Path::to_path_buf),
                cursor: cursor.map(str::to_string),
                reply,
            })
            .map_err(|_| AcpError::new("agent session worker stopped"))?;
        response
            .recv()
            .map_err(|_| AcpError::new("agent session worker stopped"))?
    }

    fn load_session(
        &mut self,
        agent_session_id: &str,
        cwd: &Path,
    ) -> Result<SessionMetadata, AcpError> {
        let (reply, response) = mpsc::channel();
        self.commands
            .send(WorkerCommand::LoadSession {
                agent_session_id: agent_session_id.to_string(),
                cwd: cwd.to_path_buf(),
                reply,
            })
            .map_err(|_| AcpError::new("agent session worker stopped"))?;
        let metadata = response
            .recv()
            .map_err(|_| AcpError::new("agent session worker stopped"))??;
        self.agent_session_id = Some(metadata.agent_session_id.clone());
        Ok(metadata)
    }

    fn resume_session(
        &mut self,
        agent_session_id: &str,
        cwd: &Path,
    ) -> Result<SessionMetadata, AcpError> {
        let (reply, response) = mpsc::channel();
        self.commands
            .send(WorkerCommand::ResumeSession {
                agent_session_id: agent_session_id.to_string(),
                cwd: cwd.to_path_buf(),
                reply,
            })
            .map_err(|_| AcpError::new("agent session worker stopped"))?;
        let metadata = response
            .recv()
            .map_err(|_| AcpError::new("agent session worker stopped"))??;
        self.agent_session_id = Some(metadata.agent_session_id.clone());
        Ok(metadata)
    }

    fn prompt(&mut self, text: &str) -> Result<(), AcpError> {
        self.commands
            .send(WorkerCommand::Prompt {
                text: text.to_string(),
            })
            .map_err(|_| AcpError::new("agent session worker stopped"))
    }

    fn cancel(&mut self) -> Result<(), AcpError> {
        let session_id = self
            .agent_session_id
            .clone()
            .ok_or_else(|| AcpError::new("session is not open"))?;
        let frame = crate::wire::cancel_notification(session_id).map_err(AcpError::from)?;
        self.writer.write_all(&frame).map_err(AcpError::from)?;
        self.writer.flush().map_err(AcpError::from)?;
        self.gate.cancel();
        Ok(())
    }

    fn respond_permission(&mut self, option_id: &str) -> Result<(), AcpError> {
        let outcome = RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
            option_id.to_string(),
        ));
        self.gate.decide(outcome)
    }

    fn set_mode(&mut self, mode_id: &str) -> Result<(), AcpError> {
        let (reply, response) = mpsc::channel();
        self.commands
            .send(WorkerCommand::SetMode {
                mode_id: mode_id.to_string(),
                reply,
            })
            .map_err(|_| AcpError::new("agent session worker stopped"))?;
        response
            .recv()
            .map_err(|_| AcpError::new("agent session worker stopped"))?
    }

    fn set_config_option(
        &mut self,
        config_id: &str,
        value_id: &str,
    ) -> Result<Vec<AcpConfigOption>, AcpError> {
        let (reply, response) = mpsc::channel();
        self.commands
            .send(WorkerCommand::SetConfigOption {
                config_id: config_id.to_string(),
                value_id: value_id.to_string(),
                reply,
            })
            .map_err(|_| AcpError::new("agent session worker stopped"))?;
        response
            .recv()
            .map_err(|_| AcpError::new("agent session worker stopped"))?
    }

    fn drain(&mut self) -> Vec<AgentEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.events.try_recv() {
            events.push(event);
        }
        events
    }

    fn is_running(&mut self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

impl Drop for ClientSession {
    fn drop(&mut self) {
        self.gate.close();
        let _ = self.commands.send(WorkerCommand::Shutdown);
        if let Some(handle) = self.handle.take() {
            drop(handle);
        }
    }
}

fn run_worker(
    mut client: Client,
    commands: Receiver<WorkerCommand>,
    events: Sender<AgentEvent>,
    running: Arc<AtomicBool>,
) {
    let mut agent_session_id: Option<String> = None;
    while let Ok(command) = commands.recv() {
        match command {
            WorkerCommand::Initialize {
                client: info,
                reply,
            } => {
                let result = match client
                    .initialize(crate::client::host_client_capabilities(), info)
                {
                    Ok(response) => {
                        let agent_info = response.agent_info;
                        Ok(InitializeSummary {
                            protocol_version: response.protocol_version.as_u16(),
                            agent_name: agent_info.as_ref().map(|info| info.name.clone()),
                            agent_title: agent_info.as_ref().and_then(|info| info.title.clone()),
                            agent_version: agent_info.as_ref().map(|info| info.version.clone()),
                            agent_capabilities: response.agent_capabilities,
                            auth_methods: response
                                .auth_methods
                                .iter()
                                .map(AuthMethodSummary::from)
                                .collect(),
                        })
                    }
                    Err(error) => Err(AcpError::new(error.to_string())),
                };
                let _ = reply.send(result);
            }
            WorkerCommand::NewSession { cwd, reply } => {
                let result = match client.new_session(cwd) {
                    Ok(response) => {
                        let metadata = SessionMetadata {
                            agent_session_id: response.session_id.to_string(),
                            modes: response
                                .modes
                                .as_ref()
                                .map(crate::metadata::AcpModeState::from),
                            config_options: response
                                .config_options
                                .as_deref()
                                .unwrap_or_default()
                                .iter()
                                .map(AcpConfigOption::from)
                                .collect(),
                        };
                        agent_session_id = Some(metadata.agent_session_id.clone());
                        Ok(metadata)
                    }
                    Err(error) => Err(AcpError::new(error.to_string())),
                };
                let _ = reply.send(result);
            }
            WorkerCommand::ListSessions { cwd, cursor, reply } => {
                let result = (|| {
                    let request = ListSessionsRequest::new().cwd(cwd).cursor(cursor);
                    let response = client
                        .list_sessions(request)
                        .map_err(|error| AcpError::new(error.to_string()))?;
                    Ok(ListedSessionPage {
                        sessions: response
                            .sessions
                            .iter()
                            .map(ListedSessionSummary::from)
                            .collect(),
                        next_cursor: response.next_cursor,
                    })
                })();
                let _ = reply.send(result);
            }
            WorkerCommand::LoadSession {
                agent_session_id: id,
                cwd,
                reply,
            } => {
                let result = (|| {
                    let response = client
                        .load_session(id.clone(), cwd)
                        .map_err(|error| AcpError::new(error.to_string()))?;
                    let metadata = SessionMetadata {
                        agent_session_id: id,
                        modes: response
                            .modes
                            .as_ref()
                            .map(crate::metadata::AcpModeState::from),
                        config_options: response
                            .config_options
                            .as_deref()
                            .unwrap_or_default()
                            .iter()
                            .map(AcpConfigOption::from)
                            .collect(),
                    };
                    agent_session_id = Some(metadata.agent_session_id.clone());
                    Ok(metadata)
                })();
                let _ = reply.send(result);
            }
            WorkerCommand::ResumeSession {
                agent_session_id: id,
                cwd,
                reply,
            } => {
                let result = (|| {
                    let response = client
                        .resume_session(id.clone(), cwd)
                        .map_err(|error| AcpError::new(error.to_string()))?;
                    let metadata = SessionMetadata {
                        agent_session_id: id,
                        modes: response
                            .modes
                            .as_ref()
                            .map(crate::metadata::AcpModeState::from),
                        config_options: response
                            .config_options
                            .as_deref()
                            .unwrap_or_default()
                            .iter()
                            .map(AcpConfigOption::from)
                            .collect(),
                    };
                    agent_session_id = Some(metadata.agent_session_id.clone());
                    Ok(metadata)
                })();
                let _ = reply.send(result);
            }
            WorkerCommand::SetConfigOption {
                config_id,
                value_id,
                reply,
            } => {
                let result = (|| {
                    let session_id = agent_session_id
                        .clone()
                        .ok_or_else(|| AcpError::new("session is not open"))?;
                    let response = client
                        .set_config_option(session_id, config_id, value_id)
                        .map_err(|error| AcpError::new(error.to_string()))?;
                    Ok(response
                        .config_options
                        .iter()
                        .map(AcpConfigOption::from)
                        .collect())
                })();
                let _ = reply.send(result);
            }
            WorkerCommand::Prompt { text } => {
                let Some(session_id) = agent_session_id.clone() else {
                    let _ = events.send(AgentEvent::Failed {
                        message: "prompt sent before session/new completed".to_string(),
                    });
                    continue;
                };
                match client.prompt(session_id, text) {
                    Ok(response) => {
                        let _ = events.send(AgentEvent::TurnEnded {
                            stop_reason: response.stop_reason,
                        });
                    }
                    Err(ClientError::Closed) => {
                        let _ = events.send(AgentEvent::Exited {
                            code: None,
                            reason: "agent connection closed".to_string(),
                        });
                        running.store(false, Ordering::SeqCst);
                        break;
                    }
                    Err(error) => {
                        let _ = events.send(AgentEvent::Failed {
                            message: error.to_string(),
                        });
                    }
                }
            }
            WorkerCommand::SetMode { mode_id, reply } => {
                let result = (|| {
                    let Some(session_id) = agent_session_id.clone() else {
                        return Err(AcpError::new("session is not open"));
                    };
                    let request = SetSessionModeRequest::new(session_id, mode_id);
                    client
                        .request(AGENT_METHOD_NAMES.session_set_mode, request)
                        .map(|_: SetSessionModeResponse| ())
                        .map_err(|error| AcpError::new(error.to_string()))
                })();
                let _ = reply.send(result);
            }
            WorkerCommand::Shutdown => break,
        }
    }
    running.store(false, Ordering::SeqCst);
}

fn normalize_notification(notification: AgentToClientNotification) -> Vec<AgentEvent> {
    match notification {
        AgentToClientNotification::SessionUpdate(notification) => {
            normalize_update(notification.update)
        }
        AgentToClientNotification::CancelRequest(_) => vec![AgentEvent::Diagnostics {
            level: DiagnosticLevel::Info,
            message: "agent requested protocol-level cancellation".to_string(),
        }],
        AgentToClientNotification::Other { method, .. } => vec![AgentEvent::Diagnostics {
            level: DiagnosticLevel::Debug,
            message: format!("ignoring unsupported agent notification: {method}"),
        }],
    }
}

fn normalize_update(update: SessionUpdate) -> Vec<AgentEvent> {
    match update {
        SessionUpdate::UserMessageChunk(chunk) => content_event(MessageRole::User, chunk.content),
        SessionUpdate::AgentMessageChunk(chunk) => {
            content_event(MessageRole::Assistant, chunk.content)
        }
        SessionUpdate::AgentThoughtChunk(chunk) => thought_event(chunk.content),
        SessionUpdate::ToolCall(tool) => vec![AgentEvent::ToolCall(normalize_tool_call(tool))],
        SessionUpdate::ToolCallUpdate(update) => {
            vec![AgentEvent::ToolCall(normalize_tool_call_update(update))]
        }
        SessionUpdate::Plan(plan) => vec![AgentEvent::Plan(PlanSummary::from(&plan))],
        SessionUpdate::AvailableCommandsUpdate(update) => vec![AgentEvent::AvailableCommands(
            update
                .available_commands
                .iter()
                .map(AvailableCommandSummary::from)
                .collect(),
        )],
        SessionUpdate::CurrentModeUpdate(update) => vec![AgentEvent::Mode {
            current_mode_id: update.current_mode_id.0.to_string(),
        }],
        SessionUpdate::ConfigOptionUpdate(update) => vec![AgentEvent::ConfigOptions(
            update
                .config_options
                .iter()
                .map(AcpConfigOption::from)
                .collect(),
        )],
        SessionUpdate::SessionInfoUpdate(update) => {
            vec![AgentEvent::SessionInfo(AcpSessionInfoUpdate::from(&update))]
        }
        SessionUpdate::UsageUpdate(update) => vec![AgentEvent::Usage(AcpUsage::from(&update))],
        _ => Vec::new(),
    }
}

fn content_event(role: MessageRole, content: WireContentBlock) -> Vec<AgentEvent> {
    NormalizedContent::from_wire(&content)
        .map(|content| vec![AgentEvent::Message { role, content }])
        .unwrap_or_default()
}

fn thought_event(content: WireContentBlock) -> Vec<AgentEvent> {
    NormalizedContent::from_wire(&content)
        .map(|content| vec![AgentEvent::Thought { content }])
        .unwrap_or_default()
}

fn normalize_tool_call(tool: WireToolCall) -> ToolCallUpdate {
    let detail = tool.locations.first().map(location_detail);
    ToolCallUpdate {
        tool_call_id: tool.tool_call_id.to_string(),
        title: tool.title,
        kind: tool_kind_string(tool.kind),
        status: map_tool_status(tool.status),
        detail,
        content: normalize_tool_content(&tool.content),
        locations: tool.locations.iter().map(ToolLocation::from).collect(),
        raw_input: normalized_raw(tool.raw_input.as_ref()),
        raw_output: normalized_raw(tool.raw_output.as_ref()),
    }
}

fn normalize_tool_call_update(update: WireToolCallUpdate) -> ToolCallUpdate {
    let detail = update
        .fields
        .locations
        .as_deref()
        .and_then(|locations| locations.first().map(location_detail));
    ToolCallUpdate {
        tool_call_id: update.tool_call_id.to_string(),
        title: update.fields.title.clone().unwrap_or_default(),
        kind: update.fields.kind.and_then(tool_kind_string),
        status: update
            .fields
            .status
            .map(map_tool_status)
            .unwrap_or(ToolCallStatus::Pending),
        detail,
        content: update
            .fields
            .content
            .as_deref()
            .map(normalize_tool_content)
            .unwrap_or_default(),
        locations: update
            .fields
            .locations
            .as_deref()
            .map(|locations| locations.iter().map(ToolLocation::from).collect())
            .unwrap_or_default(),
        raw_input: normalized_raw(update.fields.raw_input.as_ref()),
        raw_output: normalized_raw(update.fields.raw_output.as_ref()),
    }
}

fn normalize_tool_content(content: &[crate::wire::ToolCallContent]) -> Vec<NormalizedToolContent> {
    content
        .iter()
        .filter_map(NormalizedToolContent::from_wire)
        .collect()
}

fn location_detail(location: &crate::wire::ToolCallLocation) -> String {
    match location.line {
        Some(line) => format!("{}:{line}", location.path.display()),
        None => location.path.display().to_string(),
    }
}

fn tool_kind_string(kind: WireToolKind) -> Option<String> {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
}

fn map_tool_status(status: WireToolCallStatus) -> ToolCallStatus {
    match status {
        WireToolCallStatus::Pending => ToolCallStatus::Pending,
        WireToolCallStatus::InProgress => ToolCallStatus::InProgress,
        WireToolCallStatus::Completed => ToolCallStatus::Completed,
        WireToolCallStatus::Failed => ToolCallStatus::Failed,
        _ => ToolCallStatus::Pending,
    }
}

fn normalize_permission(id: RequestId, request: &RequestPermissionRequest) -> PermissionRequest {
    let title = request
        .tool_call
        .fields
        .title
        .clone()
        .unwrap_or_else(|| "Permission required".to_string());
    let detail = request
        .tool_call
        .fields
        .raw_input
        .as_ref()
        .and_then(|input| serde_json::to_string_pretty(input).ok());
    PermissionRequest {
        permission_id: id.to_string(),
        title,
        detail,
        tool_call_id: Some(request.tool_call.tool_call_id.to_string()),
        options: request
            .options
            .iter()
            .map(normalize_permission_option)
            .collect(),
    }
}

fn normalize_permission_option(option: &WirePermissionOption) -> PermissionOption {
    PermissionOption {
        option_id: option.option_id.to_string(),
        label: option.name.clone(),
        kind: map_permission_kind(option.kind),
    }
}

fn map_permission_kind(kind: WirePermissionOptionKind) -> PermissionOptionKind {
    match kind {
        WirePermissionOptionKind::AllowOnce => PermissionOptionKind::Allow,
        WirePermissionOptionKind::AllowAlways => PermissionOptionKind::AllowAlways,
        WirePermissionOptionKind::RejectOnce => PermissionOptionKind::Deny,
        WirePermissionOptionKind::RejectAlways => PermissionOptionKind::DenyAlways,
        _ => PermissionOptionKind::Custom,
    }
}

/// Adapts the legacy [`AcpConnection`] command/event surface into the
/// normalized [`AcpSession`] boundary.
///
/// A legacy adapter already performs the ACP handshake inside its own `spawn`,
/// so [`AcpSession::initialize`] and [`AcpSession::new_session`] are
/// bookkeeping only here. This bridge exists so adapters that have not yet
/// migrated (for example the in-progress OpenCode adapter) keep working while
/// `session-core` moves to the normalized surface.
pub struct LegacyAcpSession {
    connection: Box<dyn AcpConnection>,
    pending_permission_id: Option<String>,
}

impl LegacyAcpSession {
    pub fn new(connection: Box<dyn AcpConnection>) -> Self {
        Self {
            connection,
            pending_permission_id: None,
        }
    }

    fn normalize_legacy_event(&mut self, event: AcpEvent) -> AgentEvent {
        match event {
            AcpEvent::Message { role, text } => AgentEvent::Message {
                role,
                content: NormalizedContent::Text { text },
            },
            AcpEvent::ToolCall(update) => AgentEvent::ToolCall(update),
            AcpEvent::PermissionRequest(request) => {
                self.pending_permission_id = Some(request.permission_id.clone());
                AgentEvent::PermissionRequest(request)
            }
            AcpEvent::Diagnostics { level, message } => AgentEvent::Diagnostics { level, message },
            AcpEvent::TurnEnded { stop_reason } => AgentEvent::TurnEnded {
                stop_reason: parse_stop_reason(&stop_reason),
            },
            AcpEvent::Exited { code, reason } => AgentEvent::Exited { code, reason },
        }
    }
}

impl AcpSession for LegacyAcpSession {
    fn initialize(&mut self, _client: Implementation) -> Result<InitializeSummary, AcpError> {
        Ok(InitializeSummary {
            protocol_version: 1,
            agent_name: None,
            agent_title: None,
            agent_version: None,
            agent_capabilities: AgentCapabilities::default(),
            auth_methods: Vec::new(),
        })
    }

    fn new_session(&mut self, _cwd: &Path) -> Result<SessionMetadata, AcpError> {
        // A legacy connection is already bound to its agent session, but the
        // id was never surfaced. Use a stable opaque id until the adapter is
        // migrated to `ClientSession`.
        Ok(SessionMetadata {
            agent_session_id: "legacy-session-1".to_string(),
            modes: None,
            config_options: Vec::new(),
        })
    }

    fn prompt(&mut self, text: &str) -> Result<(), AcpError> {
        self.connection.send(AcpCommand::Prompt {
            text: text.to_string(),
        })
    }

    fn cancel(&mut self) -> Result<(), AcpError> {
        self.connection.send(AcpCommand::Cancel)
    }

    fn respond_permission(&mut self, option_id: &str) -> Result<(), AcpError> {
        let permission_id = self
            .pending_permission_id
            .clone()
            .ok_or_else(|| AcpError::new("no permission request is pending"))?;
        self.connection.send(AcpCommand::RespondPermission {
            permission_id,
            option_id: option_id.to_string(),
        })?;
        self.pending_permission_id = None;
        Ok(())
    }

    fn set_mode(&mut self, _mode_id: &str) -> Result<(), AcpError> {
        Err(AcpError::new(
            "session modes are unavailable on legacy agents",
        ))
    }

    fn list_sessions(
        &mut self,
        _cwd: Option<&Path>,
        _cursor: Option<&str>,
    ) -> Result<ListedSessionPage, AcpError> {
        Err(AcpError::new(
            "session listing is unavailable on legacy agents",
        ))
    }

    fn load_session(
        &mut self,
        _agent_session_id: &str,
        _cwd: &Path,
    ) -> Result<SessionMetadata, AcpError> {
        Err(AcpError::new(
            "session loading is unavailable on legacy agents",
        ))
    }

    fn resume_session(
        &mut self,
        _agent_session_id: &str,
        _cwd: &Path,
    ) -> Result<SessionMetadata, AcpError> {
        Err(AcpError::new(
            "session resuming is unavailable on legacy agents",
        ))
    }

    fn drain(&mut self) -> Vec<AgentEvent> {
        let events = self.connection.try_recv().unwrap_or_default();
        events
            .into_iter()
            .map(|event| self.normalize_legacy_event(event))
            .collect()
    }

    fn is_running(&mut self) -> bool {
        self.connection.is_running()
    }
}

fn parse_stop_reason(value: &str) -> StopReason {
    serde_json::from_value(serde_json::Value::String(value.to_string()))
        .unwrap_or(StopReason::EndTurn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{
        AvailableCommand, ContentChunk, CurrentModeUpdate, Plan, PlanEntry, PlanEntryPriority,
        PlanEntryStatus, SessionMode, SessionModeState, TextContent,
    };

    fn update(value: SessionUpdate) -> Vec<AgentEvent> {
        normalize_update(value)
    }

    #[test]
    fn plan_update_normalizes_to_plan_summary() {
        let events = update(SessionUpdate::Plan(Plan::new(vec![PlanEntry::new(
            "write tests",
            PlanEntryPriority::Medium,
            PlanEntryStatus::Pending,
        )])));
        match events.as_slice() {
            [AgentEvent::Plan(plan)] => {
                assert_eq!(plan.entries.len(), 1);
                assert_eq!(plan.entries[0].content, "write tests");
                assert_eq!(plan.entries[0].status, "pending");
            }
            other => panic!("expected plan event, got {other:?}"),
        }
    }

    #[test]
    fn available_commands_update_normalizes_to_command_summaries() {
        let events = update(SessionUpdate::AvailableCommandsUpdate(
            crate::wire::AvailableCommandsUpdate::new(vec![AvailableCommand::new(
                "compact",
                "Compact the conversation",
            )]),
        ));
        match events.as_slice() {
            [AgentEvent::AvailableCommands(commands)] => {
                assert_eq!(commands.len(), 1);
                assert_eq!(commands[0].name, "compact");
            }
            other => panic!("expected commands event, got {other:?}"),
        }
    }

    #[test]
    fn current_mode_update_normalizes_to_mode_event() {
        let events = update(SessionUpdate::CurrentModeUpdate(CurrentModeUpdate::new(
            "build",
        )));
        assert_eq!(
            events,
            vec![AgentEvent::Mode {
                current_mode_id: "build".to_string(),
            }]
        );
    }

    #[test]
    fn mode_state_round_trips_through_session_metadata() {
        let state = SessionModeState::new("plan", vec![SessionMode::new("plan", "Plan")]);
        let metadata = SessionMetadata {
            agent_session_id: "s-1".to_string(),
            modes: Some(crate::metadata::AcpModeState::from(&state)),
            config_options: Vec::new(),
        };
        assert_eq!(metadata.modes.as_ref().unwrap().current_mode_id, "plan");
    }

    #[test]
    fn config_option_update_normalizes_to_config_options_event() {
        let events =
            update(SessionUpdate::ConfigOptionUpdate(
                crate::wire::ConfigOptionUpdate::new(vec![
                    crate::wire::SessionConfigOption::boolean("brave_mode", "Brave Mode", true),
                ]),
            ));
        match events.as_slice() {
            [AgentEvent::ConfigOptions(options)] => {
                assert_eq!(options.len(), 1);
                assert_eq!(options[0].id, "brave_mode");
            }
            other => panic!("expected config options event, got {other:?}"),
        }
    }

    #[test]
    fn session_info_update_normalizes_to_session_info_event() {
        let events = update(SessionUpdate::SessionInfoUpdate(
            crate::wire::SessionInfoUpdate::new().title("Renamed"),
        ));
        match events.as_slice() {
            [AgentEvent::SessionInfo(update)] => {
                assert_eq!(update.title, Some(Some("Renamed".to_string())));
            }
            other => panic!("expected session info event, got {other:?}"),
        }
    }

    #[test]
    fn usage_update_normalizes_to_usage_event() {
        let events = update(SessionUpdate::UsageUpdate(
            crate::wire::UsageUpdate::new(1_000, 8_000).cost(crate::wire::Cost::new(0.5, "USD")),
        ));
        match events.as_slice() {
            [AgentEvent::Usage(usage)] => {
                assert_eq!(usage.used, 1_000);
                assert_eq!(usage.size, 8_000);
                assert_eq!(usage.cost.as_ref().unwrap().currency, "USD");
            }
            other => panic!("expected usage event, got {other:?}"),
        }
    }

    #[test]
    fn agent_message_chunk_normalizes_to_assistant_message() {
        let events = update(SessionUpdate::AgentMessageChunk(ContentChunk::new(
            WireContentBlock::Text(TextContent::new("hello")),
        )));
        assert_eq!(
            events,
            vec![AgentEvent::Message {
                role: MessageRole::Assistant,
                content: NormalizedContent::Text {
                    text: "hello".to_string(),
                },
            }]
        );
    }

    #[test]
    fn image_chunk_normalizes_to_rich_content() {
        let events = update(SessionUpdate::AgentMessageChunk(ContentChunk::new(
            WireContentBlock::Image(crate::wire::ImageContent::new("aGk=", "image/png")),
        )));
        assert_eq!(
            events,
            vec![AgentEvent::Message {
                role: MessageRole::Assistant,
                content: NormalizedContent::Image {
                    data_base64: "aGk=".to_string(),
                    mime_type: "image/png".to_string(),
                    uri: None,
                },
            }]
        );
    }

    #[test]
    fn thought_chunk_normalizes_to_thought() {
        let events = update(SessionUpdate::AgentThoughtChunk(ContentChunk::new(
            WireContentBlock::Text(TextContent::new("thinking")),
        )));
        assert_eq!(
            events,
            vec![AgentEvent::Thought {
                content: NormalizedContent::Text {
                    text: "thinking".to_string(),
                },
            }]
        );
    }

    #[test]
    fn tool_call_carries_content_locations_and_raw_io() {
        let tool = WireToolCall::new("tool-1", "edit_file")
            .status(WireToolCallStatus::Completed)
            .content(vec![
                crate::wire::ToolCallContent::Diff(
                    crate::wire::Diff::new("/work/src/main.rs", "new").old_text("old"),
                ),
                crate::wire::ToolCallContent::Terminal(crate::wire::Terminal::new("term-1")),
            ])
            .locations(vec![crate::wire::ToolCallLocation::new(
                "/work/src/main.rs",
            )
            .line(3)])
            .raw_input(serde_json::json!({ "path": "/work/src/main.rs" }))
            .raw_output(serde_json::json!({ "exit": 0 }));
        let update = normalize_tool_call(tool);
        assert_eq!(update.detail.as_deref(), Some("/work/src/main.rs:3"));
        assert_eq!(update.locations.len(), 1);
        assert_eq!(update.locations[0].line, Some(3));
        assert_eq!(update.content.len(), 2);
        assert!(matches!(
            &update.content[0],
            NormalizedToolContent::Diff(diff) if diff.path == "/work/src/main.rs"
        ));
        assert!(matches!(
            &update.content[1],
            NormalizedToolContent::Terminal(terminal) if terminal.terminal_id == "term-1"
        ));
        assert_eq!(update.raw_input.unwrap()["path"], "/work/src/main.rs");
        assert_eq!(update.raw_output.unwrap()["exit"], 0);
    }

    #[test]
    fn tool_call_update_drops_null_raw_output() {
        let fields = crate::wire::ToolCallUpdateFields::new()
            .status(WireToolCallStatus::Failed)
            .raw_output(serde_json::Value::Null);
        let update = normalize_tool_call_update(WireToolCallUpdate::new("tool-2", fields));
        assert!(update.raw_output.is_none());
        assert_eq!(update.status, ToolCallStatus::Failed);
    }

    #[test]
    fn tool_call_status_maps_without_cancelled() {
        assert_eq!(
            map_tool_status(WireToolCallStatus::InProgress),
            ToolCallStatus::InProgress
        );
        assert_eq!(
            map_tool_status(WireToolCallStatus::Failed),
            ToolCallStatus::Failed
        );
    }

    #[test]
    fn permission_options_map_reject_to_deny() {
        assert_eq!(
            map_permission_kind(WirePermissionOptionKind::RejectOnce),
            PermissionOptionKind::Deny
        );
        assert_eq!(
            map_permission_kind(WirePermissionOptionKind::AllowAlways),
            PermissionOptionKind::AllowAlways
        );
    }

    #[test]
    fn permission_request_uses_envelope_id() {
        let request = RequestPermissionRequest::new(
            "s-1",
            WireToolCallUpdate::new("tool-1", crate::wire::ToolCallUpdateFields::new()),
            vec![],
        );
        let normalized = normalize_permission(RequestId::Number(42), &request);
        assert_eq!(normalized.permission_id, "42");
        assert_eq!(normalized.tool_call_id.as_deref(), Some("tool-1"));
        assert_eq!(normalized.title, "Permission required");
    }
}
