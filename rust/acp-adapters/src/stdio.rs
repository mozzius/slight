//! Shared stdio launch and session plumbing for real ACP agents.
//!
//! Real agents differ in executable name, arguments, and credentials, but they
//! all share one shape: a [`ChildSpec`] over stdin/stdout handed to the
//! agent-neutral [`ClientSession`] from `acp-types::session`. This module owns
//! that shared shape so each adapter only supplies its launch/auth quirks.
//!
//! Nothing here is agent-specific. Adapters compose these helpers and add their
//! own executable name, environment variable, base arguments, and diagnostics.

use crate::process::{self, AgentProcess, ChildSpec};
use acp_types::session::{AcpSession, AgentEvent, ClientSession, InitializeSummary};
use acp_types::wire::Implementation;
use acp_types::{metadata, AcpError};
use std::collections::VecDeque;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::ChildStderr;
use std::sync::{Arc, Mutex};
use std::thread;

/// How many stderr lines an adapter retains for diagnostics.
pub const STDERR_TAIL_LIMIT: usize = 200;

/// Environment variables copied into a real agent by default. Everything else
/// is dropped (`env_clear`), so an agent process does not inherit unrelated
/// host variables such as provider tokens. Adapters and callers add
/// credentials explicitly through their config or [`crate::AgentLaunchConfig`].
pub const DEFAULT_ENV_ALLOWLIST: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "TMPDIR",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "TERM",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "XDG_STATE_HOME",
    "XDG_RUNTIME_DIR",
];

/// Locates an executable for an adapter.
///
/// Resolution order is: the explicit configured path (which may itself be a
/// bare name to search on `PATH`), then `env_var`, then `executable` on `PATH`.
/// `hint` is appended to the error so each adapter can name itself and the
/// escape hatch.
pub fn resolve_program(
    program: Option<&Path>,
    env_var: &str,
    executable: &str,
    hint: &str,
) -> Result<PathBuf, AcpError> {
    if let Some(program) = program {
        return resolve_candidate(program, hint);
    }
    if let Some(program) = std::env::var_os(env_var) {
        return resolve_candidate(Path::new(&program), hint);
    }
    which(executable).ok_or_else(|| missing_executable_error(executable, env_var, hint))
}

/// Resolves a configured path, accepting either an absolute/relative path or a
/// bare executable name to search on `PATH`.
pub fn resolve_candidate(path: &Path, hint: &str) -> Result<PathBuf, AcpError> {
    let is_bare_name = path.components().count() == 1 && !path.is_absolute();
    if is_bare_name {
        let name = path.to_string_lossy();
        return which(&name).ok_or_else(|| {
            AcpError::new(format!(
                "executable `{name}` was not found; check the configured path or PATH. {hint}"
            ))
        });
    }
    if is_executable_file(path) {
        Ok(path.to_path_buf())
    } else {
        Err(AcpError::new(format!(
            "executable `{}` does not exist or is not a file. {hint}",
            path.display()
        )))
    }
}

/// Searches `PATH` for `name`, honoring `PATHEXT` on Windows.
pub fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path) {
        for candidate in executable_names(name) {
            let full = directory.join(candidate);
            if is_executable_file(&full) {
                return Some(full);
            }
        }
    }
    None
}

fn executable_names(name: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        let mut names = vec![name.to_string()];
        if let Ok(extensions) = std::env::var("PATHEXT") {
            for extension in extensions.split(';').filter(|ext| !ext.is_empty()) {
                names.push(format!("{name}{extension}"));
            }
        }
        names
    }
    #[cfg(not(windows))]
    {
        vec![name.to_string()]
    }
}

pub fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

fn missing_executable_error(executable: &str, env_var: &str, hint: &str) -> AcpError {
    AcpError::new(format!(
        "could not find the `{executable}` executable; install it, add it to PATH, or set {env_var}. {hint}"
    ))
}

/// Builds the sanitized child environment: a portable allowlist of host
/// variables, then adapter-specific and caller-provided overrides.
pub fn sanitized_environment(
    allowlist: &[&str],
    config_environment: &[(String, String)],
    launch_environment: &[(String, String)],
) -> Vec<(String, String)> {
    let mut environment: Vec<(String, String)> = Vec::new();
    for key in allowlist {
        if let Ok(value) = std::env::var(key) {
            upsert(&mut environment, (*key).to_string(), value);
        }
    }
    for (key, value) in config_environment {
        upsert(&mut environment, key.clone(), value.clone());
    }
    for (key, value) in launch_environment {
        upsert(&mut environment, key.clone(), value.clone());
    }
    environment
}

fn upsert(environment: &mut Vec<(String, String)>, key: String, value: String) {
    if let Some(existing) = environment
        .iter_mut()
        .find(|(existing, _)| existing == &key)
    {
        existing.1 = value;
    } else {
        environment.push((key, value));
    }
}

/// A live stdio ACP agent plus its normalized [`AcpSession`].
///
/// Dropping the session kills the child process before the inner
/// [`ClientSession`] shuts down its worker, so a prompt blocked on the wire
/// cannot keep the host waiting.
pub struct StdioSession {
    process: AgentProcess,
    inner: ClientSession,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
}

