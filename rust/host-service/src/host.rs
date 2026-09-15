use crate::config::HostConfig;
use crate::logging::HostLogger;
use crate::state::{HostAdminImpl, HostAuthenticator, HostState};
use acp_adapters::claude_code::ClaudeCodeConfig;
use acp_adapters::codex::CodexConfig;
use acp_adapters::matrix::ProbeOutcome;
use acp_adapters::opencode::OpencodeConfig;
use acp_adapters::{AdapterRegistry, AgentLaunchConfig, DefaultAgentConfig};
use gateway_protocol::{AgentCatalogEntry, ServerCapabilities};
use gateway_server::{GatewayConfig, GatewayServer};
use session_core::SessionManager;
use session_store::SqliteStore;
use std::io;
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HostError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("session store error: {0}")]
    Store(#[from] session_store::StoreError),
    #[error("host has already been served")]
    AlreadyServed,
}

pub struct Host {
    config: HostConfig,
    state: Arc<HostState>,
    server: Option<GatewayServer>,
    pump: Option<JoinHandle<()>>,
    local_addrs: Vec<SocketAddr>,
}

impl Host {
    pub fn bind(config: HostConfig) -> Result<(Self, SocketAddr), HostError> {
        let adapters = AdapterRegistry::with_configured_agents(DefaultAgentConfig {
            opencode: OpencodeConfig {
                program: config.opencode_program.clone(),
                working_directory: config.agent_working_directory.clone(),
                ..OpencodeConfig::default()
            },
            claude_code: ClaudeCodeConfig {
                program: config.claude_code_program.clone(),
                bundle_dir: config.claude_code_bundle_dir.clone(),
                working_directory: config.agent_working_directory.clone(),
                allow_global_fallback: config.dev_mode,
                ..ClaudeCodeConfig::default()
            },
            codex: CodexConfig {
                program: config.codex_program.clone(),
                working_directory: config.agent_working_directory.clone(),
                // Only a development host may fall back to a globally installed
                // codex-acp; a shipped host must resolve the self-contained
                // bundle next to the host binary.
                allow_global_fallback: config.dev_mode,
                ..CodexConfig::default()
            },
        });
        let supported_agents: Vec<String> = adapters
            .kinds()
            .iter()
            .map(|kind| kind.as_str().to_string())
            .collect();
        let available_agents: Vec<String> = adapters
            .available_kinds()
            .iter()
            .map(|kind| kind.as_str().to_string())
            .collect();
        let matrix = adapters.probe_capability_matrix(&AgentLaunchConfig {
            working_directory: config.agent_working_directory.clone(),
            ..AgentLaunchConfig::default()
        });
        let agent_catalog = matrix
            .entries
            .iter()
            .map(|entry| {
                let (version, config_options, compatible) = match &entry.probe {
                    ProbeOutcome::Compatible(probe) => (
                        probe.agent_version.clone(),
                        probe.config_options.clone(),
                        true,
                    ),
                    _ => (None, Vec::new(), false),
                };
                AgentCatalogEntry {
                    id: entry.kind.as_str().to_string(),
                    display_name: entry.display_name.clone(),
                    version,
                    available: entry.availability.is_available() && compatible,
                    config_options,
                }
            })
            .collect();
        let adapters = Arc::new(adapters);
        if let Some(parent) = config
            .session_store_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        let store = Box::new(SqliteStore::open(
            &config.session_store_path,
            config.journal_limit,
        )?);
        let logger = HostLogger::new(
            &config.log_dir,
            config.log_max_bytes,
            config.log_retained_files,
        )?;
        let sessions = Arc::new(SessionManager::new(adapters, store, config.journal_limit));
        let state = HostState::new(
            config.clone(),
            sessions.clone(),
            supported_agents.clone(),
            agent_catalog,
            logger,
        );
        state.log(
            "info",
            format!("registered agents: {}", supported_agents.join(", ")),
        );
        if available_agents != supported_agents {
            state.log(
                "info",
                format!(
                    "agents missing their executable: {}",
                    supported_agents
                        .iter()
                        .filter(|agent| !available_agents.contains(agent))
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            );
        }
        let authenticator = Arc::new(HostAuthenticator::new(state.clone()));
        let admin = Arc::new(HostAdminImpl::new(state.clone()));
        let shutdown = state.shutdown_handle();
        let gateway_config = GatewayConfig {
            server_name: config.host_name.clone(),
            server_version: config.server_version.clone(),
            host_id: config.server_id.clone(),
            capabilities: ServerCapabilities {
                max_replay_events: Some(config.max_replay_events),
                max_event_journal: Some(config.journal_limit),
                ..ServerCapabilities::default()
            },
            heartbeat_interval_ms: Some(15_000),
            logger: Some({
                let state = state.clone();
                Arc::new(move |level, message| state.log(level, message.to_string()))
            }),
        };
        let mut bind_addrs = Vec::with_capacity(1 + config.additional_binds.len());
        bind_addrs.push(config.bind);
        bind_addrs.extend(config.additional_binds.iter().copied());
        let server = GatewayServer::bind_all(
            gateway_config,
            sessions,
            authenticator,
            admin,
            shutdown,
            &bind_addrs,
        )?;
        let local_addrs = server.local_addrs().to_vec();
        state.set_listener(format_addrs(&local_addrs));
        let primary_addr = local_addrs[0];
        Ok((
            Self {
                config,
                state,
                server: Some(server),
                pump: None,
                local_addrs,
            },
            primary_addr,
        ))
    }

    pub fn serve(&mut self) -> Result<(), HostError> {
        let server = self.server.take().ok_or(HostError::AlreadyServed)?;
        self.state.set_running(true);
        self.state.log(
            "info",
            format!("host listening on {}", format_addrs(&self.local_addrs)),
        );

        let sessions = self.state.sessions();
        let shutdown = self.state.shutdown_handle();
        let pump = std::thread::spawn(move || {
            while !shutdown.load(Ordering::Relaxed) {
                sessions.pump();
                std::thread::sleep(Duration::from_millis(25));
            }
        });
        self.pump = Some(pump);

        let result = server.run();
        self.state.set_running(false);
        if let Some(pump) = self.pump.take() {
            let _ = pump.join();
        }
        result.map_err(HostError::Io)
    }

    pub fn state(&self) -> Arc<HostState> {
        self.state.clone()
    }

    pub fn sessions(&self) -> Arc<SessionManager> {
        self.state.sessions()
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addrs[0]
    }

    pub fn local_addrs(&self) -> &[SocketAddr] {
        &self.local_addrs
    }

    pub fn host_name(&self) -> &str {
        &self.config.host_name
    }

    pub fn shutdown(&self) {
        self.state.shutdown_handle().store(true, Ordering::Relaxed);
    }
}

fn format_addrs(addrs: &[SocketAddr]) -> String {
    addrs
        .iter()
        .map(SocketAddr::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}
