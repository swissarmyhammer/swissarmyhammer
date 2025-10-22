//! Code outline generation and analysis
//!
//! This crate provides comprehensive code outline generation capabilities including:
//! - File discovery with glob pattern support and gitignore integration
//! - Language-aware parsing using Tree-sitter for multiple programming languages
//! - Hierarchical structure building that mirrors file system organization
//! - Symbol extraction with nested relationships and metadata
//! - Multiple output formatting options with extensible architecture
//!
//! # Example Usage
//!
//! ```rust
//! use swissarmyhammer_outline::{FileDiscovery, OutlineParser, OutlineParserConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Discover files using glob patterns
//! let file_discovery = FileDiscovery::new(vec!["**/*.rs".to_string()])?;
//! let (discovered_files, _report) = file_discovery.discover_files()?;
//!
//! // Parse files to generate outline
//! let mut parser = OutlineParser::new(OutlineParserConfig::default())?;
//! for file in &discovered_files {
//!     let content = std::fs::read_to_string(&file.path)?;
//!     let outline = parser.parse_file(&file.path, &content)?;
//!     println!("Found {} symbols in {}", outline.symbols.len(), file.path.display());
//! }
//! # Ok(())
//! # }
//! ```

use swissarmyhammer_common::{ErrorSeverity, Severity};
use thiserror::Error;

pub mod extractors;
pub mod file_discovery;
pub mod formatter;
pub mod hierarchy;
pub mod parser;
pub mod signature;
pub mod types;
pub mod utils;

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

impl Severity for OutlineError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // All outline errors are operation-level failures
            // No critical errors as outline generation failures don't prevent system operation
            OutlineError::FileSystem(_) => ErrorSeverity::Error,
            OutlineError::InvalidGlobPattern { .. } => ErrorSeverity::Error,
            OutlineError::FileDiscovery(_) => ErrorSeverity::Error,
            OutlineError::LanguageDetection(_) => ErrorSeverity::Error,
            OutlineError::TreeSitter(_) => ErrorSeverity::Error,
            OutlineError::Generation(_) => ErrorSeverity::Error,
        }
    }
}

// Re-export main types and functionality
pub use extractors::*;
pub use file_discovery::*;
pub use formatter::*;
pub use hierarchy::*;
pub use parser::*;
pub use signature::*;
pub use types::*;
pub use utils::*;

// Re-export error for convenience
pub use OutlineError as Error;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outline_error_error_severity() {
        let error = OutlineError::FileSystem(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert_eq!(error.severity(), ErrorSeverity::Error);

        let error = OutlineError::InvalidGlobPattern {
            pattern: "**[".to_string(),
            message: "invalid pattern".to_string(),
        };
        assert_eq!(error.severity(), ErrorSeverity::Error);

        let error = OutlineError::FileDiscovery("discovery failed".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);

        let error = OutlineError::LanguageDetection("unknown language".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);

        let error = OutlineError::TreeSitter("parse failed".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);

        let error = OutlineError::Generation("generation failed".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }
}
