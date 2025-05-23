//! Database implementation for triage-bot data storage.

use std::{ops::Deref, sync::Arc};

use crate::base::{
    config::Config,
    types::{Res, Void},
};
use anyhow::{Ok, anyhow};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use surrealdb::{
    Connection, RecordId, Surreal,
    engine::any::{self, Any},
    opt::auth::Root,
};
use tracing::{info, instrument};

// Traits.

/// Generic database client trait that clients must implement.
#[async_trait]
pub trait GenericDbClient: Send + Sync + 'static {
    /// Gets the channel from the database by its ID; or, creates a new channel if it doesn't exist.
    async fn get_or_create_channel(&self, channel_id: &str) -> Res<Channel>;
    /// Updates the channel prompt in the database.
    async fn update_channel_directive(&self, channel_id: &str, directive: &LlmContext) -> Res<()>;
    /// Adds a context JSON to the channel via a `has_context` edge.
    async fn add_channel_context(&self, channel_id: &str, context: &LlmContext) -> Res<()>;
    /// Adds a message to the database that can then be retrieved by the bot.
    async fn add_channel_message(&self, channel_id: &str, message: &Value) -> Res<()>;
    /// Gets additional context for the channel.
    async fn get_channel_context(&self, channel_id: &str) -> Res<String>;
    /// Searches for messages in the channel that match the search string.
    async fn search_channel_messages(&self, channel_id: &str, search_terms: &str) -> Res<String>;
}

/// Database client for triage-bot.
///
/// This is trivially cloneable and can be passed around without the need for `Arc` or `Mutex`.
#[derive(Clone)]
pub struct DbClient {
    /// The database client instance.
    pub inner: Arc<dyn GenericDbClient>,
}

impl Deref for DbClient {
    type Target = dyn GenericDbClient;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref()
    }
}

impl DbClient {
    /// Create a new database client.
    #[instrument(skip_all)]
    pub async fn surreal(config: &Config) -> Res<Self> {
        let db = SurrealDbClient::new(Some(config)).await?;
        Ok(Self { inner: Arc::new(db) })
    }

    /// Create a new in-memory database client.
    #[instrument(skip_all)]
    pub async fn surreal_memory() -> Res<Self> {
        let db = SurrealDbClient::new(None).await?;
        Ok(Self { inner: Arc::new(db) })
    }
}

// Data types.

// A Context in the database.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct LlmContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RecordId>,
    pub user_message: Value,
    pub your_notes: String,
}

// A Channel in the database.
#[derive(Debug, Serialize, Deserialize)]
pub struct Channel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RecordId>,
    pub channel_directive: LlmContext,
}

/// A message in the database.
#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RecordId>,
    pub raw: Value,
}

// SurrealDB client implementation.

/// Database client for SurrealDB.
pub struct SurrealDbClient {
    db: Surreal<Any>,
}

impl Deref for SurrealDbClient {
    type Target = Surreal<Any>;

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
    pub async fn new(config: Option<&Config>) -> Res<Self> {
        let db = if let Some(config) = config {
            let db = any::connect(&config.db_endpoint).await?;

            db.signin(Root {
                username: &config.db_username,
                password: &config.db_password,
            })
            .await?;

            db
        } else {
            any::connect("memory").await?
        };

        setup_surreal_db(&db).await?;

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
        let mut response = self
            .db
            .query("BEGIN TRANSACTION;")
            .query("LET $channel = type::thing('channel', $channel_id);")
            .query("LET $context = (CREATE context CONTENT $context_content).id;")
            .query("RELATE $channel->has_context->$context;")
            .query("COMMIT;")
            .bind(("context_content", context.clone()))
            .bind(("channel_id", channel_id.to_string()))
            .await?;

        let errors = response.take_errors();
        if !errors.is_empty() {
            return Err(anyhow!("Failed to add message to channel `{}`: {:#?}.", channel_id, errors));
        }

        info!("Added context for channel `{}`.", channel_id);

        Ok(())
    }

