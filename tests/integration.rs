use std::sync::Arc;
use slack_morphism::prelude::*;
use triage_bot::{
    base::config::{Config, ConfigInner},
    interaction::chat_event::handle_chat_event_internal,
    service::{
        chat::ChatClient,
        db::{DbClient, LlmContext},
        llm::LlmClient,
    },
    base::types::LlmResponse,
};
use serde_json::json;
use async_trait::async_trait;

/// Create a test configuration
/// If OPENAI_API_KEY is set, it will use that, otherwise uses a dummy key
async fn setup_test_config() -> Config {
    // Try to get OpenAI API key from environment
    let openai_api_key = std::env::var("OPENAI_API_KEY")
        .unwrap_or_else(|_| "dummy_key_for_testing".to_string());

    // Create minimal config for testing
    let config_inner = ConfigInner {
        openai_api_key,
        openai_model: "o3-mini".to_string(),   // Use smaller model for testing
        system_prompt: None,
        mention_addendum_prompt: None,
        openai_temperature: 0.7,
        openai_max_tokens: 512,                // Smaller response for testing
        slack_app_token: "xapp-test".to_string(),
        slack_bot_token: "xoxb-test".to_string(),
        slack_signing_secret: "secret".to_string(),
        db_endpoint: "memory".to_string(),     // Not used in test mode
        db_username: "test".to_string(),       // Not used in test mode
        db_password: "test".to_string(),       // Not used in test mode
    };

    Config {
        inner: Arc::new(config_inner),
    }
}

// Create a test version of the LlmClient for tests where we don't have an OpenAI API key
#[derive(Clone)]
struct TestLlmClient {}

#[async_trait]
impl triage_bot::service::llm::GenericLlmClient for TestLlmClient {
    async fn generate_response(
        &self, 
        _self_id: &str, 
        _channel_prompt: &str, 
        _channel_context: &str, 
        _thread_context: &str, 
        _user_message: &str
    ) -> Result<Vec<LlmResponse>, anyhow::Error> {
        // Return a predefined response for testing purposes
        Ok(vec![
            LlmResponse::UpdateChannelDirective { 
                message: "Test channel directive".to_string() 
            },
            LlmResponse::UpdateContext { 
                message: "Test context note".to_string() 
            },
            LlmResponse::ReplyToThread { 
                thread_ts: "1234567890.123456".to_string(),
                classification: triage_bot::base::types::LlmClassification::Question,
                message: "This is a test response to the thread.".to_string()
            },
        ])
    }
}

// Test basic database operations
#[tokio::test]
async fn test_db_integration() {
    // Create a test configuration 
    let config = setup_test_config().await;
    
    // Initialize the DB service (it will use in-memory mode due to #[cfg(test)])
    let db = DbClient::surreal(&config).await.expect("Failed to create DB client");
    
    // Test basic DB operations
    // First, get or create a channel
    let channel_id = "C_TEST_CHANNEL";
    let channel = db.get_or_create_channel(channel_id).await
        .expect("Failed to create channel");
    
    // Verify default channel directive
    assert!(channel.channel_directive.your_notes.contains("Channel directive has not been set yet"));
    
    // Update the channel directive
    db.update_channel_directive(
        channel_id,
        &LlmContext {
            id: None,
            user_message: json!({"test": "message"}),
            your_notes: "Test directive".to_string(),
        }
    ).await.expect("Failed to update channel directive");
    
    // Verify the update
    let updated_channel = db.get_or_create_channel(channel_id).await
        .expect("Failed to get updated channel");
    
    assert_eq!(updated_channel.channel_directive.your_notes, "Test directive");
    
    // Add context to the channel
    db.add_channel_context(
        channel_id, 
        &LlmContext {
            id: None,
            user_message: json!({"context": "test"}),
            your_notes: "Test context".to_string(),
        }
    ).await.expect("Failed to add channel context");
    
    // Verify everything works together, which confirms DB integration is working
    println!("Successfully verified DB operations in the integration test");
}

