//! The host-side ACP client boundary.
//!
//! [`AcpClient`] drives one ACP agent over a [`Transport`]. It owns JSON-RPC
//! request id allocation and response correlation, and it services the
//! bidirectional half of the protocol: while the client is waiting for a
//! response, agent-to-client requests (`session/request_permission`) and
//! notifications (`session/update`) are still routed through a
//! [`ClientHandler`]. A request that a handler chooses to `Defer` can be
//! answered later with [`AcpClient::respond_permission`] or
//! [`AcpClient::respond_result`].
//!
//! This type is deliberately agent-neutral and knows nothing about Slight's
//! `gateway-v1` contract or about launching subprocesses. Launch and process
//! lifecycle live in `acp-adapters`.

use crate::transport::Transport;
use crate::wire::{
    self, AcpMessage, AgentCapabilities, AgentToClientNotification, AgentToClientRequest,
    BooleanConfigOptionCapabilities, ClientCapabilities, ClientSessionCapabilities, ContentBlock,
    Error, InitializeRequest, InitializeResponse, ListSessionsRequest, ListSessionsResponse,
    LoadSessionRequest, LoadSessionResponse, NewSessionRequest, NewSessionResponse,
    PermissionOption, PermissionOptionKind, PromptRequest, PromptResponse, ProtocolVersion,
    RequestId, RequestPermissionOutcome, RequestPermissionResponse, ResumeSessionRequest,
    ResumeSessionResponse, SelectedPermissionOutcome, SessionConfigOptionValue,
    SessionConfigOptionsCapabilities, SessionId, SetSessionConfigOptionRequest,
    SetSessionConfigOptionResponse, AGENT_METHOD_NAMES,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use thiserror::Error;

/// Errors surfaced by [`AcpClient`].
#[derive(Debug, Error)]
pub enum ClientError {
    #[error(transparent)]
    Wire(#[from] wire::WireError),
    #[error("agent returned JSON-RPC error {code}: {message}")]
    Protocol { code: i32, message: String },
    #[error("agent closed the connection")]
    Closed,
    #[error("client is not initialized")]
    NotInitialized,
    #[error("unknown session: {0}")]
    UnknownSession(String),
    #[error("agent does not advertise the {0} capability")]
    UnsupportedCapability(&'static str),
    #[error("unsupported protocol version: {0}")]
    UnsupportedProtocol(ProtocolVersion),
    #[error("no deferred request with id {0}")]
    UnknownDeferredRequest(String),
    #[error("failed to decode response: {0}")]
    Decode(#[from] serde_json::Error),
}

impl ClientError {
    fn from_protocol_error(error: Error) -> Self {
        ClientError::Protocol {
            code: i32::from(error.code),
            message: error.message,
        }
    }
}

/// The complete set of ACP client capabilities the host advertises on
/// `initialize`.
///
/// Slight services boolean session configuration options, which it surfaces to
/// clients as normalized config-option updates. It deliberately does not
/// advertise filesystem, terminal, or elicitation methods: those are not
/// implemented at the host boundary, so advertising them would invite agent
/// requests the host cannot answer.
pub fn host_client_capabilities() -> ClientCapabilities {
    ClientCapabilities::new().session(ClientSessionCapabilities::new().config_options(
        SessionConfigOptionsCapabilities::new().boolean(BooleanConfigOptionCapabilities::new()),
    ))
}

/// How a [`ClientHandler`] wants the client to answer an agent request.
#[derive(Debug)]
pub enum HandlerOutcome {
    /// Send this JSON-RPC result (or error) immediately.
    Respond(Result<Value, Error>),
    /// Do not answer yet. The caller must later call one of the `respond_*`
    /// methods with the request id it received.
    Defer,
}

/// Handles the agent-to-client half of an ACP connection.
pub trait ClientHandler: Send {
    /// Called for every agent-to-client request, including
    /// `session/request_permission` and unknown extension methods.
    fn handle_request(&mut self, id: RequestId, request: AgentToClientRequest) -> HandlerOutcome;

    /// Called for every agent-to-client notification. The client also records
    /// notifications so callers can drain them with
    /// [`AcpClient::take_notifications`].
    fn handle_notification(&mut self, notification: AgentToClientNotification);
}

/// Cancels every permission request and rejects unknown methods.
#[derive(Debug, Default)]
pub struct DenyAllHandler;

impl ClientHandler for DenyAllHandler {
    fn handle_request(&mut self, _id: RequestId, request: AgentToClientRequest) -> HandlerOutcome {
        match request {
            AgentToClientRequest::RequestPermission(_) => HandlerOutcome::Respond(Ok(
                permission_outcome_value(RequestPermissionOutcome::Cancelled),
            )),
            AgentToClientRequest::Other { .. } => {
                HandlerOutcome::Respond(Err(Error::method_not_found()))
            }
        }
    }

    fn handle_notification(&mut self, _notification: AgentToClientNotification) {}
}

/// Approves permission requests, preferring a configured option id.
#[derive(Debug, Clone, Default)]
pub struct AutoApproveHandler {
    preferred_option_id: Option<String>,
}

impl AutoApproveHandler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Prefers the option with this id when it is offered.
    pub fn preferred(option_id: impl Into<String>) -> Self {
        Self {
            preferred_option_id: Some(option_id.into()),
        }
    }

    fn select(&self, options: &[PermissionOption]) -> RequestPermissionOutcome {
        if let Some(preferred) = &self.preferred_option_id {
            if let Some(option) = options
                .iter()
                .find(|option| option.option_id.0.as_ref() == preferred.as_str())
            {
                return RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                    option.option_id.clone(),
                ));
            }
        }
        if let Some(option) = options.iter().find(|option| {
            matches!(
                option.kind,
                PermissionOptionKind::AllowOnce | PermissionOptionKind::AllowAlways
            )
        }) {
            return RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                option.option_id.clone(),
            ));
        }
        if let Some(option) = options.first() {
            return RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                option.option_id.clone(),
            ));
        }
        RequestPermissionOutcome::Cancelled
    }
}

