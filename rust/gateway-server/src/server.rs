use crate::admin::HostAdmin;
use crate::auth::{AuthContext, Authenticator, Scope};
use crate::transport;
use acp_types::AgentKind;
use gateway_protocol::{
    command_name, event_name, events::gateway_event, AgentSessionSummaryDto,
    AgentSessionsListParams, AgentSessionsListResult, ArchiveSessionParams, AttachSessionParams,
    BeginPairingParams, ClientHello, CreateSessionParams, Frame, GatewayAck, GatewayCommand,
    GatewayEvent, ImportAgentSessionParams, ImportAgentSessionResult, PingFrame, PongFrame,
    ProtocolError, RenameSessionParams, RespondPermissionParams, RevokeDeviceParams,
    SendInputParams, ServerCapabilities, ServerWelcome, SessionArchiveResult, SessionAttachResult,
    SessionCreateResult, SessionHistoryParams, SessionHistoryResult, SessionInspectResult,
    SessionListResult, SessionRenameResult, SessionSummaryDto, SessionWorkingDirectoriesResult,
    SetSessionConfigOptionParams, SetSessionModeParams, PROTOCOL_VERSION,
};
use serde::Serialize;
use session_core::{
    CreateSessionRequest, ImportSessionRequest, SequencedEvent, SessionError, SessionManager,
};
use std::collections::{HashMap, VecDeque};
use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tungstenite::Message;

/// How long a client has to complete the WebSocket upgrade before the
/// connection is abandoned.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// Socket read timeout once upgraded. Bounds how long the connection thread
/// blocks on incoming messages so it can also drain event subscriptions and
/// send heartbeats.
const POLL_INTERVAL: Duration = Duration::from_millis(50);
/// Bounds a single blocking write so a stalled reader cannot wedge the thread.
const WRITE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct GatewayConfig {
    pub server_name: String,
    pub server_version: String,
    pub host_id: String,
    pub capabilities: ServerCapabilities,
    pub heartbeat_interval_ms: Option<u64>,
    pub logger: Option<Arc<dyn Fn(&str, &str) + Send + Sync>>,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            server_name: "slight-host".to_string(),
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            host_id: "local".to_string(),
            capabilities: ServerCapabilities::default(),
            heartbeat_interval_ms: Some(15_000),
            logger: None,
        }
    }
}

struct ServerShared {
    config: GatewayConfig,
    sessions: Arc<SessionManager>,
    authenticator: Arc<dyn Authenticator>,
    admin: Arc<dyn HostAdmin>,
    shutdown: Arc<AtomicBool>,
}

pub struct GatewayServer {
    shared: Arc<ServerShared>,
    listeners: Vec<TcpListener>,
    local_addrs: Vec<SocketAddr>,
}

impl GatewayServer {
    pub fn bind(
        config: GatewayConfig,
        sessions: Arc<SessionManager>,
        authenticator: Arc<dyn Authenticator>,
        admin: Arc<dyn HostAdmin>,
        shutdown: Arc<AtomicBool>,
        addr: SocketAddr,
    ) -> io::Result<Self> {
        Self::bind_all(config, sessions, authenticator, admin, shutdown, &[addr])
    }

