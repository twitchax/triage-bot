//! Database implementation for triage-bot data storage.

use std::{ops::Deref, sync::Arc};

use crate::base::{
    config::Config,
    types::{Res, Void},
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use surrealdb::{
    Surreal,
    engine::remote::ws::{Client, Ws},
    opt::auth::Root,
};
use tracing::{info, instrument};

// Traits.

/// Generic database client trait that clients must implement.
#[async_trait]
pub trait GenericDbClient {
    /// Gets the channel from the database by its ID; or, creates a new channel if it doesn't exist.
    async fn get_or_create_channel(&self, channel_id: &str) -> Res<Channel>;
    /// Updates the channel prompt in the database.
    async fn update_channel_prompt(&self, channel_id: &str, prompt: &str) -> Res<()>;
}

/// Database client for triage-bot.
///
/// This is trivially cloneable and can be passed around without the need for `Arc` or `Mutex`.
#[derive(Clone)]
pub struct DbClient {
    /// The database client instance.
    inner: Arc<dyn GenericDbClient + Send + Sync + 'static>,
}

impl Deref for DbClient {
    type Target = dyn GenericDbClient + Send + Sync + 'static;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref()
    }
}

impl DbClient {
    /// Create a new database client.
    #[instrument(skip_all)]
    pub async fn surreal(config: &Config) -> Res<Self> {
        let db = SurrealDbClient::new(config).await?;
        Ok(Self { inner: Arc::new(db) })
    }
}

// Data types.

// A Channel in the database.
#[derive(Debug, Serialize, Deserialize)]
pub struct Channel {
    pub id: Option<surrealdb::sql::Thing>,
    pub channel_prompt: String,
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

// SurrealDB client implementation.

/// Database client for SurrealDB.
pub struct SurrealDbClient {
    db: Surreal<Client>,
}

impl Deref for SurrealDbClient {
    type Target = Surreal<Client>;

    fn deref(&self) -> &Self::Target {
        &self.db
    }
}

impl SurrealDbClient {
    /// Create a new database client.
    ///
    /// This creates an in-memory database instance. For production, you would
    /// want to connect to a persistent database.
    #[instrument(name = "SurrealDbClient::new", skip_all)]
    pub async fn new(config: &Config) -> Res<Self> {
        // Create an in-memory database
        let db = Surreal::new::<Ws>(&config.db_endpoint).await?;

        // Authenticate with the database using the provided username and password.
        db.signin(Root {
            username: &config.db_username,
            password: &config.db_password,
        })
        .await?;

        // Use a specific namespace and database
        db.use_ns("triage").use_db("bot").await?;

        // Define schemas.

        // Schema for list of channels that the bot has been "added to" (@-mentioned).
        db.query("DEFINE TABLE channel SCHEMAFULL").await?;
        db.query("DEFINE FIELD channel_prompt ON channel TYPE string;").await?;

        info!("Database initialized successfully.");

        Ok(Self { db })
    }
}

#[async_trait]
impl GenericDbClient for SurrealDbClient {
    #[instrument(skip(self))]
    async fn get_or_create_channel(&self, channel_id: &str) -> Res<Channel> {
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

    #[instrument(skip(self))]
    async fn update_channel_prompt(&self, channel_id: &str, prompt: &str) -> Void {
        let _: Option<Channel> = self.update(("channel", channel_id)).merge(json!({ "channel_prompt": prompt })).await?;

        info!("Channel `{}` updated.", channel_id);

        Ok(())
    }
}
