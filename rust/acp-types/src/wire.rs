//! Conformant Agent Client Protocol (ACP) v1 wire codec.
//!
//! This module is intentionally separate from the Slight `gateway-v1` contract:
//! it models the JSON-RPC 2.0 messages exchanged directly between the host (an
//! ACP *client*) and an ACP *agent* subprocess. Gateway frames never appear
//! here, and ACP envelopes never leak into `gateway-protocol`.
//!
//! The domain types come from the official
//! [`agent-client-protocol-schema`](https://crates.io/crates/agent-client-protocol-schema)
//! crate so that method names, discriminators, and payload shapes stay
//! conformant instead of being a bespoke re-implementation.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

pub use agent_client_protocol_schema::v1::{
    AgentCapabilities, AudioContent, AuthMethod, AuthMethodAgent, AuthMethodId, AuthMethodTerminal,
    AvailableCommand, AvailableCommandInput, AvailableCommandsUpdate, BlobResourceContents,
    BooleanConfigOptionCapabilities, CancelNotification, CancelRequestNotification,
    ClientCapabilities, ClientSessionCapabilities, ConfigOptionUpdate, Content as ToolContent,
    ContentBlock, ContentChunk, Cost, CurrentModeUpdate, Diff, EmbeddedResource,
    EmbeddedResourceResource, Error, ErrorCode, ImageContent, Implementation, InitializeRequest,
    InitializeResponse, JsonRpcMessage, ListSessionsRequest, ListSessionsResponse,
    LoadSessionRequest, LoadSessionResponse, McpCapabilities, NewSessionRequest,
    NewSessionResponse, Notification, PermissionOption, PermissionOptionKind, Plan, PlanEntry,
    PlanEntryPriority, PlanEntryStatus, PromptCapabilities, PromptRequest, PromptResponse, Request,
    RequestId, RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse,
    ResourceLink, Response, ResumeSessionRequest, ResumeSessionResponse, SelectedPermissionOutcome,
    SessionCapabilities, SessionConfigGroupId, SessionConfigId, SessionConfigKind,
    SessionConfigOption, SessionConfigOptionCategory, SessionConfigOptionValue,
    SessionConfigOptionsCapabilities, SessionConfigSelectGroup, SessionConfigSelectOption,
    SessionConfigSelectOptions, SessionConfigValueId, SessionId, SessionInfo, SessionInfoUpdate,
    SessionListCapabilities, SessionMode, SessionModeId, SessionModeState, SessionNotification,
    SessionResumeCapabilities, SessionUpdate, SetSessionConfigOptionRequest,
    SetSessionConfigOptionResponse, SetSessionModeRequest, SetSessionModeResponse, StopReason,
    Terminal, TextContent, TextResourceContents, ToolCall, ToolCallContent, ToolCallLocation,
    ToolCallStatus, ToolCallUpdate, ToolCallUpdateFields, ToolKind, UnstructuredCommandInput,
    UsageUpdate, AGENT_METHOD_NAMES, CLIENT_METHOD_NAMES, PROTOCOL_LEVEL_METHOD_NAMES,
};
pub use agent_client_protocol_schema::ProtocolVersion;

/// The JSON-RPC version literal carried on every ACP envelope.
pub const JSONRPC_VERSION: &str = "2.0";

/// Errors produced while encoding or decoding ACP wire frames.
#[derive(Debug, Error)]
pub enum WireError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("malformed ACP frame: {0}")]
    Malformed(String),
}

