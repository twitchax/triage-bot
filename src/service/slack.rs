//! Thin wrapper around Slack-Morphism client.

use crate::{
    base::{
        config::Config,
        types::{Res, Void},
    },
    interaction,
};
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::connect::HttpConnector;
use slack_morphism::prelude::*;
use tracing::{instrument, warn};

use std::{ops::Deref, sync::Arc};

use super::{db::DbClient, llm::LlmClient};

type FullClient = slack_morphism::SlackClient<SlackClientHyperConnector<HttpsConnector<HttpConnector>>>;
type Listener = SlackClientSocketModeListener<SlackClientHyperConnector<HttpsConnector<HttpConnector>>>;

struct UserState {
    db: DbClient,
    llm: LlmClient,
}

/// Slack client for the application.
///
/// It is designed to be trivially cloneable, allowing it to be passed around
/// without the need for `Arc` or `Mutex`.
#[derive(Clone)]
pub struct SlackClient {
    inner: Arc<SlackClientInner>,
}

#[derive(Clone)]
struct SlackClientInner {
    app_token: SlackApiToken,
    bot_token: SlackApiToken,
    client: Arc<FullClient>,
    socket_mode_listener: Arc<Listener>,
    db: DbClient,
    llm: LlmClient,
}

impl Deref for SlackClient {
    type Target = slack_morphism::SlackClient<SlackClientHyperConnector<HttpsConnector<HttpConnector>>>;

    fn deref(&self) -> &Self::Target {
        &self.inner.client
    }
}

impl SlackClient {
    pub async fn new(config: &Config, db: DbClient, llm: LlmClient) -> Res<Self> {
        let app_token = SlackApiToken::new(SlackApiTokenValue(config.slack_app_token.clone()));
        let bot_token = SlackApiToken::new(SlackApiTokenValue(config.slack_bot_token.clone()));

        let https_connector = HttpsConnector::<HttpConnector>::builder().with_native_roots()?.https_only().enable_all_versions().build();
        let connector = SlackClientHyperConnector::with_connector(https_connector);
        let client = Arc::new(slack_morphism::SlackClient::new(connector));

        let socket_mode_callbacks = SlackSocketModeListenerCallbacks::new()
            .with_command_events(handle_command_event)
            .with_interaction_events(handle_interaction_event)
            .with_push_events(handle_push_event);

        let listener_environment = Arc::new(SlackClientEventsListenerEnvironment::new(client.clone()).with_user_state(UserState {
            db: db.clone(),
            llm: llm.clone(),
        }));

        let socket_mode_listener = Arc::new(SlackClientSocketModeListener::new(
            &SlackClientSocketModeConfig::new(),
            listener_environment.clone(),
            socket_mode_callbacks,
        ));

        Ok(Self {
            inner: Arc::new(SlackClientInner {
                app_token,
                bot_token,
                client,
                socket_mode_listener,
                db,
                llm,
            }),
        })
    }

    pub async fn start(&self) -> Void {
        // Register an app token to listen for events,
        self.inner.socket_mode_listener.listen_for(&self.inner.app_token).await?;

        // Start WS connections calling Slack API to get WS url for the token,
        // and wait for Ctrl-C to shutdown.
        // There are also `.start()`/`.shutdown()` available to manage manually
        self.inner.socket_mode_listener.serve().await;

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
#[instrument(skip_all)]
async fn handle_push_event(event_callback: SlackPushEventCallback, _client: Arc<SlackHyperClient>, states: SlackClientEventsUserState) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let event = event_callback.event;
    let states = states.read().await;
    let user_state = states.get_user_state::<UserState>().ok_or(anyhow::anyhow!("Failed to get user state"))?;

    match event {
        SlackEventCallbackBody::Message(slack_message_event) => {
            interaction::message::handle_message(slack_message_event, &user_state.db, &user_state.llm).await?;
        }
        SlackEventCallbackBody::AppMention(slack_app_mention_event) => {
            interaction::app_mention::handle_app_mention(slack_app_mention_event, &user_state.db, &user_state.llm).await?;
        }
        SlackEventCallbackBody::LinkShared(slack_link_shared_event) => todo!(),
        SlackEventCallbackBody::ReactionAdded(slack_reaction_added_event) => todo!(),
        SlackEventCallbackBody::ReactionRemoved(slack_reaction_removed_event) => todo!(),
        SlackEventCallbackBody::StarAdded(slack_star_added_event) => todo!(),
        SlackEventCallbackBody::StarRemoved(slack_star_removed_event) => todo!(),
        _ => {
            warn!("Received unhandled push event.")
        }
    }

    Ok(())
}
