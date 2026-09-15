mod support;

use gateway_protocol::{
    command_name, AgentSessionsListParams, AgentSessionsListResult, GatewayCommand,
    HostStatusResult, ImportAgentSessionParams, ImportAgentSessionResult, PairingCreateResult,
    SessionAttachResult, SessionCreateResult, SessionHistoryParams, SessionHistoryResult,
    SessionInspectResult, SessionListResult, SessionRecovery, SessionStatus,
    SessionWorkingDirectoriesResult,
};
use gateway_server::GatewayClient;

#[test]
fn loopback_session_flow() {
    let server = support::spawn_server();
    let addr = server.addr;

    assert!(GatewayClient::connect(addr, test_support::sample_hello("nope")).is_err());

    let mut client = GatewayClient::connect(addr, test_support::sample_hello("test"))
        .expect("client authenticates");
    assert_eq!(client.welcome().server_name, "slight-host");

    let request_id = client.next_request_id();
    let outcome = client
        .call(GatewayCommand::new(request_id, command_name::SESSION_LIST))
        .expect("list succeeds");
    let list: SessionListResult = outcome.result_as().expect("list result");
    assert!(list.sessions.is_empty());

    let command = test_support::create_session_command(&client.next_request_id(), "fake");
    let outcome = client.call(command).expect("create succeeds");
    let created: SessionCreateResult = outcome.result_as().expect("create result");
    assert_eq!(created.session.status, SessionStatus::WaitingPermission);
    let session_id = created.session.id.clone();

    let command = test_support::attach_command(&client.next_request_id(), &session_id, None);
    let outcome = client.call(command).expect("attach succeeds");
    let attached: SessionAttachResult = outcome.result_as().expect("attach result");
    assert!(attached.replayed.is_empty());
    assert!(!attached.resync_required);

    let command = GatewayCommand::new(client.next_request_id(), command_name::SESSION_HISTORY)
        .for_session(session_id.clone())
        .with_params(&SessionHistoryParams {
            before_sequence: None,
            limit: Some(100),
        });
    let outcome = client.call(command).expect("history succeeds");
    let history: SessionHistoryResult = outcome.result_as().expect("history result");
    assert!(!history.events.is_empty());

    let command = GatewayCommand::new(client.next_request_id(), command_name::SESSION_INSPECT)
        .for_session(session_id.clone());
    let outcome = client.call(command).expect("inspect succeeds");
    let detail: SessionInspectResult = outcome.result_as().expect("inspect result");
    assert!(detail.pending_permission.is_some());
    assert!(!detail.recent_events.is_empty());

    let request_id = client.next_request_id();
    let outcome = client
        .call(GatewayCommand::new(request_id, command_name::HOST_STATUS))
        .expect("status succeeds");
    let status: HostStatusResult = outcome.result_as().expect("status result");
    assert_eq!(status.host_name, "test-host");

    server.stop();
}

#[test]
fn unknown_command_returns_error() {
    let server = support::spawn_server();
    let mut client =
        GatewayClient::connect(server.addr, test_support::sample_hello("test")).unwrap();
    let request_id = client.next_request_id();
    let error = client
        .call(GatewayCommand::new(request_id, "does.not.exist"))
        .unwrap_err();
    assert!(matches!(
        error,
        gateway_server::ClientError::Server { ref code, .. } if code == "unsupported"
    ));
    server.stop();
}

#[test]
fn pairing_command_round_trips() {
    let server = support::spawn_server();
    let mut client =
        GatewayClient::connect(server.addr, test_support::sample_hello("test")).unwrap();
    let request_id = client.next_request_id();
    let command = GatewayCommand::new(request_id, command_name::PAIRING_CREATE).with_params(
        &gateway_protocol::BeginPairingParams {
            label: "my phone".to_string(),
        },
    );
    let outcome = client.call(command).expect("pairing succeeds");
    let pairing: PairingCreateResult = outcome.result_as().expect("pairing result");
    assert_eq!(pairing.code, "123456");
    assert_eq!(pairing.label, "my phone");
    server.stop();
}

