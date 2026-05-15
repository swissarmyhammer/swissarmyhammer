//! Query types for tree-sitter workspace operations
//!
//! This module provides serializable types for querying a tree-sitter workspace.
//! These types are used to communicate query results and status information.
//!
//! # Example
//!
//! ```ignore
//! use swissarmyhammer_treesitter::Workspace;
//!
//! // Open workspace - leader/client mode is handled automatically
//! let workspace = Workspace::open("/path/to/project").await?;
//!
//! // Find duplicates
//! let duplicates = workspace.find_all_duplicates(0.85, 100).await?;
//!
//! // Semantic search
//! let results = workspace.semantic_search("fn main", 10, 0.7).await?;
//! ```

mod types;

pub use types::{
    check_ready, Capture, ChunkResult, DuplicateCluster, IndexStatusInfo, QueryError,
    QueryErrorKind, QueryMatch, SimilarChunkResult,
};

// Re-export from leader-election crate for backward compatibility
pub use swissarmyhammer_leader_election::{
    ElectionConfig, ElectionError, LeaderElection, LeaderGuard,
};
