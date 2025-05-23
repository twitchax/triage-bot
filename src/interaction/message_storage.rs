//! This module handles the storage of messages in the database.

use serde::Serialize;
use tracing::{Instrument, error, instrument};

use crate::{base::types::Void, service::db::DbClient};

/// Handles the message storage event.
///
/// This function is responsible for processing message storage events and storing them in the database.
/// It spawns a new task to handle the event asynchronously.
#[instrument(skip_all)]
pub fn handle_message_storage<E>(event: E, channel_id: String, db: DbClient)
where
    E: Serialize + Send + 'static,
{
    tokio::spawn(async move {
        // Process the event.
        let result = handle_message_storage_internal(event, channel_id, &db).in_current_span().await;

        // Log any errors.
        if let Err(err) = &result {
            error!("Error while handling: {}", err);
        }
    });
}

/// Internal function to handle the message storage event.
#[instrument(skip_all)]
async fn handle_message_storage_internal<E>(event: E, channel_id: String, db: &DbClient) -> Void
where
    E: Serialize,
{
    let message = serde_json::to_value(&event).unwrap();
    let _ = db.get_or_create_channel(&channel_id).await?;

    db.add_channel_message(&channel_id, &message).await?;

    Ok(())
}
