//! Integration with Large Language Model services.
//!
//! This module provides a thin wrapper around LLM clients (e.g., OpenAI)
//! for generating responses to user queries, performing web searches,
//! and identifying relevant message search terms.
//!
//! The module defines the `GenericLlmClient` trait that can be implemented
//! for different LLM providers, with a default implementation for OpenAI.

use std::time::Duration;
use std::{
    collections::VecDeque,
    sync::{Arc, OnceLock},
};

use crate::base::{
    config::Config,
    types::{AssistantContext, MessageSearchContext, Void, WebSearchContext},
};
use crate::{
    base::types::{AssistantResponse, Res, TextOrResponse, ToolContextFunctionCallArgs},
    service::llm::BoxedCallback,
};
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ReasoningEffort,
        responses::{
            Content, CreateResponseArgs, FunctionArgs, Input, InputItem, InputMessageArgs, OutputContent, ReasoningConfigArgs, Response, ResponseFormatJsonSchema, Role, TextConfig,
            TextResponseFormat, ToolDefinition, WebSearchPreviewArgs,
        },
    },
};
use async_trait::async_trait;
use tokio::time::timeout;
use tracing::{info, instrument, warn};

use super::{GenericLlmClient, LlmClient};

// Extra methods on `LlmClient` applied by the openai implementation.

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

    /// Helper function to make OpenAI API calls with retry logic and timeout handling.
    async fn call_openai_api(&self, request_builder: CreateResponseArgs) -> Res<Response> {
        const MAX_RETRIES: u32 = 3;
        const TIMEOUT: u64 = 120; // OpenAI can be slow, especially with reasoning models
        const RETRY_DELAY_MS: u64 = 1000;

        let mut retries = 0;

        loop {
            let request = request_builder.build()?;
            let result = timeout(Duration::from_secs(TIMEOUT), self.client.responses().create(request)).await;

            match result {
                Ok(Ok(response)) => {
                    info!("OpenAI API call succeeded after {} attempts", retries + 1);
                    return Ok(response);
                }
                Ok(Err(err)) => {
                    if retries >= MAX_RETRIES {
                        return Err(anyhow::anyhow!("OpenAI API call failed after {MAX_RETRIES} retries: {err}"));
                    }
                    retries += 1;
                    warn!("OpenAI API call failed, retrying {retries}/{MAX_RETRIES}: {err}");

                    // Add exponential backoff for retries
                    let delay = Duration::from_millis(RETRY_DELAY_MS * 2_u64.pow(retries - 1));
                    tokio::time::sleep(delay).await;
                }
                Err(_) => {
                    if retries >= MAX_RETRIES {
                        return Err(anyhow::anyhow!("OpenAI API call timed out after {MAX_RETRIES} attempts"));
                    }
                    retries += 1;
                    warn!("OpenAI API call timed out, retrying {retries}/{MAX_RETRIES}");

                    // Add exponential backoff for timeouts too
                    let delay = Duration::from_millis(RETRY_DELAY_MS * 2_u64.pow(retries - 1));
                    tokio::time::sleep(delay).await;
                }
            }
        }
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

        // Create the request.
        let mut request = CreateResponseArgs::default();
        request
            .instructions(self.config.search_agent_system_directive.clone())
            .max_output_tokens(self.config.openai_max_tokens)
            .model(&self.config.openai_search_agent_model)
            .tools(search_tools)
            .text(text_config)
            .input(input);

        // Add the temperature for the non-reasoning models.
        if self.config.openai_search_agent_model.starts_with("gpt") {
            request.temperature(self.config.openai_search_agent_temperature);
        }

        // Add the reasoning effort for `o` models.
        if self.config.openai_search_agent_model.starts_with("o") {
            let reasoning_effort = parse_openai_reasoning_effort(&self.config.openai_search_agent_reasoning_effort)?;
            request.reasoning(ReasoningConfigArgs::default().effort(reasoning_effort).build()?);
        }

        // Execute the search request
        let response = self.call_openai_api(request).await?;

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

        // Create the request.
        let mut request = CreateResponseArgs::default();
        request
            .instructions(self.config.message_search_agent_system_directive.clone())
            .max_output_tokens(self.config.openai_max_tokens)
            .model(&self.config.openai_search_agent_model)
            .text(text_config)
            .input(input);

        // Add the temperature for the non-reasoning models.
        if self.config.openai_search_agent_model.starts_with("gpt") {
            request.temperature(self.config.openai_search_agent_temperature);
        }

        // Add the reasoning effort for `o` models.
        if self.config.openai_search_agent_model.starts_with("o") {
            let reasoning_effort = parse_openai_reasoning_effort(&self.config.openai_search_agent_reasoning_effort)?;
            request.reasoning(ReasoningConfigArgs::default().effort(reasoning_effort).build()?);
        }

        // Execute the message search request
        let response = self.call_openai_api(request).await?;

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
    async fn get_assistant_agent_response(&self, context: &AssistantContext, response_callback: BoxedCallback) -> Void {
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

        // Prepare the _initial_ request.

        let mut request = CreateResponseArgs::default();

        request
            .max_output_tokens(self.config.openai_max_tokens)
            .model(&self.config.openai_assistant_agent_model)
            .instructions(self.config.assistant_agent_system_directive.clone())
            .tools(tools.clone())
            .text(text_config.clone())
            .input(input);

        // Add the temperature for the non-reasoning models.
        if self.config.openai_assistant_agent_model.starts_with("gpt") {
            request.temperature(self.config.openai_assistant_agent_temperature);
        }

        // Add the reasoning effort for `o` models.
        if self.config.openai_assistant_agent_model.starts_with("o") {
            let reasoning_effort = parse_openai_reasoning_effort(&self.config.openai_assistant_agent_reasoning_effort)?;
            request.reasoning(ReasoningConfigArgs::default().effort(reasoning_effort).build()?);
        }

        // Loop over requests until we get a "final" response.
        // For example, the LLM may give a "context needed" or "search needed" response.

        let mut request_queue = VecDeque::new();
        request_queue.push_back(request);

        while let Some(request) = request_queue.pop_front() {
            // Send the request, and parse.
            let response = self.call_openai_api(request.clone()).await?;
            let results = parse_openai_response(&response)?
                .into_iter()
                .filter_map(|item| if let TextOrResponse::AssistantResponse(r) = item { Some(r) } else { None })
                .collect::<Vec<_>>();

            info!("Received {} responses from LLM", results.len());

            // Call the response callback, which should return a message to send back to the model.
            let message = response_callback(results).await?;

            // If there's a message, we need to add it to the request queue.
            if let Some(message) = message {
                let mut request = request.clone();

                request.previous_response_id(&response.id).input(Input::Items(vec![InputItem::Custom(message)]));
                request_queue.push_back(request);
                info!("Added new request to queue with response ID: {}", response.id);
            }
        }

        Ok(())
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

                    let ToolContextFunctionCallArgs { message } = serde_json::from_str(&function_call.arguments)?;

                    result.push(TextOrResponse::AssistantResponse(AssistantResponse::UpdateChannelDirective { message }));
                }
                "update_channel_context" => {
                    info!("Update context tool called ...");

                    let ToolContextFunctionCallArgs { message } = serde_json::from_str(&function_call.arguments)?;

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

/// Convert a string reasoning effort to ReasoningEffort enum.
fn parse_openai_reasoning_effort(effort: &str) -> Res<ReasoningEffort> {
    match effort.to_lowercase().as_str() {
        "low" => Ok(ReasoningEffort::Low),
        "medium" => Ok(ReasoningEffort::Medium),
        "high" => Ok(ReasoningEffort::High),
        _ => Err(crate::base::types::Err::msg(format!("Invalid reasoning effort: {effort}. Must be one of: low, medium, high"))),
    }
}

// Tests.

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio::sync::Mutex;

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

        let responses = Arc::new(Mutex::new(Vec::new()));
        let responses_clone = responses.clone();

        client
            .get_assistant_agent_response(
                &context,
                Box::new(move |response| {
                    let responses_clone = responses_clone.clone();
                    Box::pin(async move {
                        responses_clone.lock().await.push(response);

                        Ok(None)
                    })
                }),
            )
            .await
            .unwrap();

        assert!(!responses.lock().await.is_empty(), "Should return at least one response");
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