/// Encodes a value as a single newline-terminated JSON frame.
pub fn encode_line<T: Serialize>(message: &T) -> Result<Vec<u8>, WireError> {
    let mut bytes = serde_json::to_vec(message)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Decodes a single JSON frame (without its trailing newline).
pub fn decode_line<T: DeserializeOwned>(line: &str) -> Result<T, WireError> {
    Ok(serde_json::from_str(line)?)
}

/// A structurally-classified JSON-RPC 2.0 frame, before direction-specific
/// routing. Both the host and the fake agent use this to avoid duplicating
/// envelope handling.
#[derive(Debug, Clone)]
pub enum RawMessage {
    Request {
        id: RequestId,
        method: String,
        params: Option<Value>,
    },
    Notification {
        method: String,
        params: Option<Value>,
    },
    Response {
        id: RequestId,
        outcome: Result<Value, Error>,
    },
}

/// Classifies one newline-delimited JSON-RPC 2.0 frame.
///
/// The `jsonrpc` field is required, as mandated by JSON-RPC 2.0 and ACP.
pub fn classify(line: &str) -> Result<RawMessage, WireError> {
    let value: Value = serde_json::from_str(line)?;
    let object = value
        .as_object()
        .ok_or_else(|| WireError::Malformed("frame is not a JSON object".to_string()))?;

    match object.get("jsonrpc").and_then(Value::as_str) {
        Some(JSONRPC_VERSION) => {}
        Some(other) => {
            return Err(WireError::Malformed(format!(
                "unsupported jsonrpc version: {other}"
            )))
        }
        None => {
            return Err(WireError::Malformed(
                "missing required jsonrpc field".to_string(),
            ))
        }
    }

    if let Some(method) = object.get("method").and_then(Value::as_str) {
        let params = object.get("params").cloned();
        if let Some(id) = object.get("id").filter(|id| !id.is_null()) {
            return Ok(RawMessage::Request {
                id: parse_request_id(id)?,
                method: method.to_string(),
                params,
            });
        }
        return Ok(RawMessage::Notification {
            method: method.to_string(),
            params,
        });
    }

    if let Some(id) = object.get("id") {
        let id = parse_request_id(id)?;
        if let Some(error) = object.get("error") {
            let error: Error = serde_json::from_value(error.clone())?;
            return Ok(RawMessage::Response {
                id,
                outcome: Err(error),
            });
        }
        let result = object.get("result").cloned().unwrap_or(Value::Null);
        return Ok(RawMessage::Response {
            id,
            outcome: Ok(result),
        });
    }

    Err(WireError::Malformed(
        "frame is neither a request, response, nor notification".to_string(),
    ))
}

fn parse_request_id(value: &Value) -> Result<RequestId, WireError> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .map(RequestId::Number)
            .ok_or_else(|| WireError::Malformed("request id is not an integer".to_string())),
        Value::String(text) => Ok(RequestId::Str(text.clone())),
        Value::Null => Ok(RequestId::Null),
        _ => Err(WireError::Malformed(
            "request id must be a string, integer, or null".to_string(),
        )),
    }
}

/// A request the agent sends to the host and that expects a response.
#[derive(Debug, Clone)]
pub enum AgentToClientRequest {
    /// `session/request_permission`: the envelope request id is the permission
    /// request identity; the host answers with [`RequestPermissionResponse`].
    RequestPermission(RequestPermissionRequest),
    /// A request this slice does not implement. The host answers it with a
    /// JSON-RPC `method_not_found` error rather than dropping it.
    Other {
        method: String,
        params: Option<Value>,
    },
}

/// A notification the agent sends to the host.
#[derive(Debug, Clone)]
pub enum AgentToClientNotification {
    /// `session/update`: streaming message, tool-call, plan, and mode updates.
    SessionUpdate(SessionNotification),
    /// `$/cancel_request`: protocol-level request cancellation.
    CancelRequest(CancelRequestNotification),
    /// A notification this slice does not model.
    Other {
        method: String,
        params: Option<Value>,
    },
}

/// A typed message as seen by the host (the ACP client).
#[derive(Debug, Clone)]
pub enum AcpMessage {
    Request {
        id: RequestId,
        request: AgentToClientRequest,
    },
    Notification(AgentToClientNotification),
    Response {
        id: RequestId,
        outcome: Result<Value, Error>,
    },
}

/// Classifies and routes one frame destined for the host.
pub fn classify_inbound(line: &str) -> Result<AcpMessage, WireError> {
    match classify(line)? {
        RawMessage::Response { id, outcome } => Ok(AcpMessage::Response { id, outcome }),
        RawMessage::Notification { method, params } => Ok(AcpMessage::Notification(
            route_notification(&method, params)?,
        )),
        RawMessage::Request { id, method, params } => Ok(AcpMessage::Request {
            id,
            request: route_request(&method, params)?,
        }),
    }
}

fn route_request(method: &str, params: Option<Value>) -> Result<AgentToClientRequest, WireError> {
    if method == CLIENT_METHOD_NAMES.session_request_permission {
        Ok(AgentToClientRequest::RequestPermission(parse_params(
            params, method,
        )?))
    } else {
        Ok(AgentToClientRequest::Other {
            method: method.to_string(),
            params,
        })
    }
}

fn route_notification(
    method: &str,
    params: Option<Value>,
) -> Result<AgentToClientNotification, WireError> {
    if method == CLIENT_METHOD_NAMES.session_update {
        Ok(AgentToClientNotification::SessionUpdate(parse_params(
            params, method,
        )?))
    } else if method == PROTOCOL_LEVEL_METHOD_NAMES.cancel_request {
        Ok(AgentToClientNotification::CancelRequest(parse_params(
            params, method,
        )?))
    } else {
        Ok(AgentToClientNotification::Other {
            method: method.to_string(),
            params,
        })
    }
}

fn parse_params<T: DeserializeOwned>(params: Option<Value>, method: &str) -> Result<T, WireError> {
    let value = params.unwrap_or(Value::Null);
    serde_json::from_value(value)
        .map_err(|error| WireError::Malformed(format!("invalid params for {method}: {error}")))
}

