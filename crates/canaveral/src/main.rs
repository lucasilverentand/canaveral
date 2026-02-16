//! Canaveral - Universal release management CLI

mod cli;
mod exit_codes;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

use cli::Cli;

fn main() -> anyhow::Result<()> {
    let _guard = init_tracing();

    let cli = Cli::parse();
    cli.execute()
}

/// Set up tracing with two layers:
/// - Console: controlled by RUST_LOG (default: warn)
/// - File: always debug-level JSON to ~/.canaveral/logs/
fn init_tracing() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let console_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));

    if let Some(log_dir) = log_directory() {
        let file_appender = tracing_appender::rolling::daily(&log_dir, "canaveral.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .with_target(false)
                    .with_filter(console_filter),
            )
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_writer(non_blocking)
                    .with_target(true)
                    .with_filter(EnvFilter::new("debug")),
            )
            .init();

        return Some(guard);
    }

    // Fallback: console only
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_filter(console_filter),
        )
        .init();

    None
}

/// Returns the log directory path, creating it if needed.
fn log_directory() -> Option<std::path::PathBuf> {
    let log_dir = dirs::home_dir()?.join(".canaveral").join("logs");
    std::fs::create_dir_all(&log_dir).ok()?;
    Some(log_dir)
}