#[test]
fn agent_sessions_discovery_and_import() {
    let server = support::spawn_server();
    let mut client =
        GatewayClient::connect(server.addr, test_support::sample_hello("test")).unwrap();

    let command = GatewayCommand::new(client.next_request_id(), command_name::AGENT_SESSIONS_LIST)
        .with_params(&AgentSessionsListParams {
            agent: "fake".to_string(),
            working_directory_label: Some("~/work/slight".to_string()),
            cursor: None,
        });
    let outcome = client.call(command).expect("discovery succeeds");
    let discovered: AgentSessionsListResult = outcome.result_as().expect("discovery result");
    assert_eq!(discovered.sessions.len(), 2);
    assert_eq!(discovered.sessions[0].agent, "fake");
    assert_eq!(discovered.sessions[0].agent_session_id, "fake-listed-1");
    assert_eq!(discovered.sessions[0].cwd, "/tmp/slight/fake-project");

    let command = GatewayCommand::new(client.next_request_id(), command_name::AGENT_SESSION_IMPORT)
        .with_params(&ImportAgentSessionParams {
            agent: "fake".to_string(),
            agent_session_id: "fake-listed-1".to_string(),
            working_directory_label: "~/work/slight".to_string(),
            recovery: SessionRecovery::Load,
            title: Some("Imported session".to_string()),
        });
    let outcome = client.call(command).expect("import succeeds");
    let imported: ImportAgentSessionResult = outcome.result_as().expect("import result");
    assert_eq!(imported.session.title, "Imported session");
    assert_eq!(imported.session.agent, "fake");
    assert_eq!(imported.session.status, SessionStatus::Idle);

    let command = GatewayCommand::new(
        client.next_request_id(),
        command_name::SESSION_WORKING_DIRECTORIES,
    );
    let outcome = client.call(command).expect("working directories succeed");
    let paths: SessionWorkingDirectoriesResult =
        outcome.result_as().expect("working directories result");
    assert_eq!(
        paths.paths,
        vec![
            "~/work/slight",
            "/tmp/slight/fake-project",
            "/tmp/slight/other-project",
        ]
    );

    // Import replays agent-side history into Slight's local journal.
    let command = GatewayCommand::new(client.next_request_id(), command_name::SESSION_INSPECT)
        .for_session(imported.session.id.clone());
    let outcome = client.call(command).expect("inspect succeeds");
    let detail: SessionInspectResult = outcome.result_as().expect("inspect result");
    assert!(detail.recent_events.iter().any(|event| {
        event.event == "session.message"
            && event.payload.as_ref().unwrap()["text"] == "replayed question"
    }));

    let command = GatewayCommand::new(client.next_request_id(), command_name::AGENT_SESSIONS_LIST)
        .with_params(&AgentSessionsListParams {
            agent: "fake".to_string(),
            working_directory_label: Some("~/work/slight".to_string()),
            cursor: None,
        });
    let outcome = client.call(command).expect("filtered discovery succeeds");
    let remaining: AgentSessionsListResult =
        outcome.result_as().expect("filtered discovery result");
    assert_eq!(remaining.sessions.len(), 1);
    assert_eq!(remaining.sessions[0].agent_session_id, "fake-listed-2");

    server.stop();
}

#[test]
fn agent_session_import_requires_create_scope() {
    let server = support::spawn_server();
    let mut client =
        GatewayClient::connect(server.addr, test_support::sample_hello("read")).unwrap();

    let command = GatewayCommand::new(client.next_request_id(), command_name::AGENT_SESSIONS_LIST)
        .with_params(&AgentSessionsListParams {
            agent: "fake".to_string(),
            working_directory_label: None,
            cursor: None,
        });
    let list = client.call(command).expect("read scope may discover");
    let discovered: AgentSessionsListResult = list.result_as().expect("discovery result");
    assert_eq!(discovered.sessions.len(), 2);

    let command = GatewayCommand::new(client.next_request_id(), command_name::AGENT_SESSION_IMPORT)
        .with_params(&ImportAgentSessionParams {
            agent: "fake".to_string(),
            agent_session_id: "fake-listed-1".to_string(),
            working_directory_label: "~/work/slight".to_string(),
            recovery: SessionRecovery::Load,
            title: None,
        });
    let error = client.call(command).unwrap_err();
    assert!(matches!(
        error,
        gateway_server::ClientError::Server { ref code, .. } if code == "unauthorized"
    ));

    server.stop();
}

#[test]
fn agent_discovery_unknown_agent_is_invalid_params() {
    let server = support::spawn_server();
    let mut client =
        GatewayClient::connect(server.addr, test_support::sample_hello("test")).unwrap();
    let command = GatewayCommand::new(client.next_request_id(), command_name::AGENT_SESSIONS_LIST)
        .with_params(&AgentSessionsListParams {
            agent: "not-a-real-agent".to_string(),
            working_directory_label: None,
            cursor: None,
        });
    let error = client.call(command).unwrap_err();
    assert!(matches!(
        error,
        gateway_server::ClientError::Server { ref code, .. } if code == "invalid_params"
    ));
    server.stop();
}
