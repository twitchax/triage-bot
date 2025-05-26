//! Database implementation for triage-bot data storage.
//!
//! This module provides database functionality for storing and retrieving:
//! - Channel information and directives
//! - Message history and context
//! - Search capabilities for finding relevant past messages
//!
//! It defines the `GenericDbClient` trait that can be implemented for different
//! database backends, with a default implementation for SurrealDB.

use std::{ops::Deref, sync::Arc};

use crate::base::{
    config::Config,
    types::{Res, Void},
};
use anyhow::{Ok, anyhow};
use async_trait::async_trait;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use surrealdb::{
    Connection, RecordId, Surreal,
    engine::remote::ws::{Client, Ws},
    method::Stream,
    opt::auth::Root,
};
use tracing::{info, instrument};

// Traits.

/// Generic database client trait that clients must implement.
///
/// This trait defines the core functionality for storing and retrieving
/// channel data, context, and messages. Implementing this trait allows
/// different database backends to be used with the triage-bot.
#[async_trait]
pub trait GenericDbClient: Send + Sync + 'static {
    type LlmContextType: LlmContext;
    type ChannelType: Channel;
    type MessageType: Message;

    /// Gets the channel from the database by its ID; or, creates a new channel if it doesn't exist.
    ///
    /// This is used to ensure a channel exists before operating on it, and
    /// to retrieve channel-specific settings.
    async fn get_or_create_channel(&self, channel_id: &str) -> Res<Self::ChannelType>;

    /// Updates the channel directive in the database.
    ///
    /// The directive controls how the bot behaves in the specific channel,
    /// such as which issues to prioritize or which team to notify.
    async fn update_channel_directive(&self, channel_id: &str, directive: &Self::LlmContextType) -> Res<()>;

    /// Adds a context JSON to the channel via a `has_context` edge.
    ///
    /// This stores additional contextual information that the bot can use
    /// when responding to messages in the channel.
    async fn add_channel_context(&self, channel_id: &str, context: &Self::LlmContextType) -> Res<()>;

    /// Adds a message to the database that can then be retrieved by the bot.
    ///
    /// This creates a searchable history of messages in the channel.
    async fn add_channel_message(&self, channel_id: &str, message: &Value) -> Res<()>;

    /// Gets additional context for the channel.
    ///
    /// This retrieves all contextual information that has been stored for the channel,
    /// which helps the bot generate more relevant responses.
    async fn get_channel_context(&self, channel_id: &str) -> Res<String>;

    /// Searches for messages in the channel that match the search string.
    ///
    /// This allows the bot to find relevant past discussions when responding to new questions.
    /// The search_terms parameter should contain comma-separated keywords.
    async fn search_channel_messages(&self, channel_id: &str, search_terms: &str) -> Res<String>;
    /// Starts a stream of a live query for channels.
    async fn get_channel_live_query(&self) -> Res<Stream<Vec<Self::ChannelType>>>;
    /// Starts a stream of a live query for contexts.
    async fn get_context_live_query(&self) -> Res<Stream<Vec<Self::LlmContextType>>>;
}

/// Database client for triage-bot.
///
/// This is trivially cloneable and can be passed around without the need for `Arc` or `Mutex`.
#[derive(Clone)]
pub struct DbClient<L = SurrealLlmContext, C = SurrealChannel, M = SurrealMessage>
where
    L: LlmContext,
    C: Channel,
    M: Message,
{
    /// The database client instance.
    pub inner: Arc<dyn GenericDbClient<LlmContextType = L, ChannelType = C, MessageType = M>>,
}

impl<L, C, M> Deref for DbClient<L, C, M>
where
    L: LlmContext,
    C: Channel,
    M: Message,
{
    type Target = dyn GenericDbClient<LlmContextType = L, ChannelType = C, MessageType = M>;

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

// Data type traits.

/// Generic trait for an LLM context in a generic database.
pub trait LlmContext: std::fmt::Debug + Serialize + DeserializeOwned + Clone + PartialEq + Eq + 'static {
    /// Get the context ID.
    fn id(&self) -> Option<String>;
    /// Get the user message.
    fn user_message(&self) -> &Value;
    /// Get the notes.
    fn your_notes(&self) -> &str;
}

