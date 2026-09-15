use crate::config::HostConfig;
use crate::logging::HostLogger;
use gateway_protocol::{
    AgentCatalogEntry, ClientHello, DeviceListResult, DeviceRevokeResult, DeviceSummaryDto,
    HostDiagnosticsResult, HostLifecycleResult, HostRunState, HostStatusResult, LogEntryDto,
    PairingCreateResult, PairingListResult, PairingSummaryDto, ProtocolError, ServerCapabilities,
    SessionDiagnosticsDto, PROTOCOL_VERSION,
};
use gateway_server::{AuthContext, Authenticator, HostAdmin};
use session_core::{now_ms, SessionManager};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use uuid::Uuid;

struct DeviceRecord {
    device_id: String,
    label: String,
    token: String,
    created_at_ms: i64,
    last_seen_at_ms: i64,
    revoked: bool,
}

struct PairingRecord {
    pairing_id: String,
    code: String,
    label: String,
    expires_at_ms: i64,
    consumed: bool,
}

pub struct HostState {
    config: HostConfig,
    sessions: Arc<SessionManager>,
    supported_agents: Vec<String>,
    agent_catalog: Vec<AgentCatalogEntry>,
    devices: Mutex<HashMap<String, DeviceRecord>>,
    pairings: Mutex<HashMap<String, PairingRecord>>,
    logs: Mutex<VecDeque<LogEntryDto>>,
    listener: Mutex<Option<String>>,
    shutdown: Arc<AtomicBool>,
    running: AtomicBool,
    started_at: Instant,
    logger: HostLogger,
}

impl HostState {
    pub fn new(
        config: HostConfig,
        sessions: Arc<SessionManager>,
        supported_agents: Vec<String>,
        agent_catalog: Vec<AgentCatalogEntry>,
        logger: HostLogger,
    ) -> Arc<Self> {
        Arc::new(Self {
            config,
            sessions,
            supported_agents,
            agent_catalog,
            devices: Mutex::new(HashMap::new()),
            pairings: Mutex::new(HashMap::new()),
            logs: Mutex::new(VecDeque::new()),
            listener: Mutex::new(None),
            shutdown: Arc::new(AtomicBool::new(false)),
            running: AtomicBool::new(false),
            started_at: Instant::now(),
            logger,
        })
    }

    pub fn sessions(&self) -> Arc<SessionManager> {
        self.sessions.clone()
    }

    pub fn shutdown_handle(&self) -> Arc<AtomicBool> {
        self.shutdown.clone()
    }

    pub fn set_running(&self, running: bool) {
        self.running.store(running, Ordering::Relaxed);
    }

    pub fn set_listener(&self, listener: String) {
        *self.lock(&self.listener) = Some(listener);
    }

    pub fn log(&self, level: &str, message: impl Into<String>) {
        let entry = LogEntryDto {
            timestamp: gateway_protocol::time::rfc3339_from_ms(now_ms()),
            level: level.to_string(),
            message: message.into(),
        };
        let mut logs = self.lock(&self.logs);
        logs.push_back(entry);
        while logs.len() > 200 {
            logs.pop_front();
        }
        if let Some(entry) = logs.back() {
            self.logger.write(entry);
        }
    }

    fn authenticate(&self, hello: &ClientHello) -> Result<AuthContext, ProtocolError> {
        let credential = hello.credential.clone().unwrap_or_default();
        if credential.is_empty() {
            return Err(ProtocolError::Unauthenticated(
                "credential required".to_string(),
            ));
        }

        let paired = {
            let now = now_ms();
            let mut pairings = self.lock(&self.pairings);
            pairings
                .values_mut()
                .find(|pairing| {
                    pairing.code == credential && !pairing.consumed && pairing.expires_at_ms >= now
                })
                .map(|pairing| {
                    pairing.consumed = true;
                    pairing.label.clone()
                })
        };
        if let Some(label) = paired {
            let device_id = format!("device-{}", Uuid::new_v4());
            let token = Uuid::new_v4().to_string();
            let now = now_ms();
            self.lock(&self.devices).insert(
                device_id.clone(),
                DeviceRecord {
                    device_id: device_id.clone(),
                    label: label.clone(),
                    token,
                    created_at_ms: now,
                    last_seen_at_ms: now,
                    revoked: false,
                },
            );
            self.log("info", format!("pairing consumed by {label}"));
            return Ok(AuthContext::all(device_id, label));
        }

        let device = {
            let mut devices = self.lock(&self.devices);
            devices
                .values_mut()
                .find(|device| device.token == credential && !device.revoked)
                .map(|device| {
                    device.last_seen_at_ms = now_ms();
                    (device.device_id.clone(), device.label.clone())
                })
        };
        if let Some((device_id, label)) = device {
            return Ok(AuthContext::all(device_id, label));
        }

        if self.config.dev_mode {
            let accepted = match self.config.dev_token.as_deref() {
                Some(token) => token == credential,
                None => true,
            };
            if accepted {
                let label = hello.client_name.clone();
                let device_id = hello
                    .device_id
                    .clone()
                    .unwrap_or_else(|| "dev-device".to_string());
                self.log(
                    "info",
                    format!("development credential accepted for {label}"),
                );
                return Ok(AuthContext::all(device_id, label));
            }
        }

        Err(ProtocolError::Unauthenticated(
            "invalid credential".to_string(),
        ))
    }

