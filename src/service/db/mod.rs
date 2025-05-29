use std::{ops::Deref, sync::Arc};

use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use surreal::{SurrealChannel, SurrealLlmContext, SurrealMessage};
use surrealdb::method::Stream;

use crate::base::types::Res;

pub mod surreal;

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

// Data type traits.

/// Generic trait for an LLM context in a generic database.
pub trait LlmContext: std::fmt::Debug + Serialize + DeserializeOwned + Clone + PartialEq + Eq + Send + Sync + 'static {
    /// Create a new LLM context.
    fn new(user_message: Value, your_notes: String) -> Self;
    /// Get the context ID.
    fn id(&self) -> Option<String>;
    /// Get the user message.
    fn user_message(&self) -> &Value;
    /// Get the notes.
    fn your_notes(&self) -> &str;
}

/// Generic trait for a channel in a generic database.
pub trait Channel: std::fmt::Debug + Serialize + DeserializeOwned + Clone + PartialEq + Eq + Send + Sync + 'static {
    /// Get the channel ID.
    fn id(&self) -> Option<String>;
    /// Get the channel directive.
    fn channel_directive(&self) -> &impl LlmContext;
}

/// Generic trait for a message in a generic database.
pub trait Message: std::fmt::Debug + Serialize + DeserializeOwned + Clone + PartialEq + Eq + Send + Sync + 'static {
    /// Get the message ID.
    fn id(&self) -> Option<String>;
    /// Get the raw message content.
    fn raw(&self) -> &Value;
}
