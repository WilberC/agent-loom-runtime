use anyhow::{Context, Result, bail};
use std::{env, fs, path::PathBuf, time::Duration};
use url::Url;

#[derive(Clone, Debug)]
pub struct Config {
    pub control_plane_url: Url,
    pub runtime_name: String,
    pub enrollment_token: Option<String>,
    pub state_path: PathBuf,
    pub poll_interval: Duration,
    pub heartbeat_interval: Duration,
    pub request_timeout: Duration,
}
impl Config {
    pub fn from_env() -> Result<Self> {
        let url = env::var("AGENT_LOOM_CONTROL_PLANE_URL")
            .context("AGENT_LOOM_CONTROL_PLANE_URL is required")?;
        let control_plane_url = Url::parse(&url).context("invalid AGENT_LOOM_CONTROL_PLANE_URL")?;
        if !matches!(control_plane_url.scheme(), "http" | "https") {
            bail!("control-plane URL must use http(s)");
        }
        let positive = |name: &str, default: u64| -> Result<Duration> {
            let value = env::var(name)
                .ok()
                .map(|v| v.parse())
                .transpose()
                .context("invalid duration")?
                .unwrap_or(default);
            if value == 0 {
                bail!("{name} must be positive");
            }
            Ok(Duration::from_secs(value))
        };
        Ok(Self {
            control_plane_url,
            runtime_name: env::var("AGENT_LOOM_RUNTIME_NAME")
                .unwrap_or_else(|_| "agent-loom-runtime".to_owned()),
            enrollment_token: env::var("AGENT_LOOM_ENROLLMENT_TOKEN")
                .ok()
                .filter(|v| !v.is_empty()),
            state_path: env::var_os("AGENT_LOOM_STATE_PATH").map_or_else(
                || PathBuf::from("agent-loom-runtime.sqlite3"),
                PathBuf::from,
            ),
            poll_interval: positive("AGENT_LOOM_POLL_INTERVAL_SECS", 10)?,
            heartbeat_interval: positive("AGENT_LOOM_HEARTBEAT_INTERVAL_SECS", 30)?,
            request_timeout: positive("AGENT_LOOM_REQUEST_TIMEOUT_SECS", 30)?,
        })
    }
    pub fn runtime_secret(&self) -> Result<Option<String>> {
        let direct = env::var("AGENT_LOOM_RUNTIME_SECRET")
            .ok()
            .filter(|s| !s.is_empty());
        let file = env::var_os("AGENT_LOOM_RUNTIME_SECRET_FILE").map(PathBuf::from);
        if direct.is_some() && file.is_some() {
            bail!("set only one runtime secret source");
        }
        match file {
            Some(path) => Ok(Some(fs::read_to_string(path)?.trim().to_owned())),
            None => Ok(direct),
        }
    }
}
