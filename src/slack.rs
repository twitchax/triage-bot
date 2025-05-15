//! Thin wrapper around Slack-Morphism client.


use crate::{config::Config, prelude::*};
use hyper_util::client::legacy::connect::{Connect, HttpConnector};
use slack_morphism::prelude::*;

use std::sync::Arc;

pub struct SlackClient {
    token: SlackApiTokenValue,
    client: Arc<slack_morphism::SlackClient<SlackClientHyperConnector<HttpConnector>>>,
}

impl SlackClient {
    pub async fn new(config: &Config) -> Res<Self> {
        let token = SlackApiTokenValue(config.slack_bot_token.clone());

        let connector = SlackClientHyperConnector::with_connector(HttpConnector::new());
        let client = Arc::new(slack_morphism::SlackClient::new(connector));

        Ok(Self { token, client })
    }

    pub async fn run(&self) -> Void {
        // TODO: subscribe to events, messages, etc.
        info!("Slack client is running (stub)");
        tokio::signal::ctrl_c().await?;
        Ok(())
    }
}
