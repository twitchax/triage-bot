//! Integration with Large Language Model services.
//!
//! This module provides a thin wrapper around LLM clients (e.g., OpenAI)
//! for generating responses to user queries, performing web searches,
//! and identifying relevant message search terms.
//!
//! The module defines the `GenericLlmClient` trait that can be implemented
//! for different LLM providers, with a default implementation for OpenAI.

use std::{
    ops::Deref,
    sync::{Arc, OnceLock},
};

use crate::base::types::{AssistantResponse, Res, TextOrResponse};
use crate::base::{
    config::Config,
    types::{AssistantContext, MessageSearchContext, WebSearchContext},
};
use anyhow::Context;
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::responses::{
        Content, CreateResponseArgs, FunctionArgs, Input, InputItem, InputMessageArgs, OutputContent, Response, ResponseFormatJsonSchema, Role, TextConfig, TextResponseFormat, ToolDefinition,
        WebSearchPreviewArgs,
    },
};
use async_trait::async_trait;
use serde_json::Value;
use tracing::{info, instrument, warn};

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
    config: Config,
}

impl OpenAiLlmClient {
    /// Create a new OpenAI LLM client.
    #[instrument(name = "OpenAiLlmClient::new", skip_all)]
    pub fn new(config: &Config) -> Self {
        let cfg = OpenAIConfig::new().with_api_key(config.openai_api_key.clone());

        Self {
            client: Client::with_config(cfg),
            config: config.clone(),
        }
    }

    /// Build the web search input.
    #[instrument(name = "OpenAiLlmClient::build_web_search_input", skip_all)]
    fn build_web_search_input(&self, context: &WebSearchContext) -> Res<Input> {
        Ok(Input::Items(vec![
            InputItem::Message(
                InputMessageArgs::default()
                    .role(Role::Developer)
                    .content(format!("## Your User ID: `{}`\n\n", context.bot_user_id))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(Role::System)
                    .content(format!("## Channel Context\n\n{}\n\n", context.channel_context))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(Role::Developer)
                    .content(format!("## Thread Context\n\n{}\n\n", context.thread_context))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(Role::User)
                    .content(format!("# User Message\n\n{}\n\n", context.user_message))
                    .build()?,
            ),
        ]))
    }

    /// Build the message search input.
    #[instrument(name = "OpenAiLlmClient::build_message_search_input", skip_all)]
    fn build_message_search_input(&self, context: &MessageSearchContext) -> Res<Input> {
        Ok(Input::Items(vec![
            InputItem::Message(
                InputMessageArgs::default()
                    .role(Role::Developer)
                    .content(format!("## Your User ID: `{}`\n\n", context.bot_user_id))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(Role::System)
                    .content(format!("## Channel Context\n\n{}\n\n", context.channel_context))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(Role::Developer)
                    .content(format!("## Thread Context\n\n{}\n\n", context.thread_context))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(Role::User)
                    .content(format!("# User Message\n\n{}\n\n", context.user_message))
                    .build()?,
            ),
        ]))
    }

    /// Build the response input including search results.
    #[instrument(name = "OpenAiLlmClient::build_response_input", skip_all)]
    fn build_assistant_agent_input(&self, context: &AssistantContext) -> Res<Input> {
        Ok(Input::Items(vec![
            InputItem::Message(
                InputMessageArgs::default()
                    .role(Role::Developer)
                    .content(format!("## Your User ID: `{}`\n\n", context.bot_user_id))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(Role::System)
                    .content(format!("## Assistant Agent Mention Directive\n\n{}\n\n", self.config.assistant_agent_mention_directive))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(Role::Developer)
                    .content(format!("## Channel Directive\n\n{}\n\n", context.channel_directive))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(Role::Developer)
                    .content(format!("## Channel Context\n\n{}\n\n", context.channel_context))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(Role::Developer)
                    .content(format!("## Thread Context\n\n{}\n\n", context.thread_context))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(Role::Developer)
                    .content(format!("## Web Search Results\n\n{}\n\n", context.web_search_context))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(Role::Developer)
                    .content(format!("## Message Search Results (in order of likely relevance)\n\n{}\n\n", context.message_search_context))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(Role::User)
                    .content(format!("# User Message\n\n{}\n\n", context.user_message))
                    .build()?,
            ),
        ]))
    }
}

