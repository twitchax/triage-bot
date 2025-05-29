//! Common types, results, and data structures used throughout triage-bot.
//!
//! This module defines core types and structures that are shared across the application,
//! including error handling types, context structures for LLM interactions, and response
//! types from the assistant.

use serde::{Deserialize, Serialize};

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
    Bug,
    Feature,
    Question,
    Incident,
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
    NoAction,
    ReplyToThread {
        thread_ts: String,
        classification: AssistantClassification,
        message: String,
    },

    // Tool calls.
    UpdateChannelDirective {
        message: String,
    },
    UpdateContext {
        message: String,
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

/// Helper struct to handle the context for the web search LLM.
///
/// Contains all necessary information for the search agent to understand
/// the user's message and provide relevant search results.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct WebSearchContext {
    pub user_message: String,
    pub bot_user_id: String,
    pub channel_id: String,
    pub channel_context: String,
    pub thread_context: String,
}

/// Helper struct to handle the context for the message search LLM.
///
/// Contains all necessary information for the message search agent to
/// identify keywords from the user's message to find relevant channel history.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct MessageSearchContext {
    pub user_message: String,
    pub bot_user_id: String,
    pub channel_id: String,
    pub channel_context: String,
    pub thread_context: String,
}

/// Helper struct to handle the context for the assistant LLM.
///
/// Contains all necessary information for the assistant agent to understand
/// the user's message, channel settings, and relevant context to generate
/// an appropriate response.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct AssistantContext {
    pub user_message: String,
    pub bot_user_id: String,
    pub channel_id: String,
    pub thread_ts: String,
    pub channel_directive: String,
    pub channel_context: String,
    pub thread_context: String,
    pub web_search_context: String,
    pub message_search_context: String,
}
