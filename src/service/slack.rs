//! Thin wrapper around Slack-Morphism client.

use crate::{base::{config::Config, types::{Res, Void}}, interaction};
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::connect::{HttpConnector};
use slack_morphism::prelude::*;
use tracing::{instrument, warn};

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

        let listener_environment = Arc::new(SlackClientEventsListenerEnvironment::new(client.clone()));

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

// Socket mode listener callbacks.

/// Handles command events from Slack.
async fn handle_command_event(
    event: SlackCommandEvent,
    _client: Arc<SlackHyperClient>,
    _states: SlackClientEventsUserState,
) -> Result<SlackCommandEventResponse, Box<dyn std::error::Error + Send + Sync>> {
    warn!("[COMMAND] {:#?}", event);
    Ok(SlackCommandEventResponse::new(SlackMessageContent::new().with_text("No app commands are currently supported.".into())))
}

/// Handles interaction events from Slack.
async fn handle_interaction_event(event: SlackInteractionEvent, _client: Arc<SlackHyperClient>, _states: SlackClientEventsUserState) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    warn!("[INTERACTION] {:#?}", event);
    Ok(())
}

/// Handles push events from Slack.
#[instrument(skip(_states))]
async fn handle_push_event(event_callback: SlackPushEventCallback, _client: Arc<SlackHyperClient>, _states: SlackClientEventsUserState) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let event = event_callback.event;

    match event {
        SlackEventCallbackBody::Message(slack_message_event) => {
            interaction::message::handle_message(slack_message_event).await?;
        },
        SlackEventCallbackBody::AppMention(slack_app_mention_event) => {
            interaction::app_mention::handle_app_mention(slack_app_mention_event).await?;
        },
        SlackEventCallbackBody::LinkShared(slack_link_shared_event) => todo!(),
        SlackEventCallbackBody::ReactionAdded(slack_reaction_added_event) => todo!(),
        SlackEventCallbackBody::ReactionRemoved(slack_reaction_removed_event) => todo!(),
        SlackEventCallbackBody::StarAdded(slack_star_added_event) => todo!(),
        SlackEventCallbackBody::StarRemoved(slack_star_removed_event) => todo!(),
        _ => {
            warn!("[PUSH] Received unhandled push event.")
        }
    }

    Ok(())
}