#[async_trait]
impl GenericLlmClient for OpenAiLlmClient {
    #[instrument(name = "OpenAiLlmClient::execute_web_search", skip_all)]
    async fn get_web_search_agent_response(&self, context: &WebSearchContext) -> Res<String> {
        // Create a search-specific prompt input
        let input = self.build_web_search_input(context)?;

        // Prepare web search tools
        let search_tools = get_openai_search_tools().clone();

        // Text config for the search response
        let text_config = TextConfig { format: TextResponseFormat::Text };

        // Create the request
        let request = CreateResponseArgs::default()
            .instructions(self.config.search_agent_system_directive.clone())
            .max_output_tokens(self.config.openai_max_tokens)
            .temperature(self.config.openai_search_agent_temperature)
            .model(&self.config.openai_search_agent_model)
            .tools(search_tools)
            .text(text_config)
            .input(input)
            .build()?;

        // Execute the search request
        let response = self.client.responses().create(request).await?;

        // Parse the text response
        let search_results = parse_openai_response(&response)?
            .into_iter()
            .filter_map(|item| if let TextOrResponse::Text(text) = item { Some(text) } else { None })
            .collect::<Vec<String>>();

        // Combine the search results into a single string
        Ok(search_results.join("\n\n"))
    }

    #[instrument(name = "OpenAiLlmClient::execute_message_search", skip_all)]
    async fn get_message_search_agent_response(&self, context: &MessageSearchContext) -> Res<String> {
        // Create a message search-specific prompt input
        let input = self.build_message_search_input(context)?;

        // Text config for the message search response
        let text_config = TextConfig { format: TextResponseFormat::Text };

        // Create the request
        let request = CreateResponseArgs::default()
            .instructions(self.config.message_search_agent_system_directive.clone())
            .max_output_tokens(self.config.openai_max_tokens)
            .temperature(self.config.openai_search_agent_temperature) // Reuse the search agent temperature
            .model(&self.config.openai_search_agent_model) // Reuse the search agent model
            .text(text_config)
            .input(input)
            .build()?;

        // Execute the message search request
        let response = self.client.responses().create(request).await?;

        // Parse the text response
        let search_terms = parse_openai_response(&response)?
            .into_iter()
            .filter_map(|item| if let TextOrResponse::Text(text) = item { Some(text) } else { None })
            .collect::<Vec<String>>();

        // Combine the search terms into a single string
        Ok(search_terms.join(", "))
    }

    /// Generate a response from a static system prompt and user message.
    #[instrument(skip_all)]
    async fn get_assistant_agent_response(&self, context: &AssistantContext) -> Res<Vec<AssistantResponse>> {
        // Build the input with search results included
        let input = self.build_assistant_agent_input(context)?;

        // Prepare allowed tools.

        // The LLM often thinks it wants to update its context: let's not allow that unless the user explicitly asks for it.
        let tools = if context.user_message.contains("remember") || context.user_message.contains("directive") {
            get_openai_assistant_tools()
        } else {
            get_openai_restricted_tools()
        };

        // Prepare text config.

        let text_config = get_openai_text_config();

        // Loop over requests until we get a "final" response.
        // For example, the LLM may give a "context needed" or "search needed" response.

        #[allow(clippy::never_loop)]
        let result = loop {
            let mut request = CreateResponseArgs::default();

            request
                .max_output_tokens(self.config.openai_max_tokens)
                .model(&self.config.openai_assistant_agent_model)
                .instructions(self.config.assistant_agent_system_directive.clone())
                .tools(tools.clone())
                .text(text_config.clone())
                .input(input);

            if self.config.openai_assistant_agent_model == "gpt-4.1" {
                request.temperature(self.config.openai_assistant_agent_temperature);
            }

            // TODO: Add reasoning effort for `o` models.

            let request = request.build()?;

            let response = self.client.responses().create(request).await?;
            let result = parse_openai_response(&response)?
                .into_iter()
                .filter_map(|item| if let TextOrResponse::AssistantResponse(response) = item { Some(response) } else { None })
                .collect::<Vec<AssistantResponse>>();

            // TODO: This is where we might want to handle multiple responses.
            // For example, if the LLM returns a "tool call" response for adding context,
            // we could send it a message saying that the context has been added, and
            // then, it may choose to reply to the user.

            // TODO: We also may want to handle a "context needed" tool (which does not yet exist),
            // that can handle cases where the LLM needs more information to proceed.  We then ping the user, see
            // if we get anything back, and then re-run the request with the new context (preserving the request id).

            // This may change, but for now, always break after one message.
            break result;
        };

        Ok(result)
    }
}

