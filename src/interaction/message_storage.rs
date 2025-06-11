//! This module handles the storage of messages in the database.

use serde::Serialize;
use tracing::{Instrument, Span, error, instrument};

use crate::{
    base::types::Void,
    service::db::{Channel, DbClient, LlmContext, Message},
};

/// Handles the message storage event.
///
/// This function is responsible for processing message storage events and storing them in the database.
/// It spawns a new task to handle the event asynchronously.
#[instrument(skip_all)]
pub fn handle_message_storage<E, L, C, M>(event: E, channel_id: String, db: DbClient<L, C, M>)
where
    E: Serialize + Send + 'static,
    L: LlmContext,
    C: Channel,
    M: Message,
{
    tokio::spawn(
        async move {
            // Process the event.
            let result = handle_message_storage_internal(event, channel_id, &db).in_current_span().await;

            // Log any errors.
            if let Err(err) = &result {
                error!("Error while handling: {}\n\n{}", err, err.backtrace());
            }
        }
        .instrument(Span::current()),
    );
}

/// Internal function to handle the message storage event.
#[instrument(skip_all)]
async fn handle_message_storage_internal<E, L, C, M>(event: E, channel_id: String, db: &DbClient<L, C, M>) -> Void
where
    E: Serialize,
    L: LlmContext,
    C: Channel,
    M: Message,
{
    let message = serde_json::to_value(&event).unwrap();
    let _ = db.get_or_create_channel(&channel_id).await?;

    db.add_channel_message(&channel_id, &message).await?;

    Ok(())
}
