use serde::{Deserialize, Serialize};

pub type Err = anyhow::Error;
pub type Res<T> = Result<T, Err>;
pub type Void = Res<()>;

#[derive(Debug, Serialize, Deserialize)]
pub enum LlmClassification {
    Bug,
    Feature,
    Question,
    Incident,
    Other,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LlmResponse {
    NoAction,
    UpdateChannelDirective { message: String },
    UpdateContext { message: String },
    ReplyToThread { thread_ts: String, classification: LlmClassification, message: String },
}
