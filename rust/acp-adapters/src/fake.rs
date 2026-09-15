use crate::{AgentAdapter, AgentLaunchConfig};
use acp_types::content::{
    DiffSummary, NormalizedContent, NormalizedToolContent, TerminalRef, ToolLocation,
};
use acp_types::metadata::{
    AcpMode, AcpModeState, AvailableCommandSummary, ListedSessionSummary, PlanEntrySummary,
    PlanSummary, SessionMetadata,
};
use acp_types::session::{AcpSession, AgentEvent, InitializeSummary, ListedSessionPage};
use acp_types::wire::{
    AgentCapabilities, Implementation, SessionCapabilities, SessionListCapabilities,
    SessionResumeCapabilities,
};
use acp_types::{
    AcpCommand, AcpConnection, AcpError, AcpEvent, AgentDescriptor, AgentKind, DiagnosticLevel,
    MessageRole, PermissionOption, PermissionOptionKind, PermissionRequest, ToolCallStatus,
    ToolCallUpdate,
};
use std::collections::VecDeque;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct FakeAgentConfig {
    pub request_permission: bool,
    pub exit_on_cancel: bool,
    pub fail_on_prompt: bool,
}

impl Default for FakeAgentConfig {
    fn default() -> Self {
        Self {
            request_permission: true,
            exit_on_cancel: false,
            fail_on_prompt: false,
        }
    }
}

#[derive(Debug)]
pub struct FakeAgentAdapter {
    config: FakeAgentConfig,
}

impl FakeAgentAdapter {
    pub fn new(config: FakeAgentConfig) -> Self {
        Self { config }
    }
}

impl Default for FakeAgentAdapter {
    fn default() -> Self {
        Self::new(FakeAgentConfig::default())
    }
}

impl AgentAdapter for FakeAgentAdapter {
    fn kind(&self) -> AgentKind {
        AgentKind::Fake
    }

    fn descriptor(&self) -> AgentDescriptor {
        AgentDescriptor::new(AgentKind::Fake, "Fake ACP agent", Some("0.1.0".to_string()))
    }

    fn spawn(&self, _config: &AgentLaunchConfig) -> Result<Box<dyn AcpConnection>, AcpError> {
        Ok(Box::new(FakeAgentConnection::new(self.config.clone())))
    }

    fn spawn_session(&self, _config: &AgentLaunchConfig) -> Result<Box<dyn AcpSession>, AcpError> {
        Ok(Box::new(FakeAgentSession::new(self.config.clone())))
    }
}

// ---------------------------------------------------------------------------
// Legacy `AcpConnection` surface
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct FakeAgentConnection {
    config: FakeAgentConfig,
    queue: VecDeque<AcpEvent>,
    running: bool,
    prompt_count: u64,
}

impl FakeAgentConnection {
    pub fn new(config: FakeAgentConfig) -> Self {
        Self {
            config,
            queue: VecDeque::new(),
            running: true,
            prompt_count: 0,
        }
    }

    fn push(&mut self, event: AcpEvent) {
        self.queue.push_back(event);
    }
}

