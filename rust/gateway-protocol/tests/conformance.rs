use gateway_protocol::{
    decode_line, encode_line, AgentSessionsListParams, AgentSessionsListResult,
    CreateSessionParams, Frame, GatewayAck, ImportAgentSessionParams, ImportAgentSessionResult,
    SessionCreateResult, SessionInspectResult, SessionListResult, SessionRecovery,
    SessionRecoveryState, SessionStatus, PROTOCOL_VERSION,
};

const HELLO: &str = include_str!("../../../protocol/conformance/hello.json");
const SESSION_CREATE: &str =
    include_str!("../../../protocol/conformance/session-create-command.json");
const ACK_OK: &str = include_str!("../../../protocol/conformance/ack-ok.json");
const SESSION_LIST_RECOVERY: &str =
    include_str!("../../../protocol/conformance/session-list-recovery.json");
const SESSION_INSPECT_ACP: &str =
    include_str!("../../../protocol/conformance/session-inspect-acp.json");
const SESSION_TOOL_CALL_RICH: &str =
    include_str!("../../../protocol/conformance/session-tool-call-rich.json");
const MALFORMED_UNKNOWN_TYPE: &str =
    include_str!("../../../protocol/conformance/malformed-unknown-type.json");
const AGENT_SESSIONS_LIST_COMMAND: &str =
    include_str!("../../../protocol/conformance/agent-sessions-list-command.json");
const AGENT_SESSION_IMPORT_COMMAND: &str =
    include_str!("../../../protocol/conformance/agent-session-import-command.json");
const AGENT_SESSIONS_LIST_ACK: &str =
    include_str!("../../../protocol/conformance/agent-sessions-list-ack.json");
const AGENT_SESSION_IMPORT_ACK: &str =
    include_str!("../../../protocol/conformance/agent-session-import-ack.json");
const AGENT_SESSION_IMPORT_UNSUPPORTED: &str =
    include_str!("../../../protocol/conformance/agent-session-import-unsupported.json");

#[test]
fn decodes_hello_fixture() {
    let frame: Frame = decode_line(HELLO).expect("hello decodes");
    match frame {
        Frame::Hello(hello) => {
            assert_eq!(hello.protocol_version, PROTOCOL_VERSION);
            assert_eq!(hello.client_id, "conformance-client");
            assert_eq!(hello.credential.as_deref(), Some("dev"));
            let resume = hello.resume.expect("resume present");
            assert_eq!(resume.session_id.as_deref(), Some("s-1"));
            assert_eq!(resume.last_event_sequence, Some(3));
        }
        other => panic!("expected hello, got {other:?}"),
    }
}

#[test]
fn decodes_command_and_typed_params() {
    let frame: Frame = decode_line(SESSION_CREATE).expect("command decodes");
    match frame {
        Frame::Command(command) => {
            assert_eq!(command.request_id, "req-1");
            assert_eq!(command.command, "session.create");
            let params: CreateSessionParams = command.params_as().expect("params decode");
            assert_eq!(params.agent, "fake");
            assert_eq!(params.initial_prompt.as_deref(), Some("hello"));
        }
        other => panic!("expected command, got {other:?}"),
    }
}

#[test]
fn decodes_ack_result() {
    let frame: Frame = decode_line(ACK_OK).expect("ack decodes");
    match frame {
        Frame::Ack(ack) => {
            assert!(ack.ok);
            let result: SessionCreateResult = ack.result_as().expect("result decodes");
            assert_eq!(result.session.id, "s-1");
            assert_eq!(result.session.status, SessionStatus::Idle);
            assert_eq!(result.session.last_sequence, 0);
            // Older fixtures omit `recovery`; clients must default it to live.
            assert_eq!(result.session.recovery, SessionRecoveryState::Live);
        }
        other => panic!("expected ack, got {other:?}"),
    }
}

#[test]
fn decodes_session_list_recovery_states() {
    let frame: Frame = decode_line(SESSION_LIST_RECOVERY).expect("list ack decodes");
    let Frame::Ack(ack) = frame else {
        panic!("expected ack");
    };
    assert!(ack.ok);
    let result: SessionListResult = ack.result_as().expect("list result decodes");
    assert_eq!(result.sessions.len(), 4);
    assert_eq!(result.sessions[0].recovery, SessionRecoveryState::Live);
    assert_eq!(result.sessions[1].recovery, SessionRecoveryState::Recovered);
    assert!(matches!(
        &result.sessions[2].recovery,
        SessionRecoveryState::Stale { reason } if reason.contains("no longer available")
    ));
    assert!(matches!(
        &result.sessions[3].recovery,
        SessionRecoveryState::Unavailable { reason } if reason.contains("not installed")
    ));
}

