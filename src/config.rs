//! Load configuration via `config` crate with env-override support.

use serde::Deserialize;
use crate::base::Res;


#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// OpenAI API key (`OPENAI_API_KEY`)
    pub openai_api_key: String,
    /// Slack bot token (`SLACK_BOT_TOKEN`)
    pub slack_bot_token: String,
    /// Slack signing secret (`SLACK_SIGNING_SECRET`)
    pub slack_signing_secret: String,
}

impl Config {
    pub fn load(
        explicit_path: Option<&std::path::Path>,
    ) -> Res<Self> {
        let mut cfg = config::Config::builder()
            .add_source(config::Environment::default().separator("_"));

        if let Some(p) = explicit_path {
            cfg = cfg.add_source(config::File::from(p.to_path_buf()));
        } else if std::path::Path::new(".hidden/config.toml").exists() {
            cfg = cfg.add_source(config::File::with_name(".hidden/config.toml"));
        }

        Ok(cfg.build()?.try_deserialize()?)
    }
}