impl AcpConnection for FakeAgentConnection {
    fn send(&mut self, command: AcpCommand) -> Result<(), AcpError> {
        match command {
            AcpCommand::Prompt { text } => {
                self.prompt_count += 1;
                if self.config.fail_on_prompt {
                    self.push(AcpEvent::Diagnostics {
                        level: DiagnosticLevel::Error,
                        message: "fake agent failed to handle prompt".to_string(),
                    });
                    self.push(AcpEvent::Exited {
                        code: Some(1),
                        reason: "prompt_failure".to_string(),
                    });
                    self.running = false;
                    return Ok(());
                }
                self.push(AcpEvent::Message {
                    role: MessageRole::Assistant,
                    text: format!("echo: {text}"),
                });
                self.push(AcpEvent::ToolCall(ToolCallUpdate {
                    tool_call_id: format!("tool-{}", self.prompt_count),
                    title: "edit_file".to_string(),
                    kind: Some("edit".to_string()),
                    status: ToolCallStatus::InProgress,
                    detail: Some("src/main.rs".to_string()),
                    ..Default::default()
                }));
                if self.config.request_permission {
                    self.push(AcpEvent::PermissionRequest(PermissionRequest {
                        permission_id: format!("perm-{}", self.prompt_count),
                        title: "Allow edit_file?".to_string(),
                        detail: Some("src/main.rs".to_string()),
                        tool_call_id: Some(format!("tool-{}", self.prompt_count)),
                        options: vec![
                            PermissionOption {
                                option_id: "allow".to_string(),
                                label: "Allow".to_string(),
                                kind: PermissionOptionKind::Allow,
                            },
                            PermissionOption {
                                option_id: "deny".to_string(),
                                label: "Deny".to_string(),
                                kind: PermissionOptionKind::Deny,
                            },
                        ],
                    }));
                } else {
                    self.push(AcpEvent::ToolCall(ToolCallUpdate {
                        tool_call_id: format!("tool-{}", self.prompt_count),
                        title: "edit_file".to_string(),
                        kind: Some("edit".to_string()),
                        status: ToolCallStatus::Completed,
                        detail: None,
                        ..Default::default()
                    }));
                }
                Ok(())
            }
            AcpCommand::Cancel => {
                self.push(AcpEvent::Message {
                    role: MessageRole::System,
                    text: "cancelled".to_string(),
                });
                if self.config.exit_on_cancel {
                    self.push(AcpEvent::Exited {
                        code: None,
                        reason: "cancelled".to_string(),
                    });
                    self.running = false;
                }
                Ok(())
            }
            AcpCommand::RespondPermission {
                permission_id,
                option_id,
            } => {
                let tool_call_id = format!("tool-{}", self.prompt_count);
                self.push(AcpEvent::ToolCall(ToolCallUpdate {
                    tool_call_id,
                    title: "edit_file".to_string(),
                    kind: Some("edit".to_string()),
                    status: ToolCallStatus::Completed,
                    detail: Some(format!("{permission_id}:{option_id}")),
                    ..Default::default()
                }));
                self.push(AcpEvent::Message {
                    role: MessageRole::Assistant,
                    text: "done".to_string(),
                });
                Ok(())
            }
        }
    }

    fn try_recv(&mut self) -> Result<Vec<AcpEvent>, AcpError> {
        Ok(self.queue.drain(..).collect())
    }

    fn is_running(&self) -> bool {
        self.running
    }
}

// ---------------------------------------------------------------------------
// Normalized `AcpSession` surface
// ---------------------------------------------------------------------------

/// An in-memory, deterministic [`AcpSession`] for unit tests and development.
///
/// It emits the same normalized [`AgentEvent`] vocabulary as the real
/// `acp_types::session::ClientSession`, so `session-core` exercises one code
/// path whether the agent is fake or on the wire.
#[derive(Debug)]
pub struct FakeAgentSession {
    config: FakeAgentConfig,
    queue: VecDeque<AgentEvent>,
    running: bool,
    prompt_count: u64,
    agent_session_id: Option<String>,
}

/// The deterministic native session id a fresh fake session creates. Recovery
/// tests rely on it so a restarted host can resume a session it created.
pub const FAKE_SESSION_ID: &str = "fake-session-1";

impl FakeAgentSession {
    pub fn new(config: FakeAgentConfig) -> Self {
        Self {
            config,
            queue: VecDeque::new(),
            running: true,
            prompt_count: 0,
            agent_session_id: None,
        }
    }

    fn push(&mut self, event: AgentEvent) {
        self.queue.push_back(event);
    }

    /// Whether the fake agent still owns this native session. A real agent
    /// rejects `session/load` and `session/resume` for ids it cannot find; the
    /// fake mirrors that so session-core can report a stale session.
    fn knows_session(&self, agent_session_id: &str) -> bool {
        self.agent_session_id.as_deref() == Some(agent_session_id)
            || agent_session_id == FAKE_SESSION_ID
            || Self::listed_sessions()
                .iter()
                .any(|listed| listed.agent_session_id == agent_session_id)
    }

