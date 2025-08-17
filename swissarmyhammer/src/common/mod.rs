//! Common utilities module
//!
//! This module provides shared utilities to eliminate code duplication
//! throughout the SwissArmyHammer codebase.

/// Error handling utilities and context helpers
pub mod error_context;

/// Environment variable loading utilities  
pub mod env_loader;

/// File type detection and extension handling
pub mod file_types;

/// MCP error conversion utilities
pub mod mcp_errors;

/// Validation builders and error construction
pub mod validation_builders;

/// Rate limiting utilities for API operations
pub mod rate_limiter;

/// Monotonic ULID generator utility
pub mod ulid_generator;

/// Abort file utilities for consistent abort handling
pub mod abort_utils;

/// Shared parameter system for prompts and workflows
pub mod parameters;

// Re-export commonly used items
pub use abort_utils::{
    abort_file_exists, create_abort_file, create_abort_file_current_dir, read_abort_file,
    remove_abort_file,
};
pub use env_loader::{load_env_optional, load_env_parsed, load_env_string, EnvLoader};
pub use error_context::{io_error_with_context, io_error_with_message, other_error, IoResultExt};
pub use file_types::{
    extract_base_name, is_any_prompt_file, is_prompt_file, ExtensionMatcher, PROMPT_EXTENSIONS,
};
pub use mcp_errors::{mcp, McpResultExt, ToSwissArmyHammerError};
pub use parameters::{
    Parameter, ParameterError, ParameterProvider, ParameterResolver, ParameterResult,
    ParameterType, ParameterValidator,
};
pub use rate_limiter::{
    get_rate_limiter, init_rate_limiter, RateLimitStatus, RateLimiter, RateLimiterConfig,
};
pub use ulid_generator::{generate_monotonic_ulid, generate_monotonic_ulid_string};
pub use validation_builders::{quick, ValidationChain, ValidationErrorBuilder, ValidationResult};
