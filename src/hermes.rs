use command_group::{AsyncCommandGroup, AsyncGroupChild};
#[cfg(unix)]
use command_group::{Signal, UnixChildExt};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{path::PathBuf, sync::LazyLock, time::Duration};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    time,
};
use tokio_util::sync::CancellationToken;

pub const CAPABILITY: &str = "hermes.run";
const MAX_PROMPT_BYTES: usize = 128 * 1024;
const TERMINATION_GRACE_PERIOD: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunInstruction {
    pub kind: String,
    pub prompt: String,
}

impl TryFrom<Value> for RunInstruction {
    type Error = HermesError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let instruction: Self = serde_json::from_value(value)
            .map_err(|error| HermesError::InvalidInstruction(error.to_string()))?;
        if instruction.kind != CAPABILITY {
            return Err(HermesError::UnsupportedInstruction(instruction.kind));
        }
        if instruction.prompt.trim().is_empty() {
            return Err(HermesError::InvalidInstruction(
                "prompt must not be empty".into(),
            ));
        }
        if instruction.prompt.len() > MAX_PROMPT_BYTES {
            return Err(HermesError::InvalidInstruction(format!(
                "prompt exceeds {MAX_PROMPT_BYTES} bytes"
            )));
        }
        Ok(instruction)
    }
}

#[derive(Debug, Error)]
pub enum HermesError {
    #[error("invalid Hermes instruction: {0}")]
    InvalidInstruction(String),
    #[error("unsupported instruction kind: {0}")]
    UnsupportedInstruction(String),
    #[error("Hermes execution timed out after {0:?}")]
    Timeout(Duration),
    #[error("Hermes exited with status {status}: {stderr}")]
    NonZero { status: String, stderr: String },
    #[error("failed to start Hermes: {0}")]
    Start(#[source] std::io::Error),
    #[error("failed to capture Hermes output: {0}")]
    Io(#[source] std::io::Error),
    #[error("Hermes execution was cancelled")]
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct HermesRunner {
    binary: PathBuf,
    timeout: Duration,
    output_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunResult {
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub exit_code: i32,
}

impl HermesRunner {
    #[must_use]
    pub fn new(binary: PathBuf, timeout: Duration, output_limit: usize) -> Self {
        Self {
            binary,
            timeout,
            output_limit,
        }
    }

    #[must_use]
    pub fn capability(&self) -> &'static str {
        CAPABILITY
    }

    pub async fn run(&self, instruction: Value) -> Result<RunResult, HermesError> {
        self.run_with_cancellation(instruction, CancellationToken::new())
            .await
    }

    pub async fn run_with_cancellation(
        &self,
        instruction: Value,
        cancellation: CancellationToken,
    ) -> Result<RunResult, HermesError> {
        let instruction = RunInstruction::try_from(instruction)?;
        let mut command = Command::new(&self.binary);
        command
            .arg("run")
            .arg("--prompt")
            .arg(instruction.prompt)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.group_spawn().map_err(HermesError::Start)?;
        let stdout = child
            .inner()
            .stdout
            .take()
            .ok_or_else(|| HermesError::Io(std::io::Error::other("missing stdout pipe")))?;
        let stderr = child
            .inner()
            .stderr
            .take()
            .ok_or_else(|| HermesError::Io(std::io::Error::other("missing stderr pipe")))?;
        let stdout_task = tokio::spawn(read_bounded(stdout, self.output_limit));
        let stderr_task = tokio::spawn(read_bounded(stderr, self.output_limit));
        let status = tokio::select! {
            result = child.wait() => result.map_err(HermesError::Io)?,
            () = time::sleep(self.timeout) => {
                terminate(&mut child).await;
                stdout_task.abort();
                stderr_task.abort();
                return Err(HermesError::Timeout(self.timeout));
            }
            () = cancellation.cancelled() => {
                terminate(&mut child).await;
                stdout_task.abort();
                stderr_task.abort();
                return Err(HermesError::Cancelled);
            }
        };
        let stdout = stdout_task
            .await
            .map_err(|error| HermesError::Io(std::io::Error::other(error)))?
            .map_err(HermesError::Io)?;
        let stderr = stderr_task
            .await
            .map_err(|error| HermesError::Io(std::io::Error::other(error)))?
            .map_err(HermesError::Io)?;
        let stdout_text = redact(String::from_utf8_lossy(&stdout.bytes).into_owned());
        let stderr_text = redact(String::from_utf8_lossy(&stderr.bytes).into_owned());
        if !status.success() {
            return Err(HermesError::NonZero {
                status: status.to_string(),
                stderr: stderr_text,
            });
        }
        Ok(RunResult {
            stdout: stdout_text,
            stderr: stderr_text,
            stdout_truncated: stdout.truncated,
            stderr_truncated: stderr.truncated,
            exit_code: status.code().unwrap_or(-1),
        })
    }
}

struct BoundedOutput {
    bytes: Vec<u8>,
    truncated: bool,
}

async fn read_bounded<R: AsyncRead + Unpin>(
    mut reader: R,
    limit: usize,
) -> Result<BoundedOutput, std::io::Error> {
    let mut output = Vec::with_capacity(limit.min(8192));
    let mut buffer = [0_u8; 8192];
    let mut truncated = false;
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        let remaining = limit.saturating_sub(output.len());
        output.extend_from_slice(&buffer[..read.min(remaining)]);
        truncated |= read > remaining;
    }
    Ok(BoundedOutput {
        bytes: output,
        truncated,
    })
}

async fn terminate(child: &mut AsyncGroupChild) {
    #[cfg(unix)]
    let _ = child.signal(Signal::SIGTERM);
    #[cfg(not(unix))]
    let _ = child.start_kill();

    if time::timeout(TERMINATION_GRACE_PERIOD, child.wait())
        .await
        .is_err()
    {
        let _ = child.kill().await;
    }
}

static BEARER: std::sync::LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(bearer)\s+[A-Za-z0-9._~+/=-]+\b").unwrap());
static SECRET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\b(secret|token|password|api[_-]?key)\s*[:=]\s*["']?[^\s,"']+["']?"#).unwrap()
});

pub fn redact(input: String) -> String {
    let redacted = BEARER.replace_all(&input, "$1 [REDACTED]");
    SECRET.replace_all(&redacted, "$1=[REDACTED]").into_owned()
}