impl ClientHandler for AutoApproveHandler {
    fn handle_request(&mut self, _id: RequestId, request: AgentToClientRequest) -> HandlerOutcome {
        match request {
            AgentToClientRequest::RequestPermission(request) => {
                let outcome = self.select(&request.options);
                HandlerOutcome::Respond(Ok(permission_outcome_value(outcome)))
            }
            AgentToClientRequest::Other { .. } => {
                HandlerOutcome::Respond(Err(Error::method_not_found()))
            }
        }
    }

    fn handle_notification(&mut self, _notification: AgentToClientNotification) {}
}

pub(crate) fn permission_outcome_value(outcome: RequestPermissionOutcome) -> Value {
    serde_json::to_value(RequestPermissionResponse::new(outcome))
        .expect("RequestPermissionResponse is serializable")
}

/// A synchronous, correlated ACP v1 client over any newline-delimited
/// [`Transport`].
pub struct AcpClient<R, W> {
    transport: Transport<R, W>,
    handler: Box<dyn ClientHandler>,
    next_id: i64,
    initialized: bool,
    agent_capabilities: Option<AgentCapabilities>,
    sessions: HashSet<SessionId>,
    notifications: Vec<AgentToClientNotification>,
    deferred: HashSet<RequestId>,
    orphan_responses: Vec<(RequestId, Result<Value, Error>)>,
}

