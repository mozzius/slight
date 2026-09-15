//! A wire-level fake ACP agent for conformance tests.
//!
//! Unlike the in-memory fake in `acp-adapters`, this agent speaks real
//! newline-delimited JSON-RPC over any reader/writer pair. It handles
//! `initialize`, `session/new`, `session/list`, `session/load`,
//! `session/resume`, `session/prompt`, and `session/cancel`, streams
//! `session/update` notifications (including `session/load` replay), and
//! performs the full bidirectional `session/request_permission` exchange. It is
//! used both in-process (over pipe pairs) and as the `fake-acp-agent` binary for
//! subprocess tests.

use acp_types::transport::Transport;
use acp_types::wire::{
    self, classify, AgentCapabilities, ConfigOptionUpdate, ContentBlock, ContentChunk, Cost, Diff,
    Error, ImageContent, Implementation, InitializeRequest, InitializeResponse,
    ListSessionsRequest, ListSessionsResponse, LoadSessionRequest, LoadSessionResponse,
    NewSessionRequest, NewSessionResponse, PermissionOption, PermissionOptionKind, PromptRequest,
    PromptResponse, ProtocolVersion, RawMessage, RequestId, RequestPermissionOutcome,
    RequestPermissionRequest, RequestPermissionResponse, ResumeSessionRequest,
    ResumeSessionResponse, SessionCapabilities, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigSelectOption, SessionId, SessionInfo, SessionInfoUpdate, SessionListCapabilities,
    SessionMode, SessionModeState, SessionNotification, SessionResumeCapabilities, SessionUpdate,
    StopReason, Terminal, TextContent, ToolCall, ToolCallContent, ToolCallLocation, ToolCallStatus,
    ToolCallUpdate, ToolCallUpdateFields, ToolContent, UsageUpdate, WireError, AGENT_METHOD_NAMES,
    CLIENT_METHOD_NAMES,
};
use serde_json::Value;
use std::io::{BufRead, BufReader, BufWriter, PipeReader, PipeWriter, Write};
use std::thread::JoinHandle;

/// Behavior switches for the fake agent.
#[derive(Debug, Clone)]
pub struct FakeWireAgentConfig {
    /// Session id returned by `session/new`.
    pub session_id: String,
    pub agent_name: String,
    pub agent_version: String,
    /// Whether a prompt turn opens a `session/request_permission` exchange.
    pub request_permission: bool,
    /// Whether `session/cancel` is honored while awaiting a permission response.
    pub cancellation_supported: bool,
    /// Whether the agent advertises `session/list` and answers it.
    pub list_supported: bool,
    /// Whether the agent advertises the `loadSession` capability and answers
    /// `session/load` with replayed history.
    pub load_supported: bool,
    /// Whether the agent advertises `session/resume` and answers it.
    pub resume_supported: bool,
}

impl Default for FakeWireAgentConfig {
    fn default() -> Self {
        Self {
            session_id: "session-1".to_string(),
            agent_name: "fake-acp-agent".to_string(),
            agent_version: "0.1.0".to_string(),
            request_permission: true,
            cancellation_supported: true,
            list_supported: true,
            load_supported: true,
            resume_supported: true,
        }
    }
}

impl FakeWireAgentConfig {
    /// Reads overrides from the environment, for the subprocess binary.
    pub fn from_env() -> Self {
        let mut config = Self::default();
        if let Ok(value) = std::env::var("FAKE_ACP_REQUEST_PERMISSION") {
            config.request_permission = value != "0" && !value.eq_ignore_ascii_case("false");
        }
        if let Ok(value) = std::env::var("FAKE_ACP_SESSION_ID") {
            config.session_id = value;
        }
        if let Ok(value) = std::env::var("FAKE_ACP_LIST_SUPPORTED") {
            config.list_supported = value != "0" && !value.eq_ignore_ascii_case("false");
        }
        if let Ok(value) = std::env::var("FAKE_ACP_LOAD_SUPPORTED") {
            config.load_supported = value != "0" && !value.eq_ignore_ascii_case("false");
        }
        if let Ok(value) = std::env::var("FAKE_ACP_RESUME_SUPPORTED") {
            config.resume_supported = value != "0" && !value.eq_ignore_ascii_case("false");
        }
        config
    }
}

