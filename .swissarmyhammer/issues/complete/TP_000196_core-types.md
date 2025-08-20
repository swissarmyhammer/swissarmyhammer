# TP_000196: Core Types and Data Structures

## Goal
Define core data structures and types for the semantic search system.

## Context
Implement the fundamental types that will be used throughout the semantic search system, including file representations, embeddings, and search results.

## Tasks

### 1. Define Core Types in `semantic/types.rs`

```rust
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Unique identifier for indexed files
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileId(pub String);

/// Hash of file content for change detection
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentHash(pub String);

/// Programming language detected for a file
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    Rust,
    Python,
    TypeScript,
    JavaScript,
    Dart,
    Unknown,
}

/// A chunk of code extracted from a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChunk {
    pub id: String,
    pub file_path: PathBuf, 
    pub language: Language,
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
    pub chunk_type: ChunkType,
    pub content_hash: ContentHash,
}

/// Type of code chunk based on TreeSitter parsing
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChunkType {
    Function,
    Class,
    Module,
    Import,
    PlainText, // For files that fail TreeSitter parsing
}

/// Vector embedding for a code chunk
#[derive(Debug, Clone)]
pub struct Embedding {
    pub chunk_id: String,
    pub vector: Vec<f32>, // 384-dimensional for nomic-embed-code
}

/// Indexed file metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedFile {
    pub file_id: FileId,
    pub path: PathBuf,
    pub language: Language,
    pub content_hash: ContentHash,
    pub chunk_count: usize,
    pub indexed_at: chrono::DateTime<chrono::Utc>,
}

/// Search result with similarity score
#[derive(Debug, Clone)]
pub struct SemanticSearchResult {
    pub chunk: CodeChunk,
    pub similarity_score: f32,
    pub excerpt: String,
}

/// Search query parameters
#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub text: String,
    pub limit: usize,
    pub similarity_threshold: f32,
    pub language_filter: Option<Language>,
}

/// Configuration for the semantic search system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticConfig {
    pub database_path: PathBuf,
    pub embedding_model: String, 
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub similarity_threshold: f32,
}

impl Default for SemanticConfig {
    fn default() -> Self {
        Self {
            database_path: PathBuf::from(".swissarmyhammer/semantic.db"),
            embedding_model: "nomic-ai/nomic-embed-code".to_string(),
            chunk_size: 512,
            chunk_overlap: 64,
            similarity_threshold: 0.7,
        }
    }
}
```

### 2. Error Types in `semantic/mod.rs`

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SemanticError {
    #[error("Database error: {0}")]
    Database(#[from] duckdb::Error),
    
    #[error("Embedding error: {0}")]
    Embedding(String),
    
    #[error("File system error: {0}")]
    FileSystem(#[from] std::io::Error),
    
    #[error("TreeSitter parsing error: {0}")]
    TreeSitter(String),
    
    #[error("Invalid configuration: {0}")]
    Config(String),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, SemanticError>;
```

### 3. Module Exports in `semantic/mod.rs`

```rust
pub mod types;
pub mod storage;
pub mod embedding;
pub mod parser;
pub mod indexer;
pub mod searcher;
pub mod utils;

pub use types::*;
pub use storage::VectorStorage;
pub use embedding::EmbeddingEngine;
pub use parser::CodeParser;
pub use indexer::FileIndexer;
pub use searcher::SemanticSearcher;

// Re-export for convenience
pub use crate::semantic::SemanticError as Error;
pub use crate::semantic::Result;
```

## Acceptance Criteria
- [ ] All core types are defined with proper serialization
- [ ] Error types are comprehensive and follow Rust best practices
- [ ] Module structure exports are clean and logical
- [ ] Types support the workflow: file -> chunks -> embeddings -> search
- [ ] Documentation is clear on all public types
- [ ] Types compile without errors

## Architecture Notes
- FileId uses content-based hashing for consistency
- ContentHash enables smart re-indexing when files change
- CodeChunk represents the atomic unit for embedding and search
- Embedding vector dimension matches nomic-embed-code (384)
- Language enum supports all specified languages from spec

## Next Steps
After completion, proceed to TP_000197_duckdb-storage to implement the vector storage layer.