/// Parse the OpenAI text response (usually only web search available).
#[instrument(skip_all)]
pub fn parse_openai_response(response: &Response) -> Res<Vec<TextOrResponse>> {
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

                            if let Ok(response) = serde_json::from_str::<AssistantResponse>(&text.text) {
                                result.push(TextOrResponse::AssistantResponse(response));
                            } else {
                                result.push(TextOrResponse::Text(text.text.clone()));
                            }
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

                    result.push(TextOrResponse::AssistantResponse(AssistantResponse::UpdateChannelDirective { message }));
                }
                "update_channel_context" => {
                    info!("Update context tool called ...");

                    let arguments: Value = serde_json::from_str(&function_call.arguments)?;
                    let arguments = arguments.as_object().ok_or(anyhow::anyhow!("Failed to parse function call arguments."))?;
                    let message = arguments.get("message").ok_or(anyhow::anyhow!("No message in function call."))?.to_string();

                    result.push(TextOrResponse::AssistantResponse(AssistantResponse::UpdateContext { message }));
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
            }
        }
    }

    Ok(result)
}

// Statics.

static OPENAI_FULL_TOOLS: OnceLock<Vec<ToolDefinition>> = OnceLock::new();
static OPENAI_RESTRICTED_TOOLS: OnceLock<Vec<ToolDefinition>> = OnceLock::new();
static OPENAI_SEARCH_TOOLS: OnceLock<Vec<ToolDefinition>> = OnceLock::new();
static OPENAI_TEXT_CONFIG: OnceLock<TextConfig> = OnceLock::new();

