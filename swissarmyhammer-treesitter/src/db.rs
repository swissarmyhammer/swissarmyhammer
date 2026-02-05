//! SQLite-based persistent storage for the tree-sitter index
//!
//! This module provides a SQLite database for storing parsed file metadata
//! and semantic chunks with embeddings. The database uses WAL mode to support
//! concurrent readers (non-leader processes) while one leader writes.
//!
//! # Schema
//!
//! - `files`: Tracks parsed files with (file_id, path, content_hash)
//! - `chunks`: Stores semantic chunks with embeddings (file_id, start_byte, end_byte, embedding, symbol_path)
//!
//! # Usage
//!
//! Leaders open the database in read-write mode and perform batch writes per file.
//! Non-leaders open in read-only mode for queries.

use rusqlite::{params, Connection, OpenFlags, Result as SqliteResult};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// Default database filename
pub const DEFAULT_DB_FILENAME: &str = ".treesitter-index.db";

/// SQLite cache size in KB (negative value means KB, positive means pages)
const SQLITE_CACHE_SIZE_KB: &str = "-64000";

/// Expected length of MD5 hash as hex string (used in tests)
#[cfg(test)]
const MD5_HEX_LENGTH: usize = 32;

/// Number of bytes per f32 value in embedding storage
const BYTES_PER_F32: usize = 4;

/// Encode an embedding vector as raw bytes for storage
fn encode_embedding(embedding: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(embedding.len() * BYTES_PER_F32);
    for val in embedding {
        bytes.extend_from_slice(&val.to_le_bytes());
    }
    bytes
}

/// Decode an embedding vector from raw bytes
fn decode_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(BYTES_PER_F32)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// SQLite database for the tree-sitter index
///
/// This struct is `Send + Sync` safe by wrapping the connection in a Mutex.
/// This allows it to be shared across threads safely, which is necessary for
/// use in async contexts with tokio.
pub struct IndexDatabase {
    conn: Mutex<Connection>,
    is_readonly: bool,
}

impl IndexDatabase {
    /// Helper to lock the connection, panics if the mutex is poisoned
    fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().expect("IndexDatabase mutex poisoned")
    }
}

impl IndexDatabase {
    /// Open the database in read-write mode (for leaders)
    ///
    /// Creates the database and schema if it doesn't exist.
    /// Enables WAL mode for concurrent readers.
    pub fn open_readwrite(path: impl AsRef<Path>) -> SqliteResult<Self> {
        let conn = Connection::open(path)?;

        // Enable WAL mode for concurrent readers
        conn.pragma_update(None, "journal_mode", "WAL")?;

        // Performance tuning
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "cache_size", SQLITE_CACHE_SIZE_KB)?;

        let db = Self {
            conn: Mutex::new(conn),
            is_readonly: false,
        };

