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

pub mod abort_utils;
pub mod constants;
pub mod env_loader;
pub mod error;
pub mod error_context;
pub mod file_loader;
pub mod frontmatter;
pub mod fs_utils;
pub mod interactive_prompts;
pub mod parameter_conditions;
pub mod parameters;
pub mod rate_limiter;
pub mod test_organization;
pub mod test_utils;
pub mod traits;
pub mod types;
pub mod ulid_generator;
pub mod utils;
pub mod validation;

// Re-export commonly used constants for convenience
pub use constants::DEFAULT_TEST_EMBEDDING_MODEL;
pub use parameters::*;
pub use test_utils::*;
pub use validation::*;

// Re-export commonly used ULID functions for convenience
pub use utils::{generate_monotonic_ulid, generate_monotonic_ulid_string};

// Re-export file_loader for convenience
pub use file_loader::{FileEntry, FileSource, VirtualFileSystem};

// Re-export commonly used directory functions for convenience
pub use utils::{
    find_git_repository_root_from, get_or_create_swissarmyhammer_directory,
    get_or_create_swissarmyhammer_directory_from,
};

// Re-export error types for convenience
pub use error::{Result, SwissArmyHammerError};

// Re-export env_loader for convenience
pub use env_loader::EnvLoader;

// Re-export rate limiting functionality for convenience
pub use rate_limiter::{
    get_rate_limiter, init_rate_limiter, RateLimitChecker, RateLimitStatus, RateLimiter,
    RateLimiterConfig, DEFAULT_EXPENSIVE_OPERATION_LIMIT, DEFAULT_GLOBAL_RATE_LIMIT,
    DEFAULT_PER_CLIENT_RATE_LIMIT,
};

// Re-export abort utilities for convenience
pub use abort_utils::{
    abort_file_exists, create_abort_file, create_abort_file_current_dir, read_abort_file,
    remove_abort_file,
};

// Re-export test utilities for convenience (when testing)
pub use test_utils::{acquire_semantic_db_lock, create_temp_dir, ProcessGuard};

pub use error::*;
