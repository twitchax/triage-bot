use slack_morphism::events::SlackMessageEvent;
use tracing::{Instrument, error, instrument};

use crate::{
    base::types::Void,
    service::{db::DbClient, llm::LlmClient},
};

#[instrument(skip_all)]
pub fn handle_message(event: SlackMessageEvent, db: DbClient, llm: LlmClient) {
    tokio::spawn(async move {
        // Process the event.
        let result = handle_message_internal(event, &db, &llm).in_current_span().await;

        // Log any errors.
        if let Err(err) = &result {
            error!("Error while handling: {}", err);
        }
    });
}

#[instrument(skip_all)]
async fn handle_message_internal(event: SlackMessageEvent, db: &DbClient, llm: &LlmClient) -> Void {
    dbg!(event);

    // TODO: Remember to add a response mode where the bot says "go get more info from `this thread` or `this channel search` or `public internet`, etc."
    // Maybe thread information should be automatically queried (this would only be relevant for an app-mention...message is only called for top-level messages)?

    Ok(())
}
