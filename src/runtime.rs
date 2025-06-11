//! Runtime services and shared state for the triage-bot.

use tracing::instrument;

use crate::service::db::DbClient;
use crate::{base::config::Config, service::mcp::McpClient};
use crate::{
    base::types::{Res, Void},
    service::{chat::ChatClient, llm::LlmClient},
};

/// Runtime service context that can be shared across the application.
///
/// This struct holds the database client, slack client, and configuration.
/// It is designed to be trivially cloneable, allowing it to be passed around
/// without the need for `Arc` or `Mutex`.
#[derive(Clone)]
pub struct Runtime {
    /// The configuration for the application.
    pub config: Config,
    /// The database client instance.
    pub db: DbClient,
    /// The LLM client instance.
    pub llm: LlmClient,
    /// The slack client instance.
    pub chat: ChatClient,
    /// The MCP client instance.
    pub mcp: McpClient,
}

impl Runtime {
    /// Create a new runtime instance.
    #[instrument(name = "Runtime::new", skip_all)]
    pub async fn new(config: Config) -> Res<Self> {
        // Initialize the database.
        let db = DbClient::surreal(&config).await?;

        // Initialize the LLM client.
        let llm = LlmClient::openai(&config);

        // Initialize the MCP client.
        let mcp = McpClient::new(&config.mcp_config_path).await?;

        // Initialize the slack client
        let chat = ChatClient::slack(&config, db.clone(), llm.clone(), mcp.clone()).await?;

        Ok(Self { config, db, llm, chat, mcp })
    }

    pub async fn start(&self) -> Void {
        self.chat.start().await
    }
}
