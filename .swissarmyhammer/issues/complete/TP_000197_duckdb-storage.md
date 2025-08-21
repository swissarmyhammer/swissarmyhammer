# TP_000197: DuckDB Vector Storage Implementation

## Goal
Implement the DuckDB-based vector storage layer for semantic search functionality.

## Context
Create a robust vector storage system using DuckDB that can efficiently store and query code embeddings. This implements the specification requirement to use DuckDB for storing and searching vectors.

## Specification Requirements
- Use DuckDB for storing and searching vectors
- Store in .swissarmyhammer directory (already in .gitignore)
- Open and close database on demand for file lock coordination
- Support vector similarity search with cosine similarity

## Tasks

### 1. Create VectorStorage struct in `semantic/storage.rs`

```rust
use crate::semantic::{Result, SemanticError, CodeChunk, Embedding, IndexedFile, FileId, ContentHash};
use duckdb::{Connection, params};
use std::path::Path;

pub struct VectorStorage {
    db_path: PathBuf,
}

impl VectorStorage {
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        Ok(Self { db_path })
    }
    
    fn get_connection(&self) -> Result<Connection> {
        Connection::open(&self.db_path)
            .map_err(SemanticError::Database)
    }
    
    pub fn initialize(&self) -> Result<()> {
        let conn = self.get_connection()?;
        self.create_tables(&conn)?;
        Ok(())
    }
}
```

### 2. Database Schema Design

Create tables for:
- **indexed_files**: Metadata about indexed files
- **code_chunks**: Individual code chunks with metadata  
- **embeddings**: Vector embeddings linked to chunks

```sql
-- Table for file metadata
CREATE TABLE IF NOT EXISTS indexed_files (
    file_id TEXT PRIMARY KEY,
    path TEXT NOT NULL,
    language TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    chunk_count INTEGER NOT NULL,
    indexed_at TIMESTAMP NOT NULL
);

-- Table for code chunks
CREATE TABLE IF NOT EXISTS code_chunks (
    chunk_id TEXT PRIMARY KEY,
    file_id TEXT NOT NULL,
    content TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    chunk_type TEXT NOT NULL,
    language TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    FOREIGN KEY (file_id) REFERENCES indexed_files(file_id)
);

-- Table for embeddings with vector similarity
CREATE TABLE IF NOT EXISTS embeddings (
    chunk_id TEXT PRIMARY KEY,
    embedding FLOAT[384] NOT NULL, -- 384-dimensional for nomic-embed-code
    FOREIGN KEY (chunk_id) REFERENCES code_chunks(chunk_id)
);

-- Create indices for performance
CREATE INDEX IF NOT EXISTS idx_chunks_file_id ON code_chunks(file_id);
CREATE INDEX IF NOT EXISTS idx_chunks_language ON code_chunks(language);
CREATE INDEX IF NOT EXISTS idx_files_content_hash ON indexed_files(content_hash);
```

### 3. Core Storage Operations

```rust
impl VectorStorage {
    /// Store indexed file metadata
    pub fn store_indexed_file(&self, file: &IndexedFile) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT OR REPLACE INTO indexed_files 
             (file_id, path, language, content_hash, chunk_count, indexed_at) 
             VALUES (?, ?, ?, ?, ?, ?)",
            params![
                file.file_id.0,
                file.path.to_string_lossy(),
                serde_json::to_string(&file.language)?,
                file.content_hash.0,
                file.chunk_count,
                file.indexed_at.timestamp()
            ]
        )?;
        Ok(())
    }
    
    /// Store code chunk
    pub fn store_chunk(&self, chunk: &CodeChunk) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT OR REPLACE INTO code_chunks 
             (chunk_id, file_id, content, start_line, end_line, chunk_type, language, content_hash)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                chunk.id,
                chunk.file_path.to_string_lossy(), // file_id derived from path
                chunk.content,
                chunk.start_line,
                chunk.end_line,
                serde_json::to_string(&chunk.chunk_type)?,
                serde_json::to_string(&chunk.language)?,
                chunk.content_hash.0
            ]
        )?;
        Ok(())
    }
    
    /// Store embedding vector
    pub fn store_embedding(&self, embedding: &Embedding) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT OR REPLACE INTO embeddings (chunk_id, embedding) VALUES (?, ?)",
            params![embedding.chunk_id, embedding.vector]
        )?;
        Ok(())
    }
}
```