// Outbound encoders. These always emit `"jsonrpc": "2.0"` via `JsonRpcMessage`.

/// Encodes a JSON-RPC request.
pub fn encode_request<P: Serialize>(
    id: RequestId,
    method: &str,
    params: P,
) -> Result<Vec<u8>, WireError> {
    let message = JsonRpcMessage::wrap(Request {
        id,
        method: method.into(),
        params: Some(params),
    });
    encode_line(&message)
}

/// Encodes a JSON-RPC notification.
pub fn encode_notification<P: Serialize>(method: &str, params: P) -> Result<Vec<u8>, WireError> {
    let message = JsonRpcMessage::wrap(Notification {
        method: method.into(),
        params: Some(params),
    });
    encode_line(&message)
}

/// Encodes a successful JSON-RPC response.
pub fn encode_result<R: Serialize>(id: RequestId, result: R) -> Result<Vec<u8>, WireError> {
    let message = JsonRpcMessage::wrap(Response::Result { id, result });
    encode_line(&message)
}

/// Encodes a JSON-RPC error response.
pub fn encode_error(id: RequestId, error: Error) -> Result<Vec<u8>, WireError> {
    let message: JsonRpcMessage<Response<Value>> =
        JsonRpcMessage::wrap(Response::Error { id, error });
    encode_line(&message)
}

/// Builds and encodes an ACP `initialize` request.
pub fn initialize_request(
    id: RequestId,
    client_capabilities: ClientCapabilities,
    client_info: Implementation,
) -> Result<Vec<u8>, WireError> {
    let request = InitializeRequest::new(ProtocolVersion::V1)
        .client_capabilities(client_capabilities)
        .client_info(client_info);
    encode_request(id, AGENT_METHOD_NAMES.initialize, request)
}

/// Builds and encodes an ACP `session/new` request.
pub fn new_session_request(
    id: RequestId,
    cwd: impl Into<std::path::PathBuf>,
) -> Result<Vec<u8>, WireError> {
    encode_request(
        id,
        AGENT_METHOD_NAMES.session_new,
        NewSessionRequest::new(cwd),
    )
}

/// Builds and encodes an ACP `session/load` request.
pub fn load_session_request(
    id: RequestId,
    session_id: impl Into<SessionId>,
    cwd: impl Into<std::path::PathBuf>,
) -> Result<Vec<u8>, WireError> {
    encode_request(
        id,
        AGENT_METHOD_NAMES.session_load,
        LoadSessionRequest::new(session_id, cwd),
    )
}

/// Builds and encodes an ACP `session/resume` request.
pub fn resume_session_request(
    id: RequestId,
    session_id: impl Into<SessionId>,
    cwd: impl Into<std::path::PathBuf>,
) -> Result<Vec<u8>, WireError> {
    encode_request(
        id,
        AGENT_METHOD_NAMES.session_resume,
        ResumeSessionRequest::new(session_id, cwd),
    )
}

/// Builds and encodes an ACP `session/list` request.
pub fn list_sessions_request(
    id: RequestId,
    request: ListSessionsRequest,
) -> Result<Vec<u8>, WireError> {
    encode_request(id, AGENT_METHOD_NAMES.session_list, request)
}

/// Builds and encodes an ACP `session/prompt` request.
pub fn prompt_request(
    id: RequestId,
    session_id: impl Into<SessionId>,
    text: impl Into<String>,
) -> Result<Vec<u8>, WireError> {
    let request = PromptRequest::new(session_id, vec![ContentBlock::from(text)]);
    encode_request(id, AGENT_METHOD_NAMES.session_prompt, request)
}

/// Builds and encodes an ACP `session/cancel` notification.
pub fn cancel_notification(session_id: impl Into<SessionId>) -> Result<Vec<u8>, WireError> {
    encode_notification(
        AGENT_METHOD_NAMES.session_cancel,
        CancelNotification::new(session_id),
    )
}

/// Builds and encodes a `session/request_permission` response selecting an
/// option.
pub fn permission_selected(
    id: RequestId,
    option_id: impl Into<agent_client_protocol_schema::v1::PermissionOptionId>,
) -> Result<Vec<u8>, WireError> {
    let response = RequestPermissionResponse::new(RequestPermissionOutcome::Selected(
        SelectedPermissionOutcome::new(option_id),
    ));
    encode_result(id, response)
}

