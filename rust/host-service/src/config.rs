use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct HostConfig {
    pub bind: SocketAddr,
    /// Additional narrowly-scoped addresses served by the same host runtime.
    pub additional_binds: Vec<SocketAddr>,
    pub host_name: String,
    pub server_id: String,
    pub server_version: String,
    pub dev_mode: bool,
    pub dev_token: Option<String>,
    pub journal_limit: usize,
    pub max_replay_events: usize,
    pub pairing_ttl_secs: u64,
    /// Explicit path to the OpenCode executable. When `None`, the host searches
    /// `SLIGHT_OPENCODE_BIN` and `PATH`.
    pub opencode_program: Option<PathBuf>,
    /// Explicit path to the Claude Code ACP broker. When `None`, the host
    /// searches the bundled runtime, `SLIGHT_CLAUDE_CODE_BIN`, and optionally
    /// `PATH` in development mode.
    pub claude_code_program: Option<PathBuf>,
    pub claude_code_bundle_dir: Option<PathBuf>,
    /// Explicit path to the Codex ACP broker. When `None`, the host searches
    /// `SLIGHT_CODEX_BIN` and `PATH`.
    pub codex_program: Option<PathBuf>,
    /// Default working directory for agent sessions that do not specify one.
    pub agent_working_directory: Option<PathBuf>,
    /// SQLite database used for session metadata and the bounded event journal.
    pub session_store_path: PathBuf,
    /// Directory containing the host log and rotated files.
    pub log_dir: PathBuf,
    /// Maximum size of the active log before rotation.
    pub log_max_bytes: u64,
    /// Total number of retained files, including the active log.
    pub log_retained_files: usize,
}

fn default_session_store_path() -> PathBuf {
    std::env::var_os("SLIGHT_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".slight")
        })
        .join("sessions.sqlite")
}

fn default_data_dir() -> PathBuf {
    std::env::var_os("SLIGHT_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".slight")))
        .unwrap_or_else(|| PathBuf::from(".slight"))
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8787".parse().expect("valid loopback addr"),
            additional_binds: Vec::new(),
            host_name: "slight-host".to_string(),
            server_id: "slight-local".to_string(),
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            dev_mode: true,
            dev_token: Some("dev".to_string()),
            journal_limit: 1024,
            max_replay_events: 256,
            pairing_ttl_secs: 300,
            opencode_program: None,
            claude_code_program: None,
            claude_code_bundle_dir: None,
            codex_program: None,
            agent_working_directory: None,
            session_store_path: default_session_store_path(),
            log_dir: default_data_dir().join("logs"),
            log_max_bytes: 10 * 1024 * 1024,
            log_retained_files: 5,
        }
    }
}
