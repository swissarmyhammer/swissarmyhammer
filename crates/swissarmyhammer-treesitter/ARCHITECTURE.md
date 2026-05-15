# Tree-sitter Index Architecture

## Overview

The tree-sitter index uses a **Workspace > Index > ParsedFile > SemanticChunk** hierarchy with SQLite-based persistent storage.

## Key Design Decisions

### 1. SQLite Storage with WAL Mode

- **Leaders** open the database in read-write mode and perform batch writes per file
- **Non-leaders** (readers) open the database in read-only mode for queries
- **WAL mode** enables concurrent readers while one leader writes
- Database file: `.treesitter-index.db` in workspace root

### 2. No Parse Trees Stored

We don't store the tree-sitter AST because:
- Re-parsing is very fast (milliseconds for typical files)
- We store `(start_byte, end_byte)` positions for each chunk
- Chunk text can be quickly read from the original file using byte offsets
- This keeps the database small and focused on embeddings

### 3. Schema Design

```sql
-- Files table: tracks parsed files
CREATE TABLE files (
    file_id TEXT PRIMARY KEY,      -- MD5 hash of file path (hex)
    path TEXT NOT NULL UNIQUE,     -- Full file path
    content_hash BLOB NOT NULL     -- MD5 of file content (16 bytes)
);

-- Chunks table: semantic chunks with embeddings
CREATE TABLE chunks (
    file_id TEXT NOT NULL REFERENCES files(file_id) ON DELETE CASCADE,
    start_byte INTEGER NOT NULL,
    end_byte INTEGER NOT NULL,
    embedding BLOB,                -- f32 vector as raw little-endian bytes
    symbol_path TEXT,              -- e.g., "file.rs::MyStruct::method"
    PRIMARY KEY (file_id, start_byte, end_byte)
);
```

**Key decisions:**
- `file_id` is MD5 of the *path* (not content) - used as a compact foreign key
- `content_hash` is MD5 of file *content* - used to detect if re-parsing needed
- **No autoincrement ID** on chunks - the composite key `(file_id, start_byte, end_byte)` is sufficient
- Embeddings stored as raw f32 bytes (4 bytes per float) for efficiency

### 4. Leader Election

- Uses file-lock based leader election from `swissarmyhammer-leader-election`
- First process to acquire lock becomes leader
- Leader scans workspace, parses files, computes embeddings, writes to SQLite
- Non-leaders open database read-only and query it

### 5. No RPC

Previous architecture used tarpc for RPC between leader and clients. This has been removed:
- SQLite with WAL provides efficient concurrent access
- Simpler architecture with fewer moving parts
- No socket management or serialization overhead

### 6. Embedding Storage

- Embeddings are `Vec<f32>` stored as raw little-endian bytes
- 768-dimensional embedding = 3072 bytes per chunk
- Encoding/decoding via `encode_embedding()` and `decode_embedding()` helpers

## File Structure

```
src/
  db.rs        - SQLite database operations
  unified.rs   - Workspace with leader/reader modes
  index.rs     - In-memory IndexContext (used by leader)
  chunk.rs     - SemanticChunk and ChunkGraph
  parsed_file.rs - ParsedFile with AST
  query/
    mod.rs     - Query type exports
    types.rs   - Serializable result types
```

## Usage Flow

1. `Workspace::open(path)` attempts leader election
2. If leader: scan files, parse, chunk, embed, write to SQLite
3. If reader: open SQLite read-only
4. Queries work in both modes (leader uses in-memory graph, reader uses SQLite)
