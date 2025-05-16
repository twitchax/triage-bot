use slack_morphism::events::SlackAppMentionEvent;
use tracing::{error, info, instrument};

use crate::{base::types::Void, service::{db::DbClient, llm::LlmClient}};

#[instrument(skip_all)]
pub async fn handle_app_mention(event: SlackAppMentionEvent, db: &DbClient, llm: &LlmClient) -> Void {
    let result = handle_app_mention_internal(event, db, llm).await;

    if let Err(err) = &result {
        error!("Error while handling: {}", err);
    }

    result
}

#[instrument(skip_all)]
async fn handle_app_mention_internal(event: SlackAppMentionEvent, db: &DbClient, llm: &LlmClient) -> Void {
    // First, see if we have channel info in the database.

    Ok(())
}
