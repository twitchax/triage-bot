//! Binary entry point for `triage-bot`.
//!
//! This module provides the command-line interface for triage-bot with options
//! for configuration file paths and logging verbosity. It initializes the
//! necessary components and starts the service.

use clap::Parser;
use tracing_subscriber::fmt::format::FmtSpan;
use triage_bot::base::{config::Config, types::Void};

/// Triage-bot – a Slack support channel triage helper.
///
/// Configuration can come from `config.toml` or environment variables.
/// The bot monitors Slack channels and provides automated assistance
/// for support requests, tagging appropriate team members and providing
/// contextual information.
#[derive(Parser, Debug)]
#[command(version, author, about, long_about = None)]
struct Args {
    /// Override the config file path (optional).
    ///
    /// By default, the bot will look for a config file at `.hidden/config.toml`
    /// in the current directory.
    #[arg(short, long)]
    config: Option<std::path::PathBuf>,
    /// Increase log verbosity (-v, -vv, etc.).
    ///
    /// Use multiple times to increase verbosity:
    /// - No flag: INFO level
    /// - -v: DEBUG level
    /// - -vv or more: TRACE level
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

/// Main entry point for the triage-bot binary.
///
/// Sets up logging based on verbosity, loads configuration, and starts the bot.
#[tokio::main]
async fn main() -> Void {
    let args = Args::parse();

    let level = match args.verbose {
        0 => tracing::Level::INFO,
        1 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };
    tracing_subscriber::fmt()
        .without_time()
        .with_ansi(true)
        .with_level(true)
        .with_file(false)
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .with_max_level(level)
        .init();

    let config = Config::load(args.config.as_deref())?;

    triage_bot::start(config).await
}
