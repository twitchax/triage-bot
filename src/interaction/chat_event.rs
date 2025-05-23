//! This module handles the storage of messages in the database.

use serde::Serialize;
use tracing::{Instrument, error, info, instrument, warn};

use crate::{
    base::types::{AssistantClassification, AssistantContext, AssistantResponse, MessageSearchContext, Res, Void, WebSearchContext},
    service::{
        chat::ChatClient,
        db::{DbClient, LlmContext},
        llm::LlmClient,
    },
};

/// Handles the chat event.
///
/// This function is responsible for processing chat events and taking appropriate actions based on the responses from the LLM.
/// It spawns a new task to handle the event asynchronously.
/// It first retrieves the channel information and context from the database, then generates a response using the LLM,
/// and finally takes action based on the response.
#[instrument(skip_all)]
pub fn handle_chat_event<E>(event: E, channel_id: String, thread_ts: String, db: DbClient, llm: LlmClient, chat: ChatClient)
where
    E: Serialize + Send + 'static,
{
    tokio::spawn(async move {
        // Process the event.
        let result = handle_chat_event_internal(event, channel_id, thread_ts, &db, &llm, &chat).in_current_span().await;

        // Log any errors.
        if let Err(err) = &result {
            error!("Error while handling: {}", err);
        }
    });
}

/// Internal function to handle the chat event.
#[instrument(skip_all)]
async fn handle_chat_event_internal<E>(event: E, channel_id: String, thread_ts: String, db: &DbClient, llm: &LlmClient, chat: &ChatClient) -> Void
where
    E: Serialize,
{
    let user_message = serde_json::to_string(&event).unwrap();

    // First, get the channel info from the database.

    let channel = db.get_or_create_channel(&channel_id).await?;
    let channel_directive = serde_json::to_string(&channel.channel_directive)?;

    // Next, get the other context from the database.

    let channel_context = db.get_channel_context(&channel_id).await?;

    // TODO: Maybe we can also have context about specific users that we would also look up?

    // Get the thread context from the event.
    // TODO: Now that we store the messages in the database, we can also get the thread context from the database (probably better).
    let thread_context = chat.get_thread_context(&channel_id, &thread_ts).await?;

    // Compile all relevant context for the assistant agent.

    let assistant_context = compile_contexts(
        user_message.clone(),
        chat.bot_user_id().to_string(),
        channel_id.clone(),
        thread_ts.clone(),
        channel_directive.clone(),
        channel_context.clone(),
        thread_context.clone(),
        db,
        llm,
        chat,
    )
    .await?;

    // Call the assistant agent with all of the context.
    let responses = llm.get_assistant_agent_response(&assistant_context).await?;

    // Take the proper action based on the response.

    info!("Received {} responses from LLM", responses.len());

    for response in responses {
        match response {
            AssistantResponse::NoAction => warn!("No action taken."),
            AssistantResponse::UpdateChannelDirective { message } => {
                info!("Updating channel directive ...");

                let directive = LlmContext {
                    id: None,
                    user_message: serde_json::to_value(&event)?,
                    your_notes: message,
                };

                db.update_channel_directive(&channel_id, &directive).await?;
            }
            AssistantResponse::UpdateContext { message } => {
                info!("Updating context ...");

                let context = LlmContext {
                    id: None,
                    user_message: serde_json::to_value(&event)?,
                    your_notes: message,
                };

                db.add_channel_context(&channel_id, &context).await?;
            }
            AssistantResponse::ReplyToThread { thread_ts, classification, message } => {
                info!("Replying to thread ...");

                // Set the emoji.
                let emoji = match classification {
                    AssistantClassification::Question => "question",
                    AssistantClassification::Feature => "bulb",
                    AssistantClassification::Bug => "bug",
                    AssistantClassification::Incident => "warning",
                    AssistantClassification::Other => "grey_question",
                };

                let _ = chat.react_to_message(&channel_id, &thread_ts, emoji).await;
                chat.send_message(&channel_id, &thread_ts, &message).await?;
            }
        }
    }

    Ok(())
}

/// Kick off all of the "helper agents" to do their thing in parallel.
///
/// Builds a single context for the assistant agent to use.
#[instrument(skip_all)]
#[allow(clippy::too_many_arguments)]
async fn compile_contexts(
    user_message: String,
    bot_user_id: String,
    channel_id: String,
    thread_ts: String,
    channel_directive: String,
    channel_context: String,
    thread_context: String,
    db: &DbClient,
    llm: &LlmClient,
    _chat: &ChatClient,
) -> Res<AssistantContext> {
    let mut tasks = vec![];

    // Execute the search agent to gather relevant information.

    let llm_clone = llm.clone();
    let web_search_context = WebSearchContext {
        user_message: user_message.clone(),
        bot_user_id: bot_user_id.clone(),
        channel_id: channel_id.clone(),
        channel_context: channel_context.clone(),
        thread_context: thread_context.clone(),
    };

    let search_agent_task = tokio::spawn(async move { llm_clone.get_web_search_agent_response(&web_search_context).await });
    tasks.push(search_agent_task);

    // Execute the message search agent to identify relevant messages from the channel history.

    let llm_clone = llm.clone();
    let db_clone = db.clone();
    let channel_id_clone = channel_id.clone();
    let message_search_context = MessageSearchContext {
        user_message: user_message.clone(),
        bot_user_id: bot_user_id.clone(),
        channel_id: channel_id.clone(),
        channel_context: channel_context.clone(),
        thread_context: thread_context.clone(),
    };

    let message_search_task = tokio::spawn(async move {
        // Get search terms from the message search agent
        let search_terms = llm_clone.get_message_search_agent_response(&message_search_context).await?;
        
        // Search for relevant messages using the search terms
        let messages = if !search_terms.is_empty() {
            db_clone.search_messages(&channel_id_clone, &search_terms).await?
        } else {
            "No relevant messages found.".to_string()
        };
        
        Ok(messages)
    });
    tasks.push(message_search_task);

    // Wait for all tasks to complete.

    let results = futures::future::join_all(tasks)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    // Prepare results.

    let agent_responses = AssistantContext {
        user_message,
        bot_user_id,
        web_search_context: results[0].clone(),
        message_search_context: results[1].clone(),
        channel_id,
        thread_ts,
        channel_directive,
        channel_context,
        thread_context,
    };

    Ok(agent_responses)
}
