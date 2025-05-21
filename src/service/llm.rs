//! Thin wrapper around async-openai for OpenAI LLM calls.

use std::{
    ops::Deref,
    sync::{Arc, OnceLock},
};

use crate::base::types::{LlmResponse, Res};
use crate::base::{
    config::Config,
    prompts::{get_mention_directive, get_system_directive},
};
use anyhow::Context;
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        Content, CreateResponseRequestArgs, CreateResponseResponse, FunctionArgs, InputItem, InputMessageArgs, OutputContent, ResponseFormatJsonSchema, ResponseInput, ResponsesRole, TextConfig,
        TextResponseFormat, ToolDefinition, WebSearchPreviewArgs,
    },
};
use async_trait::async_trait;
use serde_json::Value;
use tracing::{info, instrument, warn};

// Traits.

/// Generic LLM client trait that clients must implement.
#[async_trait]
pub trait GenericLlmClient {
    /// Generate a response from a static system prompt and user message.
    async fn generate_response(&self, self_id: &str, channel_prompt: &str, channel_context: &str, thread_context: &str, user_message: &str) -> Res<Vec<LlmResponse>>;
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

        let system_prompt = get_system_directive(config).to_string();
        let mention_addendum_prompt = get_mention_directive(config).to_string();

        Self {
            client: Client::with_config(cfg),
            model,
            system_prompt,
            mention_addendum_prompt,
            temperature: config.openai_temperature,
            max_tokens: config.openai_max_tokens,
        }
    }

    /// Execute a search using the search agent
    async fn execute_search(&self, user_message: &str) -> Res<String> {
        // Create a search-specific prompt input
        let search_input = ResponseInput::Items(vec![
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::System)
                    .content(crate::base::prompts::SEARCH_AGENT_DIRECTIVE.to_string())
                    .build()?,
            ),
            InputItem::Message(InputMessageArgs::default().role(ResponsesRole::User).content(user_message.to_string()).build()?),
        ]);

        // Prepare web search tools
        let web_search_tool = vec![ToolDefinition::WebSearchPreview(WebSearchPreviewArgs::default().build()?)];

        // Text config for the search response
        let text_config = TextConfig { format: TextResponseFormat::Text };

        // Create the request
        let request = CreateResponseRequestArgs::default()
            .max_output_tokens(self.max_tokens)
            .temperature(0.0) // Use lower temperature for search agent
            .model(&self.model)
            .tools(web_search_tool)
            .text(text_config)
            .input(search_input)
            .build()?;

        // Execute the search request
        let response = self.client.responses().create(request).await?;

        // Parse the text response
        let search_results = parse_openai_text_response(&response)?;

        // Combine the search results into a single string
        Ok(search_results.join("\n\n"))
    }

    /// Build the response input including search results
    fn build_response_input(&self, self_id: &str, channel_directive: &str, channel_context: &str, thread_context: &str, user_message: &str, search_results: &str) -> Res<ResponseInput> {
        Ok(ResponseInput::Items(vec![
            InputItem::Message(InputMessageArgs::default().role(ResponsesRole::System).content(self.mention_addendum_prompt.clone()).build()?),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::Developer)
                    .content(format!("Your Channel Directive:\n\n{channel_directive}"))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::Developer)
                    .content(format!("Your Channel Context:\n\n{channel_context}"))
                    .build()?,
            ),
            InputItem::Message(InputMessageArgs::default().role(ResponsesRole::Developer).content(format!("Your User ID: {self_id}")).build()?),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::Developer)
                    .content(format!("Raw Thread Context:\n\n{thread_context}"))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::Developer)
                    .content(format!("Search Results:\n\n{search_results}"))
                    .build()?,
            ),
            InputItem::Message(InputMessageArgs::default().role(ResponsesRole::User).content(user_message.to_string()).build()?),
        ]))
    }
}

#[async_trait]
impl GenericLlmClient for OpenAiLlmClient {
    /// Generate a response from a static system prompt and user message.
    #[instrument(skip_all)]
    async fn generate_response(&self, self_id: &str, channel_directive: &str, channel_context: &str, thread_context: &str, user_message: &str) -> Res<Vec<LlmResponse>> {
        // First, execute the search agent to gather relevant information
        info!("Executing search agent with user message");
        let search_results = self.execute_search(user_message).await?;
        info!("Search agent completed, results length: {}", search_results.len());

        // Build the input with search results included
        let input = self.build_response_input(self_id, channel_directive, channel_context, thread_context, user_message, &search_results)?;

        // Prepare allowed tools.

        // The LLM often thinks it wants to update its context: let's not allow that unless the user explicitly asks for it.
        let tools = if user_message.contains("remember") || user_message.contains("directive") {
            get_openai_full_tools()
        } else {
            get_openai_restricted_tools()
        };

        // Prepare text config.

        let text_config = get_openai_text_config();

        // Loop over requests until we get a "final" response.
        // For example, the LLM may give a "context needed" or "search needed" response.

        #[allow(clippy::never_loop)]
        let result = loop {
            let request = CreateResponseRequestArgs::default()
                .max_output_tokens(self.max_tokens)
                .temperature(self.temperature)
                .model(&self.model)
                .instructions(self.system_prompt.clone())
                .tools(tools.clone())
                // TODO: This doesn't seem to work properly, so the OpenAI crate is likely messing up the correct web request.
                // So, disregarding, for now.
                .text(text_config.clone())
                .input(input)
                .build()?;

            let response = self.client.responses().create(request).await?;
            let result = parse_openai_structured_response(&response)?;

            // This may change, but for now, always break after one message.
            break result;
        };

        Ok(result)
    }
}