/// The client-side half of an in-process agent pipe pair.
pub type InProcessTransport = Transport<BufReader<PipeReader>, BufWriter<PipeWriter>>;

/// Spawns [`serve`] on a background thread and returns the client transport.
pub fn spawn_in_process_agent(
    config: FakeWireAgentConfig,
) -> std::io::Result<(InProcessTransport, JoinHandle<Result<(), WireError>>)> {
    let (client_read, agent_write) = std::io::pipe()?;
    let (agent_read, client_write) = std::io::pipe()?;
    let handle = std::thread::spawn(move || {
        serve(
            BufReader::new(agent_read),
            BufWriter::new(agent_write),
            config,
        )
    });
    let transport = Transport::new(BufReader::new(client_read), BufWriter::new(client_write));
    Ok((transport, handle))
}

/// Runs the fake agent until the input stream closes or a protocol error
/// occurs.
pub fn serve<R: BufRead, W: Write>(
    reader: R,
    writer: W,
    config: FakeWireAgentConfig,
) -> Result<(), WireError> {
    let mut transport = Transport::new(reader, writer);
    let mut next_agent_id: i64 = 1000;
    while let Some(line) = transport.recv_line()? {
        match classify(&line)? {
            RawMessage::Request { id, method, params } => {
                handle_request(
                    &mut transport,
                    &config,
                    &mut next_agent_id,
                    id,
                    &method,
                    params.unwrap_or(Value::Null),
                )?;
            }
            RawMessage::Notification { .. } => {
                // A standalone cancel has nothing to cancel in this fake agent.
            }
            RawMessage::Response { .. } => {
                // Late responses to permission requests are ignored.
            }
        }
    }
    Ok(())
}

fn handle_request<R: BufRead, W: Write>(
    transport: &mut Transport<R, W>,
    config: &FakeWireAgentConfig,
    next_agent_id: &mut i64,
    id: RequestId,
    method: &str,
    params: Value,
) -> Result<(), WireError> {
    if method == AGENT_METHOD_NAMES.initialize {
        let _request: InitializeRequest = serde_json::from_value(params)?;
        let response = InitializeResponse::new(ProtocolVersion::V1)
            .agent_info(Implementation::new(
                config.agent_name.clone(),
                config.agent_version.clone(),
            ))
            .agent_capabilities(agent_capabilities(config));
        transport.send_raw(&wire::encode_result(id, response)?)?;
    } else if method == AGENT_METHOD_NAMES.session_list {
        let _request: ListSessionsRequest = serde_json::from_value(params)?;
        let sessions = if config.list_supported {
            vec![SessionInfo::new("session-1", "/tmp/work")
                .title("Wire session")
                .updated_at("2026-01-01T00:00:00Z")]
        } else {
            Vec::new()
        };
        transport.send_raw(&wire::encode_result(
            id,
            ListSessionsResponse::new(sessions),
        )?)?;
    } else if method == AGENT_METHOD_NAMES.session_load {
        let request: LoadSessionRequest = serde_json::from_value(params)?;
        if !config.load_supported {
            transport.send_raw(&wire::encode_error(id, Error::method_not_found())?)?;
            return Ok(());
        }
        // Replay retained history as `session/update` notifications before the
        // load response resolves, so load history follows the live update path.
        send_update(
            transport,
            &request.session_id,
            SessionUpdate::UserMessageChunk(ContentChunk::new(ContentBlock::from("replayed user"))),
        )?;
        send_update(
            transport,
            &request.session_id,
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::from(
                "replayed assistant",
            ))),
        )?;
        let response = LoadSessionResponse::new().modes(session_mode_state());
        transport.send_raw(&wire::encode_result(id, response)?)?;
    } else if method == AGENT_METHOD_NAMES.session_resume {
        let _request: ResumeSessionRequest = serde_json::from_value(params)?;
        if !config.resume_supported {
            transport.send_raw(&wire::encode_error(id, Error::method_not_found())?)?;
            return Ok(());
        }
        let response = ResumeSessionResponse::new().modes(session_mode_state());
        transport.send_raw(&wire::encode_result(id, response)?)?;
    } else if method == AGENT_METHOD_NAMES.session_new {
        let _request: NewSessionRequest = serde_json::from_value(params)?;
        let response = NewSessionResponse::new(config.session_id.clone()).config_options(vec![
            SessionConfigOption::select(
                "model",
                "Model",
                "wire-model",
                vec![
                    SessionConfigSelectOption::new("wire-model", "Wire Model"),
                    SessionConfigSelectOption::new("wire-smart", "Wire Smart"),
                ],
            )
            .category(SessionConfigOptionCategory::Model),
            SessionConfigOption::boolean("brave_mode", "Brave Mode", false),
        ]);
        transport.send_raw(&wire::encode_result(id, response)?)?;
    } else if method == AGENT_METHOD_NAMES.session_prompt {
        let request: PromptRequest = serde_json::from_value(params)?;
        let stop_reason = handle_prompt(transport, config, next_agent_id, &request)?;
        transport.send_raw(&wire::encode_result(id, PromptResponse::new(stop_reason))?)?;
    } else {
        transport.send_raw(&wire::encode_error(id, Error::method_not_found())?)?;
    }
    Ok(())
}

