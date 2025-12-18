//! ACP Conformance Test Suite
//!
//! This crate provides conformance testing for Agent Client Protocol (ACP) implementations.
//! Tests can be run against any implementation that provides the `Agent` trait.
//!
//! # Test Organization
//!
//! Tests are organized by protocol section:
//! - `initialization`: Protocol initialization and capability negotiation
//! - `sessions`: Session setup (new, load, set_mode)
//! - `content`: Content blocks (text, image, audio, embedded resources, resource links)
//!
//! # Running Tests
//!
//! ```bash
//! # Run all conformance tests
//! cargo test
//! ```

pub mod content;
pub mod initialization;
pub mod sessions;

/// Result type for conformance tests
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during conformance testing
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Agent error: {0}")]
    Agent(#[from] agent_client_protocol::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Timeout: {0}")]
    Timeout(String),
}
