//! Runtime services and shared state for the triage-bot.

use tracing::instrument;

use crate::{base::types::{Res, Void}, service::slack::SlackClient};
use crate::base::config::Config;
use crate::service::db::DbClient;
use std::sync::Arc;

/// Runtime service context that can be shared across the application.
/// 
/// This struct holds the database client, slack client, and configuration.
/// It is designed to be trivially cloneable, allowing it to be passed around
/// without the need for `Arc` or `Mutex`.
#[derive(Clone)]
pub struct Runtime {
    /// The database client instance.
    pub db: DbClient,
    /// The slack client instance.
    pub slack: SlackClient,
    /// The configuration for the application.
    pub config: Arc<Config>,
}

impl Runtime {
    /// Create a new runtime instance.
    #[instrument(skip_all)]
    pub async fn new(config: Config) -> Res<Self> {
        // Create an Arc-wrapped config for shared access.
        let config = Arc::new(config);

        // Initialize the database.
        let db = DbClient::new(&config).await?;

        // Initialize the slack client
        let slack = SlackClient::new(&config, db.clone()).await?;
        
        Ok(Self {
            db,
            slack,
            config,
        })
    }

    pub async fn start(&self) -> Void {
        self.slack.start().await
    }
}