/// Parse the OpenAI text response (usually only web search available).
#[instrument(skip_all)]
pub fn parse_openai_text_response(response: &CreateResponseResponse) -> Res<Vec<String>> {
    let mut result = Vec::new();

    info!("LLM text response has {} outputs.", response.output.len());
    for output in &response.output {
        match output {
            OutputContent::Message(message) => {
                info!("LLM text response has {} messages.", message.content.len());

                for message_content in &message.content {
                    match message_content {
                        Content::OutputText(text) => {
                            // TODO: Handle annotations if needed.
                            if text.annotations.is_empty() {
                                info!("LLM text response has no annotations.");
                            } else {
                                info!("LLM text response has {} annotations.", text.annotations.len());
                            }

                            // Just push the raw text, do not attempt to deserialize.
                            result.push(text.text.clone());
                        }
                        Content::Refusal(reason) => {
                            return Err(anyhow::anyhow!("Request refused: {reason:#?}"));
                        }
                    }
                }
            }
            OutputContent::WebSearchCall(web_search_call) => {
                info!("Web search tool called in text response: {web_search_call:#?}");
            }
            _ => {
                warn!("Unknown output in text response: {output:#?}");
                return Err(anyhow::anyhow!("Unknown output type in text response"));
            }
        }
    }

    Ok(result)
}

/// Parse the OpenAI structured response (and, therefore, check for local tool calls).
#[instrument(skip_all)]
pub fn parse_openai_structured_response(response: &CreateResponseResponse) -> Res<Vec<LlmResponse>> {
    let mut result = Vec::new();

    info!("LLM response has {} outputs.", response.output.len());
    for output in &response.output {
        match output {
            OutputContent::Message(message) => {
                info!("LLM response has {} messages.", message.content.len());

                for message_content in &message.content {
                    match message_content {
                        Content::OutputText(text) => {
                            // TODO: Handle annotations.
                            if text.annotations.is_empty() {
                                info!("LLM response has no annotations.");
                            } else {
                                info!("LLM response has {} annotations.", text.annotations.len());
                            }

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
                warn!("Unknown output: {output:#?}");
                return Err(anyhow::anyhow!("Unknown output type"));
            }
        }
    }

    Ok(result)
}

// Statics.

static OPENAI_FULL_TOOLS: OnceLock<Vec<ToolDefinition>> = OnceLock::new();
static OPENAI_RESTRICTED_TOOLS: OnceLock<Vec<ToolDefinition>> = OnceLock::new();
static OPENAI_TEXT_CONFIG: OnceLock<TextConfig> = OnceLock::new();

fn get_openai_full_tools() -> &'static Vec<ToolDefinition> {
    OPENAI_FULL_TOOLS.get_or_init(|| {
        vec![
            ToolDefinition::WebSearchPreview(WebSearchPreviewArgs::default().build().unwrap()),
            ToolDefinition::Function(FunctionArgs::default()
                .name("set_channel_directive")
                .description("Set the channel directive for the bot.  You should only call this tool if the user @-mentions you, and says something like \"please update my channel directive\".  This is a subtle distinction, but it is important.  99% of the time, the user is asking you to reply, and this tool should not be called.  This will be provided to you in _every_ subsequent request.")
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
                .description("Update the context for the bot.  You should only call this tool if the user @-mentions you, and says something like \"please update my channel context\" or \"please remember that ...\".  This is a subtle distinction, but it is important.  99% of the time, the user is asking you to reply, and this tool should not be called.  This will be provided to you in _every_ subsequent request.")
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

fn get_openai_restricted_tools() -> &'static Vec<ToolDefinition> {
    OPENAI_RESTRICTED_TOOLS.get_or_init(|| vec![ToolDefinition::WebSearchPreview(WebSearchPreviewArgs::default().build().unwrap())])
}

fn get_openai_text_config() -> &'static TextConfig {
    OPENAI_TEXT_CONFIG.get_or_init(|| TextConfig {
        format: TextResponseFormat::JsonSchema(ResponseFormatJsonSchema {
            name: "TriageBotResponse".to_string(),
            description: Some("Format for triage bot responses.".to_string()),
            schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "type": {
                        "type": "string",
                        "enum": ["NoAction", "ReplyToThread"]
                    },
                    "thread_ts": { "type": ["string", "null"] },
                    "classification": {
                        "type": ["string", "null"],
                        "enum": ["Bug", "Feature", "Question", "Incident", "Other"]
                    },
                    "message": { "type": ["string", "null"] }
                },
                "required": ["type", "thread_ts", "classification", "message"],
                "additionalProperties": false
            })),
            strict: Some(true),
        }),
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
            async fn generate_response(&self, self_id: &str, channel_prompt: &str, channel_context: &str, thread_context: &str, user_message: &str) -> Res<Vec<LlmResponse>>;
        }
    }

    #[tokio::test]
    async fn llm_client_delegates_generate_response() {
        let mut mock = MockLlm::new();
        mock.expect_generate_response()
            .with(eq("me"), eq("dir"), eq("ctx"), eq("thread"), eq("msg"))
            .times(1)
            .returning(|_, _, _, _, _| Ok(vec![]));

        let client = LlmClient { inner: Arc::new(mock) };
        client.generate_response("me", "dir", "ctx", "thread", "msg").await.unwrap();
    }
}
