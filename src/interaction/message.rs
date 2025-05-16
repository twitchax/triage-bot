use slack_morphism::events::SlackMessageEvent;
use tracing::{error, info, instrument};

use crate::{base::types::Void, service::{db::DbClient, llm::LlmClient}};

#[instrument(skip_all)]
pub async fn handle_message(event: SlackMessageEvent, db: &DbClient, llm: &LlmClient) -> Void {
    let result = handle_message_internal(event, db, llm).await;

    if let Err(err) = &result {
        error!("Error while handling: {}", err);
    }

    result
}

#[instrument(skip_all)]
async fn handle_message_internal(event: SlackMessageEvent, db: &DbClient, llm: &LlmClient) -> Void {
    let channel = event.origin.channel;
    let user = event.sender.user;
    let text = event.content;

    info!("`{:?}` => `{:?}`: `{:?}`.", user, channel, text);

    Ok(())
}