    fn begin_pairing(&self, label: &str) -> Result<PairingCreateResult, ProtocolError> {
        let now = now_ms();
        let expires_at_ms = now + (self.config.pairing_ttl_secs as i64) * 1000;
        let pairing_id = Uuid::new_v4().to_string();
        let code = generate_pairing_code();
        self.lock(&self.pairings).insert(
            pairing_id.clone(),
            PairingRecord {
                pairing_id: pairing_id.clone(),
                code: code.clone(),
                label: label.to_string(),
                expires_at_ms,
                consumed: false,
            },
        );
        let qr_payload = format!(
            "slight://pair?host={}&code={}&addr={}",
            self.config.server_id,
            code,
            self.listener()
                .unwrap_or_else(|| self.config.bind.to_string())
        );
        self.log("info", format!("pairing created for {label}"));
        Ok(PairingCreateResult {
            pairing_id,
            code,
            label: label.to_string(),
            expires_at: gateway_protocol::time::rfc3339_from_ms(expires_at_ms),
            qr_payload,
        })
    }

    fn list_pairings(&self) -> PairingListResult {
        let pairings = self.lock(&self.pairings);
        PairingListResult {
            pairings: pairings
                .values()
                .map(|pairing| PairingSummaryDto {
                    pairing_id: pairing.pairing_id.clone(),
                    code: pairing.code.clone(),
                    label: pairing.label.clone(),
                    expires_at: gateway_protocol::time::rfc3339_from_ms(pairing.expires_at_ms),
                    consumed: pairing.consumed,
                })
                .collect(),
        }
    }

    fn list_devices(&self) -> DeviceListResult {
        let devices = self.lock(&self.devices);
        DeviceListResult {
            devices: devices.values().map(device_dto).collect(),
        }
    }

    fn revoke_device(&self, device_id: &str) -> Result<DeviceRevokeResult, ProtocolError> {
        let mut devices = self.lock(&self.devices);
        let device = devices
            .get_mut(device_id)
            .ok_or_else(|| ProtocolError::InvalidParams(format!("unknown device: {device_id}")))?;
        device.revoked = true;
        let dto = device_dto(device);
        drop(devices);
        self.log("info", format!("device revoked: {device_id}"));
        Ok(DeviceRevokeResult { device: dto })
    }

    fn status(&self) -> HostStatusResult {
        HostStatusResult {
            state: if self.shutdown.load(Ordering::Relaxed) {
                HostRunState::Stopping
            } else if self.running.load(Ordering::Relaxed) {
                HostRunState::Running
            } else {
                HostRunState::Stopped
            },
            host_name: self.config.host_name.clone(),
            server_id: self.config.server_id.clone(),
            server_version: self.config.server_version.clone(),
            protocol_version: PROTOCOL_VERSION,
            listener: self.listener(),
            session_count: self.sessions.session_count(),
            active_session_count: self.sessions.active_session_count(),
            paired_device_count: self
                .lock(&self.devices)
                .values()
                .filter(|device| !device.revoked)
                .count(),
            uptime_ms: self.started_at.elapsed().as_millis() as u64,
            supported_agents: self.supported_agents.clone(),
            agent_catalog: self.agent_catalog.clone(),
        }
    }

