use slack_morphism::events::SlackMessageEvent;
use tracing::{error, info, instrument};

use crate::base::types::Void;

#[instrument(skip_all)]
pub async fn handle_message(event: SlackMessageEvent) -> Void {
    let result = handle_message_internal(event).await;

    if let Err(err) = &result {
        error!("Error while handling: {}", err);
    }

    result
}

#[instrument(skip_all)]
async fn handle_message_internal(event: SlackMessageEvent) -> Void {
    let channel = event.origin.channel;
    let user = event.sender.user;
    let text = event.content;

    info!("`{:?}` => `{:?}`: `{:?}`.", user, channel, text);

    Ok(())
}