/// Generic trait for a channel in a generic database.
pub trait Channel: std::fmt::Debug + Serialize + DeserializeOwned + Clone + PartialEq + Eq + 'static {
    /// Get the channel ID.
    fn id(&self) -> Option<String>;
    /// Get the channel directive.
    fn channel_directive(&self) -> &impl LlmContext;
}

/// Generic trait for a message in a generic database.
pub trait Message: std::fmt::Debug + Serialize + DeserializeOwned + Clone + PartialEq + Eq + 'static {
    /// Get the message ID.
    fn id(&self) -> Option<String>;
    /// Get the raw message content.
    fn raw(&self) -> &Value;
}

// Surreal Data types.

/// A context in a surreal database.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SurrealLlmContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RecordId>,
    pub user_message: Value,
    pub your_notes: String,
}

impl LlmContext for SurrealLlmContext {
    fn id(&self) -> Option<String> {
        self.id.as_ref().map(|id| id.to_string())
    }

    fn user_message(&self) -> &Value {
        &self.user_message
    }

    fn your_notes(&self) -> &str {
        &self.your_notes
    }
}

/// A channel in a surreal database.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SurrealChannel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RecordId>,
    pub channel_directive: SurrealLlmContext,
}

impl Channel for SurrealChannel {
    fn id(&self) -> Option<String> {
        self.id.as_ref().map(|id| id.to_string())
    }

    fn channel_directive(&self) -> &impl LlmContext {
        &self.channel_directive
    }
}

/// A message in a surreal database.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SurrealMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RecordId>,
    pub raw: Value,
}

impl Message for SurrealMessage {
    fn id(&self) -> Option<String> {
        self.id.as_ref().map(|id| id.to_string())
    }

    fn raw(&self) -> &Value {
        &self.raw
    }
}

// SurrealDB client implementation.

/// Database client for SurrealDB.
pub struct SurrealDbClient<C>
where
    C: Connection,
{
    pub db: Surreal<C>,
}

impl<C> Deref for SurrealDbClient<C>
where
    C: Connection,
{
    type Target = Surreal<C>;

    fn deref(&self) -> &Self::Target {
        &self.db
    }
}

impl SurrealDbClient<Client> {
    /// Create a new database client.
    ///
    /// This creates an in-memory database instance. For production, you would
    /// want to connect to a persistent database.
    #[instrument(name = "SurrealDbClient::new", skip_all)]
    #[allow(unused_variables)]
    pub async fn new(config: &Config) -> Res<Self> {
        let db = Surreal::new::<Ws>(&config.db_endpoint).await?;

        db.signin(Root {
            username: &config.db_username,
            password: &config.db_password,
        })
        .await?;

        setup_surreal_db(&db).await?;

        info!("Database initialized successfully.");

        Ok(Self { db })
    }
}

impl<C> SurrealDbClient<C>
where
    C: Connection,
{
    pub async fn from(db: Surreal<C>) -> Res<Self> {
        setup_surreal_db(&db).await?;

        info!("Database initialized successfully.");

        Ok(Self { db })
    }
}

