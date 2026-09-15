use acp_adapters::AdapterRegistry;
use acp_types::AgentKind;
use session_core::{
    CreateSessionRequest, SessionError, SessionEvent, SessionManager, SessionStatus,
};
use session_store::InMemoryStore;
use std::sync::Arc;

fn manager(journal_limit: usize) -> Arc<SessionManager> {
    Arc::new(SessionManager::new(
        Arc::new(AdapterRegistry::with_builtins()),
        Box::new(InMemoryStore::new()),
        journal_limit,
    ))
}

fn create(manager: &SessionManager, prompt: Option<&str>) -> String {
    manager
        .create_session(CreateSessionRequest {
            agent: AgentKind::Fake,
            model: None,
            effort: None,
            working_directory_label: "~/work/slight".to_string(),
            initial_prompt: prompt.map(str::to_string),
        })
        .expect("session creates")
        .id
}

#[test]
fn create_with_prompt_reaches_permission() {
    let manager = manager(64);
    let id = create(&manager, Some("hello"));
    let detail = manager.inspect_session(&id).expect("session exists");
    assert_eq!(detail.summary.status, SessionStatus::WaitingPermission);
    assert!(detail.pending_permission.is_some());
    assert!(detail.summary.last_sequence >= 5);
    assert!(detail.running);
}

#[test]
fn archiving_hides_nothing_from_storage_and_can_be_reversed() {
    let manager = manager(64);
    let id = create(&manager, None);

    let archived = manager.archive_session(&id, true).expect("archive works");
    assert!(archived.archived);
    assert_eq!(manager.list_sessions()[0].id, id);
    assert!(manager.inspect_session(&id).unwrap().summary.archived);

    let restored = manager
        .archive_session(&id, false)
        .expect("unarchive works");
    assert!(!restored.archived);
}

#[test]
fn respond_permission_clears_pending_and_resumes() {
    let manager = manager(64);
    let id = create(&manager, Some("hello"));
    let pending = manager
        .inspect_session(&id)
        .unwrap()
        .pending_permission
        .unwrap();
    manager
        .respond_permission(&id, &pending.id, "allow")
        .expect("permission response accepted");
    let detail = manager.inspect_session(&id).unwrap();
    assert!(detail.pending_permission.is_none());
    assert_eq!(detail.summary.status, SessionStatus::Working);
}

#[test]
fn attach_replays_journal() {
    let manager = manager(64);
    let id = create(&manager, Some("hello"));
    let attached = manager.attach(&id, None).expect("attach works");
    assert!(!attached.replayed.is_empty());
    assert!(!attached.resync_required);
    assert_eq!(attached.latest_sequence, attached.summary.last_sequence);
    assert_eq!(
        attached.replayed.last().unwrap().sequence,
        attached.latest_sequence
    );
}

#[test]
fn history_returns_bounded_pages_from_newest_backwards() {
    let manager = manager(64);
    let id = create(&manager, Some("hello"));
    let attached = manager.attach(&id, None).expect("attach works");
    let newest = manager
        .history(&id, Some(attached.latest_sequence + 1), 1)
        .expect("history works");
    assert_eq!(newest.events.len(), 1);
    assert!(newest.has_more);

    let older = manager
        .history(&id, Some(newest.events[0].sequence), 1)
        .expect("older history works");
    assert_eq!(older.events.len(), 1);
    assert!(older.events[0].sequence < newest.events[0].sequence);
}

#[test]
fn attach_reports_resync_when_history_evicted() {
    let manager = manager(2);
    let id = create(&manager, Some("hello"));
    let attached = manager.attach(&id, Some(0)).expect("attach works");
    assert!(attached.resync_required);
    assert_eq!(
        attached.oldest_available_sequence,
        Some(attached.replayed[0].sequence)
    );
}

#[test]
fn subscriptions_receive_events() {
    let manager = manager(64);
    let id = create(&manager, Some("hello"));
    let receiver = manager.subscribe(&id).expect("subscribe works");
    manager.send_input(&id, "more").expect("input accepted");
    let mut received = 0;
    while receiver.try_recv().is_ok() {
        received += 1;
    }
    assert!(received > 0);
}

#[test]
fn input_is_persisted_as_a_user_message() {
    let manager = manager(64);
    let id = create(&manager, None);

    manager
        .send_input(&id, "remember this prompt")
        .expect("input accepted");

    let detail = manager.inspect_session(&id).expect("session exists");
    assert!(detail.recent_events.iter().any(|event| matches!(
        &event.event,
        SessionEvent::Message(message)
            if message.role == acp_types::MessageRole::User
                && message.text == "remember this prompt"
    )));
}

#[test]
fn cancel_clears_pending_permission() {
    let manager = manager(64);
    let id = create(&manager, Some("hello"));
    manager.cancel(&id).expect("cancel accepted");
    let detail = manager.inspect_session(&id).unwrap();
    assert!(detail.pending_permission.is_none());
}

#[test]
fn rename_updates_title_and_journal() {
    let manager = manager(64);
    let id = create(&manager, Some("hello"));
    let before = manager.inspect_session(&id).unwrap();
    let renamed = manager
        .rename_session(&id, "  New title  ")
        .expect("rename accepted");
    assert_eq!(renamed.title, "New title");
    let after = manager.inspect_session(&id).unwrap();
    assert_eq!(after.summary.title, "New title");
    assert!(after.summary.last_sequence > before.summary.last_sequence);
    assert!(after.recent_events.iter().any(|sequenced| matches!(
        &sequenced.event,
        SessionEvent::Snapshot(snapshot) if snapshot.summary.title == "New title"
    )));
}

#[test]
fn rename_rejects_empty_title() {
    let manager = manager(64);
    let id = create(&manager, Some("hello"));
    let error = manager.rename_session(&id, "   ").unwrap_err();
    assert!(matches!(error, SessionError::EmptyTitle));
}

#[test]
fn unknown_session_is_reported() {
    let manager = manager(64);
    let error = manager.inspect_session("missing").unwrap_err();
    assert!(matches!(error, SessionError::UnknownSession(_)));
}

#[test]
fn inspect_exposes_normalized_acp_metadata() {
    let manager = manager(64);
    let id = create(&manager, Some("hello"));
    let detail = manager.inspect_session(&id).unwrap();
    assert_eq!(detail.acp.protocol_version, 1);
    assert_eq!(detail.acp.agent.title.as_deref(), Some("Fake ACP agent"));
    assert!(detail.acp.capabilities.load_session);
    assert!(detail.acp.capabilities.session_list);
    assert!(detail.acp.capabilities.session_resume);
    assert!(detail.acp.auth_methods.is_empty());
    assert_eq!(detail.agent_session_id.as_deref(), Some("fake-session-1"));
}

#[test]
fn metadata_updates_are_normalized_into_session_events() {
    let manager = manager(64);
    let id = create(&manager, Some("hello"));
    let detail = manager.inspect_session(&id).unwrap();
    assert!(detail.recent_events.iter().any(
        |sequenced| matches!(&sequenced.event, SessionEvent::Plan(plan) if !plan.entries.is_empty())
    ));
    assert!(detail.recent_events.iter().any(|sequenced| matches!(
        &sequenced.event,
        SessionEvent::Commands(commands) if commands.commands.iter().any(|c| c.name == "compact")
    )));
    assert!(detail.recent_events.iter().any(|sequenced| matches!(
        &sequenced.event,
        SessionEvent::Mode(mode) if mode.current_mode_id == "build"
    )));
}