#[test]
fn decodes_agent_sessions_list_command() {
    let frame: Frame = decode_line(AGENT_SESSIONS_LIST_COMMAND).expect("discovery command decodes");
    let Frame::Command(command) = frame else {
        panic!("expected command");
    };
    assert_eq!(command.command, "agent.sessions.list");
    let params: AgentSessionsListParams = command.params_as().expect("params decode");
    assert_eq!(params.agent, "fake");
    assert_eq!(
        params.working_directory_label.as_deref(),
        Some("~/work/slight")
    );
    assert_eq!(params.cursor.as_deref(), Some("cursor-1"));
}

#[test]
fn decodes_agent_session_import_command() {
    let frame: Frame = decode_line(AGENT_SESSION_IMPORT_COMMAND).expect("import command decodes");
    let Frame::Command(command) = frame else {
        panic!("expected command");
    };
    assert_eq!(command.command, "agent.sessions.import");
    let params: ImportAgentSessionParams = command.params_as().expect("params decode");
    assert_eq!(params.agent, "fake");
    assert_eq!(params.agent_session_id, "fake-listed-1");
    assert_eq!(params.working_directory_label, "~/work/slight");
    assert_eq!(params.recovery, SessionRecovery::Load);
    assert_eq!(params.title.as_deref(), Some("Imported fake session"));
}

#[test]
fn decodes_agent_sessions_list_ack() {
    let frame: Frame = decode_line(AGENT_SESSIONS_LIST_ACK).expect("discovery ack decodes");
    let Frame::Ack(ack) = frame else {
        panic!("expected ack");
    };
    assert!(ack.ok);
    let result: AgentSessionsListResult = ack.result_as().expect("discovery result decodes");
    assert_eq!(result.sessions.len(), 2);
    assert_eq!(result.next_cursor.as_deref(), Some("cursor-2"));
    assert_eq!(result.sessions[0].agent, "fake");
    assert_eq!(result.sessions[0].agent_session_id, "fake-listed-1");
    assert_eq!(result.sessions[0].cwd, "/tmp/slight/fake-project");
    assert_eq!(
        result.sessions[0].additional_directories,
        vec!["/tmp/slight/shared".to_string()]
    );
    assert_eq!(
        result.sessions[0].title.as_deref(),
        Some("Prior fake session")
    );
    assert!(result.sessions[1].additional_directories.is_empty());
    assert!(result.sessions[1].title.is_none());
}

#[test]
fn decodes_agent_session_import_ack() {
    let frame: Frame = decode_line(AGENT_SESSION_IMPORT_ACK).expect("import ack decodes");
    let Frame::Ack(ack) = frame else {
        panic!("expected ack");
    };
    assert!(ack.ok);
    let result: ImportAgentSessionResult = ack.result_as().expect("import result decodes");
    assert_eq!(result.session.id, "s-imported");
    assert_eq!(result.session.agent, "fake");
    assert_eq!(result.session.status, SessionStatus::Idle);
}

#[test]
fn decodes_unsupported_capability_error_ack() {
    let frame: Frame = decode_line(AGENT_SESSION_IMPORT_UNSUPPORTED).expect("error ack decodes");
    let Frame::Ack(ack) = frame else {
        panic!("expected ack");
    };
    assert!(!ack.ok);
    let error = ack.error.expect("error present");
    assert_eq!(error.code, "unsupported_capability");
    assert!(!error.retryable);
    assert_eq!(
        gateway_protocol::ProtocolError::UnsupportedCapability("session/load".to_string()).code(),
        error.code
    );
}

#[test]
fn decodes_session_inspect_acp_metadata() {
    let frame: Frame = decode_line(SESSION_INSPECT_ACP).expect("inspect ack decodes");
    let Frame::Ack(ack) = frame else {
        panic!("expected ack");
    };
    assert!(ack.ok);
    let result: SessionInspectResult = ack.result_as().expect("inspect result decodes");
    assert_eq!(result.acp.protocol_version, 1);
    assert_eq!(result.acp.agent.title.as_deref(), Some("Fake ACP agent"));
    assert_eq!(result.acp.agent.name.as_deref(), Some("fake-acp-agent"));
    assert_eq!(result.acp.auth_methods.len(), 1);
    assert_eq!(result.acp.auth_methods[0].id, "device");
    assert!(result.acp.capabilities.prompt_image);
    assert!(!result.acp.capabilities.load_session);
    let modes = result.acp.modes.expect("modes present");
    assert_eq!(modes.current_mode_id, "plan");
    assert_eq!(modes.available_modes.len(), 2);
    assert_eq!(result.acp.config_options.len(), 2);
    assert_eq!(result.acp.config_options[0].id, "model");
    assert_eq!(result.acp.config_options[1].id, "brave_mode");
    let turn_ended = result
        .recent_events
        .iter()
        .find(|event| event.event == "session.turn_ended")
        .expect("turn ended event present");
    assert_eq!(
        turn_ended.payload.as_ref().unwrap()["stop_reason"],
        "end_turn"
    );
    let usage = result
        .recent_events
        .iter()
        .find(|event| event.event == "session.usage")
        .expect("usage event present");
    assert_eq!(usage.payload.as_ref().unwrap()["used"], 1024);
}

