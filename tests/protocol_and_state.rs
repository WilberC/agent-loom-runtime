use agent_loom_runtime::{
    protocol::{Command, Envelope, VERSION},
    state::State,
};
use serde_json::json;
use tempfile::tempdir;
use uuid::Uuid;
#[test]
fn envelope_rejects_unknown_fields_and_wrong_version() {
    let raw = json!({"protocol_version":"v1","message_id":Uuid::new_v4(),"sent_at":"2026-01-01T00:00:00Z","command_id":Uuid::new_v4(),"instruction":{},"unexpected":true});
    assert!(serde_json::from_value::<Envelope<Command>>(raw).is_err());
    let bad = Envelope {
        protocol_version: "v2".into(),
        message_id: Uuid::new_v4(),
        sent_at: chrono::Utc::now(),
        body: Command {
            command_id: Uuid::new_v4(),
            instruction: json!({}),
            lease_token: None,
            lease_expires_at: None,
        },
    };
    assert!(bad.validate_version().is_err());
    assert_eq!(VERSION, "v1");
}
#[test]
fn state_deduplicates_commands_and_preserves_events() {
    let dir = tempdir().unwrap();
    let state = State::open(&dir.path().join("state.sqlite3")).unwrap();
    let id = Uuid::new_v4();
    assert!(state.record_command(id, &json!({"kind":"test"})).unwrap());
    assert!(!state.record_command(id, &json!({"kind":"test"})).unwrap());
    state.queue_event("ack", &json!({"command_id":id})).unwrap();
    let event = state.pending_events().unwrap().pop().unwrap();
    state.failed_delivery(event.id, "offline").unwrap();
    assert_eq!(state.status().unwrap().queued_events, 1);
    state.delivered(event.id).unwrap();
    assert_eq!(state.status().unwrap().queued_events, 0);
}

#[test]
fn state_persists_identity_and_diagnostic_metadata() {
    let dir = tempdir().unwrap();
    let state = State::open(&dir.path().join("state.sqlite3")).unwrap();
    let id = Uuid::new_v4();
    state.save_identity(&id, "runtime-secret").unwrap();
    state.set_meta("last_error", "offline").unwrap();
    assert_eq!(
        state.identity().unwrap().unwrap().runtime_id,
        id.to_string()
    );
    assert_eq!(
        state.status().unwrap().last_error.as_deref(),
        Some("offline")
    );
}

#[test]
fn command_collection_accepts_leases_and_rejects_unknown_fields() {
    let command_id = Uuid::new_v4();
    let raw = json!({
        "protocol_version": "v1",
        "commands": [{
            "protocol_version": "v1",
            "message_id": Uuid::new_v4(),
            "sent_at": "2026-01-01T00:00:00Z",
            "command_id": command_id,
            "instruction": {"application": "example", "version": 1},
            "lease_token": Uuid::new_v4(),
            "lease_expires_at": "2026-01-01T00:01:00Z"
        }]
    });
    let collection: agent_loom_runtime::protocol::CommandCollection =
        serde_json::from_value(raw).unwrap();
    assert_eq!(collection.commands[0].body.command_id, command_id);
    let invalid = json!({"protocol_version":"v1","commands":[],"unexpected":true});
    assert!(
        serde_json::from_value::<agent_loom_runtime::protocol::CommandCollection>(invalid).is_err()
    );
}