    pub fn bind_all(
        config: GatewayConfig,
        sessions: Arc<SessionManager>,
        authenticator: Arc<dyn Authenticator>,
        admin: Arc<dyn HostAdmin>,
        shutdown: Arc<AtomicBool>,
        addrs: &[SocketAddr],
    ) -> io::Result<Self> {
        if addrs.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "at least one gateway address is required",
            ));
        }
        let mut listeners = Vec::with_capacity(addrs.len());
        let mut local_addrs = Vec::with_capacity(addrs.len());
        for addr in addrs {
            let listener = TcpListener::bind(addr)?;
            local_addrs.push(listener.local_addr()?);
            listeners.push(listener);
        }
        Ok(Self {
            shared: Arc::new(ServerShared {
                config,
                sessions,
                authenticator,
                admin,
                shutdown,
            }),
            listeners,
            local_addrs,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addrs[0]
    }

    pub fn local_addrs(&self) -> &[SocketAddr] {
        &self.local_addrs
    }

    pub fn run(self) -> io::Result<()> {
        for listener in &self.listeners {
            listener.set_nonblocking(true)?;
        }
        while !self.shared.shutdown.load(Ordering::Relaxed) {
            for listener in &self.listeners {
                match listener.accept() {
                    Ok((stream, peer)) => {
                        // The listener is nonblocking for accept polling, but
                        // connection handlers rely on timed blocking reads.
                        stream.set_nonblocking(false)?;
                        let shared = self.shared.clone();
                        std::thread::spawn(move || {
                            if let Err(error) = handle_connection(stream, peer, shared.clone()) {
                                log_message(
                                    &shared,
                                    "error",
                                    format!("connection peer={peer} failed: {error}"),
                                );
                            }
                        });
                    }
                    Err(ref error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock
                                | io::ErrorKind::Interrupted
                                | io::ErrorKind::ConnectionAborted
                        ) => {}
                    Err(error) => return Err(error),
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    }
}

struct ConnState {
    authed: Option<AuthContext>,
    welcome_sent: bool,
    subscriptions: HashMap<String, Receiver<SequencedEvent>>,
    completed_commands: HashMap<String, Frame>,
    completed_command_order: VecDeque<String>,
}

const COMPLETED_COMMAND_LIMIT: usize = 256;

fn handle_connection(
    stream: TcpStream,
    peer: SocketAddr,
    shared: Arc<ServerShared>,
) -> io::Result<()> {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT));
    let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));

    // Complete the RFC 6455 upgrade. A request that is not a WebSocket upgrade
    // (or that arrives too slowly) is rejected by tungstenite and dropped.
    let mut socket = match tungstenite::accept(stream) {
        Ok(socket) => socket,
        Err(error) => {
            log_message(
                &shared,
                "error",
                format!("websocket upgrade peer={peer} failed: {error}"),
            );
            return Err(io::Error::other(error));
        }
    };
    log_message(&shared, "debug", format!("websocket connected peer={peer}"));
    let _ = socket.get_ref().set_read_timeout(Some(POLL_INTERVAL));

    let mut outbound: VecDeque<Frame> = VecDeque::new();
    let mut state = ConnState {
        authed: None,
        welcome_sent: false,
        subscriptions: HashMap::new(),
        completed_commands: HashMap::new(),
        completed_command_order: VecDeque::new(),
    };
    let heartbeat_interval = shared
        .config
        .heartbeat_interval_ms
        .filter(|interval| *interval > 0)
        .map(Duration::from_millis);
    let mut last_heartbeat = Instant::now();

    loop {
        if shared.shutdown.load(Ordering::Relaxed) {
            break;
        }

        match socket.read() {
            Ok(Message::Text(text)) => {
                dispatch_incoming(&shared, &mut state, text.as_bytes(), &mut outbound)
            }
            Ok(Message::Binary(data)) => {
                dispatch_incoming(&shared, &mut state, &data, &mut outbound)
            }
            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) | Ok(Message::Frame(_)) => {}
            Ok(Message::Close(_)) => break,
            Err(error) => {
                // A read timeout is not fatal: fall through, flush any pending
                // work, and poll again.
                if !transport::is_would_block(&error) {
                    log_message(
                        &shared,
                        "error",
                        format!("websocket read peer={peer} failed: {error}"),
                    );
                    break;
                }
            }
        }

        for (session_id, receiver) in state.subscriptions.iter_mut() {
            while let Ok(sequenced) = receiver.try_recv() {
                outbound.push_back(Frame::Event(gateway_event(session_id, &sequenced)));
            }
        }

        if let Some(interval) = heartbeat_interval {
            if last_heartbeat.elapsed() >= interval {
                last_heartbeat = Instant::now();
                outbound.push_back(Frame::Ping(PingFrame {
                    protocol_version: PROTOCOL_VERSION,
                    nonce: uuid::Uuid::new_v4().to_string(),
                }));
            }
        }

        let mut write_failed = false;
        let mut write_blocked = false;
        while let Some(frame) = outbound.pop_front() {
            if let Ok(message) = transport::encode_message(&frame) {
                if let Err(error) = socket.write(message) {
                    if transport::is_would_block(&error) {
                        // The accepted socket can still report EAGAIN while the
                        // peer's receive window is full. Keep the frame queued
                        // and retry on the next poll instead of dropping the
                        // connection and losing the response.
                        outbound.push_front(frame);
                        write_blocked = true;
                        break;
                    }
                    log_message(
                        &shared,
                        "error",
                        format!("websocket write peer={peer} failed: {error}"),
                    );
                    write_failed = true;
                    break;
                }
            }
        }
        if !write_blocked {
            if let Err(error) = socket.flush() {
                if transport::is_would_block(&error) {
                    write_blocked = true;
                } else {
                    log_message(
                        &shared,
                        "error",
                        format!("websocket flush peer={peer} failed: {error}"),
                    );
                    write_failed = true;
                }
            }
        }
        if write_failed {
            break;
        }
        if write_blocked {
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    // Best-effort close handshake. `flush` first so a queued close reply (sent
    // in response to the peer's close) is not lost.
    let _ = socket.flush();
    let _ = socket.close(None);
    log_message(
        &shared,
        "debug",
        format!("websocket disconnected peer={peer}"),
    );
    Ok(())
}