/// Builds and encodes a `session/request_permission` response cancelling the
/// request.
pub fn permission_cancelled(id: RequestId) -> Result<Vec<u8>, WireError> {
    let response = RequestPermissionResponse::new(RequestPermissionOutcome::Cancelled);
    encode_result(id, response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request_id(id: i64) -> RequestId {
        RequestId::Number(id)
    }

    #[test]
    fn request_round_trips_with_jsonrpc_field() {
        let bytes = encode_request(
            request_id(7),
            AGENT_METHOD_NAMES.session_prompt,
            PromptRequest::new("s-1", vec![ContentBlock::from("hi")]),
        )
        .unwrap();
        let line = std::str::from_utf8(&bytes).unwrap().trim_end();
        assert!(line.starts_with(r#"{"jsonrpc":"2.0""#));
        let decoded = classify(line).unwrap();
        match decoded {
            RawMessage::Request { id, method, params } => {
                assert_eq!(id, request_id(7));
                assert_eq!(method, "session/prompt");
                let params: PromptRequest = serde_json::from_value(params.unwrap()).unwrap();
                assert_eq!(params.session_id, SessionId::new("s-1"));
            }
            other => panic!("expected request, got {other:?}"),
        }
    }

    #[test]
    fn notification_round_trips() {
        let bytes = cancel_notification("s-2").unwrap();
        let line = std::str::from_utf8(&bytes).unwrap().trim_end();
        let decoded = classify(line).unwrap();
        match decoded {
            RawMessage::Notification { method, params } => {
                assert_eq!(method, "session/cancel");
                let params: CancelNotification = serde_json::from_value(params.unwrap()).unwrap();
                assert_eq!(params.session_id, SessionId::new("s-2"));
            }
            other => panic!("expected notification, got {other:?}"),
        }
    }

    #[test]
    fn classifies_session_update_as_typed_notification() {
        let line = json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": "s-1",
                "update": {
                    "sessionUpdate": "agent_message_chunk",
                    "content": { "type": "text", "text": "hello" }
                }
            }
        })
        .to_string();
        let message = classify_inbound(&line).unwrap();
        match message {
            AcpMessage::Notification(AgentToClientNotification::SessionUpdate(notification)) => {
                match notification.update {
                    SessionUpdate::AgentMessageChunk(chunk) => match chunk.content {
                        ContentBlock::Text(text) => assert_eq!(text.text, "hello"),
                        other => panic!("expected text, got {other:?}"),
                    },
                    other => panic!("expected agent message chunk, got {other:?}"),
                }
            }
            other => panic!("expected session update, got {other:?}"),
        }
    }

    #[test]
    fn classifies_permission_request_with_envelope_id() {
        let line = json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "session/request_permission",
            "params": {
                "sessionId": "s-1",
                "toolCall": { "toolCallId": "tool-1", "title": "edit" },
                "options": [
                    { "optionId": "allow", "name": "Allow", "kind": "allow_once" }
                ]
            }
        })
        .to_string();
        match classify_inbound(&line).unwrap() {
            AcpMessage::Request { id, request } => {
                assert_eq!(id, request_id(42));
                match request {
                    AgentToClientRequest::RequestPermission(permission) => {
                        assert_eq!(permission.tool_call.tool_call_id.to_string(), "tool-1");
                    }
                    other => panic!("expected permission request, got {other:?}"),
                }
            }
            other => panic!("expected request, got {other:?}"),
        }
    }

    #[test]
    fn classifies_error_response() {
        let line = json!({
            "jsonrpc": "2.0",
            "id": "req-1",
            "error": { "code": -32601, "message": "method not found" }
        })
        .to_string();
        match classify_inbound(&line).unwrap() {
            AcpMessage::Response { id, outcome } => {
                assert_eq!(id, RequestId::Str("req-1".to_string()));
                assert!(outcome.is_err());
            }
            other => panic!("expected response, got {other:?}"),
        }
    }

    #[test]
    fn rejects_missing_jsonrpc_field() {
        let error = classify(r#"{"id":1,"method":"initialize"}"#).unwrap_err();
        assert!(matches!(error, WireError::Malformed(_)));
    }

    #[test]
    fn rejects_unknown_method_without_dropping_it() {
        let line = json!({
            "jsonrpc": "2.0",
            "method": "custom/thing",
            "params": { "x": 1 }
        })
        .to_string();
        match classify_inbound(&line).unwrap() {
            AcpMessage::Notification(AgentToClientNotification::Other { method, .. }) => {
                assert_eq!(method, "custom/thing");
            }
            other => panic!("expected other notification, got {other:?}"),
        }
    }

    #[test]
    fn permission_selected_encodes_outcome_shape() {
        let bytes = permission_selected(request_id(3), "allow").unwrap();
        let value: Value =
            serde_json::from_slice(std::str::from_utf8(&bytes).unwrap().as_bytes()).unwrap();
        assert_eq!(value["result"]["outcome"]["outcome"], "selected");
        assert_eq!(value["result"]["outcome"]["optionId"], "allow");
    }
}
