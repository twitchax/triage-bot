pub mod slack;

use std::{ops::Deref, sync::Arc};

use async_trait::async_trait;

use crate::base::types::{Res, Void};

// Traits.

/// Generic "chat" trait that clients must implement.
///
/// This trait defines the core functionality for interacting with chat platforms
/// like Slack. Implementing this trait allows different chat services to be used
/// with the triage-bot.
#[async_trait]
pub trait GenericChatClient: Send + Sync + 'static {
    /// Get the bot user ID.
    ///
    /// Returns the unique identifier for the bot in the chat platform,
    /// which is used to detect when the bot is mentioned.
    fn bot_user_id(&self) -> &str;

    /// Start the chat client listener.
    ///
    /// This sets up event listeners for the chat platform and begins processing
    /// incoming messages and events.
    async fn start(&self) -> Void;

    /// Send a message to a channel thread.
    ///
    /// Used to post responses in threads, allowing the bot to reply to user
    /// messages in a structured way.
    async fn send_message(&self, channel_id: &str, thread_ts: &str, text: &str) -> Void;

    /// React to a message with an emoji.
    ///
    /// Adds an emoji reaction to a message, which can be used to indicate
    /// the type of issue or state of a request.
    async fn react_to_message(&self, channel_id: &str, thread_ts: &str, emoji: &str) -> Void;

    /// Get the entirety of the thread context.
    ///
    /// Retrieves all messages in a thread, which provides context for
    /// generating more relevant responses.
    async fn get_thread_context(&self, channel_id: &str, thread_ts: &str) -> Res<String>;
}

// Structs.

/// Slack client for the application.
///
/// It is designed to be trivially cloneable, allowing it to be passed around
/// without the need for `Arc` or `Mutex`.
#[derive(Clone)]
pub struct ChatClient {
    inner: Arc<dyn GenericChatClient>,
}

impl Deref for ChatClient {
    type Target = dyn GenericChatClient;

    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}

impl ChatClient {
    pub fn new(inner: Arc<dyn GenericChatClient>) -> Self {
        Self { inner }
    }
}