fn log_message(shared: &ServerShared, level: &str, message: String) {
    if let Some(logger) = &shared.config.logger {
        logger(level, &message);
    }
}

fn dispatch_incoming(
    shared: &ServerShared,
    state: &mut ConnState,
    payload: &[u8],
    outbound: &mut VecDeque<Frame>,
) {
    match transport::decode_payload(payload) {
        Ok(frame) => process_frame(shared, state, frame, outbound),
        Err(error) => {
            log_message(
                shared,
                "error",
                format!("gateway frame decode failed: {error}"),
            );
            outbound.push_back(Frame::Error(error.to_wire(None)));
        }
    }
}

fn process_frame(
    shared: &ServerShared,
    state: &mut ConnState,
    frame: Frame,
    outbound: &mut VecDeque<Frame>,
) {
    match frame {
        Frame::Hello(hello) => {
            if hello.protocol_version != PROTOCOL_VERSION {
                outbound.push_back(Frame::Error(
                    ProtocolError::InvalidVersion {
                        requested: hello.protocol_version,
                        supported: PROTOCOL_VERSION,
                    }
                    .to_wire(None),
                ));
                return;
            }
            match shared.authenticator.authenticate(&hello) {
                Ok(context) => {
                    state.welcome_sent = true;
                    state.authed = Some(context);
                    outbound.push_back(Frame::Welcome(build_welcome(shared, &hello)));
                }
                Err(error) => outbound.push_back(Frame::Error(error.to_wire(None))),
            }
        }
        Frame::Command(command) => {
            let request_id = command.request_id.clone();
            log_message(
                shared,
                "debug",
                format!(
                    "gateway command command={} request_id={} session_id={}",
                    command.command,
                    request_id,
                    command.session_id.as_deref().unwrap_or("-")
                ),
            );
            if !state.welcome_sent {
                outbound.push_back(Frame::Ack(GatewayAck::error(
                    request_id,
                    ProtocolError::Unauthenticated("hello required".to_string()).body(),
                )));
                return;
            }
            if let Some(ack) = state.completed_commands.get(&request_id) {
                outbound.push_back(ack.clone());
                return;
            }
            let context = state
                .authed
                .clone()
                .unwrap_or_else(|| AuthContext::all("unauthenticated", "unauthenticated"));
            if let Err(error) = authorize(&context, &command.command) {
                let ack = Frame::Ack(GatewayAck::error(request_id, error.body()));
                remember_command(state, command.request_id, ack.clone());
                outbound.push_back(ack);
                return;
            }
            let response = dispatch(shared, state, command);
            remember_command(state, request_id, response.clone());
            outbound.push_back(response);
        }
        Frame::Ping(ping) => outbound.push_back(Frame::Pong(PongFrame {
            protocol_version: PROTOCOL_VERSION,
            nonce: ping.nonce,
        })),
        Frame::Pong(_) => {}
        other => outbound.push_back(Frame::Error(
            ProtocolError::InvalidFrame(format!("unexpected frame: {}", other.frame_type()))
                .to_wire(None),
        )),
    }
}

