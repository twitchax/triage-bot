//! Library root for `triage-bot`.

pub mod base;
pub mod interaction;
pub mod runtime;
pub mod service;

use base::{config::Config, types::Void};
use rustls::crypto;
use service::chat::ChatClient;
use tracing::info;

/// Public async entry for the binary crate.
pub async fn start(config: Config) -> Void {
    info!("Starting triage-bot ...");

    // Start the crypto provider.
    crypto::ring::default_provider().install_default().unwrap();

    // Initialize the runtime.
    let runtime = runtime::Runtime::new(config).await?;

    // Start the runtime.
    runtime.start().await?;

    Ok(())
}
