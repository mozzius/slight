#![allow(dead_code)]

use acp_adapters::AdapterRegistry;
use gateway_protocol::{
    ClientHello, DeviceListResult, DeviceRevokeResult, DeviceSummaryDto, HostDiagnosticsResult,
    HostLifecycleResult, HostRunState, HostStatusResult, PairingCreateResult, PairingListResult,
    ProtocolError, ServerCapabilities, PROTOCOL_VERSION,
};
use gateway_server::{AuthContext, Authenticator, GatewayConfig, GatewayServer, HostAdmin, Scope};
use session_core::SessionManager;
use session_store::InMemoryStore;
use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct TestAuthenticator;

impl Authenticator for TestAuthenticator {
    fn authenticate(&self, hello: &ClientHello) -> Result<AuthContext, ProtocolError> {
        match hello.credential.as_deref() {
            Some("test") => Ok(AuthContext::all("device-test", "test device")),
            Some("read") => Ok(AuthContext {
                device_id: "device-read".to_string(),
                device_label: "read-only device".to_string(),
                scopes: BTreeSet::from([Scope::ReadSessions]),
            }),
            _ => Err(ProtocolError::Unauthenticated(
                "invalid credential".to_string(),
            )),
        }
    }
}

pub struct TestAdmin {
    shutdown: Arc<AtomicBool>,
}

impl HostAdmin for TestAdmin {
    fn status(&self) -> HostStatusResult {
        HostStatusResult {
            state: HostRunState::Running,
            host_name: "test-host".to_string(),
            server_id: "test".to_string(),
            server_version: "0.0.0".to_string(),
            protocol_version: PROTOCOL_VERSION,
            listener: None,
            session_count: 0,
            active_session_count: 0,
            paired_device_count: 0,
            uptime_ms: 0,
            supported_agents: vec!["fake".to_string()],
            agent_catalog: Vec::new(),
        }
    }

    fn diagnostics(&self) -> HostDiagnosticsResult {
        HostDiagnosticsResult {
            server_version: "0.0.0".to_string(),
            protocol_version: PROTOCOL_VERSION,
            log_level: "info".to_string(),
            sessions: Vec::new(),
            recent_logs: Vec::new(),
            capabilities: ServerCapabilities::default(),
        }
    }

    fn request_start(&self) -> Result<HostLifecycleResult, ProtocolError> {
        Ok(HostLifecycleResult {
            state: HostRunState::Running,
            message: None,
        })
    }

    fn request_stop(&self) -> Result<HostLifecycleResult, ProtocolError> {
        self.shutdown.store(true, Ordering::Relaxed);
        Ok(HostLifecycleResult {
            state: HostRunState::Stopping,
            message: None,
        })
    }

    fn request_restart(&self) -> Result<HostLifecycleResult, ProtocolError> {
        Ok(HostLifecycleResult {
            state: HostRunState::Stopping,
            message: None,
        })
    }

    fn begin_pairing(&self, label: &str) -> Result<PairingCreateResult, ProtocolError> {
        Ok(PairingCreateResult {
            pairing_id: "pairing-1".to_string(),
            code: "123456".to_string(),
            label: label.to_string(),
            expires_at: "2026-01-01T00:05:00Z".to_string(),
            qr_payload: "slight://pair?code=123456".to_string(),
        })
    }

    fn list_pairings(&self) -> Result<PairingListResult, ProtocolError> {
        Ok(PairingListResult {
            pairings: Vec::new(),
        })
    }

    fn list_devices(&self) -> Result<DeviceListResult, ProtocolError> {
        Ok(DeviceListResult {
            devices: Vec::new(),
        })
    }

    fn revoke_device(&self, device_id: &str) -> Result<DeviceRevokeResult, ProtocolError> {
        Ok(DeviceRevokeResult {
            device: DeviceSummaryDto {
                device_id: device_id.to_string(),
                label: "test device".to_string(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                last_seen_at: "2026-01-01T00:00:00Z".to_string(),
                revoked: true,
            },
        })
    }
}

pub struct TestServer {
    pub addr: SocketAddr,
    pub shutdown: Arc<AtomicBool>,
}

impl TestServer {
    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn spawn_server() -> TestServer {
    spawn_server_with(GatewayConfig::default())
}

pub fn spawn_server_with(config: GatewayConfig) -> TestServer {
    let sessions = Arc::new(SessionManager::new(
        Arc::new(AdapterRegistry::with_builtins()),
        Box::new(InMemoryStore::new()),
        64,
    ));
    let shutdown = Arc::new(AtomicBool::new(false));
    let server = GatewayServer::bind(
        config,
        sessions,
        Arc::new(TestAuthenticator),
        Arc::new(TestAdmin {
            shutdown: shutdown.clone(),
        }),
        shutdown.clone(),
        "127.0.0.1:0".parse().unwrap(),
    )
    .expect("server binds");
    let addr = server.local_addr();
    std::thread::spawn(move || {
        let _ = server.run();
    });
    TestServer { addr, shutdown }
}
