//! Library root for `triage-bot`.

pub mod base;
pub mod prelude;
pub mod service;

use base::{config::Config, types::Void};
use rustls::crypto;
use service::slack::SlackClient;
use tracing::info;

/// Public async entry for the binary crate.
pub async fn start(config: Config) -> Void {
    info!("Starting triage-bot ...");

    // Start the crypto provider.
    crypto::ring::default_provider().install_default().unwrap();

    // Initialize the Slack client.
    let slack = SlackClient::new(&config).await?;

    // Start the Slack client connection via web sockets.
    slack.start().await
}
