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
use slack_morphism::{errors::{SlackClientApiError, SlackClientError}, prelude::*};
use tracing::{info, instrument, warn};

use std::{ops::Deref, sync::Arc};

use super::{db::DbClient, llm::LlmClient};

// Type aliases.

type FullClient = slack_morphism::SlackClient<SlackClientHyperConnector<HttpsConnector<HttpConnector>>>;
type Listener = SlackClientSocketModeListener<SlackClientHyperConnector<HttpsConnector<HttpConnector>>>;

// Traits.

/// Generic "chat" trait that clients must implement.
#[async_trait]
pub trait GenericChatClient {
    /// Get the bot user ID.
    fn bot_user_id(&self) -> &str;
    /// Start the chat client listener.
    async fn start(&self) -> Void;
    /// Send a message to a channel thread.
    async fn send_message(&self, channel_id: &str, thread_ts: &str, text: &str) -> Void;
    /// React to a message with an emoji.
    async fn react_to_message(&self, channel_id: &str, thread_ts: &str, emoji: &str) -> Void;
    /// Get the entirety of the thread context.
    async fn get_thread_context(&self, channel_id: &str, thread_ts: &str) -> Res<String>;
}

// Structs.

/// User state for the slack socket client.
struct SlackUserState {
    db: DbClient,
    llm: LlmClient,
    chat_client: ChatClient,
    bot_user_id: String,
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

impl From<SlackChatClient> for ChatClient {
    fn from(client: SlackChatClient) -> Self {
        Self { inner: Arc::new(client) }
    }
}

// Specific implementations.

/// Slack client implementation.
#[derive(Clone)]
struct SlackChatClient {
    app_token: SlackApiToken,
    bot_token: SlackApiToken,
    bot_user_id: String,
    client: Arc<FullClient>,
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
    /// Create a new Slack chat client.
    #[instrument(name = "SlackChatClient::new", skip_all)]
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
        let bot_user_id = bot_user.user_id.0;

        info!("Slack bot user ID: {}", bot_user_id);

        Ok(Self {
            app_token,
            bot_token,
            bot_user_id,
            client,
            db,
            llm,
        })
    }
}

#[async_trait]
impl GenericChatClient for SlackChatClient {
    fn bot_user_id(&self) -> &str {
        &self.bot_user_id
    }

    async fn start(&self) -> Void {
        // Initialize the socket mode listener.

        let socket_mode_callbacks = SlackSocketModeListenerCallbacks::new()
            .with_command_events(handle_command_event)
            .with_interaction_events(handle_interaction_event)
            .with_push_events(handle_push_event);

        // Initialize the socket mode listener environment.

        let listener_environment = Arc::new(SlackClientEventsListenerEnvironment::new(self.client.clone()).with_user_state(SlackUserState {
            db: self.db.clone(),
            llm: self.llm.clone(),
            bot_user_id: self.bot_user_id.clone(),
            chat_client: ChatClient::from(self.clone()),
        }));

        let socket_mode_listener = Arc::new(SlackClientSocketModeListener::new(
            &SlackClientSocketModeConfig::new(),
            listener_environment.clone(),
            socket_mode_callbacks,
        ));

        // Register an app token to listen for events,
        socket_mode_listener.listen_for(&self.app_token).await?;

        // Start WS connections calling Slack API to get WS url for the token,
        // and wait for Ctrl-C to shutdown.
        // There are also `.start()`/`.shutdown()` available to manage manually
        socket_mode_listener.serve().await;

        Ok(())
    }

    #[instrument(skip(self))]
    async fn send_message(&self, channel_id: &str, thread_ts: &str, text: &str) -> Void {
        let message = SlackMessageContent::new().with_text(text.to_string());

        let request = SlackApiChatPostMessageRequest::new(SlackChannelId(channel_id.to_string()), message).with_as_user(true).with_thread_ts(SlackTs(thread_ts.to_string())).with_link_names(true);

        let session = self.client.open_session(&self.bot_token);

        let _ = session.chat_post_message(&request).await.map_err(|e| anyhow::anyhow!("Failed to send message: {}", e))?;

        Ok(())
    }

    #[instrument(skip(self))]
    async fn react_to_message(&self, channel_id: &str, thread_ts: &str, emoji: &str) -> Void {
        let request = SlackApiReactionsAddRequest { channel: SlackChannelId(channel_id.to_string()), name: SlackReactionName(emoji.to_string()), timestamp: SlackTs(thread_ts.to_string()) };

        let session = self.client.open_session(&self.bot_token);

        let _ = session.reactions_add(&request).await.map_err(|e| anyhow::anyhow!("Failed to react to message: {}", e))?;

        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_thread_context(&self, channel_id: &str, thread_ts: &str) -> Res<String> {
        let request = SlackApiConversationsRepliesRequest::new(SlackChannelId(channel_id.to_string()), SlackTs(thread_ts.to_string()));
        let session = self.client.open_session(&self.bot_token);

        let response = session.conversations_replies(&request).await;

        let response = if let Err(e) = &response && let SlackClientError::ApiError(ae) = e && ae.code == "thread_not_found" {
            // If the thread is not found (due to this being a top-level message), we can just return an empty string.
            return Ok("".to_string());
        } else {
            response?
        };

        let messages = serde_json::to_string(&response.messages)?;

        Ok(messages)
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
            if text.contains(&user_state.bot_user_id) {
                warn!("Skipping message event because it mentions the bot.");
                return Ok(());
            }

            // If the message is in a thread, skip, since we don't want the bot to respond unless it is mentioned in a thread.
            if slack_message_event.origin.thread_ts.is_some() {
                warn!("Skipping message event because it is in a thread.");
                return Ok(());
            }

            let channel_id = slack_message_event.origin.channel.as_ref().ok_or(anyhow::anyhow!("Failed to get channel ID"))?.0.to_owned();
            let thread_ts = slack_message_event.origin.thread_ts.clone().unwrap_or(SlackTs("".to_string())).0;
            interaction::chat_event::handle_chat_event(slack_message_event, channel_id, thread_ts, user_state.db.clone(), user_state.llm.clone(), user_state.chat_client.clone());
        }
        SlackEventCallbackBody::AppMention(slack_app_mention_event) => {
            // Check to see if this is a thread message.  If so, we are going to want to provide the entire thread context to the LLM.

            let channel_id = slack_app_mention_event.channel.0.to_owned();
            let thread_ts = slack_app_mention_event.origin.thread_ts.clone().unwrap_or(SlackTs("".to_string())).0;
            interaction::chat_event::handle_chat_event(slack_app_mention_event, channel_id, thread_ts, user_state.db.clone(), user_state.llm.clone(), user_state.chat_client.clone());
        }
        //SlackEventCallbackBody::LinkShared(slack_link_shared_event) => todo!(),
        //SlackEventCallbackBody::ReactionAdded(slack_reaction_added_event) => todo!(),
        //SlackEventCallbackBody::ReactionRemoved(slack_reaction_removed_event) => todo!(),
        //SlackEventCallbackBody::StarAdded(slack_star_added_event) => todo!(),
        //SlackEventCallbackBody::StarRemoved(slack_star_removed_event) => todo!(),
        _ => {
            warn!("Received unhandled push event.")
        }
    }

    Ok(())
}
