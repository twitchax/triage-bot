pub mod openai;

use crate::base::types::{AssistantContext, AssistantResponse, MessageSearchContext, Res, Void, WebSearchContext};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use std::{ops::Deref, pin::Pin};

// Types.

pub type BoxedCallback = Box<dyn Fn(Vec<AssistantResponse>) -> Pin<Box<dyn Future<Output = Res<Option<Value>>> + Send>> + Send + Sync>;

// Traits.

/// Generic LLM client trait that clients must implement.
///
/// This trait defines the core functionality for interacting with large language models.
/// Implementing this trait allows different LLM providers to be used with the triage-bot.
#[async_trait]
pub trait GenericLlmClient: Send + Sync + 'static {
    /// Execute a web search using the search agent.
    ///
    /// This method takes search context about a user message and returns
    /// relevant information from web searches to help answer the query.
    async fn get_web_search_agent_response(&self, context: &WebSearchContext) -> Res<String>;

    /// Generate search terms for message search using the message search agent.
    ///
    /// This method analyzes a user message and extracts key search terms that
    /// can be used to find relevant past messages in the channel history.
    async fn get_message_search_agent_response(&self, context: &MessageSearchContext) -> Res<String>;

    /// Generate a response from the primary assistant model.
    ///
    /// This method takes a comprehensive context about the user's message,
    /// channel settings, web search results, and message search results, then
    /// generates appropriate responses or actions.
    ///
    /// The response callback is used to process the generated response asynchronously.
    /// It allows the client to handle the response in a non-blocking manner.
    ///
    /// The response callback should return a `Value` that represents any "message" back
    /// to the model.
    async fn get_assistant_agent_response(&self, context: &AssistantContext, response_callback: BoxedCallback) -> Void;
}

// Structs.

/// LLM client for the application.
///
/// This is trivially cloneable and can be passed around without the need for `Arc` or `Mutex`.
#[derive(Clone)]
pub struct LlmClient {
    inner: Arc<dyn GenericLlmClient>,
}

impl Deref for LlmClient {
    type Target = dyn GenericLlmClient;

    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}