fn remember_command(state: &mut ConnState, request_id: String, response: Frame) {
    if state.completed_commands.contains_key(&request_id) {
        return;
    }
    state.completed_command_order.push_back(request_id.clone());
    state.completed_commands.insert(request_id, response);
    while state.completed_command_order.len() > COMPLETED_COMMAND_LIMIT {
        if let Some(oldest) = state.completed_command_order.pop_front() {
            state.completed_commands.remove(&oldest);
        }
    }
}

fn build_welcome(shared: &ServerShared, hello: &ClientHello) -> ServerWelcome {
    ServerWelcome {
        protocol_version: PROTOCOL_VERSION,
        connection_id: uuid::Uuid::new_v4().to_string(),
        server_name: shared.config.server_name.clone(),
        server_version: shared.config.server_version.clone(),
        host_id: Some(shared.config.host_id.clone()),
        capabilities: shared.config.capabilities.clone(),
        heartbeat_interval_ms: shared.config.heartbeat_interval_ms,
        resync_required: hello.resume.is_some(),
        resync_reason: hello.resume.as_ref().map(|_| "attach_required".to_string()),
    }
}

fn authorize(context: &AuthContext, command: &str) -> Result<(), ProtocolError> {
    let scope = match command {
        command_name::SESSION_LIST
        | command_name::SESSION_WORKING_DIRECTORIES
        | command_name::SESSION_ATTACH
        | command_name::SESSION_DETACH
        | command_name::SESSION_INSPECT
        | command_name::SESSION_HISTORY
        | command_name::SESSION_RESUME
        | command_name::EVENTS_REPLAY
        | command_name::AGENT_SESSIONS_LIST => Some(Scope::ReadSessions),
        command_name::SESSION_CREATE | command_name::AGENT_SESSION_IMPORT => {
            Some(Scope::CreateSession)
        }
        command_name::SESSION_INPUT
        | command_name::SESSION_CANCEL
        | command_name::SESSION_RENAME
        | command_name::SESSION_ARCHIVE
        | command_name::SESSION_SET_MODE => Some(Scope::SendInput),
        command_name::SESSION_PERMISSION_RESPOND => Some(Scope::RespondPermission),
        command_name::HOST_STATUS
        | command_name::HOST_START
        | command_name::HOST_STOP
        | command_name::HOST_RESTART
        | command_name::HOST_CONFIGURATION
        | command_name::HOST_DIAGNOSTICS
        | command_name::HOST_SHUTDOWN
        | command_name::DEVICE_LIST
        | command_name::DEVICE_REVOKE
        | command_name::PAIRING_CREATE
        | command_name::PAIRING_LIST => Some(Scope::ManageHost),
        _ => None,
    };
    match scope {
        Some(scope) if !context.has(scope) => Err(ProtocolError::Unauthorized(format!(
            "{command} requires {scope:?}"
        ))),
        _ => Ok(()),
    }
}