### 4. Vector Similarity Search

```rust
impl VectorStorage {
    /// Search for similar embeddings using cosine similarity
    pub fn similarity_search(
        &self, 
        query_embedding: &[f32], 
        limit: usize,
        threshold: f32
    ) -> Result<Vec<(String, f32)>> {
        let conn = self.get_connection()?;
        
        // Use DuckDB's array_cosine_similarity function
        let mut stmt = conn.prepare(
            "SELECT e.chunk_id, array_cosine_similarity(e.embedding, ?) as similarity
             FROM embeddings e
             WHERE array_cosine_similarity(e.embedding, ?) >= ?
             ORDER BY similarity DESC
             LIMIT ?"
        )?;
        
        let rows = stmt.query_map(
            params![query_embedding, query_embedding, threshold, limit],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, f32>(1)?
                ))
            }
        )?;
        
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        
        Ok(results)
    }
    
    /// Get chunk by ID
    pub fn get_chunk(&self, chunk_id: &str) -> Result<Option<CodeChunk>> {
        let conn = self.get_connection()?;
        
        let mut stmt = conn.prepare(
            "SELECT chunk_id, file_id, content, start_line, end_line, chunk_type, language, content_hash
             FROM code_chunks WHERE chunk_id = ?"
        )?;
        
        let chunk = stmt.query_row(params![chunk_id], |row| {
            Ok(CodeChunk {
                id: row.get(0)?,
                file_path: PathBuf::from(row.get::<_, String>(1)?),
                content: row.get(2)?,
                start_line: row.get(3)?,
                end_line: row.get(4)?,
                chunk_type: serde_json::from_str(&row.get::<_, String>(5)?)
                    .map_err(|e| SemanticError::Serialization(e))?,
                language: serde_json::from_str(&row.get::<_, String>(6)?)
                    .map_err(|e| SemanticError::Serialization(e))?,
                content_hash: ContentHash(row.get(7)?),
            })
        }).optional()?;
        
        Ok(chunk)
    }
}
```

### 5. File Change Detection

```rust
impl VectorStorage {
    /// Check if file needs re-indexing based on content hash
    pub fn needs_reindexing(&self, file_path: &Path, current_hash: &ContentHash) -> Result<bool> {
        let conn = self.get_connection()?;
        
        let existing_hash: Option<String> = conn.query_row(
            "SELECT content_hash FROM indexed_files WHERE path = ?",
            params![file_path.to_string_lossy()],
            |row| row.get(0)
        ).optional()?;
        
        Ok(match existing_hash {
            Some(hash) => hash != current_hash.0,
            None => true, // File not indexed yet
        })
    }
    
    /// Remove all data for a file (for re-indexing)
    pub fn remove_file(&self, file_path: &Path) -> Result<()> {
        let conn = self.get_connection()?;
        
        // Delete embeddings for chunks of this file
        conn.execute(
            "DELETE FROM embeddings WHERE chunk_id IN 
             (SELECT chunk_id FROM code_chunks WHERE file_id = ?)",
            params![file_path.to_string_lossy()]
        )?;
        
        // Delete chunks for this file
        conn.execute(
            "DELETE FROM code_chunks WHERE file_id = ?",
            params![file_path.to_string_lossy()]
        )?;
        
        // Delete file metadata
        conn.execute(
            "DELETE FROM indexed_files WHERE path = ?",
            params![file_path.to_string_lossy()]
        )?;
        
        Ok(())
    }
}
```

## Acceptance Criteria
- [ ] VectorStorage struct properly manages DuckDB connections
- [ ] Database schema supports all required operations
- [ ] Vector similarity search works with cosine similarity
- [ ] File change detection using content hashes
- [ ] Proper error handling throughout
- [ ] Connection management follows open/close on demand pattern
- [ ] Database file is created in .swissarmyhammer directory
- [ ] All operations are atomic and handle concurrent access

## Architecture Notes
- Uses on-demand connections to avoid locking issues
- 384-dimensional vectors match nomic-embed-code model
- Cosine similarity for semantic search ranking
- Foreign key constraints maintain data integrity
- Indices optimize common query patterns

## Next Steps
After completion, proceed to TP_000198_file-hashing to implement MD5-based change detection.