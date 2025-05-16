//! Thin wrapper around async-openai for OpenAI LLM calls.

use async_openai::{
    Client, 
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, 
        ChatCompletionRequestSystemMessage,
        ChatCompletionRequestSystemMessageContent,
        ChatCompletionRequestUserMessage, 
        ChatCompletionRequestUserMessageContent,
        CreateChatCompletionRequestArgs,
    }
};
use tracing::debug;
use crate::base::config::Config;
use crate::base::types::Res;

pub struct LlmClient {
    client: Client<OpenAIConfig>,
    model: String,
}

impl LlmClient {
    pub fn new(config: &Config) -> Self {
        let cfg = OpenAIConfig::new().with_api_key(config.openai_api_key.clone());
        let model = config.openai_model.clone();
        Self { client: Client::with_config(cfg), model }
    }

    /// Generate a response from a static system prompt and user message.
    pub async fn generate_response(&self, system_prompt: &str, user_message: &str) -> Res<String> {
        debug!("Generating response with system prompt and user message");
        
        let messages = vec![
            ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                content: ChatCompletionRequestSystemMessageContent::Text(system_prompt.to_string()),
                name: None,
            }),
            ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                content: ChatCompletionRequestUserMessageContent::Text(user_message.to_string()),
                name: None,
            }),
        ];

        // Create a request with required fields. Use builder pattern to avoid having to
        // specify all fields explicitly
        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(messages)
            .temperature(0.7)
            .build()?;

        let response = self.client.chat().create(request).await?;
        let content = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
            .unwrap_or_default();
        Ok(content)
    }
}
