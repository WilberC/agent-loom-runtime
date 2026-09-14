use agent_loom_runtime::{
    protocol::{Command, Envelope, VERSION},
    state::{MAX_DETAILED_EVIDENCE_RETENTION_DAYS, State},
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::Value;
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

#[derive(Deserialize)]
struct FixtureCase {
    schema: String,
    payload: Value,
}

#[derive(Deserialize)]
struct ProtocolFixtures {
    valid: Vec<FixtureCase>,
    invalid: Vec<FixtureCase>,
}

#[test]
fn shared_protocol_fixtures_are_consumed_by_runtime_types() {
    let fixtures: ProtocolFixtures = serde_json::from_str(include_str!("fixtures/runtime-v1.json"))
        .expect("shared protocol fixture must be valid JSON");

    for case in fixtures.valid {
        assert!(
            parse_fixture(&case.schema, case.payload.clone()).is_ok(),
            "{}: {:?}",
            case.schema,
            parse_fixture(&case.schema, case.payload)
        );
    }
    for case in fixtures.invalid {
        assert!(
            parse_fixture(&case.schema, case.payload).is_err(),
            "{}",
            case.schema
        );
    }
}

fn parse_fixture(schema: &str, payload: Value) -> Result<(), String> {
    match schema {
        "registration" => parse_envelope::<agent_loom_runtime::protocol::Registration>(payload),
        "heartbeat" => parse_envelope::<agent_loom_runtime::protocol::Heartbeat>(payload),
        "command" => parse_envelope::<Command>(payload),
        "ack" => parse_envelope::<agent_loom_runtime::protocol::CommandRef>(payload),
        "progress" => parse_envelope::<agent_loom_runtime::protocol::Progress>(payload),
        "result" => parse_envelope::<agent_loom_runtime::protocol::ResultEvent>(payload),
        "error" => parse_envelope::<agent_loom_runtime::protocol::ErrorEvent>(payload),
        other => Err(format!("unknown fixture schema: {other}")),
    }
}

fn parse_envelope<T: serde::de::DeserializeOwned>(payload: Value) -> Result<(), String> {
    let envelope: Envelope<T> =
        serde_json::from_value(payload).map_err(|error| error.to_string())?;
    envelope
        .validate_version()
        .map_err(|error| error.to_string())
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
fn state_allows_reusing_outgoing_event_ids_after_delivery() {
    let dir = tempdir().unwrap();
    let state = State::open(&dir.path().join("state.sqlite3")).unwrap();

    state.queue_event("first", &json!({"sequence": 1})).unwrap();
    let first = state.pending_events().unwrap().pop().unwrap();
    state.delivered(first.id).unwrap();

    state
        .queue_event("second", &json!({"sequence": 2}))
        .unwrap();
    let second = state.pending_events().unwrap().pop().unwrap();
    assert_eq!(second.id, first.id);
    state.delivered(second.id).unwrap();

    let connection = rusqlite::Connection::open(dir.path().join("state.sqlite3")).unwrap();
    let history_count: i64 = connection
        .query_row("SELECT count(*) FROM event_history", [], |row| row.get(0))
        .unwrap();
    assert_eq!(history_count, 2);
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
fn state_prunes_old_event_and_error_evidence_but_preserves_identity_and_dedupe() {
    let dir = tempdir().unwrap();
    let state = State::open(&dir.path().join("state.sqlite3")).unwrap();
    let runtime_id = Uuid::new_v4();
    let command_id = Uuid::new_v4();
    state.save_identity(&runtime_id, "runtime-secret").unwrap();
    assert!(
        state
            .record_command(command_id, &json!({"kind":"already-seen"}))
            .unwrap()
    );
    state
        .queue_event("old", &json!({"detail":"discard me"}))
        .unwrap();
    let old_event = state.pending_events().unwrap().pop().unwrap();
    state
        .failed_delivery(old_event.id, "old delivery error")
        .unwrap();
    state.delivered(old_event.id).unwrap();
    state
        .queue_event("fresh", &json!({"detail":"keep me"}))
        .unwrap();

    let now = Utc::now();
    {
        let connection = rusqlite::Connection::open(dir.path().join("state.sqlite3")).unwrap();
        let old_created_at =
            (now - Duration::days(MAX_DETAILED_EVIDENCE_RETENTION_DAYS) - Duration::seconds(1))
                .to_rfc3339();
        connection
            .execute(
                "UPDATE event_history SET delivered_at=?1 WHERE id=?2",
                rusqlite::params![old_created_at, old_event.id],
            )
            .unwrap();
    }

    assert_eq!(state.prune_detailed_evidence(now).unwrap(), 1);
    let pending = state.pending_events().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].kind, "fresh");
    assert_eq!(
        state.identity().unwrap().unwrap().runtime_id,
        runtime_id.to_string()
    );
    assert!(
        !state
            .record_command(command_id, &json!({"kind":"duplicate"}))
            .unwrap()
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
