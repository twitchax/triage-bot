use slack_morphism::events::SlackAppMentionEvent;
use tracing::{error, info, instrument};

use crate::base::types::Void;

#[instrument]
pub async fn handle_app_mention(event: SlackAppMentionEvent) -> Void {
    let result = handle_app_mention_internal(event).await;

    if let Err(err) = &result {
        error!("[HANDLE_APP_MENTION] Error while handling: {}", err);
    }

    result
}

#[instrument]
async fn handle_app_mention_internal(event: SlackAppMentionEvent) -> Void {
    let channel = event.channel;
    let user = event.user;
    let text = event.content.text.unwrap_or_default();

    info!("[HANDLE_APP_MENTION] `{}` => `{}`: `{}`.", user, channel, text);

    Ok(())
}