    fn session_metadata(&self, agent_session_id: String) -> SessionMetadata {
        SessionMetadata {
            agent_session_id,
            modes: Some(AcpModeState {
                current_mode_id: "auto".to_string(),
                available_modes: vec![
                    AcpMode {
                        id: "none".to_string(),
                        name: "None".to_string(),
                        description: None,
                    },
                    AcpMode {
                        id: "auto".to_string(),
                        name: "Auto".to_string(),
                        description: None,
                    },
                    AcpMode {
                        id: "all".to_string(),
                        name: "All".to_string(),
                        description: None,
                    },
                ],
            }),
            config_options: Vec::new(),
        }
    }

    /// Deterministic sessions the fake agent reports for `session/list`.
    pub fn listed_sessions() -> Vec<ListedSessionSummary> {
        vec![
            ListedSessionSummary {
                agent_session_id: "fake-listed-1".to_string(),
                cwd: "/tmp/slight/fake-project".to_string(),
                additional_directories: Vec::new(),
                title: Some("Prior fake session".to_string()),
                updated_at: Some("2026-01-01T00:00:00Z".to_string()),
            },
            ListedSessionSummary {
                agent_session_id: "fake-listed-2".to_string(),
                cwd: "/tmp/slight/other-project".to_string(),
                additional_directories: Vec::new(),
                title: None,
                updated_at: None,
            },
        ]
    }
}

impl AcpSession for FakeAgentSession {
    fn initialize(&mut self, _client: Implementation) -> Result<InitializeSummary, AcpError> {
        Ok(InitializeSummary {
            protocol_version: 1,
            agent_name: Some("Fake ACP agent".to_string()),
            agent_title: Some("Fake ACP agent".to_string()),
            agent_version: Some("0.1.0".to_string()),
            agent_capabilities: AgentCapabilities::new()
                .load_session(true)
                .session_capabilities(
                    SessionCapabilities::new()
                        .list(SessionListCapabilities::new())
                        .resume(SessionResumeCapabilities::new()),
                ),
            auth_methods: Vec::new(),
        })
    }

    fn new_session(&mut self, _cwd: &Path) -> Result<SessionMetadata, AcpError> {
        let metadata = self.session_metadata(FAKE_SESSION_ID.to_string());
        self.agent_session_id = Some(metadata.agent_session_id.clone());
        Ok(metadata)
    }

    fn list_sessions(
        &mut self,
        _cwd: Option<&Path>,
        _cursor: Option<&str>,
    ) -> Result<ListedSessionPage, AcpError> {
        Ok(ListedSessionPage {
            sessions: Self::listed_sessions(),
            next_cursor: None,
        })
    }

    fn load_session(
        &mut self,
        agent_session_id: &str,
        _cwd: &Path,
    ) -> Result<SessionMetadata, AcpError> {
        if !self.knows_session(agent_session_id) {
            return Err(AcpError::new(format!(
                "unknown session: {agent_session_id}"
            )));
        }
        // Preserve load replay as normalized events: the agent replays prior
        // turns before the load response resolves, exactly like the wire agent.
        self.push(AgentEvent::Message {
            role: MessageRole::User,
            content: NormalizedContent::Text {
                text: "replayed question".to_string(),
            },
        });
        self.push(AgentEvent::Message {
            role: MessageRole::Assistant,
            content: NormalizedContent::Text {
                text: "replayed answer".to_string(),
            },
        });
        self.agent_session_id = Some(agent_session_id.to_string());
        Ok(self.session_metadata(agent_session_id.to_string()))
    }

    fn resume_session(
        &mut self,
        agent_session_id: &str,
        _cwd: &Path,
    ) -> Result<SessionMetadata, AcpError> {
        if !self.knows_session(agent_session_id) {
            return Err(AcpError::new(format!(
                "unknown session: {agent_session_id}"
            )));
        }
        self.agent_session_id = Some(agent_session_id.to_string());
        Ok(self.session_metadata(agent_session_id.to_string()))
    }

