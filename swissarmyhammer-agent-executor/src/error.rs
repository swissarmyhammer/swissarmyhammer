//! Error types for agent execution
//!
//! This module defines the error hierarchy used throughout the agent executor crate.
//! All executor operations return [`ActionResult<T>`] which is an alias for `Result<T, ActionError>`.
//!
//! # Error Hierarchy
//!
//! The [`ActionError`] enum represents all possible errors during agent execution:
//!
//! - **ClaudeError**: Claude-specific execution failures (model unavailable, API errors)
//! - **VariableError**: Variable resolution or substitution failures
//! - **ParseError**: Prompt or response parsing failures
//! - **ExecutionError**: Generic execution failures (initialization, generation errors)
//! - **IoError**: File system or I/O failures
//! - **JsonError**: JSON serialization/deserialization failures
//! - **RateLimit**: Rate limiting errors with retry timing information
//!
//! # When to Use ActionError vs ActionResult
//!
//! - **ActionError**: Use when creating or returning errors explicitly
//! - **ActionResult<T>**: Use as the return type for fallible operations
//!
//! # Error Conversion
//!
//! Several error types implement `From` traits for automatic conversion:
//!
//! - `std::io::Error` → `ActionError::IoError`
//! - `serde_json::Error` → `ActionError::JsonError`
//!
//! # Rate Limiting
//!
//! The `RateLimit` variant includes retry timing information to help callers
//! implement exponential backoff and respect API rate limits.
//!
//! # Usage
//!
//! ```rust
//! use swissarmyhammer_agent_executor::{ActionError, ActionResult};
//!
//! fn process_prompt(prompt: &str) -> ActionResult<String> {
//!     if prompt.is_empty() {
//!         return Err(ActionError::ParseError("Empty prompt".to_string()));
//!     }
//!     Ok("Success".to_string())
//! }
//! ```

use std::time::Duration;
use thiserror::Error;

/// Errors that can occur during action execution
#[derive(Debug, Error)]
pub enum ActionError {
    /// Claude command execution failed
    #[error("Claude execution failed: {0}")]
    ClaudeError(String),
    /// Variable operation failed
    #[error("Variable operation failed: {0}")]
    VariableError(String),
    /// Action parsing failed
    #[error("Action parsing failed: {0}")]
    ParseError(String),
    /// Generic action execution error
    #[error("Action execution failed: {0}")]
    ExecutionError(String),
    /// IO error during action execution
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    /// JSON parsing error
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
    /// Rate limit error with retry time
    #[error("Rate limit reached. Please wait {wait_time:?} and try again. Details: {message}")]
    RateLimit {
        /// The error message
        message: String,
        /// How long to wait before retrying
        wait_time: Duration,
    },
}

/// Result type for action operations
pub type ActionResult<T> = std::result::Result<T, ActionError>;
