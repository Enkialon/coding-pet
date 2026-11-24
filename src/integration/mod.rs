mod cli;
mod codex_hook;
mod event;
mod server;

pub use cli::{try_run_cli, CliOutcome};
pub use event::{AgentEvent, AgentEventKind};
pub use server::IntegrationServer;

/// Events delivered from background integrations to the winit UI thread.
#[derive(Debug, Clone)]
pub enum AppUserEvent {
    Agent(AgentEvent),
}
