//! SurrealDB implementation for triage-bot data storage.

use std::ops::Deref;

use crate::base::{
    config::Config,
    types::{Res, Void},
};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use surrealdb::{engine::remote::ws::{Client, Ws, Wss}, opt::auth::Root, Surreal};
use surrealdb::engine::local::{Db, Mem};
use tracing::{debug, error, info, instrument};

/// Database client for triage-bot.
///
/// This is trivially cloneable and can be passed around without the need for `Arc` or `Mutex`.
#[derive(Clone)]
pub struct DbClient {
    /// The SurrealDB client instance.
    db: Surreal<Client>,
}

impl Deref for DbClient {
    type Target = Surreal<Client>;

    fn deref(&self) -> &Self::Target {
        &self.db
    }
}

// A Channel in the database.
#[derive(Debug, Serialize, Deserialize)]
pub struct Channel {
    pub id: Option<surrealdb::sql::Thing>,
    pub channel_prompt: String,
}

/// A message record in the database.
#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: Option<surrealdb::sql::Thing>,
    pub channel_id: String,
    pub user_id: String,
    pub text: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub is_processed: bool,
}

/// A user record in the database.
#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: Option<surrealdb::sql::Thing>,
    pub user_id: String,
    pub name: String,
    pub real_name: Option<String>,
    pub email: Option<String>,
}

impl DbClient {
    /// Create a new database client.
    ///
    /// This creates an in-memory database instance. For production, you would
    /// want to connect to a persistent database.
    #[instrument(skip_all)]
    pub async fn new(config: &Config) -> Res<Self> {
        // Create an in-memory database
        let db = Surreal::new::<Ws>(&config.db_endpoint).await?;

        // Authenticate with the database using the provided username and password.
        db.signin(Root {
            username: &config.db_username,
            password: &config.db_password
        }).await?;

        // Use a specific namespace and database
        db.use_ns("triage").use_db("bot").await?;

        // Define schemas.

        // Schema for list of channels that the bot has been "added to" (@-mentioned).
        db.query("DEFINE TABLE channel SCHEMAFULL").await?;
        db.query(
            "DEFINE FIELD channel_prompt ON channel TYPE string;",
        )
        .await?;

        info!("Database initialized successfully.");

        Ok(Self { db })
    }

    /// Gets the channel from the database by its ID; or, creates a new channel if it doesn't exist.
    #[instrument(skip(self))]
    pub async fn get_or_create_channel(&self, channel_id: &str) -> Res<Channel> {
        let channel: Option<Channel> = self.select(("channel", channel_id)).await?;

        if let Some(channel) = channel {
            info!("Channel `{}` found.", channel_id);

            Ok(channel)
        } else {
            info!("Channel `{}` not found, creating a new one.", channel_id);

            let new_channel = Channel {
                id: None,
                channel_prompt: "# Channel prompt\n\nChannel prompt has not been set yet.\n\n".to_string(),
            };

            let channel: Channel = self.create(("channel", channel_id)).content(new_channel).await?.unwrap();

            Ok(channel)
        }
    }
}