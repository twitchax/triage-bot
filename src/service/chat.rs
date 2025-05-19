//! Wrapper around chat clients.

use crate::{
    base::{
        config::Config,
        types::{Res, Void},
    },
    interaction,
};
use async_trait::async_trait;
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::connect::HttpConnector;
use slack_morphism::prelude::*;
use tracing::{instrument, warn};

use std::{ops::Deref, sync::Arc};

use super::{db::DbClient, llm::LlmClient};

// Type aliases.

type FullClient = slack_morphism::SlackClient<SlackClientHyperConnector<HttpsConnector<HttpConnector>>>;
type Listener = SlackClientSocketModeListener<SlackClientHyperConnector<HttpsConnector<HttpConnector>>>;

// Traits.

/// Generic "chat" trait that clients must implement.
#[async_trait]
pub trait GenericChatClient {
    async fn start(&self) -> Void;
    async fn send_message(&self, channel_id: &str, text: &str) -> Void;
}

// Structs.

/// User state for the slack socket client.
struct SlackUserState {
    db: DbClient,
    llm: LlmClient,
    slack_client: Arc<FullClient>,
    user_id: String,
}

/// Slack client for the application.
///
/// It is designed to be trivially cloneable, allowing it to be passed around
/// without the need for `Arc` or `Mutex`.
#[derive(Clone)]
pub struct ChatClient {
    inner: Arc<dyn GenericChatClient + Send + Sync + 'static>,
}

impl Deref for ChatClient {
    type Target = dyn GenericChatClient + Send + Sync + 'static;

    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}

impl ChatClient {
    /// Creates a new Slack chat client.
    pub async fn slack(config: &Config, db: DbClient, llm: LlmClient) -> Res<Self> {
        let client = SlackChatClient::new(config, db.clone(), llm.clone()).await?;
        Ok(Self { inner: Arc::new(client) })
    }
}

// Specific implementations.

/// Slack client implementation.
#[derive(Clone)]
struct SlackChatClient {
    app_token: SlackApiToken,
    bot_token: SlackApiToken,
    user_id: String,
    client: Arc<FullClient>,
    socket_mode_listener: Arc<Listener>,
    db: DbClient,
    llm: LlmClient,
}

impl Deref for SlackChatClient {
    type Target = slack_morphism::SlackClient<SlackClientHyperConnector<HttpsConnector<HttpConnector>>>;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl SlackChatClient {
    pub async fn new(config: &Config, db: DbClient, llm: LlmClient) -> Res<Self> {
        // Initialize tokens.

        let app_token = SlackApiToken::new(SlackApiTokenValue(config.slack_app_token.clone()));
        let bot_token = SlackApiToken::new(SlackApiTokenValue(config.slack_bot_token.clone()));

        // Initialize the Slack client.

        let https_connector = HttpsConnector::<HttpConnector>::builder().with_native_roots()?.https_only().enable_all_versions().build();
        let connector = SlackClientHyperConnector::with_connector(https_connector);
        let client = Arc::new(slack_morphism::SlackClient::new(connector));

        // Get the bot's user ID.

        let session = client.open_session(&bot_token);
        let bot_user = session.auth_test().await?;
        let user_id = bot_user.user_id.0;

        // Initialize the socket mode listener.

        let socket_mode_callbacks = SlackSocketModeListenerCallbacks::new()
            .with_command_events(handle_command_event)
            .with_interaction_events(handle_interaction_event)
            .with_push_events(handle_push_event);

        // Initialize the socket mode listener environment.

        let listener_environment = Arc::new(SlackClientEventsListenerEnvironment::new(client.clone()).with_user_state(SlackUserState {
            db: db.clone(),
            llm: llm.clone(),
            user_id: user_id.clone(),
            slack_client: client.clone(),
        }));

        let socket_mode_listener = Arc::new(SlackClientSocketModeListener::new(
            &SlackClientSocketModeConfig::new(),
            listener_environment.clone(),
            socket_mode_callbacks,
        ));

        Ok(Self {
            app_token,
            bot_token,
            user_id,
            client,
            socket_mode_listener,
            db,
            llm,
        })
    }
}

#[async_trait]
impl GenericChatClient for SlackChatClient {
    async fn start(&self) -> Void {
        // Register an app token to listen for events,
        self.socket_mode_listener.listen_for(&self.app_token).await?;

        // Start WS connections calling Slack API to get WS url for the token,
        // and wait for Ctrl-C to shutdown.
        // There are also `.start()`/`.shutdown()` available to manage manually
        self.socket_mode_listener.serve().await;

        Ok(())
    }

    #[instrument(skip(self))]
    async fn send_message(&self, channel_id: &str, text: &str) -> Void {
        let message = SlackMessageContent::new().with_text(text.to_string());

        let request = SlackApiChatPostMessageRequest::new(SlackChannelId(channel_id.to_string()), message).with_as_user(true);

        let session = self.client.open_session(&self.bot_token);

        let _ = session.chat_post_message(&request).await.map_err(|e| anyhow::anyhow!("Failed to send message: {}", e))?;

        Ok(())
    }
}

// Socket mode listener callbacks for Slack..

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
    let user_state = states.get_user_state::<SlackUserState>().ok_or(anyhow::anyhow!("Failed to get user state"))?;

    match event {
        SlackEventCallbackBody::Message(slack_message_event) => {
            // If the message @mentions the bot, skip, and let the app mention handler take care of it.

            let text = slack_message_event.content.as_ref().map(|c| c.text.as_deref()).unwrap_or_default().unwrap_or_default();
            if text.contains(&user_state.user_id) {
                warn!("Skipping message event because it mentions the bot.");
                return Ok(());
            }

            interaction::message::handle_message(slack_message_event, user_state.db.clone(), user_state.llm.clone());
        }
        SlackEventCallbackBody::AppMention(slack_app_mention_event) => {
            interaction::app_mention::handle_app_mention(slack_app_mention_event, user_state.db.clone(), user_state.llm.clone());
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
