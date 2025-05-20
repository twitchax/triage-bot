use serde::Serialize;
use tracing::{error, info, instrument, warn, Instrument};

use crate::{
    base::types::{LlmClassification, Void},
    service::{chat::ChatClient, db::DbClient, llm::LlmClient},
};

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

#[instrument(skip_all)]
async fn handle_chat_event_internal<E>(event: E, channel_id: String, thread_ts: String, db: &DbClient, llm: &LlmClient, chat: &ChatClient) -> Void
where 
    E: Serialize
{
    // First, get the channel info from the database.

    let channel = db.get_or_create_channel(&channel_id).await?;
    let channel_prompt = &channel.channel_prompt;

    // TODO: Maybe we can also have context about specific users that we would also look up?

    // Get the thread context from the event.
    let thread_context = chat.get_thread_context(&channel_id, &thread_ts).await?;

    // Call the LLM with the channel prompt and the message text.

    let user_message = serde_json::to_string(&event).unwrap();
    let responses = llm.generate_response(chat.bot_user_id(), channel_prompt, &thread_context, &user_message).await?;

    // Take the proper action based on the response.

    info!("Received {} responses from LLM", responses.len());
    
    for response in responses.iter() {
        match response {
            crate::base::types::LlmResponse::NoAction => warn!("No action taken."),
            crate::base::types::LlmResponse::UpdateChannelDirective { message } => {
                info!("Updating channel directive ...");

                let message = format!("User message:\n\n{user_message}\n\nYour Notes:\n\n{message}");

                db.update_channel_prompt(&channel_id, &message).await?;
            },
            crate::base::types::LlmResponse::UpdateContext { message } => {
                info!("Updating context ...");

                let message = format!("User message:\n\n{user_message}\n\nYour Notes:\n\n{message}");
                
                // TODO: Update the context in the database.
                error!("Updating context is not yet implemented.");
            },
            crate::base::types::LlmResponse::ReplyToThread { thread_ts, classification, message } => {
                info!("Replying to thread ...");

                // Set the emoji.
                let emoji = match classification {
                    LlmClassification::Question => "question",
                    LlmClassification::Feature => "bulb",
                    LlmClassification::Bug => "bug",
                    LlmClassification::Incident => "warning",
                    LlmClassification::Other => "grey_question",
                };

                let _ = chat.react_to_message(&channel_id, thread_ts, emoji).await;
                chat.send_message(&channel_id, thread_ts, message).await?;
            },
        }
    }

    Ok(())
}