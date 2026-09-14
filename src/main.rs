use agent_loom_runtime::{config::Config, daemon, state::State, tui};
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::sync::Arc;
use tracing_subscriber::EnvFilter;
#[derive(Parser)]
#[command(name = "agent-loom", about = "Agent Loom outbound runtime")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Daemon,
    Tui,
    Doctor,
}
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("agent_loom_runtime=info".parse()?),
        )
        .json()
        .init();
    let cli = Cli::parse();
    let config = Config::from_env()?;
    let state = Arc::new(State::open(&config.state_path)?);
    match cli.command {
        Command::Daemon => daemon::run(config, state).await,
        Command::Tui => tui::run(state),
        Command::Doctor => doctor(config, state),
    }
}
fn doctor(config: Config, state: Arc<State>) -> Result<()> {
    let status = state.status()?;
    println!(
        "control_plane_url: {}\nstate_path: {}\nenrolled: {}\nknown_commands: {}\nqueued_events: {}\nlast_error: {}",
        config.control_plane_url,
        config.state_path.display(),
        status.identity_present,
        status.known_commands,
        status.queued_events,
        status.last_error.unwrap_or_else(|| "none".into())
    );
    if !status.identity_present && config.enrollment_token.is_none() {
        anyhow::bail!("not enrolled and AGENT_LOOM_ENROLLMENT_TOKEN is absent")
    }
    Ok(())
}
