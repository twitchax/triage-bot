//! Thin wrapper around async-openai for OpenAI LLM calls.

use std::{ops::Deref, sync::Arc};

use crate::base::types::{LlmResponse, Res};
use crate::base::{
    config::Config,
    prompts::{get_mention_addendum, get_system_prompt},
};
use anyhow::Context;
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{Content, CreateResponseRequestArgs, InputItem, InputMessageArgs, OutputContent, ResponseInput, ResponsesRole, ToolDefinition, WebSearchPreviewArgs},
};
use async_trait::async_trait;
use tracing::{debug, instrument, warn};

// Traits.

/// Generic LLM client trait that clients must implement.
#[async_trait]
pub trait GenericLlmClient {
    /// Generate a response from a static system prompt and user message.
    async fn generate_response(&self, self_id: &str, channel_prompt: &str, thread_context: &str, user_message: &str) -> Res<Vec<LlmResponse>>;
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
    /// Create a new OpenAI LLM client.
    #[instrument(name = "OpenAiLlmClient::new", skip_all)]
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
    #[instrument(skip(self, self_id, channel_directive, thread_context))]
    async fn generate_response(&self, self_id: &str, channel_directive: &str, thread_context: &str, user_message: &str) -> Res<Vec<LlmResponse>> {
        debug!("Generating response with system prompt and user message");

        let mut input = ResponseInput::Items(vec![
            InputItem::Message(InputMessageArgs::default().role(ResponsesRole::System).content(self.system_prompt.clone()).build()?),
            InputItem::Message(InputMessageArgs::default().role(ResponsesRole::System).content(self.mention_addendum_prompt.clone()).build()?),
            InputItem::Message(InputMessageArgs::default().role(ResponsesRole::Developer).content(channel_directive.to_string()).build()?),
            InputItem::Message(InputMessageArgs::default().role(ResponsesRole::Developer).content(format!("Your User ID: {self_id}")).build()?),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::Developer)
                    .content(format!("Raw Thread Context:\n\n{thread_context}"))
                    .build()?,
            ),
            InputItem::Message(InputMessageArgs::default().role(ResponsesRole::User).content(user_message.to_string()).build()?),
        ]);

        // Prepare allowed tools.

        let tools = vec![ToolDefinition::WebSearchPreview(WebSearchPreviewArgs::default().build()?)];

        // Loop over requests until we get a "final" response.
        // For example, the LLM may give a "context needed" or "search needed" response.

        #[allow(clippy::never_loop)]
        let result = loop {
            let request = CreateResponseRequestArgs::default().max_output_tokens(2048u32).model(&self.model).input(input).tools(tools).build()?;

            // TODO: Abstract some of this away into a function.
            let response = self.client.responses().create(request).await?;

            let content = String::new();
            let content = match response.output {
                Some(output) => {
                    let content = output.first().ok_or(anyhow::anyhow!("No output in response."))?;

                    match content {
                        OutputContent::Message(message) => {
                            let message_content = message.content.first().ok_or(anyhow::anyhow!("No message content in response."))?;

                            match message_content {
                                Content::OutputText(text) => text.text.clone(),
                                _ => {
                                    warn!("Unknown content: {message_content:#?}");
                                    return Err(anyhow::anyhow!("Unknown content type"));
                                }
                            }
                        }
                        _ => {
                            warn!("Unknown output: {content:#?}");
                            return Err(anyhow::anyhow!("Unknown output type"));
                        }
                    }
                }
                None => {
                    warn!("No output in response.");
                    return Err(anyhow::anyhow!("No output in response."));
                }
            };

            // Deserialize the response to the `LlmResult` type.
            let result: Vec<LlmResponse> = serde_json::from_str(&content).context(format!("Failed to deserialize LLM response: {content:#?}"))?;

            // This may change, but for now, always break after one message.
            break result;
        };

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::{mock, predicate::*};

    mock! {
        pub Llm {}

        #[async_trait]
        impl GenericLlmClient for Llm {
            async fn generate_response(&self, self_id: &str, channel_prompt: &str, thread_context: &str, user_message: &str) -> Res<Vec<LlmResponse>>;
        }
    }

    #[tokio::test]
    async fn llm_client_delegates_generate_response() {
        let mut mock = MockLlm::new();
        mock.expect_generate_response()
            .with(eq("me"), eq("dir"), eq("ctx"), eq("msg"))
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Ok(vec![]) }));

        let client = LlmClient { inner: Arc::new(mock) };
        client.generate_response("me", "dir", "ctx", "msg").await.unwrap();
    }
}
