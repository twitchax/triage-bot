use slack_morphism::events::SlackMessageEvent;
use tracing::{error, info, instrument};

use crate::base::types::Void;

#[instrument]
pub async fn handle_message(event: SlackMessageEvent) -> Void {
    let result = handle_message_internal(event).await;

    if let Err(err) = &result {
        error!("[HANDLE_MESSAGE] Error while handling: {}", err);
    }

    result
}

#[instrument]
async fn handle_message_internal(event: SlackMessageEvent) -> Void {
    let channel = event.origin.channel;
    let user = event.sender.user;
    let text = event.content;

    info!("[HANDLE_MESSAGE] `{:?}` => `{:?}`: `{:?}`.", user, channel, text);

    Ok(())
}