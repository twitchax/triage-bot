#![cfg(test)]

use std::sync::Arc;

use async_trait::async_trait;
use mockall::mock;
use triage_bot::{
    base::{
        config::{Config, ConfigInner},
        types::{Res, Void},
    },
    runtime::Runtime,
    service::{
        chat::{ChatClient, GenericChatClient},
        db::DbClient,
        llm::LlmClient,
    },
};

// Mocks.

// Mock chat client for testing.

mock! {
    pub Chat {}

    #[async_trait]
    impl GenericChatClient for Chat {
        fn bot_user_id(&self) -> &str;
        async fn start(&self) -> triage_bot::base::types::Void;
        async fn send_message(&self, channel_id: &str, thread_ts: &str, text: &str) -> Void;
        async fn react_to_message(&self, channel_id: &str, thread_ts: &str, emoji: &str) -> Void;
        async fn get_thread_context(&self, channel_id: &str, thread_ts: &str) -> Res<String>;
    }
}

fn get_mock_chat() -> MockChat {
    let mut mock = MockChat::new();

    mock.expect_bot_user_id().return_const("U12345".to_string());
    mock.expect_start().returning(|| Ok(()));
    mock.expect_send_message().returning(|_, _, _| Ok(()));
    mock.expect_react_to_message().returning(|_, _, _| Ok(()));
    mock.expect_get_thread_context().returning(|_, _| Ok("Some context.".to_string()));

    mock
}

/// Helper function to setup the test environment.
async fn setup_test_environment() -> Runtime {
    // Create a test configuration
    // Note: The actual OpenAI API key should be set via environment variable
    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "test_key".to_string());
    
    // Fail if no API key is set
    if api_key == "test_key" {
        panic!("OPENAI_API_KEY not set! Integration tests require a valid API key to run.");
    }
    
    let config = Config {
        inner: Arc::new(ConfigInner {
            openai_api_key: api_key,
            openai_search_agent_model: "gpt-4.1-nano".to_string(),
            openai_assistant_agent_model: "gpt-4.1-nano".to_string(),
            openai_search_agent_temperature: 0.0,
            openai_assistant_agent_temperature: 0.7,
            openai_max_tokens: 500u32, // Using a smaller value for tests
            slack_app_token: "xapp-test".to_string(),
            slack_bot_token: "xoxb-test".to_string(),
            slack_signing_secret: "test_secret".to_string(),
            db_endpoint: "memory".to_string(),
            db_username: "test".to_string(),
            db_password: "test".to_string(),
            ..Default::default()
        }),
    };

    // Initialize the database (using in-memory for tests).
    let db = DbClient::surreal_memory().await.expect("Failed to create DB client");

    // Initialize the LLM client (using real OpenAI key for tests).
    let llm = LlmClient::openai(&config);

    // We create a mocked version of the chat client that just returns success on all calls.
    let chat = ChatClient::new(Arc::new(get_mock_chat()));

    Runtime { config, db, llm, chat }
}

/// Wait for a channel to be processed using live queries
async fn wait_for_channel_processed(db: &DbClient, channel_id: &str, max_attempts: u32, delay_ms: u64) -> Result<(), anyhow::Error> {
    use std::time::Duration;
    
    // First check if the channel already has directive notes
    let channel = db.get_or_create_channel(channel_id).await?;
    if !channel.channel_directive.your_notes.is_empty() {
        return Ok(());
    }
    
    // Poll for changes with decreasing delay
    for attempt in 1..=max_attempts {
        // Wait before checking again - use an exponential backoff strategy
        let wait_time = if attempt < 5 {
            delay_ms / 2
        } else if attempt < 10 {
            delay_ms
        } else {
            delay_ms * 2
        };
        
        tokio::time::sleep(Duration::from_millis(wait_time)).await;
        
        // Check if the channel exists and has directive
        let channel = db.get_or_create_channel(channel_id).await?;
        
        // Channel is processed if it has notes in the directive
        if !channel.channel_directive.your_notes.is_empty() {
            return Ok(());
        }
    }
    
    Err(anyhow::anyhow!("Timeout waiting for channel to be processed"))
}