        db.create_schema()?;
        Ok(db)
    }

    /// Open the database in read-only mode (for non-leaders)
    ///
    /// Returns an error if the database doesn't exist.
    pub fn open_readonly(path: impl AsRef<Path>) -> SqliteResult<Self> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
            is_readonly: true,
        })
    }

    /// Check if the database is open in read-only mode
    pub fn is_readonly(&self) -> bool {
        self.is_readonly
    }

    /// Create the database schema if it doesn't exist
    fn create_schema(&self) -> SqliteResult<()> {
        self.conn().execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS files (
                file_id TEXT PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                content_hash BLOB NOT NULL
            );

            CREATE TABLE IF NOT EXISTS chunks (
                file_id TEXT NOT NULL REFERENCES files(file_id) ON DELETE CASCADE,
                start_byte INTEGER NOT NULL,
                end_byte INTEGER NOT NULL,
                embedding BLOB,
                symbol_path TEXT,
                PRIMARY KEY (file_id, start_byte, end_byte)
            );

            CREATE INDEX IF NOT EXISTS idx_chunks_file_id ON chunks(file_id);
            CREATE INDEX IF NOT EXISTS idx_files_path ON files(path);
            "#,
        )
    }

    /// Compute the file_id (MD5 hash of path as hex string)
    pub fn compute_file_id(path: &Path) -> String {
        let path_str = path.to_string_lossy();
        let hash = md5::compute(path_str.as_bytes());
        format!("{:x}", hash)
    }

    /// Check if a file exists in the database with the given content hash
    ///
    /// Returns true if the file exists AND has a matching content hash.
    pub fn file_is_current(&self, path: &Path, content_hash: &[u8; 16]) -> SqliteResult<bool> {
        let file_id = Self::compute_file_id(path);
        let conn = self.conn();
        let mut stmt = conn.prepare_cached("SELECT content_hash FROM files WHERE file_id = ?")?;

        let result: Option<Vec<u8>> = stmt.query_row([&file_id], |row| row.get(0)).ok();

        match result {
            Some(stored_hash) => Ok(stored_hash == content_hash.as_slice()),
            None => Ok(false),
        }
    }

    /// Get all file paths in the index
    pub fn list_files(&self) -> SqliteResult<Vec<PathBuf>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached("SELECT path FROM files")?;
        let rows = stmt.query_map([], |row| {
            let path: String = row.get(0)?;
            Ok(PathBuf::from(path))
        })?;

        rows.collect()
    }

    /// Get file count
    pub fn file_count(&self) -> SqliteResult<usize> {
        self.conn()
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
    }

    /// Get chunk count
    pub fn chunk_count(&self) -> SqliteResult<usize> {
        self.conn()
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
    }

    /// Get count of chunks with embeddings
    pub fn embedded_chunk_count(&self) -> SqliteResult<usize> {
        self.conn().query_row(
            "SELECT COUNT(*) FROM chunks WHERE embedding IS NOT NULL",
            [],
            |row| row.get(0),
        )
    }

    /// Begin a transaction for batch operations
    pub fn begin_transaction(&self) -> SqliteResult<()> {
        self.conn().execute("BEGIN IMMEDIATE", [])?;
        Ok(())
    }

    /// Commit the current transaction
    pub fn commit_transaction(&self) -> SqliteResult<()> {
        self.conn().execute("COMMIT", [])?;
        Ok(())
    }

    /// Rollback the current transaction
    pub fn rollback_transaction(&self) -> SqliteResult<()> {
        self.conn().execute("ROLLBACK", [])?;
        Ok(())
    }

    /// Insert or update a file record
    ///
    /// This also removes all existing chunks for the file.
    pub fn upsert_file(&self, path: &Path, content_hash: &[u8; 16]) -> SqliteResult<String> {
        let file_id = Self::compute_file_id(path);
        let path_str = path.to_string_lossy().to_string();
        let conn = self.conn();

        // Delete existing chunks for this file (cascade doesn't always work as expected)
        conn.execute("DELETE FROM chunks WHERE file_id = ?", [&file_id])?;

        // Upsert the file record
        conn.execute(
            "INSERT OR REPLACE INTO files (file_id, path, content_hash) VALUES (?, ?, ?)",
            params![&file_id, &path_str, content_hash.as_slice()],
        )?;

        Ok(file_id)
    }

    /// Insert a chunk for a file
    pub fn insert_chunk(
        &self,
        file_id: &str,
        start_byte: usize,
        end_byte: usize,
        embedding: Option<&[f32]>,
        symbol_path: &str,
    ) -> SqliteResult<()> {
        let embedding_blob = embedding.map(encode_embedding);

        self.conn().execute(
            "INSERT INTO chunks (file_id, start_byte, end_byte, embedding, symbol_path) VALUES (?, ?, ?, ?, ?)",
            params![
                file_id,
                start_byte as i64,
                end_byte as i64,
                embedding_blob,
                symbol_path
            ],
        )?;

        Ok(())
    }

    /// Get all chunks for a file
    pub fn get_chunks_for_file(&self, path: &Path) -> SqliteResult<Vec<ChunkRecord>> {
        let file_id = Self::compute_file_id(path);
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT start_byte, end_byte, embedding, symbol_path FROM chunks WHERE file_id = ?",
        )?;

        let rows = stmt.query_map([&file_id], |row| {
            let embedding_blob: Option<Vec<u8>> = row.get(2)?;
            let embedding = embedding_blob.map(|blob| decode_embedding(&blob));

            Ok(ChunkRecord {
                start_byte: row.get::<_, i64>(0)? as usize,
                end_byte: row.get::<_, i64>(1)? as usize,
                embedding,
                symbol_path: row.get(3)?,
            })
        })?;

        rows.collect()
    }

    /// Get all chunks with embeddings (for similarity search)
    pub fn get_all_embedded_chunks(&self) -> SqliteResult<Vec<EmbeddedChunkRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT f.path, c.start_byte, c.end_byte, c.embedding, c.symbol_path
            FROM chunks c
            JOIN files f ON c.file_id = f.file_id
            WHERE c.embedding IS NOT NULL
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            let embedding_blob: Vec<u8> = row.get(3)?;

            Ok(EmbeddedChunkRecord {
                path: PathBuf::from(row.get::<_, String>(0)?),
                start_byte: row.get::<_, i64>(1)? as usize,
                end_byte: row.get::<_, i64>(2)? as usize,
                embedding: decode_embedding(&embedding_blob),
                symbol_path: row.get(4)?,
            })
        })?;

        rows.collect()
    }

    /// Remove a file and all its chunks from the index
    pub fn remove_file(&self, path: &Path) -> SqliteResult<bool> {
        let file_id = Self::compute_file_id(path);
        let conn = self.conn();

        // Delete chunks first (in case cascade isn't working)
        conn.execute("DELETE FROM chunks WHERE file_id = ?", [&file_id])?;

        // Delete the file
        let deleted = conn.execute("DELETE FROM files WHERE file_id = ?", [&file_id])?;

        Ok(deleted > 0)
    }

    /// Clear all data from the database
    pub fn clear(&self) -> SqliteResult<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM chunks", [])?;
        conn.execute("DELETE FROM files", [])?;
        Ok(())
    }

    /// Get the content hash for a file
    pub fn get_content_hash(&self, path: &Path) -> SqliteResult<Option<[u8; 16]>> {
        let file_id = Self::compute_file_id(path);
        let conn = self.conn();
        let mut stmt = conn.prepare_cached("SELECT content_hash FROM files WHERE file_id = ?")?;

        let result: Option<Vec<u8>> = stmt.query_row([&file_id], |row| row.get(0)).ok();

        match result {
            Some(hash) if hash.len() == 16 => {
                let mut arr = [0u8; 16];
                arr.copy_from_slice(&hash);
                Ok(Some(arr))
            }
            _ => Ok(None),
        }
    }
}

