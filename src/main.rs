#![windows_subsystem = "windows"]

mod animation;
mod app;
mod config;
mod error;
mod integration;
mod locale;
mod pet;
mod renderer;
mod tray;
mod window;

fn main() {
    match integration::try_run_cli() {
        integration::CliOutcome::NotCli => {}
        integration::CliOutcome::Handled(Ok(())) => return,
        integration::CliOutcome::Handled(Err(message)) => {
            eprintln!("CodingPet: {message}");
            std::process::exit(2);
        }
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Set per-monitor DPI awareness before creating any windows
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::HiDpi::{
            SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        };
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    tracing::info!("CodingPet starting");

    if let Err(e) = app::run() {
        tracing::error!("Fatal error: {e}");
    }
}