#[test]
fn decodes_rich_message_and_tool_call_payloads() {
    let frame: Frame = decode_line(SESSION_TOOL_CALL_RICH).expect("rich ack decodes");
    let Frame::Ack(ack) = frame else {
        panic!("expected ack");
    };
    assert!(ack.ok);
    let result: SessionInspectResult = ack.result_as().expect("inspect result decodes");

    let message = result
        .recent_events
        .iter()
        .find(|event| event.event == "session.message")
        .expect("message event present");
    let blocks = message.payload.as_ref().unwrap()["blocks"]
        .as_array()
        .expect("blocks array");
    assert_eq!(blocks[0]["kind"], "image");
    assert_eq!(blocks[0]["mime_type"], "image/png");
    assert_eq!(blocks[0]["data_base64"], "iVBORw0KGgo=");
    assert_eq!(blocks[1]["kind"], "resource_link");
    assert_eq!(blocks[1]["name"], "src/main.rs");
    assert_eq!(blocks[2]["kind"], "resource_text");
    assert_eq!(blocks[2]["text"], "fn main() {}");

    let tool_call = result
        .recent_events
        .iter()
        .find(|event| event.event == "session.tool_call")
        .expect("tool call event present");
    let payload = tool_call.payload.as_ref().unwrap();
    let content = payload["content"].as_array().expect("content array");
    assert_eq!(content.len(), 3);
    assert_eq!(content[0]["type"], "content");
    assert_eq!(content[0]["kind"], "text");
    assert_eq!(content[1]["type"], "diff");
    assert_eq!(content[1]["path"], "/tmp/work/src/main.rs");
    assert_eq!(content[1]["old_text"], "fn main() {}");
    assert_eq!(content[2]["type"], "terminal");
    assert_eq!(content[2]["terminal_id"], "term-1");
    assert_eq!(payload["locations"][0]["line"], 12);
    assert_eq!(payload["raw_output"]["exit"], 0);
}

#[test]
fn unknown_frame_type_is_rejected() {
    let error = decode_line::<Frame>(MALFORMED_UNKNOWN_TYPE);
    assert!(error.is_err());
}

#[test]
fn unknown_fields_are_ignored() {
    let with_extra = HELLO.replace(
        "\"client_id\": \"conformance-client\",",
        "\"client_id\": \"conformance-client\", \"future_field\": {\"a\": 1},",
    );
    let frame: Frame = decode_line(&with_extra).expect("unknown fields may be ignored");
    match frame {
        Frame::Hello(hello) => assert_eq!(hello.client_id, "conformance-client"),
        other => panic!("expected hello, got {other:?}"),
    }
}

#[test]
fn frames_round_trip() {
    let command = gateway_protocol::GatewayCommand::new("req-9", "session.list");
    let frame = Frame::Command(command);
    let encoded = encode_line(&frame).expect("encode");
    let decoded: Frame = decode_line(std::str::from_utf8(&encoded).unwrap()).expect("decode");
    assert_eq!(frame, decoded);
}

#[test]
fn error_ack_round_trips() {
    let ack = GatewayAck::error(
        "req-1",
        gateway_protocol::ProtocolError::UnknownSession("s-9".to_string()).body(),
    );
    let encoded = encode_line(&Frame::Ack(ack.clone())).expect("encode");
    let decoded: Frame = decode_line(std::str::from_utf8(&encoded).unwrap()).expect("decode");
    assert_eq!(decoded, Frame::Ack(ack));
}

#[test]
fn formats_rfc3339_utc() {
    assert_eq!(
        gateway_protocol::time::rfc3339_from_ms(0),
        "1970-01-01T00:00:00Z"
    );
    assert_eq!(
        gateway_protocol::time::rfc3339_from_ms(1_700_000_000_000),
        "2023-11-14T22:13:20Z"
    );
    assert_eq!(
        gateway_protocol::time::rfc3339_from_ms(1_700_000_000_123),
        "2023-11-14T22:13:20.123Z"
    );
}