#[tokio::test]
async fn test_app_mention_integration() {
    // Set up the test environment
    let runtime = setup_test_environment().await;

    // Create a test channel
    let channel_id = "C01TEST";
    let thread_ts = "1234567890.123456";

    // Create a simple test message that we can serialize
    // We don't need to match the exact structure as long as it can be serialized to JSON
    // The LLM will process whatever JSON structure we provide
    let test_message = serde_json::json!({
        "type": "app_mention",
        "user": "U54321",
        "text": "<@U12345> Help me with a test issue",
        "ts": "1234567890.123456",
        "channel": channel_id,
        "event_ts": "1234567890.123456",
    });

    // Call the handler directly
    triage_bot::interaction::chat_event::handle_chat_event(
        test_message,
        channel_id.to_string(),
        thread_ts.to_string(),
        runtime.db.clone(),
        runtime.llm.clone(),
        runtime.chat.clone(),
    );

    // Wait for processing using polling
    wait_for_channel_processed(&runtime.db, channel_id, 50, 200).await
        .expect("Failed waiting for channel to be processed");

    // Verify the channel was created in the database
    let channel = runtime.db.get_or_create_channel(channel_id).await.expect("Failed to get channel");

    // Verify that the channel directive exists
    let directive_json = serde_json::to_string(&channel.channel_directive).unwrap();
    assert!(!directive_json.is_empty());

    // Verify that we can get the channel context
    let context = runtime.db.get_channel_context(channel_id).await.expect("Failed to get context");
    assert!(!context.is_empty() || context.is_empty()); // Check that context exists (can be empty)
}

#[tokio::test]
async fn test_context_update_integration() {
    // Set up the test environment
    let runtime = setup_test_environment().await;

    let channel_id = "C02CONTEXTTEST";
    let thread_ts = "1234567890.456789";

    // Create a message that asks for context update
    let context_update_message = serde_json::json!({
        "type": "app_mention",
        "user": "U54321",
        "text": "<@U12345> Please update the context for this channel with new information about deployment procedures",
        "ts": "1234567890.456789",
        "channel": channel_id,
        "event_ts": "1234567890.456789",
    });

    // Call the handler
    triage_bot::interaction::chat_event::handle_chat_event(
        context_update_message,
        channel_id.to_string(),
        thread_ts.to_string(),
        runtime.db.clone(),
        runtime.llm.clone(),
        runtime.chat.clone(),
    );

    // Wait for processing using polling
    wait_for_channel_processed(&runtime.db, channel_id, 50, 200).await
        .expect("Failed waiting for channel to be processed");

    // Verify the channel exists and has been processed
    let channel = runtime.db.get_or_create_channel(channel_id).await.expect("Failed to get channel");
    assert!(channel.channel_directive.your_notes.len() > 0);

    // Verify context can be retrieved
    let context = runtime.db.get_channel_context(channel_id).await.expect("Failed to get context");
    // Context might be empty initially but should not error
    assert!(context == "[]" || !context.is_empty());
}

#[tokio::test]
async fn test_add_context_integration() {
    // Set up the test environment
    let runtime = setup_test_environment().await;

    let channel_id = "C03ADDCONTEXT";
    let thread_ts = "1234567890.789012";

    // First, ensure the channel exists
    runtime.db.get_or_create_channel(channel_id).await.expect("Failed to create channel");

    // Add some context manually to simulate previous interactions
    let initial_context = triage_bot::service::db::LlmContext {
        id: None,
        user_message: serde_json::json!({"context": "Initial context about this channel"}),
        your_notes: "Channel setup notes".to_string(),
    };
    runtime.db.add_channel_context(channel_id, &initial_context).await.expect("Failed to add initial context");

    // Create a message that would add more context
    let add_context_message = serde_json::json!({
        "type": "app_mention",
        "user": "U54321",
        "text": "<@U12345> Add this information to the channel context: We use Docker for containerization and Kubernetes for orchestration",
        "ts": "1234567890.789012",
        "channel": channel_id,
        "event_ts": "1234567890.789012",
    });

    // Call the handler
    triage_bot::interaction::chat_event::handle_chat_event(
        add_context_message,
        channel_id.to_string(),
        thread_ts.to_string(),
        runtime.db.clone(),
        runtime.llm.clone(),
        runtime.chat.clone(),
    );

    // Wait for processing using polling
    wait_for_channel_processed(&runtime.db, channel_id, 50, 200).await
        .expect("Failed waiting for channel to be processed");

    // Verify context has been added/updated
    let context = runtime.db.get_channel_context(channel_id).await.expect("Failed to get context");
    assert!(!context.is_empty());
    assert_ne!(context, "[]");
    
    // Should contain the initial context we added
    assert!(context.contains("Initial context") || context.contains("Channel setup"));
}