// Test for the full integration using a mock LLM client
#[tokio::test]
async fn test_chat_event_integration() {
    // Set up test configuration
    let config = setup_test_config().await;
    
    // Create DB client
    let db = DbClient::surreal(&config).await.expect("Failed to create DB client");
    
    // Create a test LlmClient that doesn't require an API key
    let llm = LlmClient { 
        inner: Arc::new(TestLlmClient {}) 
    };
    
    // Create test ChatClient
    let chat = ChatClient::test(&config, db.clone(), llm.clone()).await
        .expect("Failed to create test Chat client");

    // Create a test app mention event
    let app_mention = SlackAppMentionEvent {
        channel: SlackChannelId("C_TEST_CHANNEL".to_string()),
        user: SlackUserId("U_TEST_USER".to_string()),
        text: "<@U_TEST_BOT> Help me with a test question?".to_string(),
        ts: SlackTs("1234567890.123456".to_string()),
        origin: SlackEventOriginInfo {
            client_msg_id: None,
            channel_type: Some("channel".to_string()),
            thread_ts: None,  // This is not a thread reply
            event_ts: SlackTs("1234567890.123456".to_string()),
            team: Some(SlackTeamId("T_TEST_TEAM".to_string())),
            user: Some(SlackUserId("U_TEST_USER".to_string())),
            channel: Some(SlackChannelId("C_TEST_CHANNEL".to_string())),
        },
    };

    // Call the handler directly with our test event
    let channel_id = app_mention.channel.0.clone();
    let thread_ts = app_mention.ts.0.clone();
    let result = handle_chat_event_internal(
        app_mention,
        channel_id.clone(),
        thread_ts.clone(),
        &db,
        &llm,
        &chat,
    ).await;

    // Verify the result is Ok
    assert!(result.is_ok(), "Handler returned an error: {:?}", result.err());

    // Verify the channel was created and updated in the database
    let channel = db.get_or_create_channel(&channel_id).await
        .expect("Failed to get channel from database");
    
    // The test LLM client should have updated this with "Test channel directive"
    assert_eq!(channel.channel_directive.your_notes, "Test channel directive");
    
    println!("Successfully verified chat event handling in the integration test");
}

// Only run this test if OPENAI_API_KEY is available
#[tokio::test]
async fn test_openai_integration() {
    // Skip this test if there's no API key
    if std::env::var("OPENAI_API_KEY").is_err() {
        println!("Skipping OpenAI integration test as OPENAI_API_KEY is not set");
        return;
    }
    
    // Set up test configuration with real OpenAI API key
    let config = setup_test_config().await;
    
    // Initialize services
    let db = DbClient::surreal(&config).await.expect("Failed to create DB client");
    let llm = LlmClient::openai(&config);
    let chat = ChatClient::test(&config, db.clone(), llm.clone()).await
        .expect("Failed to create test Chat client");

    // Create a test message event
    let message_event = SlackMessageEvent {
        channel: Some(SlackChannelId("C_TEST_CHANNEL".to_string())),
        user: Some(SlackUserId("U_TEST_USER".to_string())),
        text: Some("I'm having an issue with the API connection timing out".to_string()),
        origin: SlackEventOriginInfo {
            client_msg_id: Some("test_msg_id".to_string()),
            channel_type: Some("channel".to_string()),
            thread_ts: None,
            event_ts: SlackTs("1234567890.123456".to_string()),
            team: Some(SlackTeamId("T_TEST_TEAM".to_string())),
            user: Some(SlackUserId("U_TEST_USER".to_string())),
            channel: Some(SlackChannelId("C_TEST_CHANNEL".to_string())),
        },
    };

    // Channel and thread for the test
    let channel_id = message_event.channel.as_ref().unwrap().0.clone();
    let thread_ts = message_event.origin.ts.0.clone();

    // Call the handler directly with our test event
    let result = handle_chat_event_internal(
        message_event,
        channel_id.clone(),
        thread_ts.clone(),
        &db,
        &llm,
        &chat,
    ).await;

    // Verify the result is Ok
    assert!(result.is_ok(), "Handler returned an error with real OpenAI: {:?}", result.err());

    // Verify the channel was created in the database
    let channel = db.get_or_create_channel(&channel_id).await
        .expect("Failed to get channel from database");

    // Verify we got a meaningful response from OpenAI (should have content)
    assert!(!channel.channel_directive.your_notes.is_empty(), 
            "OpenAI didn't return a proper response");
    assert!(channel.channel_directive.your_notes != "Channel directive has not been set yet", 
            "OpenAI didn't update the channel directive");
    
    println!("OpenAI test completed successfully with directive: {}", 
             channel.channel_directive.your_notes);
}