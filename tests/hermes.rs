use agent_loom_runtime::hermes::{HermesError, HermesRunner, RunInstruction, redact};
use serde_json::json;
use std::{fs, os::unix::fs::PermissionsExt, path::Path, time::Duration};
use tempfile::tempdir;

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

#[test]
fn redaction_is_deterministic_for_common_secret_formats() {
    let input = "Bearer abc.def token: xyz password='pass' api_key=key".to_owned();
    assert_eq!(
        redact(input),
        "Bearer [REDACTED] token=[REDACTED] password=[REDACTED] api_key=[REDACTED]"
    );
}
