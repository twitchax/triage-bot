//! Integration tests for the triage-bot.
//!
//! These tests verify the end-to-end functionality of the bot by directly
//! calling the handlers with well-formed events, using the in-memory database mode,
//! and using the actual LLM client without mocking (except for specific test cases).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use mockall::mock;
use serde_json::json;
use slack_morphism::{
    api::SlackApiToken,
    prelude::{SlackApiTokenValue, SlackChannelId, SlackTs, SlackUserId, SlackMessageContent, SlackOrigin},
    socket_mode::{SlackAppMentionEvent, SlackMessageEvent},
};
use triage_bot::{
    base::{
        config::{Config, ConfigInner},
        types::{LlmClassification, LlmResponse, Res, Void},
    },
    interaction::chat_event,
    service::{
        chat::{ChatClient, GenericChatClient},
        db::{DbClient, LlmContext},
        llm::{LlmClient, GenericLlmClient},
    },
};

// Mock chat client for testing
mock! {
    pub Chat {}

    #[async_trait]
    impl GenericChatClient for Chat {
        fn bot_user_id(&self) -> &str;
        async fn start(&self) -> Void;
        async fn send_message(&self, channel_id: &str, thread_ts: &str, text: &str) -> Void;
        async fn react_to_message(&self, channel_id: &str, thread_ts: &str, emoji: &str) -> Void;
        async fn get_thread_context(&self, channel_id: &str, thread_ts: &str) -> Res<String>;
    }
}

// Mock LLM client for certain tests
mock! {
    pub Llm {}

    #[async_trait]
    impl GenericLlmClient for Llm {
        async fn generate_response(&self, self_id: &str, channel_directive: &str, channel_context: &str, thread_context: &str, user_message: &str) -> Res<Vec<LlmResponse>>;
    }
}

#[tokio::test]
async fn test_app_mention_integration() {
    // Set up the test environment
    let openai_api_key = std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "dummy_key".to_string());
    
    // Create config
    let config = Config {
        inner: Arc::new(ConfigInner {
            openai_api_key,
            openai_search_agent_model: "gpt-3.5-turbo".to_string(), // Use a smaller model for testing
            openai_assistant_agent_model: "gpt-3.5-turbo".to_string(), // Use a smaller model for testing
            openai_search_agent_temperature: 0.0,
            openai_assistant_agent_temperature: 0.7,
            openai_max_tokens: 2048u32,
            slack_app_token: "xapp-test".to_string(),
            slack_bot_token: "xoxb-test".to_string(),
            slack_signing_secret: "test-secret".to_string(),
            db_endpoint: "ws://localhost:8000".to_string(), // Not used in test mode
            db_username: "test".to_string(),
            db_password: "test".to_string(),
            ..Default::default()
        }),
    };
    
    // Create DB client (in-memory mode)
    let db_client = DbClient::surrealdb(&config).await.expect("Failed to create DB client");
    
    // Create LLM client
    let llm_client = LlmClient::openai(&config);
    
    // Create mock chat client
    let mut mock_chat = MockChat::new();
    
    // Set up mock chat client expectations
    mock_chat.expect_bot_user_id().returning(|| "U12345678");
    mock_chat.expect_get_thread_context()
        .returning(|_, _| Ok("Previous thread context".to_string()));
    mock_chat.expect_react_to_message()
        .returning(|_, _, _| Ok(()));
    mock_chat.expect_send_message()
        .returning(|_, _, _| Ok(()));
    
    let chat_client = ChatClient { inner: Arc::new(mock_chat) };
    
    // Create a test channel
    let channel_id = "C12345678";
    let thread_ts = "1234567890.123456";
    
    // Create a test app mention event
    let app_mention_event = SlackAppMentionEvent {
        channel: SlackChannelId(channel_id.to_string()),
        user: SlackUserId("U87654321".to_string()),
        text: Some("Hey <@U12345678> can you help me?".to_string()),
        ts: SlackTs("1234567890.123456".to_string()),
        origin: Default::default(),
    };
    
    // Handle the event by calling the public function
    chat_event::handle_chat_event(
        app_mention_event,
        channel_id.to_string(),
        thread_ts.to_string(),
        db_client.clone(),
        llm_client.clone(),
        chat_client.clone(),
    );
    
    // Give the background task some time to complete
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // Verify the channel directive was created
    let channel = db_client.get_or_create_channel(channel_id).await.unwrap();
    assert!(!channel.channel_directive.your_notes.is_empty(), "Channel directive should be updated");
}

