use agent_loom_runtime::{
    daemon::process_command,
    hermes::HermesRunner,
    protocol::{Command, Envelope},
    state::State,
};
use serde_json::{Value, json};
use std::{fs, os::unix::fs::PermissionsExt, path::Path, time::Duration};
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

fn fake_hermes(dir: &Path, body: &str) -> std::path::PathBuf {
    let path = dir.join("fake-hermes.sh");
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    path
}

fn command(instruction: Value) -> Envelope<Command> {
    Envelope::new(Command {
        command_id: Uuid::new_v4(),
        instruction,
        lease_token: None,
        lease_expires_at: None,
    })
}

fn event_kinds(state: &State) -> Vec<String> {
    state
        .pending_events()
        .unwrap()
        .into_iter()
        .map(|event| event.kind)
        .collect()
}

fn event_payloads(state: &State) -> Vec<Value> {
    state
        .pending_events()
        .unwrap()
        .into_iter()
        .map(|event| serde_json::from_str(&event.payload).unwrap())
        .collect()
}

#[tokio::test]
async fn process_command_queues_successful_lifecycle_in_order() {
    let dir = tempdir().unwrap();
    let state = State::open(&dir.path().join("state.db")).unwrap();
    let runner = HermesRunner::new(
        fake_hermes(dir.path(), "printf 'done'"),
        Duration::from_secs(1),
        1024,
    );
    let command = command(json!({"kind": "hermes.run", "prompt": "run"}));
    let command_id = command.body.command_id;

    assert!(
        process_command(&state, &runner, &command, CancellationToken::new())
            .await
            .unwrap()
    );
    assert_eq!(event_kinds(&state), ["ack", "progress", "result"]);
    let payloads = event_payloads(&state);
    assert_eq!(payloads[1]["progress"]["percent"], 0);
    assert_eq!(payloads[1]["progress"]["status"], "running");
    assert_eq!(payloads[2]["result"]["stdout"], "done");
    assert_eq!(payloads[0]["command_id"], command_id.to_string());
}

#[tokio::test]
async fn process_command_queues_error_for_non_zero_exit() {
    let dir = tempdir().unwrap();
    let state = State::open(&dir.path().join("state.db")).unwrap();
    let runner = HermesRunner::new(
        fake_hermes(dir.path(), "printf 'bad' >&2; exit 7"),
        Duration::from_secs(1),
        1024,
    );

    process_command(
        &state,
        &runner,
        &command(json!({"kind": "hermes.run", "prompt": "fail"})),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(event_kinds(&state), ["ack", "progress", "error"]);
    assert_eq!(
        event_payloads(&state)[2]["error"]["code"],
        "execution_failed"
    );
}

#[tokio::test]
async fn process_command_queues_invalid_and_unsupported_errors_without_starting_hermes() {
    let dir = tempdir().unwrap();
    let state = State::open(&dir.path().join("state.db")).unwrap();
    let runner = HermesRunner::new(
        dir.path().join("does-not-exist"),
        Duration::from_secs(1),
        1024,
    );

    process_command(
        &state,
        &runner,
        &command(json!({"kind": "unsupported", "prompt": "no-op"})),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    process_command(
        &state,
        &runner,
        &command(json!({"kind": "hermes.run", "prompt": ""})),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        event_kinds(&state),
        ["ack", "progress", "error", "ack", "progress", "error"]
    );
    let payloads = event_payloads(&state);
    assert_eq!(payloads[2]["error"]["code"], "unsupported_instruction");
    assert_eq!(payloads[5]["error"]["code"], "invalid_instruction");
}

#[tokio::test]
async fn process_command_queues_cancellation_error() {
    let dir = tempdir().unwrap();
    let state = State::open(&dir.path().join("state.db")).unwrap();
    let runner = HermesRunner::new(
        fake_hermes(dir.path(), "sleep 10"),
        Duration::from_secs(30),
        1024,
    );
    let cancellation = CancellationToken::new();
    cancellation.cancel();

    process_command(
        &state,
        &runner,
        &command(json!({"kind": "hermes.run", "prompt": "cancel"})),
        cancellation,
    )
    .await
    .unwrap();
    assert_eq!(event_kinds(&state), ["ack", "progress", "error"]);
    assert_eq!(
        event_payloads(&state)[2]["error"]["code"],
        "execution_cancelled"
    );
}

#[tokio::test]
async fn process_command_deduplicates_replayed_commands_before_execution() {
    let dir = tempdir().unwrap();
    let state = State::open(&dir.path().join("state.db")).unwrap();
    let runner = HermesRunner::new(
        fake_hermes(dir.path(), "printf 'once'"),
        Duration::from_secs(1),
        1024,
    );
    let command = command(json!({"kind": "hermes.run", "prompt": "replay"}));

    assert!(
        process_command(&state, &runner, &command, CancellationToken::new())
            .await
            .unwrap()
    );
    assert!(
        !process_command(&state, &runner, &command, CancellationToken::new())
            .await
            .unwrap()
    );
    assert_eq!(event_kinds(&state), ["ack", "progress", "result"]);
}