fn handle_prompt<R: BufRead, W: Write>(
    transport: &mut Transport<R, W>,
    config: &FakeWireAgentConfig,
    next_agent_id: &mut i64,
    request: &PromptRequest,
) -> Result<StopReason, WireError> {
    let session = request.session_id.clone();
    let text = prompt_text(request);

    send_update(
        transport,
        &session,
        SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::from(format!(
            "echo: {text}"
        )))),
    )?;
    send_update(
        transport,
        &session,
        SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Image(
            ImageContent::new("aGVsbG8=", "image/png").uri("file:///tmp/rich.png"),
        ))),
    )?;
    send_update(
        transport,
        &session,
        SessionUpdate::ConfigOptionUpdate(ConfigOptionUpdate::new(vec![
            SessionConfigOption::boolean("brave_mode", "Brave Mode", true),
        ])),
    )?;
    send_update(
        transport,
        &session,
        SessionUpdate::SessionInfoUpdate(SessionInfoUpdate::new().title("Wire session")),
    )?;
    send_update(
        transport,
        &session,
        SessionUpdate::UsageUpdate(UsageUpdate::new(1_024, 200_000).cost(Cost::new(0.02, "USD"))),
    )?;
    send_update(
        transport,
        &session,
        SessionUpdate::ToolCall(
            ToolCall::new("tool-1", "edit_file")
                .kind(wire::ToolKind::Edit)
                .status(ToolCallStatus::InProgress)
                .content(vec![
                    ToolCallContent::Content(ToolContent::new(ContentBlock::Text(
                        TextContent::new("patching main.rs"),
                    ))),
                    ToolCallContent::Diff(
                        Diff::new("/tmp/work/src/main.rs", "fn main() { run() }")
                            .old_text("fn main() {}"),
                    ),
                    ToolCallContent::Terminal(Terminal::new("term-1")),
                ])
                .locations(vec![ToolCallLocation::new("/tmp/work/src/main.rs").line(12)])
                .raw_input(serde_json::json!({ "path": "/tmp/work/src/main.rs" }))
                .raw_output(serde_json::json!({ "exit": 0 })),
        ),
    )?;

    if !config.request_permission {
        send_update(
            transport,
            &session,
            SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
                "tool-1",
                ToolCallUpdateFields::new().status(ToolCallStatus::Completed),
            )),
        )?;
        return Ok(StopReason::EndTurn);
    }

    let permission_id = RequestId::Number(*next_agent_id);
    *next_agent_id += 1;
    let options = vec![
        PermissionOption::new("allow", "Allow", PermissionOptionKind::AllowOnce),
        PermissionOption::new("reject", "Reject", PermissionOptionKind::RejectOnce),
    ];
    let tool_call = ToolCallUpdate::new(
        "tool-1",
        ToolCallUpdateFields::new()
            .title("edit_file")
            .status(ToolCallStatus::InProgress),
    );
    let permission_request = RequestPermissionRequest::new(session.clone(), tool_call, options);
    transport.send_raw(&wire::encode_request(
        permission_id.clone(),
        CLIENT_METHOD_NAMES.session_request_permission,
        permission_request,
    )?)?;

    match await_permission(transport, config, &permission_id)? {
        PermissionResolution::Cancelled => {
            send_update(
                transport,
                &session,
                SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
                    "tool-1",
                    ToolCallUpdateFields::new().status(ToolCallStatus::Failed),
                )),
            )?;
            Ok(StopReason::Cancelled)
        }
        PermissionResolution::Selected(option_id) => {
            send_update(
                transport,
                &session,
                SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::from(format!(
                    "permission:{option_id}"
                )))),
            )?;
            send_update(
                transport,
                &session,
                SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
                    "tool-1",
                    ToolCallUpdateFields::new().status(ToolCallStatus::Completed),
                )),
            )?;
            Ok(StopReason::EndTurn)
        }
    }
}