fn dispatch(shared: &ServerShared, state: &mut ConnState, command: GatewayCommand) -> Frame {
    let request_id = command.request_id.clone();
    match command.command.as_str() {
        command_name::SESSION_LIST => {
            let sessions = shared
                .sessions
                .list_sessions()
                .iter()
                .map(SessionSummaryDto::from)
                .collect();
            ack_ok(&request_id, &SessionListResult { sessions })
        }
        command_name::SESSION_WORKING_DIRECTORIES => ack_ok(
            &request_id,
            &SessionWorkingDirectoriesResult {
                paths: shared.sessions.workspace_paths(20),
            },
        ),
        command_name::SESSION_CREATE => {
            let params: CreateSessionParams = match command.params_as() {
                Ok(params) => params,
                Err(error) => return ack_error(&request_id, error),
            };
            let request = CreateSessionRequest {
                agent: parse_agent(&params.agent),
                model: params.model,
                effort: params.effort,
                working_directory_label: params.working_directory_label,
                initial_prompt: params.initial_prompt,
            };
            match shared.sessions.create_session(request) {
                Ok(summary) => ack_ok(
                    &request_id,
                    &SessionCreateResult {
                        session: SessionSummaryDto::from(&summary),
                    },
                ),
                Err(error) => ack_error(&request_id, session_error(error)),
            }
        }
        command_name::AGENT_SESSIONS_LIST => {
            let params: AgentSessionsListParams = match command.params_as() {
                Ok(params) => params,
                Err(error) => return ack_error(&request_id, error),
            };
            let agent = parse_agent(&params.agent);
            let cwd = params
                .working_directory_label
                .as_deref()
                .map(session_core::resolve_working_directory);
            match shared.sessions.list_agent_sessions(
                &agent,
                cwd.as_deref(),
                params.cursor.as_deref(),
            ) {
                Ok(page) => {
                    let imported = shared.sessions.imported_agent_session_ids(&agent);
                    let sessions = page
                        .sessions
                        .iter()
                        .filter(|listed| !imported.contains(&listed.agent_session_id))
                        .map(|listed| AgentSessionSummaryDto::from_listed(agent.as_str(), listed))
                        .collect();
                    ack_ok(
                        &request_id,
                        &AgentSessionsListResult {
                            sessions,
                            next_cursor: page.next_cursor,
                        },
                    )
                }
                Err(error) => ack_error(&request_id, session_error(error)),
            }
        }
        command_name::AGENT_SESSION_IMPORT => {
            let params: ImportAgentSessionParams = match command.params_as() {
                Ok(params) => params,
                Err(error) => return ack_error(&request_id, error),
            };
            let request = ImportSessionRequest {
                agent: parse_agent(&params.agent),
                agent_session_id: params.agent_session_id,
                working_directory_label: params.working_directory_label,
                recovery: params.recovery,
                title: params.title,
            };
            match shared.sessions.import_session(request) {
                Ok(summary) => ack_ok(
                    &request_id,
                    &ImportAgentSessionResult {
                        session: SessionSummaryDto::from(&summary),
                    },
                ),
                Err(error) => ack_error(&request_id, session_error(error)),
            }
        }
        command_name::SESSION_ATTACH | command_name::SESSION_RESUME => {
            let Some(session_id) = command.session_id.clone() else {
                return ack_error(
                    &request_id,
                    ProtocolError::InvalidParams("session_id required".to_string()),
                );
            };
            let params: AttachSessionParams = command.params_as().unwrap_or_default();
            match shared.sessions.attach(&session_id, params.after_sequence) {
                Ok(result) => {
                    if let Ok(receiver) = shared.sessions.subscribe(&session_id) {
                        state.subscriptions.insert(session_id.clone(), receiver);
                    }
                    ack_ok(
                        &request_id,
                        &SessionAttachResult {
                            session: SessionSummaryDto::from(&result.summary),
                            replayed: Vec::new(),
                            latest_sequence: result.latest_sequence,
                            oldest_available_sequence: result.oldest_available_sequence,
                            resync_required: result.resync_required,
                        },
                    )
                }
                Err(error) => ack_error(&request_id, session_error(error)),
            }
        }
        command_name::EVENTS_REPLAY => {
            let Some(session_id) = command.session_id.clone() else {
                return ack_error(
                    &request_id,
                    ProtocolError::InvalidParams("session_id required".to_string()),
                );
            };
            let params: AttachSessionParams = command.params_as().unwrap_or_default();
            match shared.sessions.attach(&session_id, params.after_sequence) {
                Ok(result) => {
                    if let Ok(receiver) = shared.sessions.subscribe(&session_id) {
                        state.subscriptions.insert(session_id.clone(), receiver);
                    }
                    let max_replay_events = shared
                        .config
                        .capabilities
                        .max_replay_events
                        .unwrap_or(256)
                        .max(1);
                    let replayed = result
                        .replayed
                        .into_iter()
                        .take(max_replay_events)
                        .map(|event| gateway_event(&session_id, &event))
                        .collect();
                    ack_ok(
                        &request_id,
                        &SessionAttachResult {
                            session: SessionSummaryDto::from(&result.summary),
                            replayed,
                            latest_sequence: result.latest_sequence,
                            oldest_available_sequence: result.oldest_available_sequence,
                            resync_required: result.resync_required,
                        },
                    )
                }
                Err(error) => ack_error(&request_id, session_error(error)),
            }
        }
        command_name::SESSION_HISTORY => {
            let Some(session_id) = command.session_id.clone() else {
                return ack_error(
                    &request_id,
                    ProtocolError::InvalidParams("session_id required".to_string()),
                );
            };
            let params: SessionHistoryParams = command.params_as().unwrap_or_default();
            match shared.sessions.history(
                &session_id,
                params.before_sequence,
                params.limit.unwrap_or(200),
            ) {
                Ok(result) => {
                    let events = result
                        .events
                        .iter()
                        .map(|event| gateway_event(&session_id, event))
                        .collect();
                    ack_ok(
                        &request_id,
                        &SessionHistoryResult {
                            session: SessionSummaryDto::from(&result.summary),
                            events,
                            latest_sequence: result.latest_sequence,
                            oldest_available_sequence: result.oldest_available_sequence,
                            has_more: result.has_more,
                        },
                    )
                }
                Err(error) => ack_error(&request_id, session_error(error)),
            }
        }
        command_name::SESSION_DETACH => {
            if let Some(session_id) = command.session_id.clone() {
                state.subscriptions.remove(&session_id);
            }
            Frame::Ack(GatewayAck::unit(&request_id))
        }
        command_name::SESSION_INPUT => {
            let Some(session_id) = command.session_id.clone() else {
                return ack_error(
                    &request_id,
                    ProtocolError::InvalidParams("session_id required".to_string()),
                );
            };
            let params: SendInputParams = match command.params_as() {
                Ok(params) => params,
                Err(error) => return ack_error(&request_id, error),
            };
            match shared.sessions.send_input(&session_id, &params.text) {
                Ok(()) => Frame::Ack(GatewayAck::unit(&request_id)),
                Err(error) => ack_error(&request_id, session_error(error)),
            }
        }
        command_name::SESSION_CANCEL => {
            let Some(session_id) = command.session_id.clone() else {
                return ack_error(
                    &request_id,
                    ProtocolError::InvalidParams("session_id required".to_string()),
                );
            };
            match shared.sessions.cancel(&session_id) {
                Ok(()) => Frame::Ack(GatewayAck::unit(&request_id)),
                Err(error) => ack_error(&request_id, session_error(error)),
            }
        }
        command_name::SESSION_RENAME => {
            let Some(session_id) = command.session_id.clone() else {
                return ack_error(
                    &request_id,
                    ProtocolError::InvalidParams("session_id required".to_string()),
                );
            };
            let params: RenameSessionParams = match command.params_as() {
                Ok(params) => params,
                Err(error) => return ack_error(&request_id, error),
            };
            match shared.sessions.rename_session(&session_id, &params.title) {
                Ok(summary) => ack_ok(
                    &request_id,
                    &SessionRenameResult {
                        session: SessionSummaryDto::from(&summary),
                    },
                ),
                Err(error) => ack_error(&request_id, session_error(error)),
            }
        }
        command_name::SESSION_ARCHIVE => {
            let Some(session_id) = command.session_id.clone() else {
                return ack_error(
                    &request_id,
                    ProtocolError::InvalidParams("session_id required".to_string()),
                );
            };
            let params: ArchiveSessionParams = match command.params_as() {
                Ok(params) => params,
                Err(error) => return ack_error(&request_id, error),
            };
            match shared
                .sessions
                .archive_session(&session_id, params.archived)
            {
                Ok(summary) => ack_ok(
                    &request_id,
                    &SessionArchiveResult {
                        session: SessionSummaryDto::from(&summary),
                    },
                ),
                Err(error) => ack_error(&request_id, session_error(error)),
            }
        }
        command_name::SESSION_SET_MODE => {
            let Some(session_id) = command.session_id.clone() else {
                return ack_error(
                    &request_id,
                    ProtocolError::InvalidParams("session_id required".to_string()),
                );
            };
            let params: SetSessionModeParams = match command.params_as() {
                Ok(params) => params,
                Err(error) => return ack_error(&request_id, error),
            };
            match shared.sessions.set_mode(&session_id, &params.mode_id) {
                Ok(()) => Frame::Ack(GatewayAck::unit(&request_id)),
                Err(error) => ack_error(&request_id, session_error(error)),
            }
        }
        command_name::SESSION_SET_CONFIG_OPTION => {
            let Some(session_id) = command.session_id.clone() else {
                return ack_error(
                    &request_id,
                    ProtocolError::InvalidParams("session_id required".to_string()),
                );
            };
            let params: SetSessionConfigOptionParams = match command.params_as() {
                Ok(params) => params,
                Err(error) => return ack_error(&request_id, error),
            };
            match shared.sessions.set_config_option(
                &session_id,
                &params.config_id,
                &params.value_id,
            ) {
                Ok(()) => Frame::Ack(GatewayAck::unit(&request_id)),
                Err(error) => ack_error(&request_id, session_error(error)),
            }
        }
        command_name::SESSION_PERMISSION_RESPOND => {
            let Some(session_id) = command.session_id.clone() else {
                return ack_error(
                    &request_id,
                    ProtocolError::InvalidParams("session_id required".to_string()),
                );
            };
            let params: RespondPermissionParams = match command.params_as() {
                Ok(params) => params,
                Err(error) => return ack_error(&request_id, error),
            };
            match shared.sessions.respond_permission(
                &session_id,
                &params.permission_id,
                &params.option_id,
            ) {
                Ok(()) => Frame::Ack(GatewayAck::unit(&request_id)),
                Err(error) => ack_error(&request_id, session_error(error)),
            }
        }
        command_name::SESSION_INSPECT => {
            let Some(session_id) = command.session_id.clone() else {
                return ack_error(
                    &request_id,
                    ProtocolError::InvalidParams("session_id required".to_string()),
                );
            };
            match shared.sessions.inspect_session(&session_id) {
                Ok(detail) => {
                    let recent_events = detail
                        .recent_events
                        .iter()
                        .map(|event| gateway_event(&session_id, event))
                        .collect();
                    ack_ok(
                        &request_id,
                        &SessionInspectResult {
                            session: SessionSummaryDto::from(&detail.summary),
                            agent: gateway_protocol::AgentDescriptorDto::from(&detail.agent),
                            acp: detail.acp,
                            running: detail.running,
                            pending_permission: detail
                                .pending_permission
                                .as_ref()
                                .and_then(|payload| serde_json::to_value(payload).ok()),
                            recent_events,
                        },
                    )
                }
                Err(error) => ack_error(&request_id, session_error(error)),
            }
        }
        command_name::HOST_STATUS => ack_ok(&request_id, &shared.admin.status()),
        command_name::HOST_START => match shared.admin.request_start() {
            Ok(result) => ack_ok(&request_id, &result),
            Err(error) => ack_error(&request_id, error),
        },
        command_name::HOST_STOP | command_name::HOST_SHUTDOWN => {
            match shared.admin.request_stop() {
                Ok(result) => ack_ok(&request_id, &result),
                Err(error) => ack_error(&request_id, error),
            }
        }
        command_name::HOST_RESTART => match shared.admin.request_restart() {
            Ok(result) => ack_ok(&request_id, &result),
            Err(error) => ack_error(&request_id, error),
        },
        command_name::HOST_DIAGNOSTICS => ack_ok(&request_id, &shared.admin.diagnostics()),
        command_name::HOST_CONFIGURATION => ack_ok(&request_id, &shared.admin.status()),
        command_name::PAIRING_CREATE => {
            let params: BeginPairingParams = command.params_as().unwrap_or(BeginPairingParams {
                label: "paired device".to_string(),
            });
            match shared.admin.begin_pairing(&params.label) {
                Ok(result) => ack_ok(&request_id, &result),
                Err(error) => ack_error(&request_id, error),
            }
        }
        command_name::PAIRING_LIST => match shared.admin.list_pairings() {
            Ok(result) => ack_ok(&request_id, &result),
            Err(error) => ack_error(&request_id, error),
        },
        command_name::DEVICE_LIST => match shared.admin.list_devices() {
            Ok(result) => ack_ok(&request_id, &result),
            Err(error) => ack_error(&request_id, error),
        },
        command_name::DEVICE_REVOKE => {
            let params: RevokeDeviceParams = match command.params_as() {
                Ok(params) => params,
                Err(error) => return ack_error(&request_id, error),
            };
            match shared.admin.revoke_device(&params.device_id) {
                Ok(result) => ack_ok(&request_id, &result),
                Err(error) => ack_error(&request_id, error),
            }
        }
        other => Frame::Ack(GatewayAck::error(
            &request_id,
            ProtocolError::UnsupportedCommand(other.to_string()).body(),
        )),
    }
}