    #[instrument(skip(self, message))]
    async fn add_channel_message(&self, channel_id: &str, message: &Value) -> Res<()> {
        let message = Message { id: None, raw: message.clone() };

        let mut response = self
            .db
            .query("BEGIN TRANSACTION;")
            .query("LET $channel = type::thing('channel', $channel_id);")
            .query("LET $message = (CREATE message CONTENT $message_content).id;")
            .query("RELATE $channel->has_message->$message;")
            .query("COMMIT;")
            .bind(("message_content", message))
            .bind(("channel_id", channel_id.to_string()))
            .await?;

        let errors = response.take_errors();
        if !errors.is_empty() {
            return Err(anyhow!("Failed to add message to channel `{}`: {:#?}.", channel_id, errors));
        }

        info!("Added message for channel `{}`.", channel_id);

        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_channel_context(&self, channel_id: &str) -> Res<String> {
        let context: Vec<LlmContext> = self
            .db
            .query("SELECT * FROM type::thing('channel', $channel_id)->has_context->context;")
            .bind(("channel_id", channel_id.to_string()))
            .await?
            .take(0)?;

        let result = serde_json::to_string(&context)?;

        info!("Retrieved context for channel `{}`.", channel_id);

        Ok(result)
    }

    #[instrument(skip(self))]
    async fn search_channel_messages(&self, channel_id: &str, search_terms: &str) -> Res<String> {
        let terms: Vec<String> = search_terms.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();

        if terms.is_empty() {
            return Ok("[]".to_string()); // Return empty array if no terms
        }

        // Generate the query parts.

        let mut score_list = vec![];
        let mut filter_list = vec![];
        for (k, term) in terms.iter().enumerate() {
            score_list.push(format!("search::score({k})"));
            filter_list.push(format!("raw.text @{k}@ '{term}'"));
        }

        let score = score_list.join(" + ");
        let filter = filter_list.join(" OR ");

        // Format the search terms for SurrealDB full-text search
        // Convert each term to a quoted string and join with OR
        let query_str = terms.iter().map(|term| format!("\"{term}\"")).collect::<Vec<String>>().join(" OR ");

        // Get messages from the channel that match the search terms
        // Use the full-text search capabilities
        let messages: Vec<Message> = self
            .db
            .query(format!(
                r####"
                    let $messages = SELECT id FROM type::thing('channel', $channel_id)->has_message.out.id;
                    let $messages = array::flatten($messages[*].id);

                    SELECT *, {score} AS score
                    FROM message
                    WHERE id in $messages AND ({filter})
                    ORDER BY score DESC
                    LIMIT 50;
                "####,
            ))
            .bind(("channel_id", channel_id.to_string()))
            .bind(("query_str", query_str))
            .await?
            .take(2)?;

        let result = serde_json::to_string(&messages)?;

        info!("Retrieved {} ranked messages for channel `{}` matching search terms: {}", messages.len(), channel_id, search_terms);

        Ok(result)
    }
}

// Helpers.

/// Set up the surreal database.
async fn setup_surreal_db<C: Connection>(db: &Surreal<C>) -> Void {
    // Use a specific namespace and database
    db.use_ns("triage").use_db("bot").await?;

    // Schema for contexts.
    db.query("DEFINE TABLE context SCHEMAFULL").await?;
    db.query("DEFINE FIELD user_message ON context FLEXIBLE TYPE object;").await?;
    db.query("DEFINE FIELD your_notes ON context TYPE string;").await?;

    // Schema for messages.
    db.query("DEFINE TABLE message SCHEMAFULL").await?;
    db.query("DEFINE FIELD raw ON message FLEXIBLE TYPE object;").await?;
    db.query("DEFINE FIELD raw.text ON message TYPE string;").await?;

    // Define analyzer for full-text search
    db.query("DEFINE ANALYZER en TOKENIZERS class FILTERS lowercase, snowball(english);").await?;

    // Define full-text search index for message text
    db.query("DEFINE INDEX rawTextFts ON TABLE message FIELDS raw.text SEARCH ANALYZER en BM25;").await?;

    // Schema for list of channels that the bot has been "added to" (@-mentioned).
    db.query("DEFINE TABLE channel SCHEMAFULL").await?;
    db.query("DEFINE FIELD channel_directive ON channel TYPE object;").await?;
    db.query("DEFINE FIELD channel_directive.user_message ON channel FLEXIBLE TYPE object;").await?;
    db.query("DEFINE FIELD channel_directive.your_notes ON channel TYPE string;").await?;

    // Schema for the relation between channels and contexts.
    db.query("DEFINE TABLE has_context TYPE RELATION IN channel OUT context;").await?;

    // Schema for the relation between channels and messages.
    db.query("DEFINE TABLE has_message TYPE RELATION IN channel OUT message;").await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_channel() {
        let client = DbClient::surreal_memory().await.unwrap();
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

    #[tokio::test]
    async fn test_search_messages() {
        let client = DbClient::surreal_memory().await.unwrap();

        // Add a channel
        client.get_or_create_channel("C1").await.unwrap();

        // Add messages to the channel with clear text fields
        client.add_channel_message("C1", &json!({"text": "Hello world"})).await.unwrap();
        client.add_channel_message("C1", &json!({"text": "Test message with important keyword"})).await.unwrap();
        client.add_channel_message("C1", &json!({"text": "Another test without the keyword"})).await.unwrap();
        client.add_channel_message("C1", &json!({"text": "important important important"})).await.unwrap();

        // Test searching with a single term
        let result = client.search_channel_messages("C1", "important").await;
        assert!(result.is_ok(), "Search messages should not error");

        // Test searching with multiple terms
        let result = client.search_channel_messages("C1", "Hello, test").await;
        assert!(result.is_ok(), "Search messages should not error with multiple terms");
    }
}
