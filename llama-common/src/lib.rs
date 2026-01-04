//! # Llama Common
//!
//! Shared types, traits, and utilities for the llama-agent workspace.
//! This crate provides common abstractions to ensure consistency across
//! all components in the llama-agent ecosystem.

pub mod async_utils;
pub mod config;
pub mod error;
pub mod logging;
pub mod retry;

// Re-export main traits for convenience
pub use config::ValidatedConfig;
pub use error::{ErrorCategory, LlamaError};
pub use logging::Pretty;
pub use retry::{RetryConfig, RetryManager, RetryableError};
