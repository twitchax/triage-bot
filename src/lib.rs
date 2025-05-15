//! Library root for `triage-bot`.

pub mod base;
pub mod config;
pub mod slack;
pub mod llm;
pub mod prelude;

use crate::{config::Config, slack::SlackClient};
use crate::base::Void;
use tracing::info;

/// Public async entry for the binary crate.
pub async fn run(config: Config) -> Void {
    info!("starting triage-bot");
    let slack = SlackClient::new(&config).await?;
    // Start Slack listener / task loop (placeholder).
    slack.run().await
}