    fn diagnostics(&self) -> HostDiagnosticsResult {
        let sessions = self
            .sessions
            .list_sessions()
            .iter()
            .map(|summary| {
                let detail = self.sessions.inspect_session(&summary.id).ok();
                let event_count = detail
                    .as_ref()
                    .map(|detail| detail.recent_events.len())
                    .unwrap_or(0);
                let running = detail
                    .as_ref()
                    .map(|detail| detail.running)
                    .unwrap_or(false);
                SessionDiagnosticsDto {
                    session_id: summary.id.clone(),
                    status: status_str(summary.status),
                    agent: summary.agent.clone(),
                    last_sequence: summary.last_sequence,
                    event_count,
                    running,
                }
            })
            .collect();
        HostDiagnosticsResult {
            server_version: self.config.server_version.clone(),
            protocol_version: PROTOCOL_VERSION,
            log_level: "info".to_string(),
            sessions,
            recent_logs: self.lock(&self.logs).iter().cloned().collect(),
            capabilities: ServerCapabilities {
                max_replay_events: Some(self.config.max_replay_events),
                max_event_journal: Some(self.config.journal_limit),
                ..ServerCapabilities::default()
            },
        }
    }

    fn listener(&self) -> Option<String> {
        self.lock(&self.listener).clone()
    }

    fn lock<'a, T>(&self, mutex: &'a Mutex<T>) -> std::sync::MutexGuard<'a, T> {
        mutex
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn device_dto(device: &DeviceRecord) -> DeviceSummaryDto {
    DeviceSummaryDto {
        device_id: device.device_id.clone(),
        label: device.label.clone(),
        created_at: gateway_protocol::time::rfc3339_from_ms(device.created_at_ms),
        last_seen_at: gateway_protocol::time::rfc3339_from_ms(device.last_seen_at_ms),
        revoked: device.revoked,
    }
}

fn status_str(status: session_core::SessionStatus) -> String {
    serde_json::to_value(status)
        .ok()
        .and_then(|value| value.as_str().map(String::from))
        .unwrap_or_default()
}

fn generate_pairing_code() -> String {
    let uuid = Uuid::new_v4();
    let bytes = uuid.as_bytes();
    let value = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) % 1_000_000;
    format!("{value:06}")
}

pub struct HostAuthenticator {
    state: Arc<HostState>,
}

impl HostAuthenticator {
    pub fn new(state: Arc<HostState>) -> Self {
        Self { state }
    }
}

impl Authenticator for HostAuthenticator {
    fn authenticate(&self, hello: &ClientHello) -> Result<AuthContext, ProtocolError> {
        self.state.authenticate(hello)
    }
}

pub struct HostAdminImpl {
    state: Arc<HostState>,
}

impl HostAdminImpl {
    pub fn new(state: Arc<HostState>) -> Self {
        Self { state }
    }
}

impl HostAdmin for HostAdminImpl {
    fn status(&self) -> HostStatusResult {
        self.state.status()
    }

    fn diagnostics(&self) -> HostDiagnosticsResult {
        self.state.diagnostics()
    }

    fn request_start(&self) -> Result<HostLifecycleResult, ProtocolError> {
        if self.state.shutdown.load(Ordering::Relaxed) {
            return Ok(HostLifecycleResult {
                state: HostRunState::Failed,
                message: Some(
                    "host is shutting down; restart the process or supervisor".to_string(),
                ),
            });
        }
        Ok(HostLifecycleResult {
            state: HostRunState::Running,
            message: Some("host already running".to_string()),
        })
    }

    fn request_stop(&self) -> Result<HostLifecycleResult, ProtocolError> {
        self.state.shutdown.store(true, Ordering::Relaxed);
        self.state.log("info", "shutdown requested");
        Ok(HostLifecycleResult {
            state: HostRunState::Stopping,
            message: Some("shutdown requested".to_string()),
        })
    }

    fn request_restart(&self) -> Result<HostLifecycleResult, ProtocolError> {
        self.state.shutdown.store(true, Ordering::Relaxed);
        self.state
            .log("info", "restart requested; process supervisor required");
        Ok(HostLifecycleResult {
            state: HostRunState::Stopping,
            message: Some("restart requires an external supervisor".to_string()),
        })
    }

    fn begin_pairing(&self, label: &str) -> Result<PairingCreateResult, ProtocolError> {
        self.state.begin_pairing(label)
    }

    fn list_pairings(&self) -> Result<PairingListResult, ProtocolError> {
        Ok(self.state.list_pairings())
    }

    fn list_devices(&self) -> Result<DeviceListResult, ProtocolError> {
        Ok(self.state.list_devices())
    }

    fn revoke_device(&self, device_id: &str) -> Result<DeviceRevokeResult, ProtocolError> {
        self.state.revoke_device(device_id)
    }
}
