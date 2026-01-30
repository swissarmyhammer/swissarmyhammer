//! Query protocol for leader/client tree-sitter index architecture
//!
//! This module provides the RPC protocol for querying a shared tree-sitter index.
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
//! use swissarmyhammer_treesitter::query::{IndexClient, connect_or_become_leader};
//!
//! // Connect to existing leader or become one
//! let client = connect_or_become_leader("/path/to/project").await?;
//!
//! // Find duplicates
//! let duplicates = client.find_all_duplicates(0.85, 100).await?;
//!
//! // Semantic search
//! let results = client.semantic_search("fn main", 10, 0.7).await?;
//! ```

mod client;
mod election;
mod leader;
mod service;
mod types;

pub use client::{ClientError, IndexClient};
pub use election::{ElectionError, LeaderElection, LeaderGuard};
pub use leader::IndexLeader;
pub use service::IndexService;
pub use types::*;
