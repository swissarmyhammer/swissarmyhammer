//! SwissArmyHammer Search
//!
//! This crate provides semantic search functionality using vector embeddings and TreeSitter parsing
//! for source code files. It extracts search functionality from the main library into a dedicated
//! crate for better maintainability and reuse.
//!
//! ## Features
//!
//! - **Semantic Search**: Vector embeddings for meaningful code search
//! - **Multi-Language**: TreeSitter parsing for Rust, Python, TypeScript, JavaScript, Dart
//! - **Fast Storage**: DuckDB for efficient vector similarity search
//! - **Type Safety**: Structured types for search queries and results
//! - **Clean API**: Consistent with other SwissArmyHammer domain crates
//! - **Testability**: Isolated search operations for easier testing
//!
//! ## Example Usage
//!
//! ```rust
//! use swissarmyhammer_search::{SearchOperations, SearchRequest};
//!
//! let search = SearchOperations::new().await?;
//! let results = search.query("error handling patterns").await?;
//! ```

pub mod embedding;
pub mod error;
pub mod indexer;
pub mod operations;
pub mod parser;
pub mod searcher;
pub mod storage;
pub mod types;
pub mod utils;

// Test utilities
#[cfg(test)]
pub mod test_utils;

// Integration tests
#[cfg(test)]
pub mod tests;

// Re-export main types
pub use embedding::*;
pub use error::{SearchError, SearchResult};
pub use indexer::FileIndexer;
pub use operations::SearchOperations;
pub use parser::*;
pub use searcher::SemanticSearcher;
pub use storage::VectorStorage;
pub use types::*;
pub use utils::*;

/// Version of this crate
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
