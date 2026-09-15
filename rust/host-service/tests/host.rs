use gateway_protocol::{
    command_name, DeviceListResult, DeviceRevokeResult, GatewayCommand, HostRunState,
    HostStatusResult, PairingCreateResult, SessionAttachResult, SessionCreateResult,
    SessionHistoryParams, SessionHistoryResult, SessionListResult,
};
use gateway_server::GatewayClient;
use host_service::{Host, HostConfig};
use std::sync::atomic::Ordering;

fn config(session_store_path: std::path::PathBuf) -> HostConfig {
    HostConfig {
        bind: "127.0.0.1:0".parse().unwrap(),
        additional_binds: vec!["127.0.0.1:0".parse().unwrap()],
        session_store_path,
        log_dir: std::env::temp_dir().join(format!("slight-host-logs-{}", std::process::id())),
        ..HostConfig::default()
    }
}

#[test]
fn headless_host_serves_status_sessions_and_pairing() {
    let session_store_path =
        std::env::temp_dir().join(format!("slight-host-test-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&session_store_path);
    let host_config = config(session_store_path.clone());
    let log_dir = host_config.log_dir.clone();
    let (mut host, addr) = Host::bind(host_config.clone()).expect("host binds");
    let addrs = host.local_addrs().to_vec();
    assert_eq!(addrs.len(), 2);
    assert_ne!(addrs[0], addrs[1]);
    let shutdown = host.state().shutdown_handle();
    let handle = std::thread::spawn(move || host.serve());

    let mut client =
        GatewayClient::connect(addr, test_support::sample_hello("dev")).expect("dev client");
    let _second_client = GatewayClient::connect(addrs[1], test_support::sample_hello("dev"))
        .expect("second listener accepts clients");

    let request_id = client.next_request_id();
    let outcome = client
        .call(GatewayCommand::new(request_id, command_name::HOST_STATUS))
        .expect("status succeeds");
    let status: HostStatusResult = outcome.result_as().expect("status result");
    assert_eq!(status.state, HostRunState::Running);
    assert_eq!(status.session_count, 0);
    let listeners = status.listener.expect("listeners are reported");
    assert!(listeners.contains(&addrs[0].to_string()));
    assert!(listeners.contains(&addrs[1].to_string()));
    assert!(status.supported_agents.contains(&"opencode".to_string()));

    let command = test_support::create_session_command(&client.next_request_id(), "opencode");
    let outcome = client.call(command).expect("create succeeds");
    let created: SessionCreateResult = outcome.result_as().expect("create result");
    let session_id = created.session.id.clone();

    let request_id = client.next_request_id();
    let outcome = client
        .call(GatewayCommand::new(request_id, command_name::SESSION_LIST))
        .expect("list succeeds");
    let list: SessionListResult = outcome.result_as().expect("list result");
    assert_eq!(list.sessions.len(), 1);

    let command = test_support::attach_command(&client.next_request_id(), &session_id, None);
    let outcome = client.call(command).expect("attach succeeds");
    let attached: SessionAttachResult = outcome.result_as().expect("attach result");
    assert!(attached.replayed.is_empty());

    let command = GatewayCommand::new(client.next_request_id(), command_name::SESSION_HISTORY)
        .for_session(session_id.clone())
        .with_params(&SessionHistoryParams {
            before_sequence: None,
            limit: Some(100),
        });
    let outcome = client.call(command).expect("history succeeds");
    let history: SessionHistoryResult = outcome.result_as().expect("history result");
    assert!(!history.events.is_empty());

    let command = GatewayCommand::new(client.next_request_id(), command_name::SESSION_INPUT)
        .for_session(session_id.clone())
        .with_params(&gateway_protocol::SendInputParams {
            text: "continue".to_string(),
        });
    client.call(command).expect("input accepted");

    let request_id = client.next_request_id();
    let command = GatewayCommand::new(request_id, command_name::PAIRING_CREATE).with_params(
        &gateway_protocol::BeginPairingParams {
            label: "my phone".to_string(),
        },
    );
    let outcome = client.call(command).expect("pairing succeeds");
    let pairing: PairingCreateResult = outcome.result_as().expect("pairing result");
    assert_eq!(pairing.code.len(), 6);

    let mut paired = GatewayClient::connect(addr, test_support::sample_hello(&pairing.code))
        .expect("paired client authenticates");

    let request_id = paired.next_request_id();
    let outcome = paired
        .call(GatewayCommand::new(request_id, command_name::DEVICE_LIST))
        .expect("device list succeeds");
    let devices: DeviceListResult = outcome.result_as().expect("device result");
    assert_eq!(devices.devices.len(), 1);
    let device_id = devices.devices[0].device_id.clone();

    let command = GatewayCommand::new(paired.next_request_id(), command_name::DEVICE_REVOKE)
        .with_params(&gateway_protocol::RevokeDeviceParams {
            device_id: device_id.clone(),
        });
    let outcome = paired.call(command).expect("revoke succeeds");
    let revoked: DeviceRevokeResult = outcome.result_as().expect("revoke result");
    assert!(revoked.device.revoked);

    let request_id = paired.next_request_id();
    let outcome = paired
        .call(GatewayCommand::new(
            request_id,
            command_name::HOST_DIAGNOSTICS,
        ))
        .expect("diagnostics succeeds");
    let diagnostics: gateway_protocol::HostDiagnosticsResult =
        outcome.result_as().expect("diagnostics result");
    assert_eq!(diagnostics.sessions.len(), 1);

    shutdown.store(true, Ordering::Relaxed);
    handle
        .join()
        .expect("host thread joins")
        .expect("host stops");

    let (host, _) = Host::bind(host_config).expect("restarted host binds");
    assert_eq!(host.sessions().session_count(), 1);
    drop(host);
    std::fs::remove_file(session_store_path).expect("test database removed");
    std::fs::remove_dir_all(log_dir).expect("test logs removed");
}