#[tokio::test]
async fn test_message_event_integration() {
    // Set up the test environment
    let openai_api_key = std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "dummy_key".to_string());
    
    // Create config
    let config = Config {
        inner: Arc::new(ConfigInner {
            openai_api_key,
            openai_search_agent_model: "gpt-3.5-turbo".to_string(), // Use a smaller model for testing
            openai_assistant_agent_model: "gpt-3.5-turbo".to_string(), // Use a smaller model for testing
            openai_search_agent_temperature: 0.0,
            openai_assistant_agent_temperature: 0.7,
            openai_max_tokens: 2048u32,
            slack_app_token: "xapp-test".to_string(),
            slack_bot_token: "xoxb-test".to_string(),
            slack_signing_secret: "test-secret".to_string(),
            db_endpoint: "ws://localhost:8000".to_string(), // Not used in test mode
            db_username: "test".to_string(),
            db_password: "test".to_string(),
            ..Default::default()
        }),
    };
    
    // Create DB client (in-memory mode)
    let db_client = DbClient::surrealdb(&config).await.expect("Failed to create DB client");
    
    // Create LLM client
    let llm_client = LlmClient::openai(&config);
    
    // Create mock chat client
    let mut mock_chat = MockChat::new();
    
    // Set up mock chat client expectations
    mock_chat.expect_bot_user_id().returning(|| "U12345678");
    mock_chat.expect_get_thread_context()
        .returning(|_, _| Ok("Previous thread context".to_string()));
    mock_chat.expect_react_to_message()
        .returning(|_, _, _| Ok(()));
    mock_chat.expect_send_message()
        .returning(|_, _, _| Ok(()));
    
    let chat_client = ChatClient { inner: Arc::new(mock_chat) };
    
    // Create a test channel
    let channel_id = "C12345678";
    let thread_ts = "1234567890.123456";
    
    // Create a test message event
    let message_content = SlackMessageContent::new().with_text("I need help with something".to_string());
    let message_event = SlackMessageEvent {
        channel: Some(SlackChannelId(channel_id.to_string())),
        user: Some(SlackUserId("U87654321".to_string())),
        content: Some(message_content),
        ts: SlackTs("1234567890.123456".to_string()),
        origin: Default::default(),
    };
    
    // Handle the event by calling the public function
    chat_event::handle_chat_event(
        message_event,
        channel_id.to_string(),
        thread_ts.to_string(),
        db_client.clone(),
        llm_client,
        chat_client,
    );
    
    // Give the background task some time to complete
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // Verify the channel context was updated
    let contexts = db_client.get_channel_context(channel_id).await.unwrap();
    assert!(!contexts.is_empty(), "Channel context should have been created");
}

#[tokio::test]
async fn test_llm_responses_integration() {
    // Set up the test environment
    let openai_api_key = std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "dummy_key".to_string());
    
    // Create config
    let config = Config {
        inner: Arc::new(ConfigInner {
            openai_api_key,
            openai_search_agent_model: "gpt-3.5-turbo".to_string(),
            openai_assistant_agent_model: "gpt-3.5-turbo".to_string(),
            openai_search_agent_temperature: 0.0,
            openai_assistant_agent_temperature: 0.7,
            openai_max_tokens: 2048u32,
            slack_app_token: "xapp-test".to_string(),
            slack_bot_token: "xoxb-test".to_string(),
            slack_signing_secret: "test-secret".to_string(),
            db_endpoint: "ws://localhost:8000".to_string(),
            db_username: "test".to_string(),
            db_password: "test".to_string(),
            ..Default::default()
        }),
    };
    
    // Create DB client (in-memory mode)
    let db_client = DbClient::surrealdb(&config).await.expect("Failed to create DB client");
    
    // Create mock LLM client with predetermined responses
    let mut mock_llm = MockLlm::new();
    mock_llm.expect_generate_response()
        .returning(|_, _, _, _, _| {
            Ok(vec![
                LlmResponse::UpdateChannelDirective { 
                    message: "This is a channel directive update".to_string() 
                },
                LlmResponse::UpdateContext { 
                    message: "This is a context update".to_string() 
                },
                LlmResponse::ReplyToThread { 
                    thread_ts: "1234567890.123456".to_string(),
                    classification: LlmClassification::Question,
                    message: "This is a thread reply".to_string()
                },
                LlmResponse::NoAction,
            ])
        });
    
    let llm_client = LlmClient { inner: Arc::new(mock_llm) };
    
    // Create mock chat client
    let mut mock_chat = MockChat::new();
    
    // Set up mock chat client expectations
    mock_chat.expect_bot_user_id().returning(|| "U12345678");
    mock_chat.expect_get_thread_context()
        .returning(|_, _| Ok("Previous thread context".to_string()));
    mock_chat.expect_react_to_message()
        .returning(|_, _, _| Ok(()));
    mock_chat.expect_send_message()
        .returning(|_, _, _| Ok(()));
    
    let chat_client = ChatClient { inner: Arc::new(mock_chat) };
    
    // Create a test channel
    let channel_id = "C12345678";
    let thread_ts = "1234567890.123456";
    
    // Create a test app mention event with thread
    let origin = SlackOrigin {
        thread_ts: Some(SlackTs(thread_ts.to_string())),
        channel: Some(SlackChannelId(channel_id.to_string())),
        ..Default::default()
    };
    
    let app_mention_event = SlackAppMentionEvent {
        channel: SlackChannelId(channel_id.to_string()),
        user: SlackUserId("U87654321".to_string()),
        text: Some("Hey <@U12345678> can you help me?".to_string()),
        ts: SlackTs("1234567890.123456".to_string()),
        origin,
    };
    
    // Handle the event by calling the public function
    chat_event::handle_chat_event(
        app_mention_event,
        channel_id.to_string(),
        thread_ts.to_string(),
        db_client.clone(),
        llm_client,
        chat_client,
    );
    
    // Give the background task some time to complete
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // Verify the channel directive was updated
    let channel = db_client.get_or_create_channel(channel_id).await.unwrap();
    assert_eq!(channel.channel_directive.your_notes, "This is a channel directive update");
    
    // Verify the context was updated
    let contexts = db_client.get_channel_context(channel_id).await.unwrap();
    assert!(!contexts.is_empty());
    let last_context = contexts.last().unwrap();
    assert_eq!(last_context.your_notes, "This is a context update");
}