fn ack_ok<T: Serialize>(request_id: &str, result: &T) -> Frame {
    Frame::Ack(GatewayAck::ok(request_id, result))
}

fn ack_error(request_id: &str, error: ProtocolError) -> Frame {
    Frame::Ack(GatewayAck::error(request_id, error.body()))
}

fn parse_agent(value: &str) -> AgentKind {
    match value {
        "claude_code" => AgentKind::ClaudeCode,
        "codex" => AgentKind::Codex,
        "fake" => AgentKind::Fake,
        other => AgentKind::Custom(other.to_string()),
    }
}

fn session_error(error: SessionError) -> ProtocolError {
    match error {
        SessionError::UnknownSession(id) => ProtocolError::UnknownSession(id),
        SessionError::UnknownAgent(agent) => {
            ProtocolError::InvalidParams(format!("unknown agent: {agent}"))
        }
        SessionError::NotRunning(id) => {
            ProtocolError::InvalidFrame(format!("session is not running: {id}"))
        }
        SessionError::EmptyTitle => {
            ProtocolError::InvalidParams("session title must not be empty".to_string())
        }
        SessionError::UnsupportedCapability(capability) => {
            ProtocolError::UnsupportedCapability(capability.to_string())
        }
        SessionError::NoPendingPermission(id) => {
            ProtocolError::InvalidFrame(format!("no pending permission for session: {id}"))
        }
        SessionError::PermissionMismatch { session_id, .. } => {
            ProtocolError::InvalidFrame(format!("permission mismatch for session: {session_id}"))
        }
        SessionError::Agent(error) => ProtocolError::Internal(error.to_string()),
        SessionError::Store(error) => ProtocolError::Internal(error.to_string()),
    }
}

pub fn reply_replay_complete(session_id: &str, latest_sequence: u64) -> GatewayEvent {
    gateway_protocol::events::replay_complete(session_id, latest_sequence)
}

pub const HOST_STATUS_EVENT: &str = event_name::HOST_STATUS_CHANGED;

#[cfg(test)]
mod tests {
    use super::session_error;
    use gateway_protocol::error_code;
    use session_core::SessionError;

    #[test]
    fn unsupported_capability_maps_to_stable_code() {
        let error = session_error(SessionError::UnsupportedCapability("session/load"));
        assert_eq!(error.code(), error_code::UNSUPPORTED_CAPABILITY);
        assert!(!error.retryable());
    }
}
