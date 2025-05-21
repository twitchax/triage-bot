//! Database implementation for triage-bot data storage.

use std::{ops::Deref, sync::Arc};

use crate::base::{
    config::Config,
    types::{Res, Void},
};
use anyhow::anyhow;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use surrealdb::Surreal;
#[cfg(test)]
use surrealdb::engine::local::{Db as DbConnection, Mem};
#[cfg(not(test))]
use surrealdb::{
    engine::remote::ws::{Client as DbConnection, Ws},
    opt::auth::Root,
};
use tracing::{info, instrument};

// Traits.

/// Generic database client trait that clients must implement.
#[async_trait]
pub trait GenericDbClient {
    /// Gets the channel from the database by its ID; or, creates a new channel if it doesn't exist.
    async fn get_or_create_channel(&self, channel_id: &str) -> Res<Channel>;
    /// Updates the channel prompt in the database.
    async fn update_channel_directive(&self, channel_id: &str, directive: &LlmContext) -> Res<()>;
    /// Adds a context JSON to the channel via a `has_context` edge.
    async fn add_channel_context(&self, channel_id: &str, context: &LlmContext) -> Res<()>;
}

/// Database client for triage-bot.
///
/// This is trivially cloneable and can be passed around without the need for `Arc` or `Mutex`.
#[derive(Clone)]
pub struct DbClient {
    /// The database client instance.
    inner: Arc<dyn GenericDbClient + Send + Sync + 'static>,
}

impl Deref for DbClient {
    type Target = dyn GenericDbClient + Send + Sync + 'static;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref()
    }
}

impl DbClient {
    /// Create a new database client.
    #[instrument(skip_all)]
    pub async fn surreal(config: &Config) -> Res<Self> {
        let db = SurrealDbClient::new(config).await?;
        Ok(Self { inner: Arc::new(db) })
    }
}

// Data types.

// A Context in the database.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct LlmContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<surrealdb::sql::Thing>,
    pub user_message: Value,
    pub your_notes: String,
}

// A Channel in the database.
#[derive(Debug, Serialize, Deserialize)]
pub struct Channel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<surrealdb::sql::Thing>,
    pub channel_directive: LlmContext,
}

// SurrealDB client implementation.

/// Database client for SurrealDB.
pub struct SurrealDbClient {
    db: Surreal<DbConnection>,
}

impl Deref for SurrealDbClient {
    type Target = Surreal<DbConnection>;

    fn deref(&self) -> &Self::Target {
        &self.db
    }
}

impl SurrealDbClient {
    /// Create a new database client.
    ///
    /// This creates an in-memory database instance. For production, you would
    /// want to connect to a persistent database.
    #[instrument(name = "SurrealDbClient::new", skip_all)]
    #[allow(unused_variables)]
    pub async fn new(config: &Config) -> Res<Self> {
        // Create the database connection
        #[cfg(not(test))]
        let db = Surreal::new::<Ws>(&config.db_endpoint).await?;
        #[cfg(test)]
        let db = Surreal::new::<Mem>(()).await?;

        // Authenticate with the database using the provided username and password.
        #[cfg(not(test))]
        db.signin(Root {
            username: &config.db_username,
            password: &config.db_password,
        })
        .await?;

        // Use a specific namespace and database
        db.use_ns("triage").use_db("bot").await?;

        // Define schemas.

        // Schema for contexts.
        db.query("DEFINE TABLE context SCHEMAFULL").await?;
        db.query("DEFINE FIELD user_message ON context TYPE object;").await?;
        db.query("DEFINE FIELD your_notes ON context TYPE string;").await?;

        // Schema for list of channels that the bot has been "added to" (@-mentioned).
        db.query("DEFINE TABLE channel SCHEMAFULL").await?;
        db.query("DEFINE FIELD channel_directive ON channel TYPE object;").await?;
        db.query("DEFINE FIELD channel_directive.user_message ON channel FLEXIBLE TYPE object;").await?;
        db.query("DEFINE FIELD channel_directive.your_notes ON channel TYPE string;").await?;

        // Schema for the relation between channels and contexts.
        db.query("DEFINE TABLE has_context TYPE RELATION IN channel OUT context;").await?;

        info!("Database initialized successfully.");

        Ok(Self { db })
    }
}

#[async_trait]
impl GenericDbClient for SurrealDbClient {
    #[instrument(skip(self))]
    async fn get_or_create_channel(&self, channel_id: &str) -> Res<Channel> {
        let channel: Option<Channel> = self.select(("channel", channel_id)).await?;

        if let Some(channel) = channel {
            info!("Channel `{}` found.", channel_id);

            Ok(channel)
        } else {
            info!("Channel `{}` not found, creating a new one.", channel_id);

            let new_channel = Channel {
                id: None,
                channel_directive: LlmContext {
                    id: None,
                    user_message: json!({ "ignore": "Channel directive has not been set yet." }),
                    your_notes: "No notes.".into(),
                },
            };

            let channel: Channel = self.create(("channel", channel_id)).content(new_channel).await?.ok_or(anyhow!("Failed to create channel"))?;

            Ok(channel)
        }
    }

    #[instrument(skip(self, directive))]
    async fn update_channel_directive(&self, channel_id: &str, directive: &LlmContext) -> Void {
        let _: Option<Channel> = self.update(("channel", channel_id)).merge(json!({ "channel_directive": directive })).await?;

        info!("Channel `{}` updated.", channel_id);

        Ok(())
    }

    #[instrument(skip(self, context))]
    async fn add_channel_context(&self, channel_id: &str, context: &LlmContext) -> Res<()> {
        let query = r#"
            RELATE channel:$channel_id
            ->has_context
            ->(CREATE context CONTENT $context)
        "#;

        self.query(query).bind(("channel_id", channel_id.to_string())).bind(("context", context.clone())).await?;

        info!("Added context for channel `{}`.", channel_id);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::config::{Config, ConfigInner};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_create_channel() {
        let cfg = Config {
            inner: Arc::new(ConfigInner {
                openai_api_key: String::new(),
                openai_model: "test".to_string(),
                system_prompt: None,
                mention_addendum_prompt: None,
                openai_temperature: 0.7,
                openai_max_tokens: 2048u32,
                slack_app_token: String::new(),
                slack_bot_token: String::new(),
                slack_signing_secret: String::new(),
                db_endpoint: String::new(),
                db_username: String::new(),
                db_password: String::new(),
            }),
        };

        let client = SurrealDbClient::new(&cfg).await.unwrap();
        let channel = client.get_or_create_channel("C1").await.unwrap();
        assert!(dbg!(serde_json::to_string(&channel.channel_directive).unwrap()).contains("Channel directive has not been set yet."));

        client
            .update_channel_directive(
                "C1",
                &LlmContext {
                    id: None,
                    user_message: json!({ "ignore": "new" }),
                    your_notes: "No notes.".into(),
                },
            )
            .await
            .unwrap();

        let updated = client.get_or_create_channel("C1").await.unwrap();
        assert_eq!(
            updated.channel_directive,
            LlmContext {
                id: None,
                user_message: json!({ "ignore": "new" }),
                your_notes: "No notes.".into()
            }
        );
    }
}
