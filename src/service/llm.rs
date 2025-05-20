//! Thin wrapper around async-openai for OpenAI LLM calls.

use std::{
    ops::Deref,
    sync::{Arc, OnceLock},
};

use crate::base::types::{LlmResponse, Res};
use crate::base::{
    config::Config,
    prompts::{get_mention_addendum, get_system_prompt},
};
use anyhow::Context;
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{Content, CreateResponseRequestArgs, CreateResponseResponse, FunctionArgs, InputItem, InputMessageArgs, OutputContent, ResponseInput, ResponsesRole, ToolDefinition, WebSearchPreviewArgs},
};
use async_trait::async_trait;
use serde_json::Value;
use tracing::{debug, info, instrument, warn};

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
    max_tokens: u32,
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
            max_tokens: config.openai_max_tokens,
        }
    }
}

#[async_trait]
impl GenericLlmClient for OpenAiLlmClient {
    /// Generate a response from a static system prompt and user message.
    #[instrument(skip_all)]
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

        let tools = get_openai_tools();

        // Loop over requests until we get a "final" response.
        // For example, the LLM may give a "context needed" or "search needed" response.

        #[allow(clippy::never_loop)]
        let result = loop {
            let request = CreateResponseRequestArgs::default()
                .max_output_tokens(self.max_tokens)
                .temperature(self.temperature)
                .model(&self.model)
                .input(input)
                .tools(tools.clone())
                .build()?;

            let response = self.client.responses().create(request).await?;
            let result = parse_openai_response(&response)?;

            // This may change, but for now, always break after one message.
            break result;
        };

        Ok(result)
    }
}

#[instrument(skip_all)]
pub fn parse_openai_response(response: &CreateResponseResponse) -> Res<Vec<LlmResponse>> {
    let mut result = Vec::new();

    match &response.output {
        Some(output) => {
            info!("LLM response has {} outputs.", output.len());

            for content in output {
                match content {
                    OutputContent::Message(message) => {
                        info!("LLM response has {} messages.", message.content.len());

                        for message_content in &message.content {
                            match message_content {
                                Content::OutputText(text) => {
                                    // TODO: Also look at the annotation for citations?
                                    let parsed = serde_json::from_str(&text.text).context(format!("Failed to deserialize LLM response: {text:#?}"))?;

                                    result.push(parsed);
                                }
                                Content::Refusal(reason) => {
                                    return Err(anyhow::anyhow!("Request refused: {reason:#?}"));
                                }
                            }
                        }
                    }
                    OutputContent::FunctionCall(function_call) => match function_call.name.as_str() {
                        "set_channel_directive" => {
                            info!("Channel directive tool called ...");

                            let arguments: Value = serde_json::from_str(&function_call.arguments)?;
                            let arguments = arguments.as_object().ok_or(anyhow::anyhow!("Failed to parse function call arguments."))?;
                            let message = arguments.get("message").ok_or(anyhow::anyhow!("No message in function call."))?.to_string();

                            result.push(LlmResponse::UpdateChannelDirective { message });
                        }
                        "update_channel_context" => {
                            info!("Update context tool called ...");

                            let arguments: Value = serde_json::from_str(&function_call.arguments)?;
                            let arguments = arguments.as_object().ok_or(anyhow::anyhow!("Failed to parse function call arguments."))?;
                            let message = arguments.get("message").ok_or(anyhow::anyhow!("No message in function call."))?.to_string();

                            result.push(LlmResponse::UpdateContext { message });
                        }
                        _ => {
                            warn!("Unknown function call: {function_call:#?}");
                            return Err(anyhow::anyhow!("Unknown function call."));
                        }
                    },
                    OutputContent::WebSearchCall(web_search_call) => {
                        info!("Web search tool called: {web_search_call:#?}");
                    }
                    _ => {
                        warn!("Unknown output: {content:#?}");
                        return Err(anyhow::anyhow!("Unknown output type"));
                    }
                }
            }
        }
        None => {
            warn!("No output in response.");
            return Err(anyhow::anyhow!("No output in response."));
        }
    }

    Ok(result)
}

// Statics.

static OPENAI_TOOLS: OnceLock<Vec<ToolDefinition>> = OnceLock::new();

fn get_openai_tools() -> &'static Vec<ToolDefinition> {
    OPENAI_TOOLS.get_or_init(|| {
        vec![
            ToolDefinition::WebSearchPreview(WebSearchPreviewArgs::default().build().unwrap()),
            ToolDefinition::Function(FunctionArgs::default()
                .name("set_channel_directive")
                .description("Set the channel directive for the bot.")
                .parameters(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "message": {"type": "string", "description": "Anything you want to say about the user's message about updating the channel.  This message, and anything the user provides, will be stored for future reference.  This message will be provided to you in _every_ subsequent request.  You can use slack's markdown formatting here.  This tool call does not share to the user, so you also need to generate a response to the user."},
                    },
                    "required": ["message"],
                    "additionalProperties": false
                }))
                .build().unwrap()
            ),
            ToolDefinition::Function(FunctionArgs::default()
                .name("update_channel_context")
                .description("Update the context for the bot.  This is provided to you in _every_ subsequent request, but does not replace the channel directive, which is more important.")
                .parameters(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "message": {"type": "string", "description": "Anything you want to say about the user's message about updating your understanding of the channel.  This is a subtle distinction, but it is important.  This will be provided to you upon every request.  This tool call does not share to the user, so you also need to generate a response to the user."},
                    },
                    "required": ["message"],
                    "additionalProperties": false
                }))
                .build().unwrap()
            ),
        ]
    })
}

// Tests.

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
            .returning(|_, _, _, _| Ok(vec![]));

        let client = LlmClient { inner: Arc::new(mock) };
        client.generate_response("me", "dir", "ctx", "msg").await.unwrap();
    }
}
