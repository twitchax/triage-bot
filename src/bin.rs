//! Binary entry point for `triage-bot`.
//!
//! This module provides the command-line interface for triage-bot with options
//! for configuration file paths and logging verbosity. It initializes the
//! necessary components and starts the service.

use clap::Parser;
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::{Protocol, WithExportConfig};
use tracing_subscriber::{fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt};
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

    // Construct the level filter.

    let level = match args.verbose {
        0 => tracing::Level::INFO,
        1 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };

    let level_filter = tracing_subscriber::filter::LevelFilter::from_level(level);

    // Prepare the log layer.

    let stdout = tracing_subscriber::fmt::layer()
        .without_time()
        .with_ansi(true)
        .with_level(true)
        .with_file(false)
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE);

    // Prepare the otlp layer.

    let exporter = opentelemetry_otlp::SpanExporter::builder().with_http().with_protocol(Protocol::HttpBinary).build()?;
    let tracer = opentelemetry_sdk::trace::SdkTracerProvider::builder().with_simple_exporter(exporter).build().tracer("triage-bot");
    let otel = tracing_opentelemetry::layer().with_tracer(tracer);

    tracing_subscriber::registry().with(otel).with(level_filter).with(stdout).init();

    let config = Config::load(args.config.as_deref())?;

    triage_bot::start(config).await
}