enum PermissionResolution {
    Cancelled,
    Selected(String),
}

fn await_permission<R: BufRead, W: Write>(
    transport: &mut Transport<R, W>,
    config: &FakeWireAgentConfig,
    permission_id: &RequestId,
) -> Result<PermissionResolution, WireError> {
    loop {
        let Some(line) = transport.recv_line()? else {
            return Err(WireError::Malformed(
                "agent input closed while awaiting permission".to_string(),
            ));
        };
        match classify(&line)? {
            RawMessage::Response { id, outcome } if &id == permission_id => {
                let value = outcome.map_err(|error| {
                    WireError::Malformed(format!("permission request failed: {error}"))
                })?;
                let response: RequestPermissionResponse = serde_json::from_value(value)?;
                return Ok(match response.outcome {
                    RequestPermissionOutcome::Cancelled => PermissionResolution::Cancelled,
                    RequestPermissionOutcome::Selected(selected) => {
                        PermissionResolution::Selected(selected.option_id.0.to_string())
                    }
                    // Future outcome variants are treated as a refusal.
                    _ => PermissionResolution::Cancelled,
                });
            }
            RawMessage::Notification { method, .. }
                if method == AGENT_METHOD_NAMES.session_cancel && config.cancellation_supported =>
            {
                return Ok(PermissionResolution::Cancelled);
            }
            _ => continue,
        }
    }
}

fn agent_capabilities(config: &FakeWireAgentConfig) -> AgentCapabilities {
    let mut capabilities = AgentCapabilities::new().load_session(config.load_supported);
    let session = SessionCapabilities::new();
    let session = if config.list_supported {
        session.list(SessionListCapabilities::new())
    } else {
        session
    };
    let session = if config.resume_supported {
        session.resume(SessionResumeCapabilities::new())
    } else {
        session
    };
    capabilities = capabilities.session_capabilities(session);
    capabilities
}

fn session_mode_state() -> SessionModeState {
    SessionModeState::new(
        "auto",
        vec![
            SessionMode::new("none", "None"),
            SessionMode::new("auto", "Auto"),
        ],
    )
}

fn send_update<R: BufRead, W: Write>(
    transport: &mut Transport<R, W>,
    session: &SessionId,
    update: SessionUpdate,
) -> Result<(), WireError> {
    let notification = SessionNotification::new(session.clone(), update);
    transport.send_raw(&wire::encode_notification(
        CLIENT_METHOD_NAMES.session_update,
        notification,
    )?)
}

