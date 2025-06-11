#![cfg(test)]

use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use mockall::mock;
use serde_json::json;
use surrealdb::{Action, Surreal, engine::local::Mem};
use tracing::Level;
use tracing_subscriber::fmt::format::FmtSpan;
use triage_bot::{
    base::{
        config::Config,
        types::{Res, Void},
    },
    runtime::Runtime,
    service::{
        chat::{ChatClient, GenericChatClient},
        db::{DbClient, surreal::SurrealDbClient},
        llm::LlmClient,
        mcp::McpClient,
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

async fn setup_test_db() -> Res<DbClient> {
    let surreal = Surreal::new::<Mem>(()).await?;
    let db = SurrealDbClient::from(surreal).await?;
    let client = DbClient { inner: Arc::new(db) };

    Ok(client)
}

/// Helper function to setup the test environment.
async fn setup_test_environment() -> Runtime {
    // Occasionally, we want to see debug logs in tests.
    tracing_subscriber::fmt()
        .without_time()
        .with_ansi(true)
        .with_level(true)
        .with_file(false)
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .with_max_level(Level::INFO)
        .init();

    // Create a test configuration
    // Note: The actual OpenAI API key should be set via environment variable
    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set! Integration tests require a valid API key to run.");

    let config_json = json!({
        "openai_api_key": api_key,
        "openai_search_agent_model": "gpt-4.1-mini",
        "openai_assistant_agent_model": "gpt-4.1-mini",
        "openai_search_agent_temperature": 0.1,
        "openai_assistant_agent_temperature": 0.1,
        "openai_max_tokens": 500,
        "slack_app_token": "xapp-test",
        "slack_bot_token": "xoxb-test",
        "slack_signing_secret": "test_secret",
        "db_endpoint": "memory",
        "db_username": "test",
        "db_password": "test",
        "mcp_config_path": "tests/mcp.json",
    });

    let config = Config {
        inner: Arc::new(serde_json::from_value(config_json).unwrap()),
    };

    // Initialize the database (using in-memory for tests).
    let db = setup_test_db().await.unwrap();

    // Initialize the LLM client (using real OpenAI key for tests).
    let llm = LlmClient::openai(&config);

    // We create a mocked version of the chat client that just returns success on all calls.
    let chat = ChatClient::new(Arc::new(get_mock_chat()));

    // Create an MCP client from the test version.
    let mcp = McpClient::new(&config.mcp_config_path).await.expect("Failed to create MCP client");

    Runtime { config, db, llm, chat, mcp }
}

#[tokio::test]
async fn test_app_mention_integration() {
    // Set up the test environment
    let mut runtime = setup_test_environment().await;

    // Create a test channel
    let channel_id = "C01TEST";
    let thread_ts = "1234567890.123456";

    // Create a simple test message that we can serialize
    // We don't need to match the exact structure as long as it can be serialized to JSON
    // The LLM will process whatever JSON structure we provide
    let test_message = json!({
        "type": "app_mention",
        "user": "U54321",
        "text": "<@U12345> Help me with a test issue",
        "ts": "1234567890.123456",
        "channel": channel_id,
        "event_ts": "1234567890.123456",
    });

    // Create an mpsc channel to get notification on when a message is sent.
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);

    // Start a live query to ensure the channel is processed.
    let mut live_query = runtime.db.get_channel_live_query().await.expect("Failed to start live query");

    // Override the chat mock to expect a message send.
    let mut chat_mock = MockChat::new();
    chat_mock.expect_bot_user_id().return_const("U12345".to_string());
    chat_mock.expect_get_thread_context().returning(move |_, _| Ok("Test context".to_string()));
    chat_mock.expect_react_to_message().returning(move |_, _, _| Ok(()));
    chat_mock.expect_send_message().withf(move |c, t, _| c == channel_id && t == thread_ts).returning(move |_, _, m| {
        let m = m.to_string();
        let tx = tx.clone();
        tokio::spawn(async move {
            // Simulate sending a message by sending it through the mpsc channel
            tx.send(m).await.expect("Failed to send message");
        });

        Ok(())
    });
    runtime.chat = ChatClient::new(Arc::new(chat_mock));

    // Call the handler directly
    triage_bot::interaction::chat_event::handle_chat_event(
        test_message,
        channel_id.to_string(),
        thread_ts.to_string(),
        runtime.db.clone(),
        runtime.llm.clone(),
        runtime.chat.clone(),
        runtime.mcp.clone(),
    );

    // First, we should detect the channel creation.
    let event = live_query.next().await.expect("Failed to get live query event").unwrap();
    assert_eq!(event.action, Action::Create, "Expected channel creation event");
    assert_eq!(event.data.id.unwrap().key().to_string(), channel_id.to_string(), "Expected event for 'channel' table");

    // Next, we should see if we get a message sent.
    let sent_message = rx.recv().await.expect("Failed to receive message");
    assert!(sent_message.len() > 10, "Expected sent message");
}

#[tokio::test]
async fn test_directive_update_integration() {
    // Set up the test environment
    let runtime = setup_test_environment().await;

    let channel_id = "C02CONTEXTTEST";
    let thread_ts = "1234567890.456789";

    // Create a message that asks for context update.
    let message = "<@U12345> Please update the directive for this channel: talk like a pirate, and @horse-oncall is the horse expert.";
    let context_update_message = serde_json::json!({
        "type": "app_mention",
        "user": "U54321",
        "text": message,
        "ts": "1234567890.456789",
        "channel": channel_id,
        "event_ts": "1234567890.456789",
    });

    // Start a live query to ensure the channel is processed.
    let mut live_query = runtime.db.get_channel_live_query().await.expect("Failed to start live query");

    // Call the handler
    triage_bot::interaction::chat_event::handle_chat_event(
        context_update_message,
        channel_id.to_string(),
        thread_ts.to_string(),
        runtime.db.clone(),
        runtime.llm.clone(),
        runtime.chat.clone(),
        runtime.mcp.clone(),
    );

    // First, we should detect the channel creation.
    let event = live_query.next().await.expect("Failed to get live query event").unwrap();
    assert_eq!(event.action, Action::Create, "Expected channel creation event");
    assert_eq!(event.data.id.unwrap().key().to_string(), channel_id.to_string(), "Expected event for 'channel' table");

    // Second, we should detect the context update.
    let event = live_query.next().await.expect("Failed to get context update event").unwrap();
    assert_eq!(event.action, Action::Update, "Expected context update event");
    assert_eq!(event.data.id.unwrap().key().to_string(), channel_id.to_string(), "Expected event for 'channel' table");
    assert_eq!(
        event.data.channel_directive.user_message.as_object().unwrap().get("text").unwrap(),
        message,
        "Expected context to be updated"
    );
}

#[tokio::test]
async fn test_add_context_integration() {
    // Set up the test environment
    let runtime = setup_test_environment().await;

    let channel_id = "C03ADDCONTEXT";
    let thread_ts = "1234567890.789012";

    // Create a message that would add more context
    let message = "<@U12345> Please remember in your context that @oswald-chesterfield is the expert on penguins.  Do not update the directive, just add this to the context.";
    let add_context_message = serde_json::json!({
        "type": "app_mention",
        "user": "U54321",
        "text": message,
        "ts": "1234567890.789012",
        "channel": channel_id,
        "event_ts": "1234567890.789012",
    });

    // Start a live query to ensure the context is processed.
    let mut live_query = runtime.db.get_context_live_query().await.expect("Failed to start live query");

    // Call the handler
    triage_bot::interaction::chat_event::handle_chat_event(
        add_context_message,
        channel_id.to_string(),
        thread_ts.to_string(),
        runtime.db.clone(),
        runtime.llm.clone(),
        runtime.chat.clone(),
        runtime.mcp.clone(),
    );

    // We should detect the context creation.
    let event = live_query.next().await.expect("Failed to get live query event").unwrap();
    assert_eq!(event.action, Action::Create, "Expected context creation event");
    assert_eq!(event.data.user_message.as_object().unwrap().get("text").unwrap(), message, "Expected context to be updated");
}

#[tokio::test]
async fn test_message_search_integration() {
    // Set up the test environment
    let mut runtime = setup_test_environment().await;

    let channel_id = "C04SEARCHTEST";
    let thread_ts = "1234567890.111111";

    // Create channel and add some messages
    runtime.db.get_or_create_channel(channel_id).await.expect("Failed to create channel");

    // Add some test messages to search through
    runtime
        .db
        .add_channel_message(
            channel_id,
            &serde_json::json!({
                "text": "@pamela-lillian-isley is a poison ivy expert, and should be tagged when needed!",
                "user": "@pamela-lillian-isley",
                "ts": "1234567890.100001"
            }),
        )
        .await
        .expect("Failed to add message");

    runtime
        .db
        .add_channel_message(
            channel_id,
            &serde_json::json!({
                "text": "Thanks, @horse-oncall, for helping me shoe Bob Thee Stallion!",
                "user": "U222",
                "ts": "1234567890.100002"
            }),
        )
        .await
        .expect("Failed to add message");

    // Create an mpsc channel to get notification on when a message is sent.
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);

    // Override the chat mock to expect a message send.
    let mut chat_mock = MockChat::new();
    chat_mock.expect_bot_user_id().return_const("U12345".to_string());
    chat_mock.expect_get_thread_context().returning(move |_, _| Ok("Test context".to_string()));
    chat_mock.expect_react_to_message().returning(move |_, _, _| Ok(()));
    chat_mock.expect_send_message().withf(move |c, t, _| c == channel_id && t == thread_ts).returning(move |_, _, m| {
        let m = m.to_string();
        let tx = tx.clone();
        tokio::spawn(async move {
            // Simulate sending a message by sending it through the mpsc channel
            tx.send(m).await.expect("Failed to send message");
        });

        Ok(())
    });
    runtime.chat = ChatClient::new(Arc::new(chat_mock));

    // Create a message that would trigger message search
    let search_message = serde_json::json!({
        "type": "app_mention",
        "user": "U54321",
        "text": "Looks like Tommy the Duck is having issues with a poison ivy itch.  Can anyone help?",
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
        runtime.mcp.clone(),
    );

    // Next, we should see if we get a message sent.
    let sent_message = rx.recv().await.expect("Failed to receive message");
    assert!(sent_message.len() > 10, "Expected sent message");
    //assert!(sent_message.contains("@pamela-lillian-isley"), "Expected message tagging in @pamela-lillian-isley");
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
        "text": "<@U12345> I need help with sunflowers!",
        "ts": "1234567890.222222",
        "channel": channel1,
        "event_ts": "1234567890.222222",
    });

    let message2 = serde_json::json!({
        "type": "app_mention",
        "user": "U54321",
        "text": "<@U12345> I need help with roses!",
        "ts": "1234567890.222223",
        "channel": channel2,
        "event_ts": "1234567890.222223",
    });

    // Start a live query to ensure the channel is processed.
    let mut live_query = runtime.db.get_channel_live_query().await.expect("Failed to start live query");

    // Process both messages
    triage_bot::interaction::chat_event::handle_chat_event(
        message1,
        channel1.to_string(),
        thread_ts.to_string(),
        runtime.db.clone(),
        runtime.llm.clone(),
        runtime.chat.clone(),
        runtime.mcp.clone(),
    );
    triage_bot::interaction::chat_event::handle_chat_event(
        message2,
        channel2.to_string(),
        thread_ts.to_string(),
        runtime.db.clone(),
        runtime.llm.clone(),
        runtime.chat.clone(),
        runtime.mcp.clone(),
    );

    // Get the event for both channels.
    let event1 = live_query.next().await.expect("Failed to get live query event").unwrap();
    let event2 = live_query.next().await.expect("Failed to get live query event").unwrap();

    // Check that both channels were created and isolated.
    assert_eq!(event1.action, Action::Create, "Expected channel creation event for channel 1");
    assert_eq!(event1.data.id.unwrap().key().to_string(), channel1.to_string(), "Expected event for 'channel' table for channel 1");
    assert_eq!(event2.action, Action::Create, "Expected channel creation event for channel 2");
    assert_eq!(event2.data.id.unwrap().key().to_string(), channel2.to_string(), "Expected event for 'channel' table for channel 2");
}

