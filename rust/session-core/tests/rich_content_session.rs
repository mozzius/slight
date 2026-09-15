//! End-to-end preservation of rich ACP content and tool updates.
//!
//! Drives `session-core` with the wire-level fake agent, which emits an image
//! message chunk and a tool call carrying content blocks, a file diff, a
//! terminal reference, file locations, and raw input/output. The test asserts
//! those survive normalization into `session.tool_call` and `session.message`
//! gateway payloads.

use acp_adapters::wire_agent::{spawn_in_process_agent, FakeWireAgentConfig};
use acp_adapters::{AdapterRegistry, AgentAdapter, AgentLaunchConfig};
use acp_types::session::{AcpSession, ClientSession};
use acp_types::{AcpError, AgentDescriptor, AgentKind};
use session_core::{
    ContentKind, CreateSessionRequest, SessionDetail, SessionEvent, SessionManager,
    ToolCallContentPayload,
};
use session_store::InMemoryStore;
use std::sync::Arc;
use std::time::{Duration, Instant};

struct WireFakeAdapter {
    config: FakeWireAgentConfig,
    kind: AgentKind,
}

impl AgentAdapter for WireFakeAdapter {
    fn kind(&self) -> AgentKind {
        self.kind.clone()
    }

    fn descriptor(&self) -> AgentDescriptor {
        AgentDescriptor::new(
            self.kind.clone(),
            "Wire fake agent",
            Some("0.1.0".to_string()),
        )
    }

    fn spawn(
        &self,
        _config: &AgentLaunchConfig,
    ) -> Result<Box<dyn acp_types::AcpConnection>, AcpError> {
        Err(AcpError::new("wire adapter uses the normalized boundary"))
    }

    fn spawn_session(&self, _config: &AgentLaunchConfig) -> Result<Box<dyn AcpSession>, AcpError> {
        let (transport, handle) = spawn_in_process_agent(self.config.clone())
            .map_err(|error| AcpError::new(error.to_string()))?;
        std::thread::spawn(move || {
            let _ = handle.join();
        });
        Ok(Box::new(ClientSession::new(transport)))
    }
}

fn manager() -> Arc<SessionManager> {
    let kind = AgentKind::Custom("wire-rich".to_string());
    let mut registry = AdapterRegistry::new();
    registry.register(Arc::new(WireFakeAdapter {
        config: FakeWireAgentConfig::default(),
        kind,
    }));
    Arc::new(SessionManager::new(
        Arc::new(registry),
        Box::new(InMemoryStore::new()),
        128,
    ))
}

fn pump_until(
    manager: &SessionManager,
    id: &str,
    predicate: impl Fn(&SessionDetail) -> bool,
) -> SessionDetail {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        manager.pump();
        if let Ok(detail) = manager.inspect_session(id) {
            if predicate(&detail) {
                return detail;
            }
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for the expected rich event"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn rich_tool_call_and_message_content_are_preserved() {
    let manager = manager();
    let id = manager
        .create_session(CreateSessionRequest {
            agent: AgentKind::Custom("wire-rich".to_string()),
            model: Some("auto".to_string()),
            effort: Some("medium".to_string()),
            working_directory_label: "~/work/slight".to_string(),
            initial_prompt: Some("hello".to_string()),
        })
        .expect("session creates")
        .id;

    let detail = pump_until(&manager, &id, |detail| {
        detail.recent_events.iter().any(|sequenced| {
            matches!(&sequenced.event, SessionEvent::ToolCall(payload) if !payload.content.is_empty())
        })
    });

    let tool_call = detail
        .recent_events
        .iter()
        .find_map(|sequenced| match &sequenced.event {
            SessionEvent::ToolCall(payload) if !payload.content.is_empty() => Some(payload),
            _ => None,
        })
        .expect("rich tool call present");

    assert_eq!(tool_call.content.len(), 3);
    assert!(tool_call.content.iter().any(|content| matches!(
        content,
        ToolCallContentPayload::Content(block) if block.kind == ContentKind::Text && block.text == "patching main.rs"
    )));
    assert!(tool_call.content.iter().any(|content| matches!(
        content,
        ToolCallContentPayload::Diff { path, old_text, .. }
            if path == "/tmp/work/src/main.rs" && old_text.is_some()
    )));
    assert!(tool_call.content.iter().any(|content| matches!(
        content,
        ToolCallContentPayload::Terminal { terminal_id } if terminal_id == "term-1"
    )));
    assert!(tool_call
        .locations
        .iter()
        .any(|location| { location.path == "/tmp/work/src/main.rs" && location.line == Some(12) }));
    assert_eq!(tool_call.raw_output.as_ref().unwrap()["exit"], 0);
    assert!(!tool_call.raw_input.is_none());
}

#[test]
fn rich_media_blocks_survive_message_normalization() {
    let manager = manager();
    let id = manager
        .create_session(CreateSessionRequest {
            agent: AgentKind::Custom("wire-rich".to_string()),
            model: Some("auto".to_string()),
            effort: Some("medium".to_string()),
            working_directory_label: "~/work/slight".to_string(),
            initial_prompt: Some("hello".to_string()),
        })
        .expect("session creates")
        .id;

    let detail = pump_until(&manager, &id, |detail| {
        detail.recent_events.iter().any(|sequenced| {
            matches!(
                &sequenced.event,
                SessionEvent::Message(message)
                    if message.blocks.iter().any(|block| block.kind == ContentKind::Image)
            )
        })
    });

    let image = detail
        .recent_events
        .iter()
        .find_map(|sequenced| match &sequenced.event {
            SessionEvent::Message(message) => message
                .blocks
                .iter()
                .find(|block| block.kind == ContentKind::Image),
            _ => None,
        })
        .expect("image block present");
    assert_eq!(image.mime_type.as_deref(), Some("image/png"));
    assert_eq!(image.data_base64.as_deref(), Some("aGVsbG8="));
    assert_eq!(image.uri.as_deref(), Some("file:///tmp/rich.png"));
}
