//! Tree-sitter based code parsing and indexing
//!
//! This crate provides an in-memory index of parsed files using tree-sitter,
//! with support for:
//!
//! - 30+ programming languages
//! - Gitignore pattern support
//! - File watching for automatic updates
//! - Async parsing with progress callbacks for MCP tool integration
//! - Semantic chunking based on AST structure
//!
//! # Quick Start
//!
//! ```ignore
//! use swissarmyhammer_treesitter::IndexContext;
//!
//! // Create an index context for a path
//! let mut context = IndexContext::new("/path/to/project");
//!
//! // Scan and parse files
//! let result = context.scan().await?;
//! println!("Parsed {} files", result.files_parsed);
//!
//! // Get a parsed file
//! if let Some(parsed) = context.get("src/main.rs")? {
//!     println!("Lines: {}", parsed.line_count());
//! }
//! ```
//!
//! # Async Parsing with Progress
//!
//! For MCP tool integration, you can use async parsing with progress callbacks:
//!
//! ```ignore
//! use swissarmyhammer_treesitter::IndexContext;
//!
//! // Create context with progress callback
//! let mut context = IndexContext::new("/path/to/project")
//!     .with_progress(|status| {
//!         println!("{}: {}/{}", status.message, status.files_parsed, status.files_total);
//!     });
//!
//! // Scan with progress reporting
//! let result = context.scan().await?;
//! ```
//!
//! # Semantic Chunking
//!
//! Extract semantic chunks from parsed files for embeddings:
//!
//! ```ignore
//! use swissarmyhammer_treesitter::{IndexContext, chunk_file};
//! use std::sync::Arc;
//!
//! let mut context = IndexContext::new("/path/to/project");
//! context.scan().await?;
//!
//! let parsed = context.get("src/main.rs")?.unwrap();
//! let chunks = chunk_file(Arc::new(parsed.clone()));
//!
//! for chunk in chunks {
//!     println!("{} bytes", chunk.byte_len());
//! }
//! ```
//!
//! # Supported Languages
//!
//! The index supports 30+ languages including:
//! - Systems: Rust, C, C++, Go, Zig
//! - Web: JavaScript, TypeScript, HTML, CSS
//! - Backend: Python, Java, Ruby, PHP, C#
//! - Mobile: Swift, Kotlin, Dart
//! - Functional: Haskell, OCaml, Elixir, Scala
//! - Config: JSON, YAML, TOML, Markdown
//! - Shell: Bash
//! - Data: SQL

pub mod chunk;
pub mod db;
pub mod error;
pub mod index;
pub mod language;
pub mod parsed_file;
pub mod query;
mod unified;
pub mod watcher;

#[cfg(test)]
pub(crate) mod test_utils;

// Re-export main types
pub use chunk::{
    chunk_file, ChunkGraph, ChunkSource, QuerySource, SemanticChunk, SimilarChunk, SimilarityQuery,
};
pub use error::{Result, TreeSitterError};
pub use index::{IndexConfig, IndexContext, IndexStats, IndexStatus, ScanResult};
pub use language::{LanguageConfig, LanguageRegistry};
pub use parsed_file::ParsedFile;
pub use watcher::{WorkspaceWatcher, WorkspaceWatcherCallback};

// Query types and leader election for workspace architecture
pub use query::{
    ChunkResult, DuplicateCluster, ElectionConfig, ElectionError, IndexStatusInfo,
    LeaderElection, LeaderGuard, QueryError, QueryErrorKind, QueryMatch, SimilarChunkResult,
};

// Database types
pub use db::{ChunkRecord, EmbeddedChunkRecord, IndexDatabase};

// Workspace with automatic leader/client mode using SQLite storage
pub use unified::{Workspace, WorkspaceBuilder};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_public_api_accessible() {
        // Verify main types are accessible
        let _context = IndexContext::new("/some/path");
        let _config = IndexConfig::default();
    }

    #[test]
    fn test_language_registry_accessible() {
        let registry = LanguageRegistry::global();
        // is_supported checks file extensions, not language names
        assert!(registry.is_supported("rs"));
        assert!(registry.is_supported("json"));
        assert!(registry.is_supported("yaml"));
        assert!(registry.is_supported("toml"));
        assert!(registry.is_supported("md"));
    }

    #[test]
    fn test_error_types_accessible() {
        let err = TreeSitterError::unsupported_language(PathBuf::from("test.xyz"));
        assert!(format!("{}", err).contains("Unsupported"));
    }
}
