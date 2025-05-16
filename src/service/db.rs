//! SurrealDB implementation for triage-bot data storage.

use crate::base::{config::Config, types::{Res, Void}};
use serde::{Deserialize, Serialize};
use surrealdb::{engine::local::{Db, Mem}};
use surrealdb::Surreal;
use tracing::{info, error, debug};
use anyhow::anyhow;

/// Database client for triage-bot.
#[derive(Clone)]
pub struct DbClient {
    /// The SurrealDB client instance.
    db: Surreal<Db>,
}

// A Channel in the database.
#[derive(Debug, Serialize, Deserialize)]
pub struct Channel {
    pub id: Option<surrealdb::sql::Thing>,
    pub channel_id: String,
    pub name: String,
    pub is_active: bool,
    pub first_mentioned_at: chrono::DateTime<chrono::Utc>,
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
    pub async fn new(config: &Config) -> Res<Self> {
        // Create an in-memory database
        let db = Surreal::new::<Mem>(()).await?;

        // Use a specific namespace and database
        db.use_ns("triage").use_db("bot").await?;

        // Define schemas

        // Schema for list of channels that the bot has been "added to" (@-mentioned).
        db.query("DEFINE TABLE channel SCHEMAFULL").await?;
        db.query(
            "DEFINE FIELD channel_id ON channel TYPE string;
             DEFINE FIELD name ON channel TYPE string;
             DEFINE FIELD is_active ON channel TYPE bool;
             DEFINE FIELD first_mentioned_at ON channel TYPE datetime;
             DEFINE INDEX channel_id_idx ON TABLE channel COLUMNS channel_id UNIQUE;"
        ).await?;

        info!("Database initialized successfully");

        Ok(Self { db })
    }
}

