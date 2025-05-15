//! Thin wrapper around async-openai.

use async_openai::{config::OpenAIConfig, Client};

use crate::base::config::Config;

pub struct LlmClient {
    #[allow(dead_code)]
    client: Client<OpenAIConfig>,
}

impl LlmClient {
    pub fn new(config: &Config) -> Self {
        let cfg = OpenAIConfig::new().with_api_key(config.openai_api_key.clone());
        Self {
            client: Client::with_config(cfg),
        }
    }
}