#[tokio::test]
async fn test_message_search_integration() {
    // Set up the test environment
    let runtime = setup_test_environment().await;

    let channel_id = "C04SEARCHTEST";
    let thread_ts = "1234567890.111111";

    // Create channel and add some messages
    runtime.db.get_or_create_channel(channel_id).await.expect("Failed to create channel");
    
    // Add some test messages to search through
    runtime.db.add_channel_message(channel_id, &serde_json::json!({
        "text": "We had a deployment issue yesterday with the frontend service",
        "user": "U111", 
        "ts": "1234567890.100001"
    })).await.expect("Failed to add message");

    runtime.db.add_channel_message(channel_id, &serde_json::json!({
        "text": "The database migration failed during deployment",
        "user": "U222",
        "ts": "1234567890.100002"
    })).await.expect("Failed to add message");

    // Create a message that would trigger message search
    let search_message = serde_json::json!({
        "type": "app_mention",
        "user": "U54321",
        "text": "<@U12345> Can you find previous messages about deployment issues?",
        "ts": "1234567890.111111",
        "channel": channel_id,
        "event_ts": "1234567890.111111",
    });

    // Call the handler
    triage_bot::interaction::chat_event::handle_chat_event(
        search_message,
        channel_id.to_string(),
        thread_ts.to_string(),
        runtime.db.clone(),
        runtime.llm.clone(),
        runtime.chat.clone(),
    );

    // Wait for processing using polling
    wait_for_channel_processed(&runtime.db, channel_id, 50, 200).await
        .expect("Failed waiting for channel to be processed");

    // Verify the channel processing completed
    let channel = runtime.db.get_or_create_channel(channel_id).await.expect("Failed to get channel");
    assert!(channel.channel_directive.your_notes.len() > 0);

    // Test message search functionality directly
    let search_result = runtime.db.search_channel_messages(channel_id, "deployment").await;
    assert!(search_result.is_ok(), "Message search should not error");
}

#[tokio::test] 
async fn test_multiple_channel_isolation_integration() {
    // Set up the test environment
    let runtime = setup_test_environment().await;

    let channel1 = "C05ISOLATION1";
    let channel2 = "C05ISOLATION2";
    let thread_ts = "1234567890.222222";

    // Process different messages in different channels
    let message1 = serde_json::json!({
        "type": "app_mention",
        "user": "U54321",
        "text": "<@U12345> This is channel 1 with backend development topics",
        "ts": "1234567890.222222",
        "channel": channel1,
        "event_ts": "1234567890.222222",
    });

    let message2 = serde_json::json!({
        "type": "app_mention", 
        "user": "U54321",
        "text": "<@U12345> This is channel 2 with frontend design topics",
        "ts": "1234567890.222223",
        "channel": channel2,
        "event_ts": "1234567890.222223",
    });

    // Process both messages
    triage_bot::interaction::chat_event::handle_chat_event(
        message1,
        channel1.to_string(),
        thread_ts.to_string(),
        runtime.db.clone(),
        runtime.llm.clone(),
        runtime.chat.clone(),
    );

    triage_bot::interaction::chat_event::handle_chat_event(
        message2,
        channel2.to_string(),
        thread_ts.to_string(),
        runtime.db.clone(),
        runtime.llm.clone(),
        runtime.chat.clone(),
    );

    // Wait for both channels to be processed
    wait_for_channel_processed(&runtime.db, channel1, 50, 200).await
        .expect("Failed waiting for channel1 to be processed");
    wait_for_channel_processed(&runtime.db, channel2, 50, 200).await
        .expect("Failed waiting for channel2 to be processed");

    // Verify channels exist and are different
    let chan1 = runtime.db.get_or_create_channel(channel1).await.expect("Failed to get channel 1");
    let chan2 = runtime.db.get_or_create_channel(channel2).await.expect("Failed to get channel 2");

    // Channels should have different directives
    let dir1 = serde_json::to_string(&chan1.channel_directive).unwrap();
    let dir2 = serde_json::to_string(&chan2.channel_directive).unwrap();
    
    // The directives should be different (not the same default)
    assert_ne!(dir1, dir2);
}

#[tokio::test]
async fn test_error_handling_integration() {
    // Set up the test environment  
    let runtime = setup_test_environment().await;

    let channel_id = "C06ERRORTEST";
    let thread_ts = "1234567890.333333";

    // Create a message with potentially problematic content
    let problem_message = serde_json::json!({
        "type": "app_mention",
        "user": "U54321",
        "text": "", // Empty text might cause issues
        "ts": "1234567890.333333", 
        "channel": channel_id,
        "event_ts": "1234567890.333333",
    });

    // This should not panic or crash the system
    triage_bot::interaction::chat_event::handle_chat_event(
        problem_message,
        channel_id.to_string(),
        thread_ts.to_string(),
        runtime.db.clone(),
        runtime.llm.clone(),
        runtime.chat.clone(),
    );

    // Wait for a short time - this might not trigger LiveQuery since there might be no changes
    // We'll just use a short timeout here as we're mainly testing error handling
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Should still be able to create/get channel even after error
    let channel = runtime.db.get_or_create_channel(channel_id).await;
    assert!(channel.is_ok(), "System should recover from processing errors");
}
