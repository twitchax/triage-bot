//! Thin wrapper around async-openai for OpenAI LLM calls.

use std::{ops::Deref, sync::Arc};

use crate::base::types::{LlmResponse, Res};
use crate::base::{
    config::Config,
    prompts::{get_mention_addendum, get_system_prompt},
};
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage, ChatCompletionRequestSystemMessageContent, ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent,
        CreateChatCompletionRequestArgs,
    },
};
use async_trait::async_trait;
use tracing::{debug, instrument};

// Traits.

/// Generic LLM client trait that clients must implement.
#[async_trait]
pub trait GenericLlmClient {
    /// Generate a response from a static system prompt and user message.
    async fn generate_response(&self, channel_prompt: &str, user_message: &str) -> Res<Vec<LlmResponse>>;
}

// Structs.

/// LLM client for the application.
///
/// This is trivially cloneable and can be passed around without the need for `Arc` or `Mutex`.
#[derive(Clone)]
pub struct LlmClient {
    inner: Arc<dyn GenericLlmClient + Send + Sync + 'static>,
}

impl Deref for LlmClient {
    type Target = dyn GenericLlmClient + Send + Sync + 'static;

    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}

impl LlmClient {
    pub fn openai(config: &Config) -> Self {
        let client = OpenAiLlmClient::new(config);
        Self { inner: Arc::new(client) }
    }
}

// Specific implementations.

/// OpenAI LLM client implementation.
#[derive(Clone)]
pub struct OpenAiLlmClient {
    client: Client<OpenAIConfig>,
    model: String,
    system_prompt: String,
    mention_addendum_prompt: String,
    temperature: f32,
}

impl OpenAiLlmClient {
    pub fn new(config: &Config) -> Self {
        let cfg = OpenAIConfig::new().with_api_key(config.openai_api_key.clone());
        let model = config.openai_model.clone();

        let system_prompt = get_system_prompt(config).to_string();
        let mention_addendum_prompt = get_mention_addendum(config).to_string();

        Self {
            client: Client::with_config(cfg),
            model,
            system_prompt,
            mention_addendum_prompt,
            temperature: config.openai_temperature,
        }
    }
}

#[async_trait]
impl GenericLlmClient for OpenAiLlmClient {
    /// Generate a response from a static system prompt and user message.
    #[instrument(skip(self))]
    async fn generate_response(&self, channel_prompt: &str, user_message: &str) -> Res<Vec<LlmResponse>> {
        debug!("Generating response with system prompt and user message");

        let mut messages = vec![
            ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                content: ChatCompletionRequestSystemMessageContent::Text(self.system_prompt.clone()),
                name: Some("System".to_string()),
            }),
            ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                content: ChatCompletionRequestSystemMessageContent::Text(self.mention_addendum_prompt.clone()),
                name: Some("System".to_string()),
            }),
            ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                content: ChatCompletionRequestSystemMessageContent::Text(channel_prompt.to_string()),
                name: Some("ChannelAdmin".to_string()),
            }),
            ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                content: ChatCompletionRequestUserMessageContent::Text(user_message.to_string()),
                name: Some("User".to_string()),
            }),
        ];

        // Loop over requests until we get a "final" response.
        // For example, the LLM may give a "context needed" or "search needed" response.

        #[allow(clippy::never_loop)]
        let result = loop {
            let request = CreateChatCompletionRequestArgs::default().model(&self.model).messages(messages).temperature(self.temperature).build()?;

            let response = self.client.chat().create(request).await?;
            let content = response.choices.first().and_then(|choice| choice.message.content.clone()).unwrap_or_default();

            // deserialize the response to the `LlmResult` type.
            let result: Vec<LlmResponse> = serde_json::from_str(&content)?;

            // This may change, but for now, always break after one message.
            break result;
        };

        Ok(result)
    }
}
