//! # SwissArmyHammer Common
//!
//! This crate provides foundational types, traits, and utilities shared across
//! the SwissArmyHammer ecosystem. It serves as the base dependency for all other
//! SwissArmyHammer crates, establishing common patterns and abstractions.
//!
//! ## Modules
//!
//! - [`constants`] - Shared constants used throughout the ecosystem
//! - [`traits`] - Common trait definitions for shared behaviors  
//! - [`types`] - Core type definitions and newtypes for domain safety
//! - [`utils`] - Utility functions and helpers
//!
//! ## Design Principles
//!
//! This crate follows the SwissArmyHammer architectural principles:
//! - Type safety through newtypes and strong typing
//! - Comprehensive error handling with structured error types
//! - Serialization support for all public types
//! - Documentation-driven development with clear API contracts

#![warn(missing_docs)]

pub mod constants;
pub mod env_loader;
pub mod error;
pub mod traits;
pub mod types;
pub mod utils;

// Re-export commonly used constants for convenience
pub use constants::DEFAULT_TEST_EMBEDDING_MODEL;

// Re-export commonly used ULID functions for convenience
pub use utils::{generate_monotonic_ulid, generate_monotonic_ulid_string};

// Re-export commonly used directory functions for convenience
pub use utils::{
    find_git_repository_root_from, get_or_create_swissarmyhammer_directory,
    get_or_create_swissarmyhammer_directory_from,
};

// Re-export error types for convenience
pub use error::{Result, SwissArmyHammerError};

// Re-export env_loader for convenience
pub use env_loader::EnvLoader;