/// A chunk record from the database
#[derive(Debug, Clone)]
pub struct ChunkRecord {
    /// Start byte offset in the file
    pub start_byte: usize,
    /// End byte offset in the file
    pub end_byte: usize,
    /// Embedding vector (if computed)
    pub embedding: Option<Vec<f32>>,
    /// Symbol path (e.g., "file.rs::MyStruct::method")
    pub symbol_path: String,
}

/// A chunk record with file path (for queries across all files)
#[derive(Debug, Clone)]
pub struct EmbeddedChunkRecord {
    /// File path containing this chunk
    pub path: PathBuf,
    /// Start byte offset in the file
    pub start_byte: usize,
    /// End byte offset in the file
    pub end_byte: usize,
    /// Embedding vector
    pub embedding: Vec<f32>,
    /// Symbol path (e.g., "file.rs::MyStruct::method")
    pub symbol_path: String,
}

/// Determine the database path for a workspace
pub fn database_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(DEFAULT_DB_FILENAME)
}

/// Ensure the root .gitignore contains tree-sitter database entries
///
/// This function checks if the .gitignore file at the workspace root contains
/// the necessary entries for tree-sitter database files. If not, it appends them.
///
/// Entries added:
/// - `.treesitter-index.db`
/// - `.treesitter-index.db-shm`
/// - `.treesitter-index.db-wal`
///
/// # Errors
///
/// Returns an error if the .gitignore file cannot be read or written.
pub fn ensure_root_gitignore(workspace_root: &Path) -> std::io::Result<()> {
    let gitignore_path = workspace_root.join(".gitignore");

    // Required entries for tree-sitter database files
    let required_entries = [
        ".treesitter-index.db",
        ".treesitter-index.db-shm",
        ".treesitter-index.db-wal",
    ];

    // Read existing content or start with empty string
    let existing_content = if gitignore_path.exists() {
        std::fs::read_to_string(&gitignore_path)?
    } else {
        String::new()
    };

    // Check which entries are missing
    let mut missing_entries = Vec::new();
    for entry in &required_entries {
        // Check if entry exists as a line (avoiding partial matches)
        if !existing_content.lines().any(|line| line.trim() == *entry) {
            missing_entries.push(*entry);
        }
    }

    // If all entries exist, nothing to do
    if missing_entries.is_empty() {
        return Ok(());
    }

    // Append missing entries
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&gitignore_path)?;

    // Add a section header if we're adding entries
    if !existing_content.is_empty() && !existing_content.ends_with('\n') {
        writeln!(file)?;
    }

    writeln!(file)?;
    writeln!(file, "# Tree-sitter index database files")?;
    for entry in missing_entries {
        writeln!(file, "{}", entry)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Test content hash for file operations
    const TEST_HASH: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
    /// Zero hash for simple test cases
    const ZERO_HASH: [u8; 16] = [0; 16];

    fn setup_db() -> (TempDir, IndexDatabase) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db = IndexDatabase::open_readwrite(&db_path).unwrap();
        (dir, db)
    }

    #[test]
    fn test_encode_decode_embedding() {
        let original = vec![1.0f32, 2.5, -3.7, 0.0];
        let encoded = encode_embedding(&original);
        let decoded = decode_embedding(&encoded);
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_create_schema() {
        let (_dir, db) = setup_db();
        assert_eq!(db.file_count().unwrap(), 0);
        assert_eq!(db.chunk_count().unwrap(), 0);
    }

    #[test]
    fn test_compute_file_id() {
        let path = Path::new("/test/file.rs");
        let file_id = IndexDatabase::compute_file_id(path);
        assert_eq!(file_id.len(), MD5_HEX_LENGTH);

        // Same path should give same ID
        let file_id2 = IndexDatabase::compute_file_id(path);
        assert_eq!(file_id, file_id2);

        // Different path should give different ID
        let file_id3 = IndexDatabase::compute_file_id(Path::new("/other/file.rs"));
        assert_ne!(file_id, file_id3);
    }

    #[test]
    fn test_upsert_file() {
        let (_dir, db) = setup_db();
        let path = Path::new("/test/file.rs");

        let file_id = db.upsert_file(path, &TEST_HASH).unwrap();
        assert!(!file_id.is_empty());
        assert_eq!(db.file_count().unwrap(), 1);

        // Upsert again should not create duplicate
        db.upsert_file(path, &TEST_HASH).unwrap();
        assert_eq!(db.file_count().unwrap(), 1);
    }

    #[test]
    fn test_file_is_current() {
        let (_dir, db) = setup_db();
        let path = Path::new("/test/file.rs");

        // File doesn't exist yet
        assert!(!db.file_is_current(path, &TEST_HASH).unwrap());

        // Add the file
        db.upsert_file(path, &TEST_HASH).unwrap();

        // Now it should be current
        assert!(db.file_is_current(path, &TEST_HASH).unwrap());

        // Different hash should not be current
        assert!(!db.file_is_current(path, &ZERO_HASH).unwrap());
    }

    #[test]
    fn test_insert_and_get_chunks() {
        let (_dir, db) = setup_db();
        let path = Path::new("/test/file.rs");

        let file_id = db.upsert_file(path, &ZERO_HASH).unwrap();

        // Insert a chunk without embedding
        db.insert_chunk(&file_id, 0, 100, None, "test.rs::main")
            .unwrap();

        // Insert a chunk with embedding
        let embedding = vec![1.0f32, 2.0, 3.0];
        db.insert_chunk(&file_id, 100, 200, Some(&embedding), "test.rs::other")
            .unwrap();

        assert_eq!(db.chunk_count().unwrap(), 2);

        // Get chunks for file
        let chunks = db.get_chunks_for_file(path).unwrap();
        assert_eq!(chunks.len(), 2);

        // Check first chunk (no embedding)
        assert_eq!(chunks[0].start_byte, 0);
        assert_eq!(chunks[0].end_byte, 100);
        assert!(chunks[0].embedding.is_none());
        assert_eq!(chunks[0].symbol_path, "test.rs::main");

        // Check second chunk (with embedding)
        assert_eq!(chunks[1].start_byte, 100);
        assert_eq!(chunks[1].end_byte, 200);
        assert_eq!(
            chunks[1].embedding.as_ref().unwrap(),
            &vec![1.0f32, 2.0, 3.0]
        );
    }

    #[test]
    fn test_get_all_embedded_chunks() {
        let (_dir, db) = setup_db();
        let path1 = Path::new("/test/a.rs");
        let path2 = Path::new("/test/b.rs");

        let file_id1 = db.upsert_file(path1, &ZERO_HASH).unwrap();
        let file_id2 = db.upsert_file(path2, &ZERO_HASH).unwrap();

        // Insert chunks
        let embedding = vec![1.0f32, 2.0, 3.0];
        db.insert_chunk(&file_id1, 0, 100, Some(&embedding), "a.rs::main")
            .unwrap();
        db.insert_chunk(&file_id1, 100, 200, None, "a.rs::no_embed")
            .unwrap(); // No embedding
        db.insert_chunk(&file_id2, 0, 50, Some(&embedding), "b.rs::func")
            .unwrap();

        // Get all embedded chunks
        let embedded = db.get_all_embedded_chunks().unwrap();
        assert_eq!(embedded.len(), 2); // Only chunks with embeddings
    }

    #[test]
    fn test_embedded_chunk_count() {
        let (_dir, db) = setup_db();

        // Empty database should have 0 embedded chunks
        assert_eq!(db.embedded_chunk_count().unwrap(), 0);

        let path = Path::new("/test/file.rs");
        let file_id = db.upsert_file(path, &ZERO_HASH).unwrap();

        // Insert chunk without embedding
        db.insert_chunk(&file_id, 0, 100, None, "test.rs::no_embed")
            .unwrap();
        assert_eq!(db.chunk_count().unwrap(), 1);
        assert_eq!(db.embedded_chunk_count().unwrap(), 0);

        // Insert chunk with embedding
        let embedding = vec![1.0f32, 2.0, 3.0];
        db.insert_chunk(&file_id, 100, 200, Some(&embedding), "test.rs::with_embed")
            .unwrap();
        assert_eq!(db.chunk_count().unwrap(), 2);
        assert_eq!(db.embedded_chunk_count().unwrap(), 1);

        // Insert another chunk with embedding
        db.insert_chunk(
            &file_id,
            200,
            300,
            Some(&embedding),
            "test.rs::also_embedded",
        )
        .unwrap();
        assert_eq!(db.chunk_count().unwrap(), 3);
        assert_eq!(db.embedded_chunk_count().unwrap(), 2);
    }

    #[test]
    fn test_remove_file() {
        let (_dir, db) = setup_db();
        let path = Path::new("/test/file.rs");

        let file_id = db.upsert_file(path, &ZERO_HASH).unwrap();
        db.insert_chunk(&file_id, 0, 100, None, "test").unwrap();

        assert_eq!(db.file_count().unwrap(), 1);
        assert_eq!(db.chunk_count().unwrap(), 1);

        // Remove the file
        let removed = db.remove_file(path).unwrap();
        assert!(removed);

        assert_eq!(db.file_count().unwrap(), 0);
        assert_eq!(db.chunk_count().unwrap(), 0);
    }

    #[test]
    fn test_list_files() {
        let (_dir, db) = setup_db();

        db.upsert_file(Path::new("/test/a.rs"), &ZERO_HASH).unwrap();
        db.upsert_file(Path::new("/test/b.rs"), &ZERO_HASH).unwrap();

        let files = db.list_files().unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_clear() {
        let (_dir, db) = setup_db();
        let path = Path::new("/test/file.rs");

        let file_id = db.upsert_file(path, &ZERO_HASH).unwrap();
        db.insert_chunk(&file_id, 0, 100, None, "test").unwrap();

        db.clear().unwrap();

        assert_eq!(db.file_count().unwrap(), 0);
        assert_eq!(db.chunk_count().unwrap(), 0);
    }

    #[test]
    fn test_upsert_clears_old_chunks() {
        let (_dir, db) = setup_db();
        let path = Path::new("/test/file.rs");

        let file_id = db.upsert_file(path, &ZERO_HASH).unwrap();
        db.insert_chunk(&file_id, 0, 100, None, "old_chunk")
            .unwrap();

        assert_eq!(db.chunk_count().unwrap(), 1);

        // Upsert same file again (with different hash)
        let new_hash: [u8; 16] = [1; 16];
        db.upsert_file(path, &new_hash).unwrap();

        // Old chunks should be deleted
        assert_eq!(db.chunk_count().unwrap(), 0);
    }

    #[test]
    fn test_readonly_mode() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");

        // First create the database in read-write mode
        {
            let db = IndexDatabase::open_readwrite(&db_path).unwrap();
            db.upsert_file(Path::new("/test/file.rs"), &ZERO_HASH)
                .unwrap();
        }

        // Now open in read-only mode
        let db = IndexDatabase::open_readonly(&db_path).unwrap();
        assert!(db.is_readonly());

        // Should be able to read
        assert_eq!(db.file_count().unwrap(), 1);
    }

    #[test]
    fn test_database_path() {
        let root = Path::new("/workspace/project");
        let db_path = database_path(root);
        assert_eq!(
            db_path,
            PathBuf::from("/workspace/project/.treesitter-index.db")
        );
    }

    #[test]
    fn test_transaction() {
        let (_dir, db) = setup_db();
        let path = Path::new("/test/file.rs");

        db.begin_transaction().unwrap();
        db.upsert_file(path, &ZERO_HASH).unwrap();
        db.commit_transaction().unwrap();

        assert_eq!(db.file_count().unwrap(), 1);
    }

    #[test]
    fn test_transaction_rollback() {
        let (_dir, db) = setup_db();
        let path = Path::new("/test/file.rs");

        db.begin_transaction().unwrap();
        db.upsert_file(path, &ZERO_HASH).unwrap();
        db.rollback_transaction().unwrap();

        assert_eq!(db.file_count().unwrap(), 0);
    }

    #[test]
    fn test_ensure_root_gitignore_creates_new() {
        let temp = TempDir::new().unwrap();
        let gitignore_path = temp.path().join(".gitignore");

        // Ensure .gitignore doesn't exist yet
        assert!(!gitignore_path.exists());

        // Run ensure_root_gitignore
        ensure_root_gitignore(temp.path()).unwrap();

        // Verify .gitignore was created with all entries
        assert!(gitignore_path.exists());
        let content = std::fs::read_to_string(&gitignore_path).unwrap();
        assert!(content.contains("# Tree-sitter index database files"));
        assert!(content.contains(".treesitter-index.db"));
        assert!(content.contains(".treesitter-index.db-shm"));
        assert!(content.contains(".treesitter-index.db-wal"));
    }

    #[test]
    fn test_ensure_root_gitignore_appends_missing() {
        let temp = TempDir::new().unwrap();
        let gitignore_path = temp.path().join(".gitignore");

        // Create .gitignore with some existing content
        std::fs::write(&gitignore_path, "# Existing content\n*.log\n").unwrap();

        // Run ensure_root_gitignore
        ensure_root_gitignore(temp.path()).unwrap();

        // Verify existing content is preserved and new entries added
        let content = std::fs::read_to_string(&gitignore_path).unwrap();
        assert!(content.contains("# Existing content"));
        assert!(content.contains("*.log"));
        assert!(content.contains("# Tree-sitter index database files"));
        assert!(content.contains(".treesitter-index.db"));
        assert!(content.contains(".treesitter-index.db-shm"));
        assert!(content.contains(".treesitter-index.db-wal"));
    }

    #[test]
    fn test_ensure_root_gitignore_idempotent() {
        let temp = TempDir::new().unwrap();
        let gitignore_path = temp.path().join(".gitignore");

        // Create .gitignore with tree-sitter entries already present
        let initial_content = r#"# Existing
*.log

# Tree-sitter index database files
.treesitter-index.db
.treesitter-index.db-shm
.treesitter-index.db-wal
"#;
        std::fs::write(&gitignore_path, initial_content).unwrap();

        // Run ensure_root_gitignore
        ensure_root_gitignore(temp.path()).unwrap();

        // Verify content hasn't changed (idempotent)
        let content = std::fs::read_to_string(&gitignore_path).unwrap();
        assert_eq!(content, initial_content);
    }

    #[test]
    fn test_ensure_root_gitignore_partial_entries() {
        let temp = TempDir::new().unwrap();
        let gitignore_path = temp.path().join(".gitignore");

        // Create .gitignore with only one of the entries
        std::fs::write(&gitignore_path, "# Existing\n.treesitter-index.db\n").unwrap();

        // Run ensure_root_gitignore
        ensure_root_gitignore(temp.path()).unwrap();

        // Verify missing entries were added
        let content = std::fs::read_to_string(&gitignore_path).unwrap();
        assert!(content.contains(".treesitter-index.db"));
        assert!(content.contains(".treesitter-index.db-shm"));
        assert!(content.contains(".treesitter-index.db-wal"));

        // Count occurrences - should only have one .treesitter-index.db
        let db_count = content.lines().filter(|l| l.trim() == ".treesitter-index.db").count();
        assert_eq!(db_count, 1, "Should not duplicate existing entries");
    }
}
