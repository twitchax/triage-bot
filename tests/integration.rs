use std::{env, sync::Arc};

use slack_morphism::{
    api::{SlackApiChatPostMessageRequest, SlackApiChatPostMessageResponse},
    prelude::{SlackClientHyperConnector, SlackHyperClient},
    SlackApiToken, SlackClient, SlackClientHyperHttpsConnector,
};
use triage_bot::{
    base::{
        config::{Config, ConfigInner},
        types::LlmResponse,
    },
    service::{
        chat::{ChatClient, SlackClientEventsUserState},
        db::DbClient,
        llm::LlmClient,
    },
};

/// A helper function to create a test configuration.
/// 
/// This configuration uses the OPENAI_API_KEY environment variable
/// for the LLM and sets up an in-memory database.
fn create_test_config() -> Config {
    let openai_api_key = env::var("OPENAI_API_KEY").unwrap_or_else(|_| {
        panic!("OPENAI_API_KEY environment variable is required for integration tests")
    });
    
    Config {
        inner: Arc::new(ConfigInner {
            openai_api_key,
            openai_search_agent_model: "gpt-3.5-turbo".to_string(), // Using a cheaper model for tests
            openai_assistant_agent_model: "gpt-3.5-turbo".to_string(), // Using a cheaper model for tests
            openai_search_agent_temperature: 0.0,
            openai_assistant_agent_temperature: 0.7,
            openai_max_tokens: 256u32, // Using fewer tokens for tests
            slack_app_token: "xapp-test".to_string(),
            slack_bot_token: "xoxb-test".to_string(),
            slack_signing_secret: "secret".to_string(),
            db_endpoint: "memory".to_string(), // This is ignored in test mode
            db_username: "test".to_string(),
            db_password: "test".to_string(),
            ..Default::default()
        }),
    }
}

/// Create a mock Slack app mention event
fn create_app_mention_event() -> slack_morphism::events::SlackAppMentionEvent {
    use slack_morphism::prelude::*;
    
    SlackAppMentionEvent {
        client_msg_id: Some("test-msg-id".to_string()),
        origin: SlackEventOrigin {
            type_field: "app_mention".to_string(),
            user: SlackUserId("U123456".to_string()),
            channel: Some(SlackChannelId("C123456".to_string())),
            ts: SlackTs("1234567890.123456".to_string()),
            team: Some(SlackTeamId("T123456".to_string())),
            thread_ts: Some(SlackTs("1234567890.123456".to_string())),
            channel_type: Some("channel".to_string()),
            event_ts: "1234567890.123456".to_string(),
        },
        channel: SlackChannelId("C123456".to_string()),
        user: SlackUserId("U123456".to_string()),
        text: "<@UBOT123> Hello bot, can you help me with a critical issue?".to_string(),
        ts: SlackTs("1234567890.123456".to_string()),
    }
}

// Create a mock Slack push event callback containing the app mention
fn create_push_event_callback(app_mention: slack_morphism::events::SlackAppMentionEvent) -> slack_morphism::prelude::SlackPushEventCallback {
    use slack_morphism::prelude::*;
    
    SlackPushEventCallback {
        token: "test-token".to_string(),
        team_id: SlackTeamId("T123456".to_string()),
        api_app_id: "A123456".to_string(),
        event: SlackEventCallbackBody::AppMention(app_mention),
        event_id: "Ev123456".to_string(),
        event_time: 1234567890,
        authed_users: None,
        authorizations: vec![SlackEventAuthorization {
            enterprise_id: None,
            team_id: SlackTeamId("T123456".to_string()),
            user_id: SlackUserId("U123456".to_string()),
            is_bot: false,
        }],
        event_context: "ctx".to_string(),
    }
}

// Mock implementation of SlackHyperClient for testing
struct MockSlackClient;

impl SlackHyperClient for MockSlackClient {
    fn get_api_token(&self) -> Option<&SlackApiToken> {
        None
    }
    
    fn get_http_client(&self) -> &SlackClient<SlackClientHyperHttpsConnector> {
        unimplemented!("Not needed for this test")
    }
}

#[tokio::test]
async fn test_handle_push_event() {
    // Skip test if OPENAI_API_KEY is not available
    if env::var("OPENAI_API_KEY").is_err() {
        println!("Skipping integration test: OPENAI_API_KEY not available");
        return;
    }
    
    // Create test configuration
    let config = create_test_config();
    
    // Initialize the database client
    let db = DbClient::surreal(&config).await.expect("Failed to create DB client");
    
    // Initialize the LLM client
    let llm = LlmClient::openai(&config);
    
    // Create a test channel and add initial context
    let channel_id = "C123456";
    let _channel = db.get_or_create_channel(channel_id).await.expect("Failed to create channel");
    
    // Create a mock Slack client
    let client = Arc::new(MockSlackClient);
    
    // Create a chat client for testing
    let chat_client = ChatClient::from_client(client.clone());
    
    // Create user state
    let user_state = SlackClientEventsUserState {
        db: db.clone(),
        llm: llm.clone(),
        chat_client,
    };
    
    // Create a test app mention event
    let app_mention = create_app_mention_event();
    
    // Create a push event callback
    let push_event = create_push_event_callback(app_mention);
    
    // Call the push event handler directly
    let result = triage_bot::service::chat::handle_push_event_for_testing(push_event, client, user_state).await;
    
    // Verify the result
    assert!(result.is_ok(), "Push event handler failed: {:?}", result);
    
    // Check if the DB was updated with a channel context
    let updated_channel = db.get_or_create_channel(channel_id).await.expect("Failed to get channel");
    println!("Channel directive: {:?}", updated_channel.channel_directive);
}
}