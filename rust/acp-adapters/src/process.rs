//! Subprocess lifecycle for ACP agents.
//!
//! ACP agents are plain child processes that speak JSON-RPC over stdin/stdout.
//! This module launches one and hands back the two independent halves the rest
//! of the host needs:
//!
//! - [`AgentProcess`] owns the OS process (id, wait, kill), and
//! - [`ChildTransport`] is the newline-delimited [`Transport`] over the child's
//!   `stdout`/`stdin`.
//!
//! Splitting them lets the connection layer own the pipes while the host still
//! supervises the process. Nothing here knows about ACP methods; adapter
//! launch configuration is layered on top.

use acp_types::transport::Transport;
use acp_types::AcpError;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};

/// The stdio transport attached to a spawned ACP agent.
pub type ChildTransport = Transport<BufReader<ChildStdout>, BufWriter<ChildStdin>>;

/// How to locate and launch an ACP agent.
#[derive(Debug, Clone)]
pub struct ChildSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub working_directory: Option<PathBuf>,
    pub environment: Vec<(String, String)>,
    /// When false (the default) the child's stderr is discarded so it cannot
    /// fill its pipe buffer and deadlock the agent.
    pub inherit_stderr: bool,
    /// When true the child inherits only the environment in `environment`
    /// instead of the host's full environment. Adapters use this to avoid
    /// leaking unrelated host variables into an agent process.
    pub env_clear: bool,
    /// When true the child's stderr is piped back to the host for diagnostics
    /// instead of being inherited or discarded. The caller is responsible for
    /// draining the returned [`ChildStderr`]. Takes precedence over
    /// `inherit_stderr`.
    pub capture_stderr: bool,
}

impl ChildSpec {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            working_directory: None,
            environment: Vec::new(),
            inherit_stderr: false,
            env_clear: false,
            capture_stderr: false,
        }
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    pub fn working_directory(mut self, directory: impl Into<PathBuf>) -> Self {
        self.working_directory = Some(directory.into());
        self
    }

    pub fn environment(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment.push((key.into(), value.into()));
        self
    }

    /// Clears the inherited environment before applying `environment`.
    pub fn env_clear(mut self) -> Self {
        self.env_clear = true;
        self
    }

    /// Pipes the child's stderr back to the host for diagnostics.
    pub fn capture_stderr(mut self) -> Self {
        self.capture_stderr = true;
        self
    }
}

/// A spawned agent process together with its stdio transport.
pub struct SpawnedChild {
    pub process: AgentProcess,
    pub transport: ChildTransport,
    /// Present only when [`ChildSpec::capture_stderr`] was set.
    pub stderr: Option<ChildStderr>,
}

/// A running ACP agent process.
#[derive(Debug)]
pub struct AgentProcess {
    child: Child,
}

impl AgentProcess {
    /// The OS process id.
    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// Returns the exit status if the process has already exited.
    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>, AcpError> {
        Ok(self.child.try_wait()?)
    }

    pub fn is_running(&mut self) -> Result<bool, AcpError> {
        Ok(self.try_wait()?.is_none())
    }

    /// Blocks until the process exits.
    pub fn wait(&mut self) -> Result<ExitStatus, AcpError> {
        Ok(self.child.wait()?)
    }

    /// Kills the process. Killing an already-exited process is not an error.
    pub fn kill(&mut self) -> Result<(), AcpError> {
        match self.child.kill() {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

impl Drop for AgentProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Spawns an ACP agent and returns its process handle and stdio transport.
pub fn spawn(spec: &ChildSpec) -> Result<SpawnedChild, AcpError> {
    let mut command = Command::new(&spec.program);
    let stderr = if spec.capture_stderr {
        Stdio::piped()
    } else if spec.inherit_stderr {
        Stdio::inherit()
    } else {
        Stdio::null()
    };
    command
        .args(&spec.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(stderr);
    if let Some(directory) = &spec.working_directory {
        command.current_dir(directory);
    }
    if spec.env_clear {
        command.env_clear();
    }
    for (key, value) in &spec.environment {
        command.env(key, value);
    }

    let mut child = command.spawn()?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| AcpError::new("agent process has no stdin pipe"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AcpError::new("agent process has no stdout pipe"))?;
    let stderr = if spec.capture_stderr {
        child.stderr.take()
    } else {
        None
    };
    let transport = Transport::new(BufReader::new(stdout), BufWriter::new(stdin));

    Ok(SpawnedChild {
        process: AgentProcess { child },
        transport,
        stderr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_builder_records_launch_configuration() {
        let spec = ChildSpec::new("/bin/echo")
            .args(["--help"])
            .working_directory("/tmp")
            .environment("FOO", "bar");
        assert_eq!(spec.program, PathBuf::from("/bin/echo"));
        assert_eq!(spec.args, vec!["--help".to_string()]);
        assert_eq!(spec.working_directory, Some(PathBuf::from("/tmp")));
        assert_eq!(
            spec.environment,
            vec![("FOO".to_string(), "bar".to_string())]
        );
        assert!(!spec.env_clear);
        assert!(!spec.capture_stderr);
    }

    #[test]
    fn env_clear_removes_inherited_variables() {
        std::env::set_var("SLIGHT_PROCESS_TEST_INHERITED", "leaked");
        let spec = ChildSpec::new("/bin/sh")
            .args([
                "-c",
                "printf 'inherited=%s set=%s' \"$SLIGHT_PROCESS_TEST_INHERITED\" \"$SLIGHT_PROCESS_TEST_SET\"",
            ])
            .env_clear()
            .environment("SLIGHT_PROCESS_TEST_SET", "set");
        let mut spawned = spawn(&spec).unwrap();
        let output = spawned.transport.recv_line().unwrap().unwrap();
        assert_eq!(output, "inherited= set=set");
        assert!(spawned.process.wait().unwrap().success());
    }

    #[test]
    fn capture_stderr_returns_the_pipe() {
        let spec = ChildSpec::new("/bin/sh")
            .args(["-c", "echo oops >&2"])
            .capture_stderr();
        let mut spawned = spawn(&spec).unwrap();
        let mut stderr = spawned.stderr.take().expect("stderr is captured");
        let mut output = String::new();
        use std::io::Read;
        stderr.read_to_string(&mut output).unwrap();
        assert!(output.contains("oops"));
        assert!(spawned.process.wait().unwrap().success());
    }

    #[test]
    fn spawns_a_process_and_reports_exit() {
        let spec = ChildSpec::new("/bin/sh").args(["-c", "exit 0"]);
        let mut spawned = spawn(&spec).unwrap();
        let status = spawned.process.wait().unwrap();
        assert!(status.success());
    }

    #[test]
    fn spawn_of_missing_program_errors() {
        let spec = ChildSpec::new("/definitely/not/a/real/binary");
        assert!(spawn(&spec).is_err());
    }
}
