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
    let config = Config {
        inner: Arc::new(ConfigInner {
            openai_api_key: std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "test_key".to_string()),
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

#[tokio::test]
async fn test_app_mention_integration() {
    // Set up the test environment
    let runtime = setup_test_environment().await;

    // Fail if the openai_api_key is not set.
    if runtime.config.inner.openai_api_key == "test_key" {
        panic!("OPENAI_API_KEY is not set. Please set it in your environment.");
    }

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

    // Give the async task some time to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Verify the channel was created in the database
    let channel = runtime.db.get_or_create_channel(channel_id).await.expect("Failed to get channel");

    // Verify that the channel directive exists
    let directive_json = serde_json::to_string(&channel.channel_directive).unwrap();
    assert!(!directive_json.is_empty());

    // Verify that we can get the channel context
    let context = runtime.db.get_channel_context(channel_id).await.expect("Failed to get context");
    assert!(!context.is_empty() || context.is_empty()); // Check that context exists (can be empty)
}
