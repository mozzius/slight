use crate::frames::{GatewayEvent, PROTOCOL_VERSION};
use serde_json::Value;
use session_core::SequencedEvent;

pub mod event_name {
    pub const SESSION_STATUS: &str = "session.status";
    pub const SESSION_MESSAGE: &str = "session.message";
    pub const SESSION_TOOL_CALL: &str = "session.tool_call";
    pub const SESSION_PLAN: &str = "session.plan";
    pub const SESSION_MODE: &str = "session.mode";
    pub const SESSION_COMMANDS: &str = "session.commands";
    pub const SESSION_CONFIG_OPTIONS: &str = "session.config_options";
    pub const SESSION_INFO: &str = "session.info";
    pub const SESSION_USAGE: &str = "session.usage";
    pub const SESSION_TURN_ENDED: &str = "session.turn_ended";
    pub const SESSION_PERMISSION_REQUEST: &str = "session.permission_request";
    pub const SESSION_PERMISSION_RESOLVED: &str = "session.permission_resolved";
    pub const SESSION_EXIT: &str = "session.exit";
    pub const SESSION_SNAPSHOT: &str = "session.snapshot";
    pub const SESSION_DIAGNOSTICS: &str = "session.diagnostics";
    pub const REPLAY_COMPLETE: &str = "replay.complete";
    pub const HOST_STATUS_CHANGED: &str = "host.status_changed";
}

pub fn gateway_event(session_id: &str, sequenced: &SequencedEvent) -> GatewayEvent {
    GatewayEvent {
        protocol_version: PROTOCOL_VERSION,
        session_id: session_id.to_string(),
        sequence: sequenced.sequence,
        event: sequenced.event.name().to_string(),
        at: Some(crate::time::rfc3339_from_ms(sequenced.timestamp_ms)),
        payload: Some(sequenced.event.payload()),
    }
}

pub fn replay_complete(session_id: &str, latest_sequence: u64) -> GatewayEvent {
    GatewayEvent {
        protocol_version: PROTOCOL_VERSION,
        session_id: session_id.to_string(),
        sequence: latest_sequence,
        event: event_name::REPLAY_COMPLETE.to_string(),
        at: None,
        payload: Some(serde_json::json!({ "latest_sequence": latest_sequence })),
    }
}

pub fn event_payload(value: Value) -> Option<Value> {
    Some(value)
}
