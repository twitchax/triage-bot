//! Service integrations for external APIs and clients.
//!
//! This module contains implementations for various services used by the triage-bot:
//! - Chat services (e.g., Slack)
//! - Database services (e.g., SurrealDB)
//! - LLM services (e.g., OpenAI)
//!
//! Each service module defines both generic traits and concrete implementations,
//! allowing for extensibility and easy testing.

pub mod chat;
pub mod db;
pub mod llm;
pub mod mcp;
