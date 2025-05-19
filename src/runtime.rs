//! Runtime services and shared state for the triage-bot.

use tracing::instrument;

use crate::{base::config::Config, service::chat::GenericChatClient};
use crate::service::db::DbClient;
use crate::{
    base::types::{Res, Void},
    service::{llm::LlmClient, chat::ChatClient},
};
use std::sync::Arc;

/// Runtime service context that can be shared across the application.
///
/// This struct holds the database client, slack client, and configuration.
/// It is designed to be trivially cloneable, allowing it to be passed around
/// without the need for `Arc` or `Mutex`.
#[derive(Clone)]
pub struct Runtime<D, L, C>
where
    C: GenericChatClient + Send + Sync + 'static,
{
    /// The configuration for the application.
    pub config: Config,
    /// The database client instance.
    pub db: DbClient,
    /// The LLM client instance.
    pub llm: LlmClient,
    /// The slack client instance.
    pub chat: ChatClient<C>,
}

impl Runtime {
    /// Create a new runtime instance.
    #[instrument(skip_all)]
    pub async fn new(config: Config) -> Res<Self> {
        // Initialize the database.
        let db = DbClient::new(&config).await?;

        // Initialize the LLM client.
        let llm = LlmClient::new(&config);

        // Initialize the slack client
        let slack = ChatClient::slack(&config, db.clone(), llm.clone()).await?;

        Ok(Self { config, db, llm, chat })
    }

    pub async fn start(&self) -> Void {
        self.chat.start().await
    }
}
