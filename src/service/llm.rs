//! Thin wrapper around async-openai for OpenAI LLM calls.

use std::{ops::Deref, sync::Arc};

use crate::base::types::{LlmResult, Res};
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
use tracing::{debug, instrument};

/// LLM client for OpenAI API.
///
/// This is trivially cloneable and can be passed around without the need for `Arc` or `Mutex`.
#[derive(Clone)]
pub struct LlmClient {
    inner: Arc<LlmClientInner>,
}

pub struct LlmClientInner {
    client: Client<OpenAIConfig>,
    model: String,
    system_prompt: String,
    mention_addendum_prompt: String,
    temperature: f32,
}

impl Deref for LlmClient {
    type Target = Client<OpenAIConfig>;

    fn deref(&self) -> &Self::Target {
        &self.inner.client
    }
}

impl LlmClient {
    pub fn new(config: &Config) -> Self {
        let cfg = OpenAIConfig::new().with_api_key(config.openai_api_key.clone());
        let model = config.openai_model.clone();

        let system_prompt = get_system_prompt(config).to_string();
        let mention_addendum_prompt = get_mention_addendum(config).to_string();

        Self {
            inner: Arc::new(LlmClientInner {
                client: Client::with_config(cfg),
                model,
                system_prompt,
                mention_addendum_prompt,
                temperature: config.openai_temperature,
            }),
        }
    }

    /// Generate a response from a static system prompt and user message.
    #[instrument(skip(self))]
    pub async fn generate_response(&self, user_message: &str) -> Res<LlmResult> {
        debug!("Generating response with system prompt and user message");

        let mut messages = vec![
            ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                content: ChatCompletionRequestSystemMessageContent::Text(self.inner.system_prompt.clone()),
                name: Some("System".to_string()),
            }),
            ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                content: ChatCompletionRequestSystemMessageContent::Text(self.inner.mention_addendum_prompt.clone()),
                name: Some("System".to_string()),
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
            let request = CreateChatCompletionRequestArgs::default().model(&self.inner.model).messages(messages).temperature(0.7).build()?;

            let response = self.inner.client.chat().create(request).await?;
            let content = response.choices.first().and_then(|choice| choice.message.content.clone()).unwrap_or_default();

            // deserialize the response to the `LlmResult` type.
            let result: LlmResult = serde_json::from_str(&content)?;

            // This may change, but for now, always break after one message.
            break result;
        };

        Ok(result)
    }
}
