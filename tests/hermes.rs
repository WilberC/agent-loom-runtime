use agent_loom_runtime::hermes::{HermesError, HermesRunner, RunInstruction, redact};
use serde_json::json;
use std::{fs, os::unix::fs::PermissionsExt, path::Path, time::Duration};
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

fn fake_hermes(dir: &Path, body: &str) -> std::path::PathBuf {
    let path = dir.join("fake-hermes.sh");
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    path
}

fn instruction(prompt: &str) -> serde_json::Value {
    json!({"kind": "hermes.run", "prompt": prompt})
}

#[test]
fn instruction_validation_is_typed_and_strict() {
    assert!(RunInstruction::try_from(instruction("hello")).is_ok());
    assert!(matches!(
        RunInstruction::try_from(json!({"kind":"other","prompt":"hello"})),
        Err(HermesError::UnsupportedInstruction(_))
    ));
    assert!(RunInstruction::try_from(json!({"kind":"hermes.run","prompt":" "})).is_err());
    assert!(
        RunInstruction::try_from(json!({"kind":"hermes.run","prompt":"ok","extra":true})).is_err()
    );
}

#[tokio::test]
async fn runner_executes_only_the_configured_binary_and_returns_bounded_output() {
    let dir = tempdir().unwrap();
    let binary = fake_hermes(dir.path(), r#"printf 'out-%s' "$3"; printf 'err' >&2"#);
    let runner = HermesRunner::new(binary, Duration::from_secs(2), 8);
    let result = runner.run(instruction("prompt")).await.unwrap();
    assert_eq!(result.stdout, "out-prom");
    assert_eq!(result.stderr, "err");
    assert!(result.stdout_truncated);
    assert!(!result.stderr_truncated);
    assert_eq!(result.exit_code, 0);
}

#[tokio::test]
async fn runner_reports_non_zero_exit_and_timeout() {
    let dir = tempdir().unwrap();
    let failing = fake_hermes(dir.path(), "echo 'token=secret-value' >&2; exit 7");
    let runner = HermesRunner::new(failing, Duration::from_secs(2), 1024);
    let error = runner.run(instruction("fail")).await.unwrap_err();
    assert!(matches!(error, HermesError::NonZero { .. }));
    assert!(error.to_string().contains("token=[REDACTED]"));

    let hanging = fake_hermes(dir.path(), "sleep 10");
    let runner = HermesRunner::new(hanging, Duration::from_millis(30), 1024);
    assert!(matches!(
        runner.run(instruction("timeout")).await,
        Err(HermesError::Timeout(_))
    ));
}

#[tokio::test]
async fn cancellation_terminates_the_entire_process_group() {
    if std::process::Command::new("kill")
        .arg("-0")
        .arg(std::process::id().to_string())
        .stderr(std::process::Stdio::null())
        .status()
        .is_err()
    {
        return;
    }
    let dir = tempdir().unwrap();
    let child_pid_file = dir.path().join("child.pid");
    let hanging = fake_hermes(
        dir.path(),
        &format!(
            "sleep 30 & child=$!; printf '%s' \"$child\" > '{}'; wait",
            child_pid_file.display()
        ),
    );
    let runner = HermesRunner::new(hanging, Duration::from_secs(30), 1024);
    let cancellation = CancellationToken::new();
    let cancel = cancellation.clone();
    let task = tokio::spawn(async move {
        runner
            .run_with_cancellation(instruction("cancel"), cancel)
            .await
    });

    for _ in 0..50 {
        if child_pid_file.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(
        child_pid_file.exists(),
        "fake Hermes did not start its child"
    );
    cancellation.cancel();
    assert!(matches!(task.await.unwrap(), Err(HermesError::Cancelled)));

    assert_process_group_child_is_gone(&child_pid_file).await;
}

#[tokio::test]
async fn timeout_terminates_the_entire_process_group() {
    if std::process::Command::new("kill")
        .arg("-0")
        .arg(std::process::id().to_string())
        .stderr(std::process::Stdio::null())
        .status()
        .is_err()
    {
        return;
    }
    let dir = tempdir().unwrap();
    let child_pid_file = dir.path().join("child.pid");
    let hanging = fake_hermes(
        dir.path(),
        &format!(
            "sleep 30 & child=$!; printf '%s' \"$child\" > '{}'; wait",
            child_pid_file.display()
        ),
    );
    let runner = HermesRunner::new(hanging, Duration::from_millis(30), 1024);
    assert!(matches!(
        runner.run(instruction("timeout")).await,
        Err(HermesError::Timeout(_))
    ));
    assert_process_group_child_is_gone(&child_pid_file).await;
}

async fn assert_process_group_child_is_gone(child_pid_file: &Path) {
    let child_pid = fs::read_to_string(child_pid_file).unwrap();
    tokio::time::sleep(Duration::from_millis(25)).await;
    let still_running = std::process::Command::new("kill")
        .args(["-0", child_pid.trim()])
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
        .success();
    assert!(!still_running, "process-group child survived termination");
}

#[test]
fn redaction_is_deterministic_for_common_secret_formats() {
    let input = "Bearer abc.def token: xyz password='pass' api_key=key".to_owned();
    assert_eq!(
        redact(input),
        "Bearer [REDACTED] token=[REDACTED] password=[REDACTED] api_key=[REDACTED]"
    );
}