impl StdioSession {
    /// The most recent lines captured from the child's stderr.
    pub fn stderr_tail(&self) -> Vec<String> {
        self.stderr_tail
            .lock()
            .map(|lines| lines.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// The child process id.
    pub fn process_id(&self) -> u32 {
        self.process.id()
    }
}

impl AcpSession for StdioSession {
    fn initialize(&mut self, client: Implementation) -> Result<InitializeSummary, AcpError> {
        self.inner.initialize(client)
    }

    fn new_session(&mut self, cwd: &Path) -> Result<metadata::SessionMetadata, AcpError> {
        self.inner.new_session(cwd)
    }

    fn list_sessions(
        &mut self,
        cwd: Option<&Path>,
        cursor: Option<&str>,
    ) -> Result<acp_types::session::ListedSessionPage, AcpError> {
        self.inner.list_sessions(cwd, cursor)
    }

    fn load_session(
        &mut self,
        agent_session_id: &str,
        cwd: &Path,
    ) -> Result<metadata::SessionMetadata, AcpError> {
        self.inner.load_session(agent_session_id, cwd)
    }

    fn resume_session(
        &mut self,
        agent_session_id: &str,
        cwd: &Path,
    ) -> Result<metadata::SessionMetadata, AcpError> {
        self.inner.resume_session(agent_session_id, cwd)
    }

    fn prompt(&mut self, text: &str) -> Result<(), AcpError> {
        self.inner.prompt(text)
    }

    fn cancel(&mut self) -> Result<(), AcpError> {
        self.inner.cancel()
    }

    fn respond_permission(&mut self, option_id: &str) -> Result<(), AcpError> {
        self.inner.respond_permission(option_id)
    }

    fn set_mode(&mut self, mode_id: &str) -> Result<(), AcpError> {
        self.inner.set_mode(mode_id)
    }

    fn set_config_option(
        &mut self,
        config_id: &str,
        value_id: &str,
    ) -> Result<Vec<acp_types::metadata::AcpConfigOption>, AcpError> {
        self.inner.set_config_option(config_id, value_id)
    }

    fn drain(&mut self) -> Vec<AgentEvent> {
        self.inner.drain()
    }

    fn is_running(&mut self) -> bool {
        self.inner.is_running()
    }
}

/// Spawns a stdio ACP agent and wraps its transport in a normalized session.
pub fn spawn_session(spec: ChildSpec) -> Result<StdioSession, AcpError> {
    let process::SpawnedChild {
        process,
        transport,
        stderr,
    } = process::spawn(&spec)?;

    let stderr_tail = Arc::new(Mutex::new(VecDeque::new()));
    if let Some(stderr) = stderr {
        spawn_stderr_reader(stderr, Arc::clone(&stderr_tail));
    }

    Ok(StdioSession {
        process,
        inner: ClientSession::new(transport),
        stderr_tail,
    })
}

/// Drains the child's stderr into a bounded in-memory tail so a later
/// diagnostic can explain a failed launch without streaming raw agent logs
/// into the session transcript.
fn spawn_stderr_reader(stderr: ChildStderr, tail: Arc<Mutex<VecDeque<String>>>) {
    thread::spawn(move || {
        let reader = std::io::BufReader::new(stderr);
        for line in reader.lines() {
            let Ok(line) = line else { break };
            if let Ok(mut tail) = tail.lock() {
                tail.push_back(line);
                while tail.len() > STDERR_TAIL_LIMIT {
                    tail.pop_front();
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overrides_apply_after_allowlist() {
        let environment = sanitized_environment(
            &["PATH"],
            &[("SLIGHT_TEST_KEY".to_string(), "from-config".to_string())],
            &[
                ("SLIGHT_TEST_KEY".to_string(), "from-launch".to_string()),
                ("ANTHROPIC_API_KEY".to_string(), "secret".to_string()),
            ],
        );
        let find = |key: &str| {
            environment
                .iter()
                .find(|(candidate, _)| candidate == key)
                .map(|(_, value)| value.clone())
        };
        assert_eq!(find("SLIGHT_TEST_KEY").as_deref(), Some("from-launch"));
        assert_eq!(find("ANTHROPIC_API_KEY").as_deref(), Some("secret"));
        assert_eq!(find("SLIGHT_ENSURE_ABSENT"), None);
    }

    #[test]
    fn missing_explicit_program_reports_hint() {
        let error = resolve_program(
            Some(Path::new("/definitely/not/a/real/agent")),
            "SLIGHT_TEST_BIN",
            "test-agent",
            "Install test-agent.",
        )
        .unwrap_err();
        assert!(error.to_string().contains("does not exist"));
        assert!(error.to_string().contains("Install test-agent."));
    }

    #[test]
    fn missing_default_executable_reports_env_var() {
        let error = resolve_program(
            None,
            "SLIGHT_DEFINITELY_UNSET_BIN",
            "definitely-not-a-real-agent-xyz",
            "Install it.",
        )
        .unwrap_err();
        assert!(error.to_string().contains("SLIGHT_DEFINITELY_UNSET_BIN"));
    }
}