    fn prompt(&mut self, text: &str) -> Result<(), AcpError> {
        self.prompt_count += 1;
        if self.config.fail_on_prompt {
            self.push(AgentEvent::Diagnostics {
                level: DiagnosticLevel::Error,
                message: "fake agent failed to handle prompt".to_string(),
            });
            self.push(AgentEvent::Exited {
                code: Some(1),
                reason: "prompt_failure".to_string(),
            });
            self.running = false;
            return Ok(());
        }
        self.push(AgentEvent::Message {
            role: MessageRole::Assistant,
            content: NormalizedContent::Text {
                text: format!("echo: {text}"),
            },
        });
        self.push(AgentEvent::Plan(PlanSummary {
            entries: vec![PlanEntrySummary {
                content: "edit the file".to_string(),
                priority: "high".to_string(),
                status: "in_progress".to_string(),
            }],
        }));
        self.push(AgentEvent::AvailableCommands(vec![
            AvailableCommandSummary {
                name: "compact".to_string(),
                description: "Compact the conversation".to_string(),
                input_hint: None,
            },
        ]));
        self.push(AgentEvent::Mode {
            current_mode_id: "build".to_string(),
        });
        self.push(AgentEvent::ToolCall(ToolCallUpdate {
            tool_call_id: format!("tool-{}", self.prompt_count),
            title: "edit_file".to_string(),
            kind: Some("edit".to_string()),
            status: ToolCallStatus::InProgress,
            detail: Some("src/main.rs".to_string()),
            content: vec![
                NormalizedToolContent::Content(Box::new(NormalizedContent::Text {
                    text: "patching main".to_string(),
                })),
                NormalizedToolContent::Diff(DiffSummary {
                    path: "src/main.rs".to_string(),
                    old_text: Some("fn main() {}".to_string()),
                    new_text: "fn main() { run() }".to_string(),
                }),
                NormalizedToolContent::Terminal(TerminalRef {
                    terminal_id: "term-1".to_string(),
                }),
            ],
            locations: vec![ToolLocation {
                path: "src/main.rs".to_string(),
                line: Some(12),
            }],
            raw_input: Some(serde_json::json!({ "path": "src/main.rs" })),
            raw_output: None,
        }));
        if self.config.request_permission {
            self.push(AgentEvent::PermissionRequest(PermissionRequest {
                permission_id: format!("perm-{}", self.prompt_count),
                title: "Allow edit_file?".to_string(),
                detail: Some("src/main.rs".to_string()),
                tool_call_id: Some(format!("tool-{}", self.prompt_count)),
                options: vec![
                    PermissionOption {
                        option_id: "allow".to_string(),
                        label: "Allow".to_string(),
                        kind: PermissionOptionKind::Allow,
                    },
                    PermissionOption {
                        option_id: "deny".to_string(),
                        label: "Deny".to_string(),
                        kind: PermissionOptionKind::Deny,
                    },
                ],
            }));
        } else {
            self.push(AgentEvent::ToolCall(ToolCallUpdate {
                tool_call_id: format!("tool-{}", self.prompt_count),
                title: "edit_file".to_string(),
                kind: Some("edit".to_string()),
                status: ToolCallStatus::Completed,
                detail: None,
                ..Default::default()
            }));
        }
        Ok(())
    }

    fn cancel(&mut self) -> Result<(), AcpError> {
        self.push(AgentEvent::Message {
            role: MessageRole::System,
            content: NormalizedContent::Text {
                text: "cancelled".to_string(),
            },
        });
        if self.config.exit_on_cancel {
            self.push(AgentEvent::Exited {
                code: None,
                reason: "cancelled".to_string(),
            });
            self.running = false;
        }
        Ok(())
    }

    fn respond_permission(&mut self, option_id: &str) -> Result<(), AcpError> {
        let tool_call_id = format!("tool-{}", self.prompt_count);
        self.push(AgentEvent::ToolCall(ToolCallUpdate {
            tool_call_id,
            title: "edit_file".to_string(),
            kind: Some("edit".to_string()),
            status: ToolCallStatus::Completed,
            detail: Some(option_id.to_string()),
            ..Default::default()
        }));
        self.push(AgentEvent::Message {
            role: MessageRole::Assistant,
            content: NormalizedContent::Text {
                text: "done".to_string(),
            },
        });
        Ok(())
    }

    fn set_mode(&mut self, mode_id: &str) -> Result<(), AcpError> {
        self.push(AgentEvent::Mode {
            current_mode_id: mode_id.to_string(),
        });
        Ok(())
    }

    fn drain(&mut self) -> Vec<AgentEvent> {
        self.queue.drain(..).collect()
    }

    fn is_running(&mut self) -> bool {
        self.running
    }
}
