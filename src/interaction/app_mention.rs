use slack_morphism::events::SlackAppMentionEvent;
use tracing::{error, instrument, warn, Instrument};

use crate::{base::types::Void, service::{db::DbClient, llm::LlmClient}};

#[instrument(skip_all)]
pub fn handle_app_mention(event: SlackAppMentionEvent, db: DbClient, llm: LlmClient) {
    tokio::spawn(async move {
        // Process the event.
        let result = handle_app_mention_internal(event, &db, &llm).in_current_span().await;

        // Log any errors.
        if let Err(err) = &result {
            error!("Error while handling: {}", err);
        }
    });
}

#[instrument(skip_all)]
async fn handle_app_mention_internal(event: SlackAppMentionEvent, db: &DbClient, llm: &LlmClient) -> Void {
    let channel_id = &event.channel.0;

    // First, get the channel info from the database.

    let channel = db.get_or_create_channel(channel_id).await?;
    let channel_promopt = &channel.channel_prompt;

    // TODO: Maybe we can also have context about specific users that we would also look up?

    // Call the LLM with the channel prompt and the message text.

    let full_message = serde_json::to_string(&event).unwrap();
    let response = llm.generate_response(channel_promopt, &full_message).await?;

    dbg!(response);

    Ok(())
}