fn prompt_text(request: &PromptRequest) -> String {
    request
        .prompt
        .iter()
        .find_map(|block| match block {
            ContentBlock::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use acp_types::client::{AcpClient, AutoApproveHandler};
    use acp_types::wire::{ClientCapabilities, SessionId, StopReason};

    #[test]
    fn initialize_new_session_and_prompt_round_trip() {
        let (transport, handle) = spawn_in_process_agent(FakeWireAgentConfig::default()).unwrap();
        let mut client = AcpClient::new(transport, Box::new(AutoApproveHandler::new()));

        let init = client
            .initialize(
                ClientCapabilities::default(),
                Implementation::new("slight-test", "0.1.0"),
            )
            .unwrap();
        assert_eq!(init.protocol_version, ProtocolVersion::V1);
        assert!(client.is_initialized());

        let session = client.new_session("/tmp").unwrap();
        assert_eq!(session.session_id, SessionId::new("session-1"));

        let response = client.prompt("session-1", "hello").unwrap();
        assert_eq!(response.stop_reason, StopReason::EndTurn);
        assert!(client.sessions().contains(&SessionId::new("session-1")));
        assert_eq!(client.orphan_responses().len(), 0);

        drop(client);
        let result = handle.join().unwrap();
        assert!(result.is_ok());
    }

    #[test]
    fn lists_loads_and_resumes_sessions_over_the_wire() {
        use acp_types::wire::AgentToClientNotification;

        let (transport, handle) = spawn_in_process_agent(FakeWireAgentConfig::default()).unwrap();
        let mut client = AcpClient::new(transport, Box::new(AutoApproveHandler::new()));
        let init = client
            .initialize(
                ClientCapabilities::default(),
                Implementation::new("slight-test", "0.1.0"),
            )
            .unwrap();
        assert!(init.agent_capabilities.load_session);
        assert!(init.agent_capabilities.session_capabilities.list.is_some());
        assert!(init
            .agent_capabilities
            .session_capabilities
            .resume
            .is_some());

        let listed = client.list_sessions(ListSessionsRequest::new()).unwrap();
        assert_eq!(listed.sessions.len(), 1);
        assert_eq!(listed.sessions[0].session_id, SessionId::new("session-1"));

        let loaded = client.load_session("session-1", "/tmp/work").unwrap();
        assert_eq!(
            loaded.modes.as_ref().unwrap().current_mode_id.0.as_ref(),
            "auto"
        );

        let replay: Vec<&str> = client
            .notifications()
            .iter()
            .filter_map(|notification| match notification {
                AgentToClientNotification::SessionUpdate(update) => match &update.update {
                    SessionUpdate::UserMessageChunk(_) => Some("user"),
                    SessionUpdate::AgentMessageChunk(_) => Some("assistant"),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        assert_eq!(replay, vec!["user", "assistant"]);

        client.resume_session("session-1", "/tmp/work").unwrap();
        assert!(client.sessions().contains(&SessionId::new("session-1")));

        drop(client);
        let result = handle.join().unwrap();
        assert!(result.is_ok());
    }

    #[test]
    fn permission_selection_is_observed_by_the_agent() {
        use acp_types::wire::{AgentToClientNotification, SessionUpdate};

        let (transport, handle) = spawn_in_process_agent(FakeWireAgentConfig::default()).unwrap();
        let mut client =
            AcpClient::new(transport, Box::new(AutoApproveHandler::preferred("allow")));
        client
            .initialize(
                ClientCapabilities::default(),
                Implementation::new("slight-test", "0.1.0"),
            )
            .unwrap();
        client.new_session("/tmp").unwrap();
        let response = client.prompt("session-1", "run").unwrap();
        assert_eq!(response.stop_reason, StopReason::EndTurn);

        let saw_permission_echo = client.notifications().iter().any(|notification| {
            let AgentToClientNotification::SessionUpdate(notification) = notification else {
                return false;
            };
            match &notification.update {
                SessionUpdate::AgentMessageChunk(chunk) => match &chunk.content {
                    acp_types::wire::ContentBlock::Text(text) => text.text == "permission:allow",
                    _ => false,
                },
                _ => false,
            }
        });
        assert!(
            saw_permission_echo,
            "agent should echo the selected permission"
        );

        drop(client);
        let result = handle.join().unwrap();
        assert!(result.is_ok());
    }
}