impl<R: BufRead + Send, W: Write + Send> AcpClient<R, W> {
    pub fn new(transport: Transport<R, W>, handler: Box<dyn ClientHandler>) -> Self {
        Self {
            transport,
            handler,
            next_id: 0,
            initialized: false,
            agent_capabilities: None,
            sessions: HashSet::new(),
            notifications: Vec::new(),
            deferred: HashSet::new(),
            orphan_responses: Vec::new(),
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub fn sessions(&self) -> &HashSet<SessionId> {
        &self.sessions
    }

    /// The agent capabilities negotiated during `initialize`, if any.
    pub fn agent_capabilities(&self) -> Option<&AgentCapabilities> {
        self.agent_capabilities.as_ref()
    }

    pub fn notifications(&self) -> &[AgentToClientNotification] {
        &self.notifications
    }

    pub fn take_notifications(&mut self) -> Vec<AgentToClientNotification> {
        std::mem::take(&mut self.notifications)
    }

    pub fn deferred_requests(&self) -> &HashSet<RequestId> {
        &self.deferred
    }

    pub fn orphan_responses(&self) -> &[(RequestId, Result<Value, Error>)] {
        &self.orphan_responses
    }

    /// Consumes the client and returns its transport. Useful in tests and when
    /// handing the pipe half to a different owner.
    pub fn into_transport(self) -> Transport<R, W> {
        self.transport
    }

    /// Allocates the next request id.
    pub fn next_request_id(&mut self) -> RequestId {
        self.next_id += 1;
        RequestId::Number(self.next_id)
    }

    /// Sends a request and blocks until its matching response arrives,
    /// servicing inbound requests and notifications meanwhile.
    pub fn request<P, Resp>(&mut self, method: &str, params: P) -> Result<Resp, ClientError>
    where
        P: Serialize,
        Resp: DeserializeOwned,
    {
        let id = self.next_request_id();
        let frame = wire::encode_request(id.clone(), method, params)?;
        self.transport.send_raw(&frame)?;
        let value = self.wait_for_response(&id)?;
        Ok(serde_json::from_value(value)?)
    }

    /// Sends a notification. Notifications are fire-and-forget and are never
    /// correlated with a response.
    pub fn notify<P: Serialize>(&mut self, method: &str, params: P) -> Result<(), ClientError> {
        let frame = wire::encode_notification(method, params)?;
        self.transport.send_raw(&frame)?;
        Ok(())
    }

    /// Reads and processes exactly one frame.
    ///
    /// Agent-to-client requests are dispatched to the handler (and answered
    /// unless deferred); notifications are recorded. Responses are returned to
    /// the caller so it can correlate them.
    pub fn next_message(&mut self) -> Result<Option<AcpMessage>, ClientError> {
        let Some(message) = self.transport.recv_inbound()? else {
            return Ok(None);
        };
        match &message {
            AcpMessage::Request { id, request } => {
                match self.handler.handle_request(id.clone(), request.clone()) {
                    HandlerOutcome::Respond(Ok(value)) => {
                        let frame = wire::encode_result(id.clone(), value)?;
                        self.transport.send_raw(&frame)?;
                    }
                    HandlerOutcome::Respond(Err(error)) => {
                        let frame = wire::encode_error(id.clone(), error)?;
                        self.transport.send_raw(&frame)?;
                    }
                    HandlerOutcome::Defer => {
                        self.deferred.insert(id.clone());
                    }
                }
            }
            AcpMessage::Notification(notification) => {
                self.handler.handle_notification(notification.clone());
                self.notifications.push(notification.clone());
            }
            AcpMessage::Response { .. } => {}
        }
        Ok(Some(message))
    }

    /// Blocks until the response with `id` arrives, buffering any other
    /// responses as orphans.
    pub fn wait_for_response(&mut self, id: &RequestId) -> Result<Value, ClientError> {
        loop {
            match self.next_message()? {
                None => return Err(ClientError::Closed),
                Some(AcpMessage::Response { id: got, outcome }) if &got == id => {
                    return outcome.map_err(ClientError::from_protocol_error);
                }
                Some(AcpMessage::Response { id: got, outcome }) => {
                    self.orphan_responses.push((got, outcome));
                }
                Some(AcpMessage::Request { .. }) | Some(AcpMessage::Notification(_)) => {}
            }
        }
    }

    /// Performs `initialize` and records that the connection is ready.
    pub fn initialize(
        &mut self,
        client_capabilities: wire::ClientCapabilities,
        client_info: wire::Implementation,
    ) -> Result<InitializeResponse, ClientError> {
        let request = InitializeRequest::new(ProtocolVersion::V1)
            .client_capabilities(client_capabilities)
            .client_info(client_info);
        let response: InitializeResponse = self.request(AGENT_METHOD_NAMES.initialize, request)?;
        if response.protocol_version != ProtocolVersion::V1 {
            return Err(ClientError::UnsupportedProtocol(response.protocol_version));
        }
        self.initialized = true;
        self.agent_capabilities = Some(response.agent_capabilities.clone());
        Ok(response)
    }

    /// Creates a session with the agent and remembers its id.
    pub fn new_session(
        &mut self,
        cwd: impl Into<PathBuf>,
    ) -> Result<NewSessionResponse, ClientError> {
        self.ensure_initialized()?;
        let request = NewSessionRequest::new(cwd);
        let response: NewSessionResponse = self.request(AGENT_METHOD_NAMES.session_new, request)?;
        self.sessions.insert(response.session_id.clone());
        Ok(response)
    }

    /// Lists existing agent sessions, filtered by `cwd` when the request sets
    /// one. Requires the negotiated `sessionCapabilities.list` capability.
    pub fn list_sessions(
        &mut self,
        request: ListSessionsRequest,
    ) -> Result<ListSessionsResponse, ClientError> {
        self.ensure_initialized()?;
        self.ensure_list_supported()?;
        self.request(AGENT_METHOD_NAMES.session_list, request)
    }

    /// Loads an existing agent session, replaying its history as
    /// `session/update` notifications. Replay notifications are delivered to
    /// the [`ClientHandler`] while the response is awaited, then the session id
    /// is remembered so prompts can follow. Requires the negotiated top-level
    /// `loadSession` capability.
    pub fn load_session(
        &mut self,
        session_id: impl Into<SessionId>,
        cwd: impl Into<PathBuf>,
    ) -> Result<LoadSessionResponse, ClientError> {
        self.ensure_initialized()?;
        if !self.load_supported() {
            return Err(ClientError::UnsupportedCapability("session/load"));
        }
        let session_id = session_id.into();
        let request = LoadSessionRequest::new(session_id.clone(), cwd);
        let response: LoadSessionResponse =
            self.request(AGENT_METHOD_NAMES.session_load, request)?;
        self.sessions.insert(session_id);
        Ok(response)
    }

    /// Resumes an existing agent session without replaying its history.
    /// Requires the negotiated `sessionCapabilities.resume` capability.
    pub fn resume_session(
        &mut self,
        session_id: impl Into<SessionId>,
        cwd: impl Into<PathBuf>,
    ) -> Result<ResumeSessionResponse, ClientError> {
        self.ensure_initialized()?;
        if !self.resume_supported() {
            return Err(ClientError::UnsupportedCapability("session/resume"));
        }
        let session_id = session_id.into();
        let request = ResumeSessionRequest::new(session_id.clone(), cwd);
        let response: ResumeSessionResponse =
            self.request(AGENT_METHOD_NAMES.session_resume, request)?;
        self.sessions.insert(session_id);
        Ok(response)
    }

    /// Changes a selectable session configuration option, such as the model.
    pub fn set_config_option(
        &mut self,
        session_id: impl Into<SessionId>,
        config_id: impl Into<crate::wire::SessionConfigId>,
        value_id: impl Into<crate::wire::SessionConfigValueId>,
    ) -> Result<SetSessionConfigOptionResponse, ClientError> {
        self.ensure_initialized()?;
        let request = SetSessionConfigOptionRequest::new(
            session_id,
            config_id,
            SessionConfigOptionValue::value_id(value_id),
        );
        self.request(AGENT_METHOD_NAMES.session_set_config_option, request)
    }

    /// Sends a text prompt for a known session. The turn completes when the
    /// agent returns a stop reason; permission requests are serviced by the
    /// handler while waiting.
    pub fn prompt(
        &mut self,
        session_id: impl Into<SessionId>,
        text: impl Into<String>,
    ) -> Result<PromptResponse, ClientError> {
        self.ensure_initialized()?;
        let session_id = session_id.into();
        if !self.sessions.contains(&session_id) {
            return Err(ClientError::UnknownSession(session_id.to_string()));
        }
        let request = PromptRequest::new(session_id, vec![ContentBlock::from(text)]);
        self.request(AGENT_METHOD_NAMES.session_prompt, request)
    }

    /// Sends the `session/cancel` notification for an in-flight turn.
    pub fn cancel(&mut self, session_id: impl Into<SessionId>) -> Result<(), ClientError> {
        let frame = wire::cancel_notification(session_id.into())?;
        self.transport.send_raw(&frame)?;
        Ok(())
    }

    /// Answers a previously deferred `session/request_permission` request.
    pub fn respond_permission(
        &mut self,
        id: RequestId,
        outcome: RequestPermissionOutcome,
    ) -> Result<(), ClientError> {
        self.take_deferred(&id)?;
        let frame = wire::encode_result(id, RequestPermissionResponse::new(outcome))?;
        self.transport.send_raw(&frame)?;
        Ok(())
    }

    /// Answers a previously deferred request with an arbitrary JSON-RPC result.
    pub fn respond_result<Resp: Serialize>(
        &mut self,
        id: RequestId,
        result: Resp,
    ) -> Result<(), ClientError> {
        self.take_deferred(&id)?;
        let frame = wire::encode_result(id, result)?;
        self.transport.send_raw(&frame)?;
        Ok(())
    }

    /// Answers a previously deferred request with a JSON-RPC error.
    pub fn respond_error(&mut self, id: RequestId, error: Error) -> Result<(), ClientError> {
        self.take_deferred(&id)?;
        let frame = wire::encode_error(id, error)?;
        self.transport.send_raw(&frame)?;
        Ok(())
    }

    fn ensure_initialized(&self) -> Result<(), ClientError> {
        if self.initialized {
            Ok(())
        } else {
            Err(ClientError::NotInitialized)
        }
    }

    fn load_supported(&self) -> bool {
        self.agent_capabilities
            .as_ref()
            .is_some_and(|capabilities| capabilities.load_session)
    }

    fn resume_supported(&self) -> bool {
        self.agent_capabilities
            .as_ref()
            .is_some_and(|capabilities| capabilities.session_capabilities.resume.is_some())
    }

    fn ensure_list_supported(&self) -> Result<(), ClientError> {
        let supported = self
            .agent_capabilities
            .as_ref()
            .is_some_and(|capabilities| capabilities.session_capabilities.list.is_some());
        if supported {
            Ok(())
        } else {
            Err(ClientError::UnsupportedCapability("session/list"))
        }
    }

    fn take_deferred(&mut self, id: &RequestId) -> Result<(), ClientError> {
        if self.deferred.remove(id) {
            Ok(())
        } else {
            Err(ClientError::UnknownDeferredRequest(id.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{Implementation, StopReason};
    use std::io::{BufReader, Cursor};

    fn client_with_frames(script: &str) -> AcpClient<BufReader<Cursor<Vec<u8>>>, Vec<u8>> {
        let transport = Transport::new(
            BufReader::new(Cursor::new(script.as_bytes().to_vec())),
            Vec::new(),
        );
        AcpClient::new(transport, Box::new(AutoApproveHandler::new()))
    }

    fn writer_text(client: AcpClient<BufReader<Cursor<Vec<u8>>>, Vec<u8>>) -> String {
        let (_, writer) = client.into_transport().split();
        String::from_utf8(writer.into_inner()).unwrap()
    }

    #[test]
    fn request_correlates_response_by_id() {
        let script = "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"stopReason\":\"end_turn\"}}\n";
        let mut client = client_with_frames(script);
        let response: PromptResponse = client
            .request(
                AGENT_METHOD_NAMES.session_prompt,
                PromptRequest::new("s-1", Vec::<ContentBlock>::new()),
            )
            .unwrap();
        assert_eq!(response.stop_reason, StopReason::EndTurn);
    }

    #[test]
    fn services_permission_request_while_waiting() {
        let script = concat!(
            "{\"jsonrpc\":\"2.0\",\"id\":42,\"method\":\"session/request_permission\",\"params\":{\"sessionId\":\"s-1\",\"toolCall\":{\"toolCallId\":\"t-1\",\"status\":\"pending\"},\"options\":[{\"optionId\":\"allow\",\"name\":\"Allow\",\"kind\":\"allow_once\"}]}}\n",
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"stopReason\":\"end_turn\"}}\n",
        );
        let mut client = client_with_frames(script);
        let response: PromptResponse = client
            .request(
                AGENT_METHOD_NAMES.session_prompt,
                PromptRequest::new("s-1", Vec::<ContentBlock>::new()),
            )
            .unwrap();
        assert_eq!(response.stop_reason, StopReason::EndTurn);
        let sent = writer_text(client);
        // The client sent its prompt (id 1) and answered the permission (id 42).
        assert!(sent.contains(r#""id":1"#));
        assert!(sent.contains(r#""id":42"#));
        assert!(sent.contains(r#""outcome":"selected""#));
        assert!(sent.contains(r#""optionId":"allow""#));
    }

    #[test]
    fn surfaces_jsonrpc_error_as_protocol_error() {
        let script =
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"error\":{\"code\":-32601,\"message\":\"nope\"}}\n";
        let mut client = client_with_frames(script);
        let error = client
            .request::<_, PromptResponse>(
                AGENT_METHOD_NAMES.session_prompt,
                PromptRequest::new("s-1", Vec::<ContentBlock>::new()),
            )
            .unwrap_err();
        match error {
            ClientError::Protocol { code, message } => {
                assert_eq!(code, -32601);
                assert_eq!(message, "nope");
            }
            other => panic!("expected protocol error, got {other}"),
        }
    }

    #[test]
    fn closed_transport_is_reported() {
        let mut client = client_with_frames("");
        let error = client
            .request::<_, PromptResponse>(
                AGENT_METHOD_NAMES.session_prompt,
                PromptRequest::new("s-1", Vec::<ContentBlock>::new()),
            )
            .unwrap_err();
        assert!(matches!(error, ClientError::Closed));
    }

    #[test]
    fn typed_lifecycle_requires_initialize_then_known_session() {
        let mut client = client_with_frames("");
        assert!(matches!(
            client.new_session("/tmp").unwrap_err(),
            ClientError::NotInitialized
        ));
    }

    #[test]
    fn load_is_rejected_when_capability_is_not_advertised() {
        let script = "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1}}\n";
        let mut client = client_with_frames(script);
        client
            .initialize(
                ClientCapabilities::default(),
                Implementation::new("slight-test", "0.1.0"),
            )
            .unwrap();
        let error = client.load_session("s-1", "/tmp").unwrap_err();
        assert!(matches!(
            error,
            ClientError::UnsupportedCapability("session/load")
        ));
    }

    #[test]
    fn negotiated_capabilities_gate_and_remember_loaded_sessions() {
        let script = concat!(
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{\"loadSession\":true,\"sessionCapabilities\":{\"list\":{},\"resume\":{}}}}}\n",
            "{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"modes\":{\"currentModeId\":\"auto\",\"availableModes\":[{\"id\":\"auto\",\"name\":\"Auto\"}]}}}\n",
            "{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"sessions\":[{\"sessionId\":\"s-1\",\"cwd\":\"/tmp\",\"title\":\"Prior\"}]}}\n",
            "{\"jsonrpc\":\"2.0\",\"id\":4,\"result\":{}}\n",
        );
        let mut client = client_with_frames(script);
        client
            .initialize(
                ClientCapabilities::default(),
                Implementation::new("slight-test", "0.1.0"),
            )
            .unwrap();

        let loaded = client.load_session("s-1", "/tmp").unwrap();
        assert_eq!(
            loaded.modes.as_ref().unwrap().current_mode_id.0.as_ref(),
            "auto"
        );
        assert!(client.sessions().contains(&SessionId::new("s-1")));

        let listed = client.list_sessions(ListSessionsRequest::new()).unwrap();
        assert_eq!(listed.sessions.len(), 1);
        assert_eq!(listed.sessions[0].title.as_deref(), Some("Prior"));

        client.resume_session("s-2", "/tmp").unwrap();
        assert!(client.sessions().contains(&SessionId::new("s-2")));
    }

    #[test]
    fn list_is_rejected_when_capability_is_not_advertised() {
        let script = "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1}}\n";
        let mut client = client_with_frames(script);
        client
            .initialize(
                ClientCapabilities::default(),
                Implementation::new("slight-test", "0.1.0"),
            )
            .unwrap();
        let error = client
            .list_sessions(ListSessionsRequest::new())
            .unwrap_err();
        assert!(matches!(
            error,
            ClientError::UnsupportedCapability("session/list")
        ));
    }

    #[test]
    fn host_capabilities_advertise_boolean_config_options_only() {
        let value = serde_json::to_value(host_client_capabilities()).unwrap();
        assert_eq!(
            value["session"]["configOptions"]["boolean"],
            serde_json::json!({})
        );
        assert_eq!(value["terminal"], serde_json::json!(false));
        assert_eq!(value["fs"]["readTextFile"], serde_json::json!(false));
        assert_eq!(value["fs"]["writeTextFile"], serde_json::json!(false));
    }
}