#[tokio::test]
async fn test_mcp_access() {
    // Set up the test environment
    let mut runtime = setup_test_environment().await;

    // Create a test channel
    let channel_id = "C06MCPTEST";
    let thread_ts = "1234567890.333333";

    // Create an mpsc channel to get notification on when a message is sent.
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);

    // Override the chat mock to expect a message send.
    let mut chat_mock = MockChat::new();
    chat_mock.expect_bot_user_id().return_const("U12345".to_string());
    chat_mock.expect_get_thread_context().returning(move |_, _| Ok("Test context".to_string()));
    chat_mock.expect_react_to_message().returning(move |_, _, _| Ok(()));
    chat_mock.expect_send_message().withf(move |c, t, _| c == channel_id && t == thread_ts).returning(move |_, _, m| {
        let m = m.to_string();
        let tx = tx.clone();
        tokio::spawn(async move {
            // Simulate sending a message by sending it through the mpsc channel
            tx.send(m).await.expect("Failed to send message");
        });

        Ok(())
    });
    runtime.chat = ChatClient::new(Arc::new(chat_mock));

    // Create a message that would trigger MCP access
    let mcp_message = serde_json::json!({
        "type": "app_mention",
        "user": "U54321",
        "text": "<@U12345> Use the `everything__add` tool to add 5 and 6.",
        "ts": "1234567890.333333",
        "channel": channel_id,
        "event_ts": "1234567890.333333",
    });

    // Call the handler
    triage_bot::interaction::chat_event::handle_chat_event(
        mcp_message,
        channel_id.to_string(),
        thread_ts.to_string(),
        runtime.db.clone(),
        runtime.llm.clone(),
        runtime.chat.clone(),
        runtime.mcp.clone(),
    );

    // Next, we should see if we get a message sent.
    let sent_message = rx.recv().await.expect("Failed to receive message");
    assert!(sent_message.len() > 10, "Expected sent message");
}