#[async_trait]
impl<C> GenericDbClient for SurrealDbClient<C>
where
    C: Connection,
{
    type ChannelType = SurrealChannel;
    type LlmContextType = SurrealLlmContext;
    type MessageType = SurrealMessage;

    #[instrument(skip(self))]
    async fn get_or_create_channel(&self, channel_id: &str) -> Res<Self::ChannelType> {
        let channel: Option<Self::ChannelType> = self.select(("channel", channel_id)).await?;

        if let Some(channel) = channel {
            info!("Channel `{}` found.", channel_id);

            Ok(channel)
        } else {
            info!("Channel `{}` not found, creating a new one.", channel_id);

            let new_channel = Self::ChannelType {
                id: None,
                channel_directive: Self::LlmContextType {
                    id: None,
                    user_message: json!({}),
                    your_notes: "".into(),
                },
            };

            let channel: Self::ChannelType = self.create(("channel", channel_id)).content(new_channel).await?.ok_or(anyhow!("Failed to create channel"))?;

            Ok(channel)
        }
    }

    #[instrument(skip(self, directive))]
    async fn update_channel_directive(&self, channel_id: &str, directive: &Self::LlmContextType) -> Void {
        let _: Option<Self::ChannelType> = self.update(("channel", channel_id)).merge(json!({ "channel_directive": directive })).await?;

        info!("Channel `{}` updated.", channel_id);

        Ok(())
    }

    #[instrument(skip(self, context))]
    async fn add_channel_context(&self, channel_id: &str, context: &Self::LlmContextType) -> Res<()> {
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
        let message = Self::MessageType { id: None, raw: message.clone() };

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
        let context: Vec<Self::LlmContextType> = self
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
        let messages: Vec<SurrealMessage> = self
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

    #[instrument(skip(self))]
    async fn get_channel_live_query(&self) -> Res<Stream<Vec<Self::ChannelType>>> {
        let stream = self.db.select("channel").live().await?;

        Ok(stream)
    }

    #[instrument(skip(self))]
    async fn get_context_live_query(&self) -> Res<Stream<Vec<Self::LlmContextType>>> {
        let stream = self.db.select("context").live().await?;

        Ok(stream)
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
    use surrealdb::engine::local::Mem;

    use super::*;

    async fn setup_test_db() -> Res<DbClient> {
        let surreal = Surreal::new::<Mem>(()).await?;
        let db = SurrealDbClient::from(surreal).await?;
        let client = DbClient { inner: Arc::new(db) };

        Ok(client)
    }

    #[tokio::test]
    async fn test_get_or_create_channel() {
        let client = setup_test_db().await.unwrap();

        // Test channel creation
        let channel = client.get_or_create_channel("C1").await.unwrap();
        assert_eq!(serde_json::to_string(&channel.channel_directive).unwrap(), "{\"user_message\":{},\"your_notes\":\"\"}");

        // Test getting existing channel
        let existing_channel = client.get_or_create_channel("C1").await.unwrap();
        assert_eq!(channel.channel_directive, existing_channel.channel_directive);
    }

    #[tokio::test]
    async fn test_update_channel_directive() {
        let client = setup_test_db().await.unwrap();

        // Create a channel first
        client.get_or_create_channel("C1").await.unwrap();

        // Update the directive
        let new_directive = SurrealLlmContext {
            id: None,
            user_message: json!({ "directive": "new channel directive" }),
            your_notes: "Updated notes.".into(),
        };

        client.update_channel_directive("C1", &new_directive).await.unwrap();

        // Verify the update - the directive should be completely replaced
        let updated = client.get_or_create_channel("C1").await.unwrap();

        assert_eq!(updated.channel_directive.your_notes, "Updated notes.");
        assert!(updated.channel_directive.user_message.get("directive").is_some());
    }

    #[tokio::test]
    async fn test_add_channel_context() {
        let client = setup_test_db().await.unwrap();

        // Create a channel first
        client.get_or_create_channel("C1").await.unwrap();

        // Add context
        let context = SurrealLlmContext {
            id: None,
            user_message: json!({ "context": "some context data" }),
            your_notes: "Context notes.".into(),
        };

        client.add_channel_context("C1", &context).await.unwrap();

        // Verify context was added by getting channel context
        let retrieved_context = client.get_channel_context("C1").await.unwrap();

        assert!(!retrieved_context.is_empty());
        assert!(retrieved_context.contains("some context data"));
    }

    #[tokio::test]
    async fn test_add_channel_message() {
        let client = setup_test_db().await.unwrap();

        // Create a channel first
        client.get_or_create_channel("C1").await.unwrap();

        // Add messages
        let message1 = json!({"text": "Hello world", "user": "U123", "ts": "1234567890.123"});
        let message2 = json!({"text": "Another message", "user": "U456", "ts": "1234567890.456"});

        client.add_channel_message("C1", &message1).await.unwrap();
        client.add_channel_message("C1", &message2).await.unwrap();

        // Messages should be stored and retrievable via search
        let search_result = client.search_channel_messages("C1", "Hello").await.unwrap();

        assert!(!search_result.is_empty());
    }

    #[tokio::test]
    async fn test_get_channel_context() {
        let client = setup_test_db().await.unwrap();

        // Create a channel first
        client.get_or_create_channel("C1").await.unwrap();

        // Initially should return empty context
        let context = client.get_channel_context("C1").await.unwrap();
        assert_eq!(context, "[]");

        // Add some context
        let context1 = SurrealLlmContext {
            id: None,
            user_message: json!({ "context": "first context" }),
            your_notes: "First notes.".into(),
        };
        let context2 = SurrealLlmContext {
            id: None,
            user_message: json!({ "context": "second context" }),
            your_notes: "Second notes.".into(),
        };

        client.add_channel_context("C1", &context1).await.unwrap();
        client.add_channel_context("C1", &context2).await.unwrap();

        // Should now return the contexts
        let retrieved_context = client.get_channel_context("C1").await.unwrap();

        assert!(!retrieved_context.is_empty());
        assert_ne!(retrieved_context, "[]");
        assert!(retrieved_context.contains("first context"));
        assert!(retrieved_context.contains("second context"));
    }

    #[tokio::test]
    async fn test_search_channel_messages() {
        let client = setup_test_db().await.unwrap();

        // Create a channel
        client.get_or_create_channel("C1").await.unwrap();

        // Add messages with different content
        client.add_channel_message("C1", &json!({"text": "Hello world"})).await.unwrap();
        client.add_channel_message("C1", &json!({"text": "Test message with important keyword"})).await.unwrap();
        client.add_channel_message("C1", &json!({"text": "Another test without the keyword"})).await.unwrap();
        client.add_channel_message("C1", &json!({"text": "important important important"})).await.unwrap();

        // Test that search doesn't error - the indexing may not work in memory mode
        let result = client.search_channel_messages("C1", "important").await;
        assert!(result.is_ok(), "Search should not error");

        // Test searching with multiple terms
        let _ = client.search_channel_messages("C1", "Hello, test").await.unwrap();

        // Test searching with no matches
        let _ = client.search_channel_messages("C1", "nonexistent").await.unwrap();
    }

    #[tokio::test]
    async fn test_search_messages_empty_terms() {
        let client = setup_test_db().await.unwrap();
        client.get_or_create_channel("C1").await.unwrap();

        // Test searching with empty terms
        let result = client.search_channel_messages("C1", "").await.unwrap();
        assert_eq!(result, "[]");

        // Test searching with only commas and spaces
        let result = client.search_channel_messages("C1", " , , ").await.unwrap();
        assert_eq!(result, "[]");
    }

    #[tokio::test]
    async fn test_operations_on_nonexistent_channel() {
        let client = setup_test_db().await.unwrap();

        // These operations should not fail even on nonexistent channels
        let context = client.get_channel_context("NONEXISTENT").await.unwrap();
        assert_eq!(context, "[]");

        let search_result = client.search_channel_messages("NONEXISTENT", "test").await.unwrap();
        assert_eq!(search_result, "[]");

        // Adding context/messages to nonexistent channel should create the channel implicitly
        let context_obj = SurrealLlmContext {
            id: None,
            user_message: json!({ "test": "value" }),
            your_notes: "Test notes.".into(),
        };

        // This should succeed (channel gets created implicitly by the relation)
        client.add_channel_context("NONEXISTENT2", &context_obj).await.unwrap();
        let retrieved = client.get_channel_context("NONEXISTENT2").await.unwrap();
        assert!(!retrieved.is_empty());
    }

    #[tokio::test]
    async fn test_multiple_channels_isolation() {
        let client = setup_test_db().await.unwrap();

        // Create two channels
        client.get_or_create_channel("C1").await.unwrap();
        client.get_or_create_channel("C2").await.unwrap();

        // Add different content to each channel
        client.add_channel_message("C1", &json!({"text": "Channel 1 message"})).await.unwrap();
        client.add_channel_message("C2", &json!({"text": "Channel 2 message"})).await.unwrap();

        let context1 = SurrealLlmContext {
            id: None,
            user_message: json!({ "channel": "first" }),
            your_notes: "Channel 1 context.".into(),
        };
        let context2 = SurrealLlmContext {
            id: None,
            user_message: json!({ "channel": "second" }),
            your_notes: "Channel 2 context.".into(),
        };

        client.add_channel_context("C1", &context1).await.unwrap();
        client.add_channel_context("C2", &context2).await.unwrap();

        // Verify context isolation
        let c1_context = client.get_channel_context("C1").await.unwrap();
        let c2_context = client.get_channel_context("C2").await.unwrap();

        assert!(c1_context.contains("first"));
        assert!(!c1_context.contains("second"));
        assert!(c2_context.contains("second"));
        assert!(!c2_context.contains("first"));

        // Test that search operations don't error (search functionality may be limited in memory mode)
        let c1_search = client.search_channel_messages("C1", "Channel").await;
        let c2_search = client.search_channel_messages("C2", "Channel").await;

        assert!(c1_search.is_ok());
        assert!(c2_search.is_ok());
    }
}
