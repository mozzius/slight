//! End-to-end conformance tests for the ACP v1 client against the wire-level
//! fake agent.
//!
//! These exercise the full path: subprocess spawn, newline-delimited stdio
//! framing, JSON-RPC correlation, initialize/session lifecycle, streaming
//! `session/update` notifications, and the bidirectional permission exchange.

use acp_adapters::process::{spawn, ChildSpec, SpawnedChild};
use acp_adapters::wire_agent::{spawn_in_process_agent, FakeWireAgentConfig};
use acp_types::client::{AcpClient, AutoApproveHandler};
use acp_types::wire::{
    AgentToClientNotification, ClientCapabilities, Implementation, ProtocolVersion, SessionId,
    SessionUpdate, StopReason,
};

fn count_agent_chunks(
    client: &AcpClient<impl std::io::BufRead + Send, impl std::io::Write + Send>,
) -> usize {
    client
        .notifications()
        .iter()
        .filter(|notification| {
            matches!(
                notification,
                AgentToClientNotification::SessionUpdate(notification)
                    if matches!(notification.update, SessionUpdate::AgentMessageChunk(_))
            )
        })
        .count()
}

#[test]
fn in_process_client_agent_round_trip() {
    let (transport, handle) = spawn_in_process_agent(FakeWireAgentConfig::default()).unwrap();
    let mut client = AcpClient::new(transport, Box::new(AutoApproveHandler::new()));

    let init = client
        .initialize(
            ClientCapabilities::default(),
            Implementation::new("slight-test", "0.1.0"),
        )
        .unwrap();
    assert_eq!(init.protocol_version, ProtocolVersion::V1);

    let session = client.new_session("/tmp").unwrap();
    assert_eq!(session.session_id, SessionId::new("session-1"));

    let response = client.prompt("session-1", "hello").unwrap();
    assert_eq!(response.stop_reason, StopReason::EndTurn);
    assert!(count_agent_chunks(&client) >= 1);
    assert!(client.orphan_responses().is_empty());

    drop(client);
    assert!(handle.join().unwrap().is_ok());
}

#[test]
fn subprocess_agent_round_trip() {
    let program = env!("CARGO_BIN_EXE_fake-acp-agent");
    let SpawnedChild {
        mut process,
        transport,
        ..
    } = spawn(&ChildSpec::new(program)).unwrap();

    let mut client = AcpClient::new(transport, Box::new(AutoApproveHandler::preferred("allow")));
    let init = client
        .initialize(
            ClientCapabilities::default(),
            Implementation::new("slight-test", "0.1.0"),
        )
        .unwrap();
    assert_eq!(init.protocol_version, ProtocolVersion::V1);

    let session = client.new_session(std::env::temp_dir()).unwrap();
    assert_eq!(session.session_id, SessionId::new("session-1"));

    let response = client.prompt("session-1", "subprocess hello").unwrap();
    assert_eq!(response.stop_reason, StopReason::EndTurn);
    assert!(count_agent_chunks(&client) >= 1);

    drop(client);
    let status = process.wait().unwrap();
    assert!(
        status.success(),
        "fake agent should exit cleanly, got {status}"
    );
}

#[test]
fn initialize_negotiates_and_rejects_pre_init_session() {
    let (transport, handle) = spawn_in_process_agent(FakeWireAgentConfig::default()).unwrap();
    let mut client = AcpClient::new(transport, Box::new(AutoApproveHandler::new()));

    assert!(matches!(
        client.new_session("/tmp").unwrap_err(),
        acp_types::client::ClientError::NotInitialized
    ));

    client
        .initialize(
            ClientCapabilities::default(),
            Implementation::new("slight-test", "0.1.0"),
        )
        .unwrap();
    assert!(client.is_initialized());

    drop(client);
    assert!(handle.join().unwrap().is_ok());
}
