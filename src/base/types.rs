//! Common types, results, and data structures used throughout triage-bot.
//!
//! This module defines core types and structures that are shared across the application,
//! including error handling types, context structures for LLM interactions, and response
//! types from the assistant.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Standard error type used throughout the application.
pub type Err = anyhow::Error;
/// Standard result type with unified error handling.
pub type Res<T> = Result<T, Err>;
/// Convenience type for operations that return nothing but may fail.
pub type Void = Res<()>;

/// The classification of the assistant's response.
/// This is used to determine the type of action to take based on the assistant's response.
#[derive(Debug, Serialize, Deserialize)]
pub enum AssistantClassification {
    /// Bug classification indicates that the issue is a bug in the system.
    Bug,
    /// Feature classification indicates that the issue is a feature request.
    Feature,
    /// Question classification indicates that the issue is a question that needs to be answered.
    Question,
    /// Incident classification indicates that the issue is an incident that needs to be handled.
    Incident,
    /// Other classification indicates that the issue does not fit into any of the above categories.
    Other,
}

/// An enum representing the different types of responses from the LLM.
///
/// This includes both direct responses (like replies or taking no action)
/// and tool calls that perform operations like updating context or directives.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AssistantResponse {
    // Responses.
    /// Indicates that the assistant will take no action.
    NoAction,
    /// A direct reply to a thread in Slack.
    ReplyToThread {
        /// The timestamp of the thread to reply to.
        thread_ts: String,
        /// The classification of the response, used to determine the type of action.
        classification: AssistantClassification,
        /// The message to send in the thread.
        message: String,
    },

    // Built-in Tool calls.
    /// Update the channel directive with a message.
    UpdateChannelDirective {
        /// The unique identifier for the call, used to track the response.
        call_id: String,
        /// The message that represents what the bot "thinks about" the directive update.
        message: String,
    },
    /// Update the channel context with a message.
    UpdateContext {
        /// The unique identifier for the call, used to track the response.
        call_id: String,
        /// The message that represents what the bot "thinks about" the context update.
        message: String,
    },

    // MCP Tool calls.
    /// A call to an MCP tool with a specific name and arguments.
    McpTool {
        /// The unique identifier for the call, used to track the response.
        call_id: String,
        /// The name of the MCP tool to call.
        name: String,
        /// The arguments to pass to the MCP tool.
        arguments: Value,
    },
}

impl AssistantResponse {
    /// Check if the response is a tool call.
    pub fn is_tool_call(&self) -> bool {
        matches!(self, AssistantResponse::UpdateChannelDirective { .. } | AssistantResponse::UpdateContext { .. })
    }
}

/// An enum representing either raw text, or an LLM response.
///
/// This is used to encapsulate the different types of messages that can be sent
/// by the assistant, allowing for both simple text messages and more complex
/// responses that may include tool calls or other structured data.
#[derive(Debug, Serialize, Deserialize)]
pub enum TextOrResponse {
    /// A raw text message.
    Text(String),
    /// A response from the LLM.
    AssistantResponse(AssistantResponse),
}

/// Arguments for the direct / context update function tools.
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolContextFunctionCallArgs {
    /// The message that represents what the bot "thinks about" the directive / context update.
    pub message: String,
}

/// Definition of a tool, as sent to the LLM.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssistantTool {
    /// The name of the tool.
    pub name: String,
    /// The description of the tool.
    pub description: Option<String>,
    /// The parameters that the tool accepts.
    pub parameters: serde_json::Value,
}

/// Helper struct to handle the context for the web search LLM.
///
/// Contains all necessary information for the search agent to understand
/// the user's message and provide relevant search results.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct WebSearchContext {
    /// The user's message that will be used to search for relevant information.
    pub user_message: String,
    /// The bot's user ID, used to identify the bot in the context of the search.
    pub bot_user_id: String,
    /// The channel ID where the search is being performed.
    pub channel_id: String,
    /// The context of the channel, which may include settings or metadata relevant to the search.
    pub channel_context: String,
    /// The context of the thread, which may include previous messages or relevant information.
    pub thread_context: String,
}

/// Helper struct to handle the context for the message search LLM.
///
/// Contains all necessary information for the message search agent to
/// identify keywords from the user's message to find relevant channel history.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct MessageSearchContext {
    /// The user's message that will be used to search for relevant information.
    pub user_message: String,
    /// The bot's user ID, used to identify the bot in the context of the search.
    pub bot_user_id: String,
    /// The channel ID where the search is being performed.
    pub channel_id: String,
    /// The context of the channel, which may include settings or metadata relevant to the search.
    pub channel_context: String,
    /// The context of the thread, which may include previous messages or relevant information.
    pub thread_context: String,
}

/// Helper struct to handle the context for the assistant LLM.
///
/// Contains all necessary information for the assistant agent to understand
/// the user's message, channel settings, and relevant context to generate
/// an appropriate response.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct AssistantContext {
    /// The user's message that will be processed by the assistant.
    pub user_message: String,
    /// The bot's user ID, used to identify the bot in the context of the assistant.
    pub bot_user_id: String,
    /// The channel ID where the assistant is operating.
    pub channel_id: String,
    /// The timestamp of the thread where the assistant is responding.
    pub thread_ts: String,
    /// The context of the channel, which may include settings or metadata relevant to the assistant's operation.
    pub channel_directive: String,
    /// The context of the thread, which may include previous messages or relevant information.
    pub channel_context: String,
    /// The context of the thread, which may include previous messages or relevant information.
    pub thread_context: String,
    /// The web search context, which may include search results or relevant information gathered from the web.
    pub web_search_context: String,
    /// The message search context, which may include keywords or relevant information gathered from the channel history.
    pub message_search_context: String,
    /// A list of tools that the assistant can use to perform actions or gather information.
    pub tools: Vec<AssistantTool>,
}
