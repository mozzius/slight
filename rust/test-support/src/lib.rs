use acp_adapters::AdapterRegistry;
use gateway_protocol::{
    command_name, ClientHello, ClientHello as Hello, GatewayCommand, ResumeRequest,
};
use session_core::SessionManager;
use session_store::InMemoryStore;
use std::sync::Arc;

pub mod fixtures {
    pub const HELLO: &str = include_str!("../../../protocol/conformance/hello.json");
    pub const SESSION_CREATE_COMMAND: &str =
        include_str!("../../../protocol/conformance/session-create-command.json");
    pub const SESSION_RENAME_COMMAND: &str =
        include_str!("../../../protocol/conformance/session-rename-command.json");
    pub const ACK_OK: &str = include_str!("../../../protocol/conformance/ack-ok.json");
    pub const AGENT_SESSIONS_LIST_COMMAND: &str =
        include_str!("../../../protocol/conformance/agent-sessions-list-command.json");
    pub const AGENT_SESSION_IMPORT_COMMAND: &str =
        include_str!("../../../protocol/conformance/agent-session-import-command.json");
    pub const AGENT_SESSIONS_LIST_ACK: &str =
        include_str!("../../../protocol/conformance/agent-sessions-list-ack.json");
    pub const AGENT_SESSION_IMPORT_ACK: &str =
        include_str!("../../../protocol/conformance/agent-session-import-ack.json");
    pub const MALFORMED_UNKNOWN_TYPE: &str =
        include_str!("../../../protocol/conformance/malformed-unknown-type.json");
}

pub fn fake_adapters() -> Arc<AdapterRegistry> {
    Arc::new(AdapterRegistry::with_builtins())
}

pub fn fake_manager(journal_limit: usize) -> Arc<SessionManager> {
    Arc::new(SessionManager::new(
        fake_adapters(),
        Box::new(InMemoryStore::new()),
        journal_limit,
    ))
}

pub fn sample_hello(credential: &str) -> ClientHello {
    Hello {
        protocol_version: gateway_protocol::PROTOCOL_VERSION,
        client_id: "test-client".to_string(),
        client_name: "test".to_string(),
        client_version: "0.1.0".to_string(),
        device_id: Some("test-device".to_string()),
        credential: Some(credential.to_string()),
        resume: None,
    }
}

pub fn sample_resume_hello(credential: &str, session_id: &str, sequence: u64) -> ClientHello {
    ClientHello {
        resume: Some(ResumeRequest {
            session_id: Some(session_id.to_string()),
            last_event_sequence: Some(sequence),
        }),
        ..sample_hello(credential)
    }
}

pub fn create_session_command(request_id: &str, agent: &str) -> GatewayCommand {
    GatewayCommand::new(request_id, command_name::SESSION_CREATE).with_params(
        &gateway_protocol::CreateSessionParams {
            agent: agent.to_string(),
            model: None,
            effort: None,
            working_directory_label: "~".to_string(),
            initial_prompt: Some("hello".to_string()),
        },
    )
}

pub fn rename_session_command(request_id: &str, session_id: &str, title: &str) -> GatewayCommand {
    GatewayCommand::new(request_id, command_name::SESSION_RENAME)
        .for_session(session_id)
        .with_params(&gateway_protocol::RenameSessionParams {
            title: title.to_string(),
        })
}

pub fn attach_command(request_id: &str, session_id: &str, after: Option<u64>) -> GatewayCommand {
    GatewayCommand::new(request_id, command_name::SESSION_ATTACH)
        .for_session(session_id)
        .with_params(&gateway_protocol::AttachSessionParams {
            after_sequence: after,
        })
}
