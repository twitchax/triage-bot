//! Event handling and user interactions for triage-bot.
//!
//! This module provides functionality for handling chat and message events:
//! - Processing incoming messages and @-mentions
//! - Managing message storage and retrieval
//! - Coordinating responses between services (LLM, database, chat)

pub mod chat_event;
pub mod message_storage;
