//! Thin wrapper around async-openai for OpenAI LLM calls.

use std::{
    ops::Deref,
    sync::{Arc, OnceLock},
};

use crate::base::types::{AssistantResponse, Res};
use crate::base::{
    config::Config,
    types::{AssistantContext, MessageSearchContext, WebSearchContext},
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
pub trait GenericLlmClient: Send + Sync + 'static {
    /// Execute a web search using the search agent.
    async fn get_web_search_agent_response(&self, context: &WebSearchContext) -> Res<String>;
    /// Generate search terms for message search using the message search agent.
    async fn get_message_search_agent_response(&self, context: &MessageSearchContext) -> Res<String>;
    /// Generate a response from the primary assistant model.
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
    fn build_web_search_input(&self, context: &WebSearchContext) -> Res<ResponseInput> {
        Ok(ResponseInput::Items(vec![
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::Developer)
                    .content(format!("## Your User ID: `{}`\n\n", context.bot_user_id))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::System)
                    .content(format!("## Channel Context\n\n{}\n\n", context.channel_context))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::Developer)
                    .content(format!("## Thread Context\n\n{}\n\n", context.thread_context))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::User)
                    .content(format!("# User Message\n\n{}\n\n", context.user_message))
                    .build()?,
            ),
        ]))
    }

    /// Build the message search input.
    #[instrument(name = "OpenAiLlmClient::build_message_search_input", skip_all)]
    fn build_message_search_input(&self, context: &MessageSearchContext) -> Res<ResponseInput> {
        Ok(ResponseInput::Items(vec![
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::Developer)
                    .content(format!("## Your User ID: `{}`\n\n", context.bot_user_id))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::System)
                    .content(format!("## Channel Context\n\n{}\n\n", context.channel_context))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::Developer)
                    .content(format!("## Thread Context\n\n{}\n\n", context.thread_context))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::User)
                    .content(format!("# User Message\n\n{}\n\n", context.user_message))
                    .build()?,
            ),
        ]))
    }

    /// Build the response input including search results.
    #[instrument(name = "OpenAiLlmClient::build_response_input", skip_all)]
    fn build_assistant_agent_input(&self, context: &AssistantContext) -> Res<ResponseInput> {
        Ok(ResponseInput::Items(vec![
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::Developer)
                    .content(format!("## Your User ID: `{}`\n\n", context.bot_user_id))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::System)
                    .content(format!("## Assistant Agent Mention Directive\n\n{}\n\n", self.config.assistant_agent_mention_directive))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::Developer)
                    .content(format!("## Channel Directive\n\n{}\n\n", context.channel_directive))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::Developer)
                    .content(format!("## Channel Context\n\n{}\n\n", context.channel_context))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::Developer)
                    .content(format!("## Thread Context\n\n{}\n\n", context.thread_context))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::Developer)
                    .content(format!("## Web Search Results\n\n{}\n\n", context.web_search_context))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::Developer)
                    .content(format!("## Channel Message History\n\n{}\n\n", context.message_search_context))
                    .build()?,
            ),
            InputItem::Message(
                InputMessageArgs::default()
                    .role(ResponsesRole::User)
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
        let request = CreateResponseRequestArgs::default()
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
        let search_results = parse_openai_text_response(&response)?;

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
        let request = CreateResponseRequestArgs::default()
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
        let search_terms = parse_openai_text_response(&response)?;

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
            let request = CreateResponseRequestArgs::default()
                .max_output_tokens(self.config.openai_max_tokens)
                .temperature(self.config.openai_assistant_agent_temperature)
                .model(&self.config.openai_assistant_agent_model)
                .instructions(self.config.assistant_agent_system_directive.clone())
                .tools(tools.clone())
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
pub fn parse_openai_structured_response(response: &CreateResponseResponse) -> Res<Vec<AssistantResponse>> {
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

                    result.push(AssistantResponse::UpdateChannelDirective { message });
                }
                "update_channel_context" => {
                    info!("Update context tool called ...");

                    let arguments: Value = serde_json::from_str(&function_call.arguments)?;
                    let arguments = arguments.as_object().ok_or(anyhow::anyhow!("Failed to parse function call arguments."))?;
                    let message = arguments.get("message").ok_or(anyhow::anyhow!("No message in function call."))?.to_string();

                    result.push(AssistantResponse::UpdateContext { message });
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
    use super::*;
    use mockall::{mock, predicate::*};

    mock! {
        pub Llm {}

        #[async_trait]
        impl GenericLlmClient for Llm {
            async fn get_web_search_agent_response(&self, context: &WebSearchContext) -> Res<String>;
            async fn get_message_search_agent_response(&self, context: &MessageSearchContext) -> Res<String>;
            async fn get_assistant_agent_response(&self, context: &AssistantContext) -> Res<Vec<AssistantResponse>>;
        }
    }

    #[tokio::test]
    async fn llm_client_delegates_get_assistant_agent_response() {
        let mut mock = MockLlm::new();
        mock.expect_get_assistant_agent_response()
            .with(eq(AssistantContext {
                user_message: "msg".to_string(),
                bot_user_id: "me".to_string(),
                channel_id: "dir".to_string(),
                thread_ts: "thread".to_string(),
                channel_directive: "search".to_string(),
                channel_context: "ctx".to_string(),
                thread_context: "ctx".to_string(),
                web_search_context: "ctx".to_string(),
                message_search_context: "ctx".to_string(),
            }))
            .times(1)
            .returning(|_| Ok(vec![]));

        let client = LlmClient { inner: Arc::new(mock) };
        client
            .get_assistant_agent_response(&AssistantContext {
                user_message: "msg".to_string(),
                bot_user_id: "me".to_string(),
                channel_id: "dir".to_string(),
                thread_ts: "thread".to_string(),
                channel_directive: "search".to_string(),
                channel_context: "ctx".to_string(),
                thread_context: "ctx".to_string(),
                web_search_context: "ctx".to_string(),
                message_search_context: "ctx".to_string(),
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn llm_client_delegates_get_message_search_agent_response() {
        let mut mock = MockLlm::new();
        mock.expect_get_message_search_agent_response()
            .with(eq(MessageSearchContext {
                user_message: "msg".to_string(),
                bot_user_id: "me".to_string(),
                channel_id: "channel".to_string(),
                channel_context: "ctx".to_string(),
                thread_context: "ctx".to_string(),
            }))
            .times(1)
            .returning(|_| Ok("search, terms".to_string()));

        let client = LlmClient { inner: Arc::new(mock) };
        let result = client
            .get_message_search_agent_response(&MessageSearchContext {
                user_message: "msg".to_string(),
                bot_user_id: "me".to_string(),
                channel_id: "channel".to_string(),
                channel_context: "ctx".to_string(),
                thread_context: "ctx".to_string(),
            })
            .await
            .unwrap();
        
        assert_eq!(result, "search, terms");
    }
}
