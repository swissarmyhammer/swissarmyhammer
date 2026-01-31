//! Query protocol for leader/client tree-sitter workspace architecture
//!
//! This module provides the RPC protocol for querying a shared tree-sitter workspace.
//! One process acts as the "leader" holding the actual index in memory, while
//! other processes connect as clients and send queries over a Unix socket.
//!
//! # Architecture
//!
//! - **Leader**: Owns the `IndexContext`, maintains file watchers, handles queries
//! - **Client**: Connects to leader via Unix socket, sends queries, receives results
//! - **Election**: File-lock based leader election (first process wins)
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

mod client;
pub(crate) mod server;
pub(crate) mod service;
mod types;

pub use client::{ClientError, IndexClient};
pub use service::IndexService;
pub use types::{
    check_ready, Capture, ChunkResult, DuplicateCluster, IndexStatusInfo, QueryError,
    QueryErrorKind, QueryMatch, SimilarChunkResult,
};

// Re-export from leader-election crate for backward compatibility
pub use swissarmyhammer_leader_election::{
    ElectionConfig, ElectionError, LeaderElection, LeaderGuard,
};
