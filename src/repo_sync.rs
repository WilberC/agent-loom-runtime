use base64::Engine;
use serde::Deserialize;
use serde_json::Value;
use std::{
    path::{Component, Path, PathBuf},
    process::Stdio,
};
use thiserror::Error;
use tokio::process::Command;

pub const CAPABILITY: &str = "repo.sync";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncInstruction {
    pub kind: String,
    pub repository_url: String,
    pub operation: Operation,
    pub reference: String,
    pub worktree: PathBuf,
    #[serde(default)]
    pub credential: Option<Credential>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Pull,
    Push,
    PullPush,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Credential {
    pub username: String,
    pub token: String,
}

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("invalid repository sync instruction: {0}")]
    Invalid(String),
    #[error("repository sync command failed: {0}")]
    Command(String),
    #[error("failed to start git: {0}")]
    Start(#[source] std::io::Error),
    #[error("failed to read git output: {0}")]
    Io(#[source] std::io::Error),
}

pub fn parse(value: Value) -> Result<SyncInstruction, SyncError> {
    let instruction: SyncInstruction =
        serde_json::from_value(value).map_err(|e| SyncError::Invalid(e.to_string()))?;
    if instruction.kind != CAPABILITY {
        return Err(SyncError::Invalid("unsupported kind".into()));
    }
    if !matches!(instruction.repository_url.get(..8), Some("https://")) {
        return Err(SyncError::Invalid("repository_url must use https".into()));
    }
    if instruction.reference.is_empty() || instruction.reference.contains([' ', '\n', '\r']) {
        return Err(SyncError::Invalid(
            "reference must be a non-empty safe ref".into(),
        ));
    }
    if instruction.worktree.is_absolute()
        || instruction
            .worktree
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::RootDir))
    {
        return Err(SyncError::Invalid(
            "worktree must be relative to the configured sync root".into(),
        ));
    }
    if instruction
        .credential
        .as_ref()
        .is_some_and(|c| c.token.is_empty())
    {
        return Err(SyncError::Invalid(
            "credential token must not be empty".into(),
        ));
    }
    Ok(instruction)
}

pub async fn run(instruction: SyncInstruction, root: &Path) -> Result<String, SyncError> {
    let worktree = root.join(&instruction.worktree);
    if matches!(instruction.operation, Operation::Push | Operation::PullPush)
        && !worktree.join(".git").is_dir()
    {
        return Err(SyncError::Invalid(
            "push operations require an existing worktree".into(),
        ));
    }

    if !worktree.join(".git").is_dir()
        && matches!(instruction.operation, Operation::Pull | Operation::PullPush)
    {
        tokio::fs::create_dir_all(root)
            .await
            .map_err(SyncError::Io)?;
        let mut command = Command::new("git");
        command.args([
            "clone",
            "--branch",
            &instruction.reference,
            &instruction.repository_url,
        ]);
        command.arg(&worktree);
        return execute(command, instruction.credential.as_ref()).await;
    }

    if matches!(instruction.operation, Operation::Pull | Operation::PullPush) {
        let mut command = Command::new("git");
        command.args([
            "-C",
            worktree.to_str().unwrap_or_default(),
            "pull",
            "--ff-only",
            "origin",
            &instruction.reference,
        ]);
        execute(command, instruction.credential.as_ref()).await?;
    }

    if matches!(instruction.operation, Operation::Push | Operation::PullPush) {
        let mut command = Command::new("git");
        command.args([
            "-C",
            worktree.to_str().unwrap_or_default(),
            "push",
            "origin",
        ]);
        command.arg(format!("HEAD:{}", instruction.reference));
        return execute(command, instruction.credential.as_ref()).await;
    }

    Ok(String::new())
}

async fn execute(
    mut command: Command,
    credential: Option<&Credential>,
) -> Result<String, SyncError> {
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(credential) = credential {
        let auth = base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{}", credential.username, credential.token));
        command
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "http.extraHeader")
            .env("GIT_CONFIG_VALUE_0", format!("Authorization: Basic {auth}"));
    }
    let output = command.output().await.map_err(SyncError::Start)?;
    if !output.status.success() {
        return Err(SyncError::Command(redact(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )));
    }
    Ok(redact(
        String::from_utf8_lossy(&output.stdout).trim().to_owned(),
    ))
}

fn redact(mut text: String) -> String {
    for marker in ["GIT_TOKEN=", "token="] {
        if let Some(start) = text.find(marker) {
            text.replace_range(start + marker.len().., "[REDACTED]");
        }
    }
    text
}
