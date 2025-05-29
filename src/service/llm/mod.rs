pub mod openai;

use crate::base::types::{AssistantContext, AssistantResponse, MessageSearchContext, Res, WebSearchContext};
use async_trait::async_trait;
use std::ops::Deref;
use std::sync::Arc;

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
    async fn get_assistant_agent_response(&self, context: &AssistantContext) -> Res<Vec<AssistantResponse>>;
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
