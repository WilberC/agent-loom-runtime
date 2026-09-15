use crate::protocol::{CommandCollection, Envelope, Registration, RegistrationResponse};
use anyhow::{Context, Result, bail};
use reqwest::{Client, StatusCode};
use serde::Serialize;
use uuid::Uuid;

pub struct ControlPlaneClient {
    client: Client,
    base: reqwest::Url,
    identity: Option<(Uuid, String)>,
}
impl ControlPlaneClient {
    pub fn new(
        base: &url::Url,
        timeout: std::time::Duration,
        identity: Option<(Uuid, String)>,
    ) -> Result<Self> {
        Ok(Self {
            client: Client::builder().timeout(timeout).build()?,
            base: reqwest::Url::parse(base.as_ref())?,
            identity,
        })
    }
    fn url(&self, path: &str) -> Result<reqwest::Url> {
        self.base
            .join(path.trim_start_matches('/'))
            .context("build API URL")
    }
    fn authenticated(&self, request: reqwest::RequestBuilder) -> Result<reqwest::RequestBuilder> {
        let (id, secret) = self.identity.as_ref().context("runtime is not enrolled")?;
        Ok(request.header("Authorization", format!("Runtime {id}.{secret}")))
    }
    pub async fn register(
        &self,
        enrollment: &str,
        body: &Envelope<Registration>,
    ) -> Result<RegistrationResponse> {
        let response = self
            .client
            .post(self.url("/api/v1/runtimes/register")?)
            .header("X-Enrollment-Token", enrollment)
            .header("Idempotency-Key", body.message_id.to_string())
            .json(body)
            .send()
            .await?
            .error_for_status()?;
        let registered: RegistrationResponse = response.json().await?;
        if registered.protocol_version != "v1" {
            bail!("control plane returned incompatible protocol version")
        }
        Ok(registered)
    }
    pub async fn heartbeat<T: Serialize>(&self, body: &T) -> Result<()> {
        self.authenticated(self.client.post(self.url("/api/v1/runtime/heartbeat")?))?
            .json(body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
    pub async fn commands(&self) -> Result<CommandCollection> {
        let response = self
            .authenticated(self.client.get(self.url("/api/v1/runtime/commands")?))?
            .send()
            .await?
            .error_for_status()?;
        let body: CommandCollection = response.json().await?;
        if body.protocol_version != "v1" {
            bail!("command collection has incompatible version")
        }
        for command in &body.commands {
            command.validate_version()?;
        }
        Ok(body)
    }
    pub async fn event<T: Serialize>(&self, kind: &str, body: &T) -> Result<bool> {
        let response = self
            .authenticated(
                self.client
                    .post(self.url(&format!("/api/v1/runtime/events/{kind}"))?),
            )?
            .json(body)
            .send()
            .await?;
        if response.status() == StatusCode::CONFLICT {
            return Ok(false);
        }
        response.error_for_status()?;
        Ok(true)
    }
}
