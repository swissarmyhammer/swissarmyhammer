//! Outline generation functionality for Tree-sitter based code analysis
//!
//! This module provides comprehensive code outline generation capabilities including:
//! - File discovery with glob pattern support and gitignore integration
//! - Language-aware parsing using Tree-sitter for multiple programming languages
//! - Hierarchical structure building that mirrors file system organization
//! - Symbol extraction with nested relationships and metadata
//! - Multiple output formatting options with extensible architecture
//!
//! The module is organized into several key components:
//! - [`file_discovery`]: File system traversal and pattern matching
//! - [`parser`]: Tree-sitter integration and language-specific parsing
//! - [`extractors`]: Language-specific symbol extraction logic
//! - [`hierarchy`]: Organization of parsed symbols into hierarchical structures
//! - [`types`]: Core data structures and type definitions
//! - [`utils`]: Utility functions and helpers

use thiserror::Error;

pub mod extractors;
pub mod file_discovery;
pub mod formatter;
pub mod hierarchy;
pub mod parser;
pub mod signature;
pub mod types;
pub mod utils;

#[cfg(test)]
mod integration_tests;

/// Outline-specific errors
#[derive(Error, Debug)]
pub enum OutlineError {
    /// File system operation failed
    #[error("File system error: {0}")]
    FileSystem(#[from] std::io::Error),

    /// Invalid glob pattern
    #[error("Invalid glob pattern '{pattern}': {message}")]
    InvalidGlobPattern {
        /// The invalid pattern
        pattern: String,
        /// Error message from glob parser
        message: String,
    },

    /// File discovery operation failed
    #[error("File discovery failed: {0}")]
    FileDiscovery(String),

    /// Language detection failed
    #[error("Language detection error: {0}")]
    LanguageDetection(String),

    /// Tree-sitter parsing failed
    #[error("TreeSitter parsing error: {0}")]
    TreeSitter(String),

    /// Generic outline generation error
    #[error("Outline generation error: {0}")]
    Generation(String),
}

/// Result type for outline operations
pub type Result<T> = std::result::Result<T, OutlineError>;

impl From<crate::error::SwissArmyHammerError> for OutlineError {
    fn from(err: crate::error::SwissArmyHammerError) -> Self {
        OutlineError::Generation(format!("SwissArmyHammer error: {err}"))
    }
}

pub use extractors::*;
pub use file_discovery::*;
pub use formatter::{FormatterConfig, SortOrder as FormatterSortOrder, YamlFormatter};
pub use hierarchy::*;
pub use parser::*;
pub use signature::*;
pub use types::*;
pub use utils::*;

// Re-export for convenience
pub use OutlineError as Error;
