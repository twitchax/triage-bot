//! Binary entry point for `triage-bot`.

use clap::Parser;
use triage_bot::base::{config::Config, types::Void};

/// Triage-bot – a Slack support channel triage helper.
///
/// Configuration can come from `config.toml` or environment variables.
/// See `Config` struct for the list of keys.
#[derive(Parser, Debug)]
#[command(version, author, about, long_about = None)]
struct Args {
    /// Override the config file path (optional).
    #[arg(short, long)]
    config: Option<std::path::PathBuf>,
    /// Increase log verbosity (-v, -vv, etc.).
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[tokio::main]
async fn main() -> Void {
    let args = Args::parse();

    let level = match args.verbose {
        0 => tracing::Level::INFO,
        1 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };
    tracing_subscriber::fmt()
        .with_ansi(true)
        .with_level(true)
        .with_file(false)
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_max_level(level)
        .init();

    let config = Config::load(args.config.as_deref())?;

    triage_bot::start(config).await
}
