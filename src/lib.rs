//! Library root for `triage-bot`.
//!
//! Triage-bot is an OpenAI-powered assistant for Slack support channels designed to:
//! - Tag oncalls and notify appropriate team members
//! - Prioritize and classify support issues
//! - Suggest solutions based on context and web searches
//! - Streamline communication by providing relevant information
//!
//! The bot integrates with Slack for chat, SurrealDB for storage,
//! and OpenAI for intelligent responses. The architecture is built around
//! extensible traits that allow for different implementations of each service.

#[deny(missing_docs)]
pub mod base;
pub mod interaction;
pub mod runtime;
pub mod service;

use base::{config::Config, types::Void};
use rustls::crypto;
use tracing::info;

/// Public async entry for the binary crate.
///
/// Sets up necessary services and starts the triage-bot runtime:
/// - Initializes the crypto provider
/// - Creates the runtime context with database, LLM, and chat clients
/// - Starts the main event loop for processing messages
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
