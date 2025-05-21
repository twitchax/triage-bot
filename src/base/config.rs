//! Load configuration via `config` crate with env-override support.

use std::{ops::Deref, sync::Arc};

use serde::Deserialize;

use super::types::Res;

/// Default OpenAI model to use
fn default_openai_model() -> String {
    "gpt-4.1".to_string()
}

/// Default sampling temperature for OpenAI model
fn default_openai_temperature() -> f32 {
    0.7
}

/// Default max output tokens for OpenAI model
fn default_openai_max_tokens() -> u32 {
    2048
}

/// Configuration for the triage-bot application.
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub inner: Arc<ConfigInner>,
}

impl Deref for Config {
    type Target = ConfigInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ConfigInner {
    /// OpenAI API key (`OPENAI_API_KEY`).
    pub openai_api_key: String,
    /// OpenAI model to use (`OPENAI_MODEL`).
    #[serde(default = "default_openai_model")]
    pub openai_model: String,
    /// Optional custom system prompt to override the default (`SYSTEM_PROMPT`).
    #[serde(default)]
    pub system_prompt: Option<String>,
    /// Optional custom mention addendum prompt to override the default (`MENTION_ADDENDUM_PROMPT`).
    #[serde(default)]
    pub mention_addendum_prompt: Option<String>,
    /// Sampling temperature to use for OpenAI model (`OPENAI_TEMPERATURE`).
    /// Value between 0 and 2. Higher values like 0.8 make output more random,
    /// while lower values like 0.2 make it more focused and deterministic.
    #[serde(default = "default_openai_temperature")]
    pub openai_temperature: f32,
    /// Max output tokens for OpenAI model (`OPENAI_MAX_TOKENS`).
    /// Maximum number of tokens that can be generated in the response.
    #[serde(default = "default_openai_max_tokens")]
    pub openai_max_tokens: u32,
    /// Slack app token (`SLACK_APP_TOKEN`).
    pub slack_app_token: String,
    /// Slack bot token (`SLACK_BOT_TOKEN`).
    pub slack_bot_token: String,
    /// Slack signing secret (`SLACK_SIGNING_SECRET`).
    pub slack_signing_secret: String,
    /// Database endpoint URL (`DB_ENDPOINT`).
    pub db_endpoint: String,
    /// Database username (`DB_USERNAME`).
    pub db_username: String,
    /// Database password (`DB_PASSWORD`).
    pub db_password: String,
}

impl Config {
    pub fn load(explicit_path: Option<&std::path::Path>) -> Res<Self> {
        let mut cfg = config::Config::builder().add_source(config::Environment::default().prefix("TRIAGE_BOT"));

        if let Some(p) = explicit_path {
            cfg = cfg.add_source(config::File::from(p.to_path_buf()));
        } else if std::path::Path::new(".hidden/config.toml").exists() {
            cfg = cfg.add_source(config::File::with_name(".hidden/config.toml"));
        }

        let result = Config {
            inner: Arc::new(cfg.build()?.try_deserialize()?),
        };

        if result.openai_temperature < 0.0 || result.openai_temperature > 2.0 {
            return Err(anyhow::anyhow!("OpenAI temperature must be between 0 and 2."));
        }

        if result.openai_max_tokens < 1 || result.openai_max_tokens > 128000 {
            return Err(anyhow::anyhow!("OpenAI max tokens must be between 1 and 128000."));
        }

        Ok(result)
    }
}
