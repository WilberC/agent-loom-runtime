use crate::{
    client::ControlPlaneClient,
    config::Config,
    hermes::{HermesError, HermesRunner},
    protocol::{
        CommandRef, Envelope, ErrorBody, ErrorEvent, Heartbeat, Progress, Registration, ResultEvent,
    },
    state::State,
};
use anyhow::{Context, Result};
use serde_json::json;
use std::sync::Arc;
use tokio::time::{MissedTickBehavior, interval};
use tracing::{info, warn};

pub const CAPABILITIES: &[&str] = &[crate::hermes::CAPABILITY];
pub async fn run(config: Config, state: Arc<State>) -> Result<()> {
    let identity = state.identity()?;
    let client = ControlPlaneClient::new(
        &config.control_plane_url,
        config.request_timeout,
        identity
            .as_ref()
            .map(|i| -> Result<(uuid::Uuid, String)> {
                Ok((
                    i.runtime_id.parse::<uuid::Uuid>()?,
                    i.runtime_secret.clone(),
                ))
            })
            .transpose()
            .context("invalid locally stored runtime identity")?,
    )?;
    let client = if state.identity()?.is_none() {
        let enrollment = config
            .enrollment_token
            .as_deref()
            .context("runtime is not enrolled; AGENT_LOOM_ENROLLMENT_TOKEN is required")?;
        let response = client
            .register(
                enrollment,
                &Envelope::new(Registration {
                    runtime_name: config.runtime_name.clone(),
                    capabilities: CAPABILITIES.iter().map(ToString::to_string).collect(),
                }),
            )
            .await?;
        state.save_identity(&response.runtime_id, &response.runtime_secret)?;
        ControlPlaneClient::new(
            &config.control_plane_url,
            config.request_timeout,
            Some((response.runtime_id, response.runtime_secret)),
        )?
    } else {
        client
    };
    info!(runtime=%config.runtime_name,"runtime daemon started");
    let mut polling = interval(config.poll_interval);
    polling.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut heartbeats = interval(config.heartbeat_interval);
    heartbeats.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let runner = HermesRunner::new(
        config.hermes_binary,
        config.hermes_timeout,
        config.hermes_output_limit,
    );
    loop {
        tokio::select! { _=polling.tick()=> { if let Err(error)=cycle(&client,&state,&runner).await { warn!(%error,"runtime cycle failed; will reconnect"); state.set_meta("last_error",&error.to_string())?; } }, _=heartbeats.tick()=> { let heartbeat=Envelope::new(Heartbeat{capabilities:CAPABILITIES.iter().map(ToString::to_string).collect()}); if let Err(error)=client.heartbeat(&heartbeat).await {warn!(%error,"heartbeat failed");state.set_meta("last_error",&error.to_string())?;} }, _=tokio::signal::ctrl_c()=> {info!("shutdown signal received");return Ok(());} }
    }
}
async fn cycle(client: &ControlPlaneClient, state: &State, runner: &HermesRunner) -> Result<()> {
    state.prune_detailed_evidence(chrono::Utc::now())?;
    flush(client, state).await?;
    let commands = client.commands().await?;
    for command in commands.commands {
        if !state.record_command(command.body.command_id, &command.body.instruction)? {
            continue;
        }
        let ack = Envelope::new(CommandRef {
            command_id: command.body.command_id,
        });
        state.queue_event("ack", &serde_json::to_value(ack)?)?;
        let progress = Envelope::new(Progress {
            command_id: command.body.command_id,
            progress: json!({"status": "running"}),
        });
        state.queue_event("progress", &serde_json::to_value(progress)?)?;
        match runner.run(command.body.instruction).await {
            Ok(result) => {
                let event = Envelope::new(ResultEvent {
                    command_id: command.body.command_id,
                    result: serde_json::to_value(result)?,
                });
                state.queue_event("result", &serde_json::to_value(event)?)?;
            }
            Err(error) => {
                let event = Envelope::new(ErrorEvent {
                    command_id: command.body.command_id,
                    error: ErrorBody {
                        code: error_code(&error),
                        details: [("message".to_owned(), json!(error.to_string()))].into(),
                    },
                });
                state.queue_event("error", &serde_json::to_value(event)?)?;
            }
        }
    }
    flush(client, state).await
}

fn error_code(error: &HermesError) -> String {
    match error {
        HermesError::InvalidInstruction(_) => "invalid_instruction",
        HermesError::UnsupportedInstruction(_) => "unsupported_instruction",
        HermesError::Timeout(_) => "execution_timeout",
        HermesError::NonZero { .. } => "execution_failed",
        HermesError::Start(_) => "executor_unavailable",
        HermesError::Io(_) => "execution_io_error",
    }
    .to_owned()
}
async fn flush(client: &ControlPlaneClient, state: &State) -> Result<()> {
    for event in state.pending_events()? {
        let payload: serde_json::Value =
            serde_json::from_str(&event.payload).context("corrupt queued event")?;
        match client.event(&event.kind, &payload).await {
            Ok(_) => state.delivered(event.id)?,
            Err(error) => {
                state.failed_delivery(event.id, &error.to_string())?;
                return Err(error);
            }
        }
    }
    Ok(())
}