/// Get the OpenAI assistant tools.
fn get_openai_assistant_tools() -> &'static Vec<ToolDefinition> {
    OPENAI_FULL_TOOLS.get_or_init(|| {
        vec![
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

/// Get the OpenAI restricted assistant tools.
///
/// This is used when we don't want the assistant to call context updating tools.
fn get_openai_restricted_tools() -> &'static Vec<ToolDefinition> {
    OPENAI_RESTRICTED_TOOLS.get_or_init(Vec::new)
}

/// Get the OpenAI search tools.
fn get_openai_search_tools() -> &'static Vec<ToolDefinition> {
    OPENAI_SEARCH_TOOLS.get_or_init(|| vec![ToolDefinition::WebSearchPreview(WebSearchPreviewArgs::default().build().unwrap())])
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
    use serde_json::json;

    use super::*;
    use crate::base::config::ConfigInner;

    fn create_test_config() -> Config {
        Config {
            inner: Arc::new(ConfigInner {
                openai_api_key: std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "test_key".to_string()),
                openai_search_agent_model: "gpt-4.1-mini".to_string(),
                openai_assistant_agent_model: "gpt-4.1-mini".to_string(),
                openai_search_agent_temperature: 0.0,
                openai_assistant_agent_temperature: 0.1,
                openai_max_tokens: 200u32, // Small for tests
                ..Default::default()
            }),
        }
    }

    fn fail_if_no_api_key() {
        if std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "test_key".to_string()) == "test_key" {
            panic!("OPENAI_API_KEY not set! Tests require a valid API key to run.");
        }
    }

    fn create_test_web_search_context(message: &str) -> WebSearchContext {
        WebSearchContext {
            user_message: message.to_string(),
            bot_user_id: "U12345".to_string(),
            channel_id: "C12345".to_string(),
            channel_context: "Test channel context".to_string(),
            thread_context: "Test thread context".to_string(),
        }
    }

    fn create_test_message_search_context(message: &str) -> MessageSearchContext {
        MessageSearchContext {
            user_message: message.to_string(),
            bot_user_id: "U12345".to_string(),
            channel_id: "C12345".to_string(),
            channel_context: "Test channel context".to_string(),
            thread_context: "Test thread context".to_string(),
        }
    }

    fn create_test_assistant_context(message: &str) -> AssistantContext {
        AssistantContext {
            user_message: message.to_string(),
            bot_user_id: "U12345".to_string(),
            channel_id: "C12345".to_string(),
            thread_ts: "1234567890.123456".to_string(),
            channel_directive: "Be helpful and concise".to_string(),
            channel_context: "General help channel".to_string(),
            thread_context: "User conversation".to_string(),
            web_search_context: "".to_string(),
            message_search_context: "".to_string(),
        }
    }

    #[tokio::test]
    async fn test_llm_client_get_web_search_agent_response() {
        fail_if_no_api_key();

        let config = create_test_config();
        let client = LlmClient::openai(&config);
        let context = create_test_web_search_context("What is Rust programming language?");

        let response = client.get_web_search_agent_response(&context).await.unwrap();

        assert!(!response.is_empty(), "Response should not be empty");
    }

    #[tokio::test]
    async fn test_llm_client_get_message_search_agent_response() {
        fail_if_no_api_key();

        let config = create_test_config();
        let client = LlmClient::openai(&config);
        let context = create_test_message_search_context("Find messages about deployment issues");

        let response = client.get_message_search_agent_response(&context).await.unwrap();

        assert!(!response.is_empty(), "Response should not be empty");
        // The response should contain search terms
        assert!(response.len() > 2, "Search terms should be meaningful");
    }

    #[tokio::test]
    async fn test_llm_client_get_assistant_agent_response() {
        fail_if_no_api_key();

        let config = create_test_config();
        let client = LlmClient::openai(&config);

        let message = json!({
            "channel": "C12345",
            "client_msg_id": "baa0e432-88fb-421a-a510-be2ebe434923",
            "text": "Hello, can you help me with a simple question?",
            "ts": "1234567890.123456",
            "user": "U08STHUHMU1"
        });

        let context = create_test_assistant_context(&message.to_string());

        let responses = client.get_assistant_agent_response(&context).await.unwrap();

        assert!(!responses.is_empty(), "Should return at least one response");
    }

    #[tokio::test]
    async fn test_llm_client_error_handling_invalid_api_key() {
        let mut config = create_test_config();
        // Use an invalid API key to test error handling
        let config_inner = Arc::make_mut(&mut config.inner);
        config_inner.openai_api_key = "sk-invalid-key-for-testing".to_string();

        let client = LlmClient::openai(&config);
        let context = create_test_web_search_context("test");

        let result = client.get_web_search_agent_response(&context).await;
        assert!(result.is_err(), "Should fail with invalid API key");
    }

    #[tokio::test]
    async fn test_llm_client_handles_empty_context() {
        fail_if_no_api_key();

        let config = create_test_config();
        let client = LlmClient::openai(&config);
        let mut context = create_test_message_search_context("");
        context.channel_context = "".to_string();
        context.thread_context = "".to_string();

        let _ = client.get_message_search_agent_response(&context).await.unwrap();
    }

    #[tokio::test]
    async fn test_llm_client_large_context_handling() {
        fail_if_no_api_key();

        let config = create_test_config();
        let client = LlmClient::openai(&config);

        // Create a very large context to test token limits
        let large_context = "context ".repeat(1000);
        let mut context = create_test_web_search_context("Simple question");
        context.channel_context = large_context;

        let _ = client.get_web_search_agent_response(&context).await.unwrap();
    }
}
