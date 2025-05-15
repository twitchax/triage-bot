//! Thin wrapper around Slack-Morphism client.


use crate::{base::config::Config, prelude::*};
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::connect::{Connect, HttpConnector};
use slack_morphism::prelude::*;

use std::sync::Arc;

pub struct SlackClient {
    token: SlackApiToken,
    socket_mode_listener: Arc<SlackClientSocketModeListener<SlackClientHyperConnector<HttpsConnector<HttpConnector>>>>,
}

impl SlackClient {
    pub async fn new(config: &Config) -> Res<Self> {
        let token = SlackApiToken::new(SlackApiTokenValue(config.slack_bot_token.clone()));

        let https_connector = HttpsConnector::<HttpConnector>::builder().with_native_roots()?.https_only().enable_all_versions().build();
        let connector = SlackClientHyperConnector::with_connector(https_connector);
        let client = Arc::new(slack_morphism::SlackClient::new(connector));

        // Placeholder for starting the Slack client.
        let socket_mode_callbacks = SlackSocketModeListenerCallbacks::new()
            .with_command_events(handle_command_event)
            .with_interaction_events(handle_interaction_event)
            .with_push_events(handle_push_event);

        let listener_environment = Arc::new(
            SlackClientEventsListenerEnvironment::new(client.clone())
        );

        let socket_mode_listener = Arc::new(SlackClientSocketModeListener::new(
            &SlackClientSocketModeConfig::new(),
            listener_environment.clone(),
            socket_mode_callbacks,
        ));

        Ok(Self { token, socket_mode_listener })
    }

    pub async fn start(&self) -> Void {
        // Register an app token to listen for events, 
        self.socket_mode_listener.listen_for(&self.token).await?;

        // Start WS connections calling Slack API to get WS url for the token, 
        // and wait for Ctrl-C to shutdown.
        // There are also `.start()`/`.shutdown()` available to manage manually 
        self.socket_mode_listener.serve().await;

        Ok(())
    }
}

/// Handles command events from Slack.
async fn handle_command_event(
    event: SlackCommandEvent,
    _client: Arc<SlackHyperClient>,
    _states: SlackClientEventsUserState,
) -> Result<SlackCommandEventResponse, Box<dyn std::error::Error + Send + Sync>> {
    info!("[COMMAND] {:#?}", event);
    Ok(SlackCommandEventResponse::new(
        SlackMessageContent::new().with_text("Working on it".into()),
    ))
}

/// Handles interaction events from Slack.
async fn handle_interaction_event(
    event: SlackInteractionEvent,
    _client: Arc<SlackHyperClient>,
    _states: SlackClientEventsUserState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("[INTERACTION] {:#?}", event);
    
    Ok(())
}

/// Handles push events from Slack.
async fn handle_push_event(
    event: SlackPushEventCallback,
    _client: Arc<SlackHyperClient>,
    _states: SlackClientEventsUserState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("[PUSH] {:#?}", event);
    
    Ok(())
}