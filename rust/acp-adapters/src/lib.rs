pub mod claude_code;
pub mod codex;
pub mod fake;
pub mod matrix;
pub mod opencode;
pub mod process;
pub mod stdio;
pub mod wire_agent;

use acp_types::session::{AcpSession, LegacyAcpSession};
use acp_types::{AcpConnection, AcpError, AgentDescriptor, AgentKind};
use matrix::{AdapterCapability, CapabilityMatrix};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Default)]
pub struct AgentLaunchConfig {
    pub working_directory: Option<PathBuf>,
    pub environment: Vec<(String, String)>,
    pub initial_prompt: Option<String>,
    pub raw_args: Vec<String>,
}

pub trait AgentAdapter: Send + Sync {
    fn kind(&self) -> AgentKind;
    fn descriptor(&self) -> AgentDescriptor;

    /// Legacy launch path that exposes the older `AcpConnection` command/event
    /// surface. Adapters that have migrated to the normalized session boundary
    /// should override [`AgentAdapter::spawn_session`] instead.
    fn spawn(&self, config: &AgentLaunchConfig) -> Result<Box<dyn AcpConnection>, AcpError>;

    /// Launch path that exposes the normalized [`AcpSession`] boundary used by
    /// `session-core`. The default wraps the legacy [`AcpConnection`] so
    /// adapters can migrate one at a time; override it to drive the ACP client
    /// directly.
    fn spawn_session(&self, config: &AgentLaunchConfig) -> Result<Box<dyn AcpSession>, AcpError> {
        let connection = self.spawn(config)?;
        Ok(Box::new(LegacyAcpSession::new(connection)))
    }

    /// Whether the executable this adapter launches can be located on this
    /// host. Adapters that always work (for example the in-memory fake) use the
    /// default.
    fn is_available(&self) -> bool {
        true
    }

    /// Static launch/auth/support facts used to build the capability matrix.
    ///
    /// The default is intentionally conservative: it derives availability from
    /// [`AgentAdapter::is_available`] and leaves launch details empty. Real
    /// adapters override it so the matrix names their executable, arguments,
    /// and auth path.
    fn capability(&self) -> AdapterCapability {
        AdapterCapability::placeholder(
            self.kind(),
            self.descriptor().display_name,
            self.is_available(),
        )
    }
}

/// Per-adapter configuration for the adapters registered by
/// [`AdapterRegistry::with_default_agents`].
#[derive(Debug, Clone, Default)]
pub struct DefaultAgentConfig {
    pub opencode: opencode::OpencodeConfig,
    pub claude_code: claude_code::ClaudeCodeConfig,
    pub codex: codex::CodexConfig,
}

#[derive(Default)]
pub struct AdapterRegistry {
    adapters: Vec<Arc<dyn AgentAdapter>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(fake::FakeAgentAdapter::default()));
        registry
    }

    /// Real agents this build knows how to launch. Adapters are registered
    /// regardless of whether their executable is present;
    /// callers consult [`AdapterRegistry::available_kinds`] or
    /// [`AdapterRegistry::capability_matrix`] before advertising them to
    /// clients.
    pub fn with_default_agents() -> Self {
        Self::with_configured_agents(DefaultAgentConfig::default())
    }

    /// Real agents with per-adapter launch configuration.
    pub fn with_configured_agents(config: DefaultAgentConfig) -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(opencode::OpencodeAdapter::new(config.opencode)));
        registry.register(Arc::new(claude_code::ClaudeCodeAdapter::new(
            config.claude_code,
        )));
        registry.register(Arc::new(codex::CodexAdapter::new(config.codex)));
        registry
    }

    pub fn register(&mut self, adapter: Arc<dyn AgentAdapter>) {
        self.adapters.push(adapter);
    }

    pub fn get(&self, kind: &AgentKind) -> Option<Arc<dyn AgentAdapter>> {
        self.adapters
            .iter()
            .find(|adapter| &adapter.kind() == kind)
            .cloned()
    }

    /// Every registered adapter kind, including ones whose executable is
    /// missing.
    pub fn kinds(&self) -> Vec<AgentKind> {
        self.adapters.iter().map(|adapter| adapter.kind()).collect()
    }

    /// Registered adapter kinds whose executable is present on this host.
    pub fn available_kinds(&self) -> Vec<AgentKind> {
        self.adapters
            .iter()
            .filter(|adapter| adapter.is_available())
            .map(|adapter| adapter.kind())
            .collect()
    }

    /// Static capability report for every registered adapter, without probing.
    pub fn capability_matrix(&self) -> CapabilityMatrix {
        CapabilityMatrix {
            generated_at_ms: now_ms(),
            entries: self
                .adapters
                .iter()
                .map(|adapter| adapter.capability())
                .collect(),
        }
    }

    /// Capability report that additionally negotiates ACP `initialize` against
    /// every available adapter. Missing executables and failed handshakes are
    /// recorded as explicit outcomes rather than treated as support.
    pub fn probe_capability_matrix(&self, launch: &AgentLaunchConfig) -> CapabilityMatrix {
        let mut matrix = self.capability_matrix();
        for (adapter, entry) in self.adapters.iter().zip(matrix.entries.iter_mut()) {
            entry.probe = matrix::probe_adapter(adapter.as_ref(), launch);
        }
        matrix
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}
