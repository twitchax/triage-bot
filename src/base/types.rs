use serde::{Deserialize, Serialize};

pub type Err = anyhow::Error;
pub type Res<T> = Result<T, Err>;
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

/// Helper struct to handle the context for the search LLM.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct WebSearchContext {
    pub user_message: String,
    pub bot_user_id: String,
    pub channel_id: String,
    pub channel_context: String,
    pub thread_context: String,
}

/// Helper struct to handle the context for the assistant LLM.
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
}
