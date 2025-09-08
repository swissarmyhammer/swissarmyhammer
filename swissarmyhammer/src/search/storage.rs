//! DuckDB vector storage for semantic search
//!
//! This module provides a DuckDB-based vector storage implementation for code chunks
//! and their embeddings. It supports efficient vector similarity search using cosine
//! similarity and manages the database schema for semantic search operations.

use crate::error::{Result, SwissArmyHammerError};
use crate::search::{
    types::{
        CodeChunk, ContentHash, Embedding, IndexStats, IndexedFile, Language, SemanticSearchResult,
    },
    utils::SemanticUtils,
    SemanticConfig,
};
use duckdb::{Connection, ToSql};
use serde_json;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing;

/// Vector storage for code chunks and embeddings using DuckDB
///
/// This implementation provides persistent storage operations using DuckDB for
/// semantic search functionality with vector similarity search capabilities.
pub struct VectorStorage {
    db_path: PathBuf,
    _config: SemanticConfig,
    /// DuckDB connection for persistent storage
    connection: Arc<Mutex<Connection>>,
}

impl Clone for VectorStorage {
    fn clone(&self) -> Self {
        Self {
            db_path: self.db_path.clone(),
            _config: self._config.clone(),
            connection: Arc::clone(&self.connection),
        }
    }
}

impl VectorStorage {
    // SQL schema constants
    const CREATE_INDEXED_FILES_TABLE: &'static str = r#"
        CREATE TABLE IF NOT EXISTS indexed_files (
            file_id TEXT PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            language TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            chunk_count INTEGER NOT NULL,
            indexed_at TIMESTAMP NOT NULL
        )
    "#;

    const CREATE_CODE_CHUNKS_TABLE: &'static str = r#"
        CREATE TABLE IF NOT EXISTS code_chunks (
            chunk_id TEXT PRIMARY KEY,
            file_path TEXT NOT NULL,
            language TEXT NOT NULL,
            content TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            chunk_type TEXT NOT NULL,
            content_hash TEXT NOT NULL
        )
    "#;

    const CREATE_EMBEDDINGS_TABLE: &'static str = r#"
        CREATE TABLE IF NOT EXISTS embeddings (
            chunk_id TEXT PRIMARY KEY,
            vector TEXT NOT NULL,
            FOREIGN KEY (chunk_id) REFERENCES code_chunks(chunk_id)
        )
    "#;

    const CREATE_FILE_PATH_INDEX: &'static str =
        "CREATE INDEX IF NOT EXISTS idx_chunks_file_path ON code_chunks(file_path)";

    const CREATE_LANGUAGE_INDEX: &'static str =
        "CREATE INDEX IF NOT EXISTS idx_chunks_language ON code_chunks(language)";

    const CREATE_PATH_INDEX: &'static str =
        "CREATE INDEX IF NOT EXISTS idx_files_path ON indexed_files(path)";

    /// Create a new vector storage instance
    pub fn new(config: SemanticConfig) -> Result<Self> {
        let db_path = config.database_path.clone();

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Create DuckDB connection
        let connection = Connection::open(&db_path).map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to open DuckDB connection: {e}"))
        })?;

        Ok(Self {
            db_path,
            _config: config,
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    /// Initialize the database schema
    pub fn initialize(&self) -> Result<()> {
        tracing::info!(
            "Initializing DuckDB vector storage at: {}",
            self.db_path.display()
        );

        let conn = self.connection.lock().map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to acquire connection lock: {e}"))
        })?;

        // Create indexed_files table
        conn.execute(Self::CREATE_INDEXED_FILES_TABLE, [])
            .map_err(|e| {
                SwissArmyHammerError::Storage(format!("Failed to create indexed_files table: {e}"))
            })?;

        // Create code_chunks table
        conn.execute(Self::CREATE_CODE_CHUNKS_TABLE, [])
            .map_err(|e| {
                SwissArmyHammerError::Storage(format!("Failed to create code_chunks table: {e}"))
            })?;

        // Create embeddings table
        conn.execute(Self::CREATE_EMBEDDINGS_TABLE, [])
            .map_err(|e| {
                SwissArmyHammerError::Storage(format!("Failed to create embeddings table: {e}"))
            })?;

        // Create indexes for better performance
        conn.execute(Self::CREATE_FILE_PATH_INDEX, [])
            .map_err(|e| {
                SwissArmyHammerError::Storage(format!("Failed to create file_path index: {e}"))
            })?;

        conn.execute(Self::CREATE_LANGUAGE_INDEX, []).map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to create language index: {e}"))
        })?;

        conn.execute(Self::CREATE_PATH_INDEX, []).map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to create path index: {e}"))
        })?;

        tracing::info!("Database schema initialized successfully");
        Ok(())
    }

    /// Store indexed file metadata
    pub fn store_indexed_file(&self, file: &IndexedFile) -> Result<()> {
        tracing::debug!("Storing indexed file: {}", file.path.display());

        let conn = self.connection.lock().map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to acquire connection lock: {e}"))
        })?;

        conn.execute(
            r#"
            INSERT INTO indexed_files 
            (file_id, path, language, content_hash, chunk_count, indexed_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(path) DO UPDATE SET
                language = excluded.language,
                content_hash = excluded.content_hash,
                chunk_count = excluded.chunk_count,
                indexed_at = excluded.indexed_at
            "#,
            [
                &file.file_id.0 as &dyn ToSql,
                &file.path.to_string_lossy(),
                &format!("{:?}", file.language),
                &file.content_hash.0,
                &file.chunk_count as &dyn ToSql,
                &file.indexed_at.to_rfc3339(),
            ],
        )
        .map_err(|e| SwissArmyHammerError::Storage(format!("Failed to store indexed file: {e}")))?;

        tracing::debug!("Successfully stored indexed file: {}", file.path.display());
        Ok(())
    }

    /// Store a code chunk
    pub fn store_chunk(&self, chunk: &CodeChunk) -> Result<()> {
        tracing::debug!("Storing chunk: {}", chunk.id);

        let conn = self.connection.lock().map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to acquire connection lock: {e}"))
        })?;

        conn.execute(
            r#"
            INSERT OR REPLACE INTO code_chunks 
            (chunk_id, file_path, language, content, start_line, end_line, chunk_type, content_hash)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            [
                &chunk.id as &dyn ToSql,
                &chunk.file_path.to_string_lossy(),
                &format!("{:?}", chunk.language),
                &chunk.content,
                &chunk.start_line as &dyn ToSql,
                &chunk.end_line as &dyn ToSql,
                &format!("{:?}", chunk.chunk_type),
                &chunk.content_hash.0,
            ],
        )
        .map_err(|e| SwissArmyHammerError::Storage(format!("Failed to store chunk: {e}")))?;

        tracing::debug!("Successfully stored chunk: {}", chunk.id);
        Ok(())
    }

    /// Store an embedding for a code chunk
    pub fn store_embedding(&self, embedding: &Embedding) -> Result<()> {
        tracing::debug!("Storing embedding for chunk: {}", embedding.chunk_id);

        let conn = self.connection.lock().map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to acquire connection lock: {e}"))
        })?;

        // Convert vector to JSON for storage
        let vector_str = serde_json::to_string(&embedding.vector).map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to serialize vector: {e}"))
        })?;

        conn.execute(
            "INSERT OR REPLACE INTO embeddings (chunk_id, vector) VALUES (?, ?)",
            [&embedding.chunk_id as &dyn ToSql, &vector_str],
        )
        .map_err(|e| SwissArmyHammerError::Storage(format!("Failed to store embedding: {e}")))?;

        tracing::debug!(
            "Successfully stored embedding for chunk: {}",
            embedding.chunk_id
        );
        Ok(())
    }

    /// Search for similar chunks using vector similarity
    pub fn similarity_search(
        &self,
        query_embedding: &[f32],
        limit: usize,
        threshold: f32,
    ) -> Result<Vec<SemanticSearchResult>> {
        tracing::debug!(
            "Searching for similar embeddings: query_dim={}, limit={}, threshold={}",
            query_embedding.len(),
            limit,
            threshold
        );

        let conn = self.connection.lock().map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to acquire connection lock: {e}"))
        })?;

        // Query to get all embeddings with their corresponding chunks
        let mut stmt = conn.prepare(
            r#"
            SELECT 
                e.chunk_id, e.vector,
                c.file_path, c.language, c.content, c.start_line, c.end_line, c.chunk_type, c.content_hash
            FROM embeddings e
            JOIN code_chunks c ON e.chunk_id = c.chunk_id
            "#
        ).map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to prepare similarity search query: {e}"))
        })?;

        let rows = stmt
            .query_map([], |row| {
                let chunk_id: String = row.get(0)?;
                let vector_str: String = row.get(1)?;
                let file_path: String = row.get(2)?;
                let language_str: String = row.get(3)?;
                let content: String = row.get(4)?;
                let start_line: i64 = row.get(5)?;
                let end_line: i64 = row.get(6)?;
                let chunk_type_str: String = row.get(7)?;
                let content_hash: String = row.get(8)?;

                // Parse vector from JSON with robust handling for corrupted data
                let vector = match serde_json::from_str::<Vec<f32>>(&vector_str) {
                    Ok(vec) => Some(vec),
                    Err(_e) => {
                        // Try to parse as a single float and convert to a vector
                        match serde_json::from_str::<f32>(&vector_str) {
                            Ok(single_float) => {
                                tracing::warn!(
                                    "Found corrupted vector data stored as single float instead of array for chunk {}: {}. Skipping this chunk.",
                                    chunk_id,
                                    single_float
                                );
                                // Return None to skip this corrupted record
                                None
                            }
                            Err(_) => {
                                // Neither array nor single float, this is truly corrupted
                                tracing::error!(
                                    "Corrupted vector data for chunk {}: {}. Skipping this chunk.",
                                    chunk_id,
                                    vector_str
                                );
                                None
                            }
                        }
                    }
                };

                // If vector parsing failed, skip this record
                let vector = match vector {
                    Some(v) => v,
                    None => return Ok(None), // Skip this record
                };

                // Parse language and chunk type
                let language = Self::parse_language(&language_str);
                let chunk_type = Self::parse_chunk_type(&chunk_type_str);

                Ok(Some((
                    chunk_id,
                    vector,
                    file_path,
                    language,
                    content,
                    start_line,
                    end_line,
                    chunk_type,
                    content_hash,
                )))
            })
            .map_err(|e| {
                SwissArmyHammerError::Storage(format!(
                    "Failed to execute similarity search query: {e}"
                ))
            })?;

        let mut results = Vec::new();

        // Calculate similarity for each embedding
        for row_result in rows {
            let row_data = row_result.map_err(|e| {
                SwissArmyHammerError::Storage(format!("Failed to process row: {e}"))
            })?;

            // Skip corrupted records that returned None
            let (
                chunk_id,
                vector,
                file_path,
                language,
                content,
                start_line,
                end_line,
                chunk_type,
                content_hash,
            ) = match row_data {
                Some(data) => data,
                None => continue, // Skip corrupted records
            };

            let similarity = SemanticUtils::cosine_similarity(query_embedding, &vector);

            if similarity >= threshold {
                let chunk = CodeChunk {
                    id: chunk_id,
                    file_path: PathBuf::from(file_path),
                    language,
                    content: content.clone(),
                    start_line: start_line as usize,
                    end_line: end_line as usize,
                    chunk_type,
                    content_hash: ContentHash(content_hash),
                };

                results.push(SemanticSearchResult {
                    chunk,
                    similarity_score: similarity,
                    excerpt: content,
                });
            }
        }

        // Sort by similarity (descending) and limit results
        results.sort_by(|a, b| {
            b.similarity_score
                .partial_cmp(&a.similarity_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        tracing::debug!("Found {} similar chunks", results.len());
        Ok(results)
    }

    /// Search for similar chunks with detailed embedding information for debugging
    pub fn similarity_search_with_details(
        &self,
        query_embedding: &[f32],
        limit: usize,
        threshold: f32,
    ) -> Result<Vec<(String, f32, Vec<f32>)>> {
        tracing::debug!(
            "Searching for similar embeddings with details: query_dim={}, limit={}, threshold={}",
            query_embedding.len(),
            limit,
            threshold
        );

        let conn = self.connection.lock().map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to acquire connection lock: {e}"))
        })?;

        let mut stmt = conn
            .prepare("SELECT chunk_id, vector FROM embeddings")
            .map_err(|e| {
                SwissArmyHammerError::Storage(format!(
                    "Failed to prepare detailed search query: {e}"
                ))
            })?;

        let rows = stmt
            .query_map([], |row| {
                let chunk_id: String = row.get(0)?;
                let vector_str: String = row.get(1)?;

                // Parse vector from JSON with robust handling for corrupted data
                let vector = match serde_json::from_str::<Vec<f32>>(&vector_str) {
                    Ok(vec) => Some(vec),
                    Err(_e) => {
                        // Try to parse as a single float and convert to a vector
                        match serde_json::from_str::<f32>(&vector_str) {
                            Ok(single_float) => {
                                tracing::warn!(
                                    "Found corrupted vector data stored as single float instead of array for chunk {}: {}. Skipping this chunk.",
                                    chunk_id,
                                    single_float
                                );
                                // Return None to skip this corrupted record
                                None
                            }
                            Err(_) => {
                                // Neither array nor single float, this is truly corrupted
                                tracing::error!(
                                    "Corrupted vector data for chunk {}: {}. Skipping this chunk.",
                                    chunk_id,
                                    vector_str
                                );
                                None
                            }
                        }
                    }
                };

                // If vector parsing failed, skip this record
                let vector = match vector {
                    Some(v) => v,
                    None => return Ok(None), // Skip this record
                };

                Ok(Some((chunk_id, vector)))
            })
            .map_err(|e| {
                SwissArmyHammerError::Storage(format!(
                    "Failed to execute detailed search query: {e}"
                ))
            })?;

        let mut results = Vec::new();

        // Calculate similarity for each embedding
        for row_result in rows {
            let row_data = row_result.map_err(|e| {
                SwissArmyHammerError::Storage(format!("Failed to process detailed search row: {e}"))
            })?;

            // Skip corrupted records that returned None
            let (chunk_id, vector) = match row_data {
                Some(data) => data,
                None => continue, // Skip corrupted records
            };

            let similarity = SemanticUtils::cosine_similarity(query_embedding, &vector);

            if similarity >= threshold {
                results.push((chunk_id, similarity, vector));
            }
        }

        // Sort by similarity (descending) and limit results
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        tracing::debug!("Found {} similar chunks with details", results.len());
        Ok(results)
    }

    /// Get chunk by ID
    pub fn get_chunk(&self, chunk_id: &str) -> Result<Option<CodeChunk>> {
        tracing::debug!("Getting chunk: {}", chunk_id);

        let conn = self.connection.lock().map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to acquire connection lock: {e}"))
        })?;

        let mut stmt = conn.prepare(
            "SELECT chunk_id, file_path, language, content, start_line, end_line, chunk_type, content_hash FROM code_chunks WHERE chunk_id = ?"
        ).map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to prepare get_chunk query: {e}"))
        })?;

        let mut rows = stmt
            .query_map([chunk_id], |row| {
                let chunk_id: String = row.get(0)?;
                let file_path: String = row.get(1)?;
                let language_str: String = row.get(2)?;
                let content: String = row.get(3)?;
                let start_line: i64 = row.get(4)?;
                let end_line: i64 = row.get(5)?;
                let chunk_type_str: String = row.get(6)?;
                let content_hash: String = row.get(7)?;

                // Parse language and chunk type
                let language = Self::parse_language(&language_str);
                let chunk_type = Self::parse_chunk_type(&chunk_type_str);

                Ok(CodeChunk {
                    id: chunk_id,
                    file_path: PathBuf::from(file_path),
                    language,
                    content,
                    start_line: start_line as usize,
                    end_line: end_line as usize,
                    chunk_type,
                    content_hash: ContentHash(content_hash),
                })
            })
            .map_err(|e| {
                SwissArmyHammerError::Storage(format!("Failed to execute get_chunk query: {e}"))
            })?;

        match rows.next() {
            Some(Ok(chunk)) => Ok(Some(chunk)),
            Some(Err(e)) => Err(SwissArmyHammerError::Storage(format!(
                "Failed to parse chunk: {e}"
            ))),
            None => Ok(None),
        }
    }

    /// Get all chunks for testing purposes
    #[cfg(test)]
    pub fn get_all_chunks(&self) -> Result<std::collections::HashMap<String, CodeChunk>> {
        let conn = self.connection.lock().map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to acquire connection lock: {e}"))
        })?;

        let mut stmt = conn.prepare(
            "SELECT chunk_id, file_path, language, content, start_line, end_line, chunk_type, content_hash FROM code_chunks"
        ).map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to prepare get_all_chunks query: {e}"))
        })?;

        let rows = stmt
            .query_map([], |row| {
                let chunk_id: String = row.get(0)?;
                let file_path: String = row.get(1)?;
                let language_str: String = row.get(2)?;
                let content: String = row.get(3)?;
                let start_line: i64 = row.get(4)?;
                let end_line: i64 = row.get(5)?;
                let chunk_type_str: String = row.get(6)?;
                let content_hash: String = row.get(7)?;

                // Parse language and chunk type
                let language = Self::parse_language(&language_str);
                let chunk_type = Self::parse_chunk_type(&chunk_type_str);

                Ok((
                    chunk_id.clone(),
                    CodeChunk {
                        id: chunk_id,
                        file_path: PathBuf::from(file_path),
                        language,
                        content,
                        start_line: start_line as usize,
                        end_line: end_line as usize,
                        chunk_type,
                        content_hash: ContentHash(content_hash),
                    },
                ))
            })
            .map_err(|e| {
                SwissArmyHammerError::Storage(format!(
                    "Failed to execute get_all_chunks query: {e}"
                ))
            })?;

        let mut chunks = std::collections::HashMap::new();
        for row_result in rows {
            let (chunk_id, chunk) = row_result.map_err(|e| {
                SwissArmyHammerError::Storage(format!("Failed to parse chunk: {e}"))
            })?;
            chunks.insert(chunk_id, chunk);
        }

        Ok(chunks)
    }

    /// Get all chunks for a file
    pub fn get_file_chunks(&self, file_path: &Path) -> Result<Vec<CodeChunk>> {
        let conn = self.connection.lock().map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to acquire connection lock: {e}"))
        })?;

        let mut stmt = conn.prepare(
            "SELECT chunk_id, file_path, language, content, start_line, end_line, chunk_type, content_hash FROM code_chunks WHERE file_path = ?"
        ).map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to prepare get_file_chunks query: {e}"))
        })?;

        let file_path_str = file_path.to_string_lossy();
        let rows = stmt
            .query_map([&file_path_str as &dyn ToSql], |row| {
                let chunk_id: String = row.get(0)?;
                let file_path: String = row.get(1)?;
                let language_str: String = row.get(2)?;
                let content: String = row.get(3)?;
                let start_line: i64 = row.get(4)?;
                let end_line: i64 = row.get(5)?;
                let chunk_type_str: String = row.get(6)?;
                let content_hash: String = row.get(7)?;

                // Parse language and chunk type
                let language = Self::parse_language(&language_str);
                let chunk_type = Self::parse_chunk_type(&chunk_type_str);

                Ok(CodeChunk {
                    id: chunk_id,
                    file_path: PathBuf::from(file_path),
                    language,
                    content,
                    start_line: start_line as usize,
                    end_line: end_line as usize,
                    chunk_type,
                    content_hash: ContentHash(content_hash),
                })
            })
            .map_err(|e| {
                SwissArmyHammerError::Storage(format!(
                    "Failed to execute get_file_chunks query: {e}"
                ))
            })?;

        let mut file_chunks = Vec::new();
        for row_result in rows {
            let chunk = row_result.map_err(|e| {
                SwissArmyHammerError::Storage(format!("Failed to parse file chunk: {e}"))
            })?;
            file_chunks.push(chunk);
        }

        tracing::debug!(
            "Found {} chunks for file: {}",
            file_chunks.len(),
            file_path.display()
        );
        Ok(file_chunks)
    }

    /// Check if file needs re-indexing based on content hash
    pub fn needs_reindexing(&self, file_path: &Path, current_hash: &ContentHash) -> Result<bool> {
        tracing::debug!("Checking if file needs reindexing: {}", file_path.display());

        // Get the stored hash for this file
        match self.get_file_hash(file_path)? {
            Some(stored_hash) => {
                // File exists in index - compare hashes
                let needs_reindexing = stored_hash != *current_hash;
                if needs_reindexing {
                    tracing::debug!("File {} has changed (hash mismatch)", file_path.display());
                } else {
                    tracing::debug!("File {} unchanged (hash match)", file_path.display());
                }
                Ok(needs_reindexing)
            }
            None => {
                // File not in index - needs indexing
                tracing::debug!("File {} not in index - needs indexing", file_path.display());
                Ok(true)
            }
        }
    }

    /// Check if a file has been indexed
    pub fn is_file_indexed(&self, file_path: &Path) -> Result<bool> {
        tracing::debug!("Checking if file is indexed: {}", file_path.display());

        let conn = self.connection.lock().map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to acquire connection lock: {e}"))
        })?;

        let mut stmt = conn
            .prepare("SELECT 1 FROM indexed_files WHERE path = ? LIMIT 1")
            .map_err(|e| {
                SwissArmyHammerError::Storage(format!(
                    "Failed to prepare is_file_indexed query: {e}"
                ))
            })?;

        let file_path_str = file_path.to_string_lossy();
        let mut rows = stmt
            .query_map([&file_path_str as &dyn ToSql], |_row| Ok(true))
            .map_err(|e| {
                SwissArmyHammerError::Storage(format!(
                    "Failed to execute is_file_indexed query: {e}"
                ))
            })?;

        Ok(rows.next().is_some())
    }

    /// Remove all data for a file (for re-indexing)
    pub fn remove_file(&self, file_path: &Path) -> Result<()> {
        tracing::debug!("Removing file: {}", file_path.display());

        let conn = self.connection.lock().map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to acquire connection lock: {e}"))
        })?;

        let file_path_str = file_path.to_string_lossy();

        // Remove embeddings for chunks in this file (CASCADE should handle this, but let's be explicit)
        conn.execute(
            "DELETE FROM embeddings WHERE chunk_id IN (SELECT chunk_id FROM code_chunks WHERE file_path = ?)",
            [&file_path_str as &dyn ToSql],
        ).map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to remove embeddings for file: {e}"))
        })?;

        // Remove chunks for this file
        let removed_chunks = conn
            .execute(
                "DELETE FROM code_chunks WHERE file_path = ?",
                [&file_path_str as &dyn ToSql],
            )
            .map_err(|e| {
                SwissArmyHammerError::Storage(format!("Failed to remove chunks for file: {e}"))
            })?;

        // Remove indexed file metadata
        conn.execute(
            "DELETE FROM indexed_files WHERE path = ?",
            [&file_path_str as &dyn ToSql],
        )
        .map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to remove indexed file metadata: {e}"))
        })?;

        tracing::debug!(
            "Removed {} chunks for file: {}",
            removed_chunks,
            file_path.display()
        );
        Ok(())
    }

    /// Get statistics about the stored data
    pub fn get_stats(&self) -> Result<StorageStats> {
        tracing::debug!("Getting storage statistics");

        let conn = self.connection.lock().map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to acquire connection lock: {e}"))
        })?;

        // Get total chunks count
        let total_chunks: usize = conn
            .query_row("SELECT COUNT(*) FROM code_chunks", [], |row| {
                let count: i64 = row.get(0)?;
                Ok(count as usize)
            })
            .map_err(|e| {
                SwissArmyHammerError::Storage(format!("Failed to get chunk count: {e}"))
            })?;

        // Get total files count
        let total_files: usize = conn
            .query_row("SELECT COUNT(*) FROM indexed_files", [], |row| {
                let count: i64 = row.get(0)?;
                Ok(count as usize)
            })
            .map_err(|e| SwissArmyHammerError::Storage(format!("Failed to get file count: {e}")))?;

        // Get total embeddings count
        let total_embeddings: usize = conn
            .query_row("SELECT COUNT(*) FROM embeddings", [], |row| {
                let count: i64 = row.get(0)?;
                Ok(count as usize)
            })
            .map_err(|e| {
                SwissArmyHammerError::Storage(format!("Failed to get embedding count: {e}"))
            })?;

        // Get database file size
        let database_size_bytes = std::fs::metadata(&self.db_path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);

        tracing::debug!(
            "Storage stats: {} chunks, {} files, {} embeddings, {} bytes",
            total_chunks,
            total_files,
            total_embeddings,
            database_size_bytes
        );

        Ok(StorageStats {
            total_chunks,
            total_files,
            total_embeddings,
            database_size_bytes,
        })
    }

    /// Get chunks by language
    pub fn get_chunks_by_language(&self, language: &Language) -> Result<Vec<CodeChunk>> {
        tracing::debug!("Getting chunks by language: {:?}", language);

        let conn = self.connection.lock().map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to acquire connection lock: {e}"))
        })?;

        let language_str = format!("{language:?}");
        let mut stmt = conn.prepare(
            "SELECT chunk_id, file_path, language, content, start_line, end_line, chunk_type, content_hash FROM code_chunks WHERE language = ?"
        ).map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to prepare get_chunks_by_language query: {e}"))
        })?;

        let rows = stmt
            .query_map([&language_str as &dyn ToSql], |row| {
                let chunk_id: String = row.get(0)?;
                let file_path: String = row.get(1)?;
                let language_str: String = row.get(2)?;
                let content: String = row.get(3)?;
                let start_line: i64 = row.get(4)?;
                let end_line: i64 = row.get(5)?;
                let chunk_type_str: String = row.get(6)?;
                let content_hash: String = row.get(7)?;

                // Parse language and chunk type
                let language = Self::parse_language(&language_str);
                let chunk_type = Self::parse_chunk_type(&chunk_type_str);

                Ok(CodeChunk {
                    id: chunk_id,
                    file_path: PathBuf::from(file_path),
                    language,
                    content,
                    start_line: start_line as usize,
                    end_line: end_line as usize,
                    chunk_type,
                    content_hash: ContentHash(content_hash),
                })
            })
            .map_err(|e| {
                SwissArmyHammerError::Storage(format!(
                    "Failed to execute get_chunks_by_language query: {e}"
                ))
            })?;

        let mut language_chunks = Vec::new();
        for row_result in rows {
            let chunk = row_result.map_err(|e| {
                SwissArmyHammerError::Storage(format!("Failed to parse chunk by language: {e}"))
            })?;
            language_chunks.push(chunk);
        }

        tracing::debug!(
            "Found {} chunks for language: {:?}",
            language_chunks.len(),
            language
        );
        Ok(language_chunks)
    }

    /// Perform database maintenance (vacuum, analyze, etc.)
    pub fn maintenance(&self) -> Result<()> {
        tracing::info!("Performing database maintenance");

        let conn = self.connection.lock().map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to acquire connection lock: {e}"))
        })?;

        // Perform VACUUM to reclaim space and optimize storage
        tracing::debug!("Running VACUUM operation");
        conn.execute("VACUUM", [])
            .map_err(|e| SwissArmyHammerError::Storage(format!("Failed to run VACUUM: {e}")))?;

        // Perform ANALYZE to update table statistics for query optimization
        tracing::debug!("Running ANALYZE operation");
        conn.execute("ANALYZE", [])
            .map_err(|e| SwissArmyHammerError::Storage(format!("Failed to run ANALYZE: {e}")))?;

        // Also analyze specific tables for better statistics
        for table in &["indexed_files", "code_chunks", "embeddings"] {
            tracing::debug!("Running ANALYZE on table: {}", table);
            let analyze_sql = format!("ANALYZE {table}");
            conn.execute(&analyze_sql, []).map_err(|e| {
                SwissArmyHammerError::Storage(format!("Failed to analyze table {table}: {e}"))
            })?;
        }

        tracing::info!("Database maintenance completed successfully");
        Ok(())
    }

    /// Check if file exists in index
    pub fn file_exists(&self, file_path: &Path) -> Result<bool> {
        // This is the same as is_file_indexed, so just call that method
        self.is_file_indexed(file_path)
    }

    /// Get file hash from index
    pub fn get_file_hash(&self, file_path: &Path) -> Result<Option<ContentHash>> {
        let conn = self.connection.lock().map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to acquire connection lock: {e}"))
        })?;

        let mut stmt = conn
            .prepare("SELECT content_hash FROM indexed_files WHERE path = ?")
            .map_err(|e| {
                SwissArmyHammerError::Storage(format!("Failed to prepare get_file_hash query: {e}"))
            })?;

        let file_path_str = file_path.to_string_lossy();
        let mut rows = stmt
            .query_map([&file_path_str as &dyn ToSql], |row| {
                let content_hash: String = row.get(0)?;
                Ok(ContentHash(content_hash))
            })
            .map_err(|e| {
                SwissArmyHammerError::Storage(format!("Failed to execute get_file_hash query: {e}"))
            })?;

        match rows.next() {
            Some(Ok(hash)) => Ok(Some(hash)),
            Some(Err(e)) => Err(SwissArmyHammerError::Storage(format!(
                "Failed to parse file hash: {e}"
            ))),
            None => Ok(None),
        }
    }

    /// Get statistics about indexed files
    pub fn get_index_stats(&self) -> Result<IndexStats> {
        let conn = self.connection.lock().map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to acquire connection lock: {e}"))
        })?;

        // Get file count
        let file_count: usize = conn
            .query_row("SELECT COUNT(*) FROM indexed_files", [], |row| {
                let count: i64 = row.get(0)?;
                Ok(count as usize)
            })
            .map_err(|e| SwissArmyHammerError::Storage(format!("Failed to get file count: {e}")))?;

        // Get chunk count
        let chunk_count: usize = conn
            .query_row("SELECT COUNT(*) FROM code_chunks", [], |row| {
                let count: i64 = row.get(0)?;
                Ok(count as usize)
            })
            .map_err(|e| {
                SwissArmyHammerError::Storage(format!("Failed to get chunk count: {e}"))
            })?;

        // Get embedding count
        let embedding_count: usize = conn
            .query_row("SELECT COUNT(*) FROM embeddings", [], |row| {
                let count: i64 = row.get(0)?;
                Ok(count as usize)
            })
            .map_err(|e| {
                SwissArmyHammerError::Storage(format!("Failed to get embedding count: {e}"))
            })?;

        Ok(IndexStats {
            file_count,
            chunk_count,
            embedding_count,
        })
    }

    /// Store indexed file (for internal use in testing/development)
    pub fn store_indexed_file_internal(&self, file: &IndexedFile) -> Result<()> {
        // This is the same as store_indexed_file, so just call that method
        self.store_indexed_file(file)
    }

    /// Store multiple chunks and embeddings in a single transaction
    pub fn store_chunks_and_embeddings_transaction(
        &self,
        chunks: &[CodeChunk],
        embeddings: &[Embedding],
    ) -> Result<()> {
        tracing::debug!(
            "Storing {} chunks and {} embeddings in transaction",
            chunks.len(),
            embeddings.len()
        );

        let conn = self.connection.lock().map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to acquire connection lock: {e}"))
        })?;

        // Begin transaction
        conn.execute("BEGIN TRANSACTION", []).map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to begin transaction: {e}"))
        })?;

        // Store chunks
        for chunk in chunks {
            if let Err(e) = conn.execute(
                r#"
                INSERT OR REPLACE INTO code_chunks 
                (chunk_id, file_path, language, content, start_line, end_line, chunk_type, content_hash)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                "#,
                [
                    &chunk.id as &dyn ToSql,
                    &chunk.file_path.to_string_lossy(),
                    &format!("{:?}", chunk.language),
                    &chunk.content,
                    &chunk.start_line as &dyn ToSql,
                    &chunk.end_line as &dyn ToSql,
                    &format!("{:?}", chunk.chunk_type),
                    &chunk.content_hash.0,
                ],
            ) {
                // Rollback on error
                let _ = conn.execute("ROLLBACK", []);
                return Err(SwissArmyHammerError::Storage(format!(
                    "Failed to store chunk in transaction: {e}"
                )));
            }
        }

        // Store embeddings
        for embedding in embeddings {
            let vector_str = serde_json::to_string(&embedding.vector).map_err(|e| {
                // Rollback on error
                let _ = conn.execute("ROLLBACK", []);
                SwissArmyHammerError::Storage(format!("Failed to serialize vector: {e}"))
            })?;

            if let Err(e) = conn.execute(
                "INSERT OR REPLACE INTO embeddings (chunk_id, vector) VALUES (?, ?)",
                [&embedding.chunk_id as &dyn ToSql, &vector_str],
            ) {
                // Rollback on error
                let _ = conn.execute("ROLLBACK", []);
                return Err(SwissArmyHammerError::Storage(format!(
                    "Failed to store embedding in transaction: {e}"
                )));
            }
        }

        // Commit transaction
        conn.execute("COMMIT", []).map_err(|e| {
            // Try to rollback on commit failure
            let _ = conn.execute("ROLLBACK", []);
            SwissArmyHammerError::Storage(format!("Failed to commit transaction: {e}"))
        })?;

        tracing::debug!(
            "Successfully stored {} chunks and {} embeddings in transaction",
            chunks.len(),
            embeddings.len()
        );
        Ok(())
    }

    /// Remove file data in a transaction
    pub fn remove_file_transaction(&self, file_path: &Path) -> Result<()> {
        tracing::debug!("Removing file in transaction: {}", file_path.display());

        let conn = self.connection.lock().map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to acquire connection lock: {e}"))
        })?;

        let file_path_str = file_path.to_string_lossy();

        // Begin transaction
        conn.execute("BEGIN TRANSACTION", []).map_err(|e| {
            SwissArmyHammerError::Storage(format!("Failed to begin transaction: {e}"))
        })?;

        // Remove embeddings for chunks in this file
        if let Err(e) = conn.execute(
            "DELETE FROM embeddings WHERE chunk_id IN (SELECT chunk_id FROM code_chunks WHERE file_path = ?)",
            [&file_path_str as &dyn ToSql],
        ) {
            let _ = conn.execute("ROLLBACK", []);
            return Err(SwissArmyHammerError::Storage(format!(
                "Failed to remove embeddings for file in transaction: {e}"
            )));
        }

        // Remove chunks for this file
        if let Err(e) = conn.execute(
            "DELETE FROM code_chunks WHERE file_path = ?",
            [&file_path_str as &dyn ToSql],
        ) {
            let _ = conn.execute("ROLLBACK", []);
            return Err(SwissArmyHammerError::Storage(format!(
                "Failed to remove chunks for file in transaction: {e}"
            )));
        }

        // Remove indexed file metadata
        if let Err(e) = conn.execute(
            "DELETE FROM indexed_files WHERE path = ?",
            [&file_path_str as &dyn ToSql],
        ) {
            let _ = conn.execute("ROLLBACK", []);
            return Err(SwissArmyHammerError::Storage(format!(
                "Failed to remove indexed file metadata in transaction: {e}"
            )));
        }

        // Commit transaction
        conn.execute("COMMIT", []).map_err(|e| {
            let _ = conn.execute("ROLLBACK", []);
            SwissArmyHammerError::Storage(format!("Failed to commit transaction: {e}"))
        })?;

        tracing::debug!(
            "Successfully removed file in transaction: {}",
            file_path.display()
        );
        Ok(())
    }

    /// Parse Language enum from string representation
    fn parse_language(s: &str) -> Language {
        match s {
            "Rust" => Language::Rust,
            "Python" => Language::Python,
            "TypeScript" => Language::TypeScript,
            "JavaScript" => Language::JavaScript,
            "Dart" => Language::Dart,
            _ => Language::Unknown,
        }
    }

    /// Parse ChunkType enum from string representation
    fn parse_chunk_type(s: &str) -> crate::search::types::ChunkType {
        match s {
            "Function" => crate::search::types::ChunkType::Function,
            "Class" => crate::search::types::ChunkType::Class,
            "Module" => crate::search::types::ChunkType::Module,
            "Import" => crate::search::types::ChunkType::Import,
            _ => crate::search::types::ChunkType::PlainText,
        }
    }

    /// Explicitly close the database connection
    ///
    /// This method allows for explicit cleanup of DuckDB connections,
    /// which can help prevent assertion failures during Drop.
    pub fn close(&self) -> Result<()> {
        if let Ok(conn) = self.connection.lock() {
            // Test connection validity before attempting to close
            match conn.execute("SELECT 1", []) {
                Ok(_) => {
                    // Connection is valid, perform explicit close
                    drop(conn);
                    tracing::debug!("Successfully closed VectorStorage database connection");
                    Ok(())
                }
                Err(e) => {
                    tracing::warn!("Connection test failed during close: {}", e);
                    // Connection was already invalid, just drop it
                    drop(conn);
                    Ok(())
                }
            }
        } else {
            tracing::warn!("Could not acquire connection lock during close");
            Ok(())
        }
    }
}

/// Enhanced Drop implementation for VectorStorage
///
/// This implementation ensures proper cleanup of DuckDB connections
/// by validating the connection before attempting to drop it.
impl Drop for VectorStorage {
    fn drop(&mut self) {
        // Attempt graceful cleanup with connection validation
        if let Err(e) = self.close() {
            tracing::warn!("Error during VectorStorage drop cleanup: {}", e);
        }
    }
}

/// Statistics about the vector storage
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StorageStats {
    /// Total number of code chunks stored
    pub total_chunks: usize,
    /// Total number of indexed files
    pub total_files: usize,
    /// Total number of embeddings stored
    pub total_embeddings: usize,
    /// Database file size in bytes
    pub database_size_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::types::{ChunkType, FileId};
    use chrono::Utc;
    use tempfile::TempDir;

    /// Test helper that ensures database files are cleaned up
    struct TestDatabaseGuard {
        _temp_dir: TempDir,
        _db_path: PathBuf,
    }

    impl TestDatabaseGuard {
        fn new(temp_dir: TempDir, db_path: PathBuf) -> Self {
            Self {
                _temp_dir: temp_dir,
                _db_path: db_path,
            }
        }
    }

    impl Drop for TestDatabaseGuard {
        fn drop(&mut self) {
            // TempDir will automatically clean up all files when dropped
            // No manual cleanup needed
        }
    }

    fn create_test_config() -> (SemanticConfig, TestDatabaseGuard) {
        // Create a temporary directory for the test database
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_semantic.db");

        let config = SemanticConfig {
            database_path: db_path.clone(),
            embedding_model: swissarmyhammer_config::DEFAULT_TEST_EMBEDDING_MODEL.to_string(),
            chunk_size: 512,
            chunk_overlap: 64,
            similarity_threshold: 0.7,
            excerpt_length: 200,
            context_lines: 2,
            simple_search_threshold: 0.5,
            code_similarity_threshold: 0.7,
            content_preview_length: 100,
            min_chunk_size: 50,
            max_chunk_size: 2000,
            max_chunks_per_file: 100,
            max_file_size_bytes: 10 * 1024 * 1024,
        };

        let guard = TestDatabaseGuard::new(temp_dir, db_path);
        (config, guard)
    }

    fn create_test_chunk() -> CodeChunk {
        CodeChunk {
            id: "test-chunk-1".to_string(),
            file_path: PathBuf::from("test.rs"),
            language: Language::Rust,
            content: "fn main() { println!(\"Hello, world!\"); }".to_string(),
            start_line: 1,
            end_line: 1,
            chunk_type: ChunkType::Function,
            content_hash: ContentHash("test-hash".to_string()),
        }
    }

    fn create_test_embedding() -> Embedding {
        let mut vector = vec![0.1; 384]; // 384-dimensional vector
        vector[0] = 0.1;
        vector[1] = 0.2;
        vector[2] = 0.3;

        Embedding {
            chunk_id: "test-chunk-1".to_string(),
            vector,
        }
    }

    fn create_test_indexed_file() -> IndexedFile {
        IndexedFile {
            file_id: FileId("test-file-1".to_string()),
            path: PathBuf::from("test.rs"),
            language: Language::Rust,
            content_hash: ContentHash("file-hash".to_string()),
            chunk_count: 1,
            indexed_at: Utc::now(),
        }
    }

    #[test]
    fn test_vector_storage_creation() {
        let (config, _guard) = create_test_config();
        let storage = VectorStorage::new(config);
        assert!(storage.is_ok());
    }

    #[test]
    fn test_initialize() {
        let (config, _guard) = create_test_config();
        let storage = VectorStorage::new(config).unwrap();
        assert!(storage.initialize().is_ok());
    }

    #[test]
    fn test_store_chunk() {
        let (config, _guard) = create_test_config();
        let storage = VectorStorage::new(config).unwrap();
        storage.initialize().unwrap();
        let chunk = create_test_chunk();
        assert!(storage.store_chunk(&chunk).is_ok());
    }

    #[test]
    fn test_store_embedding() {
        let (config, _guard) = create_test_config();
        let storage = VectorStorage::new(config).unwrap();
        storage.initialize().unwrap();
        // First store the chunk that the embedding references
        let chunk = create_test_chunk();
        storage.store_chunk(&chunk).unwrap();
        // Then store the embedding
        let embedding = create_test_embedding();
        assert!(storage.store_embedding(&embedding).is_ok());
    }

    #[test]
    fn test_store_indexed_file() {
        let (config, _guard) = create_test_config();
        let storage = VectorStorage::new(config).unwrap();
        storage.initialize().unwrap();
        let indexed_file = create_test_indexed_file();
        assert!(storage.store_indexed_file(&indexed_file).is_ok());
    }

    #[test]
    fn test_empty_search() {
        let (config, _guard) = create_test_config();
        let storage = VectorStorage::new(config).unwrap();
        storage.initialize().unwrap();
        let query = vec![0.1; 384]; // 384-dimensional query vector
        let results = storage.similarity_search(&query, 10, 0.5);
        assert!(results.is_ok());
        assert_eq!(results.unwrap().len(), 0);
    }

    #[test]
    fn test_get_chunk() {
        let (config, _guard) = create_test_config();
        let storage = VectorStorage::new(config).unwrap();
        storage.initialize().unwrap();
        let result = storage.get_chunk("test-chunk-1");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_needs_reindexing() {
        let (config, _guard) = create_test_config();
        let storage = VectorStorage::new(config).unwrap();
        storage.initialize().unwrap();
        let path = Path::new("test.rs");
        let hash = ContentHash("test-hash".to_string());
        let result = storage.needs_reindexing(path, &hash);
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should always return true for placeholder
    }

    #[test]
    fn test_is_file_indexed() {
        let (config, _guard) = create_test_config();
        let storage = VectorStorage::new(config).unwrap();
        storage.initialize().unwrap();
        let path = Path::new("test.rs");
        let result = storage.is_file_indexed(path);
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should always return false for placeholder
    }

    #[test]
    fn test_remove_file() {
        let (config, _guard) = create_test_config();
        let storage = VectorStorage::new(config).unwrap();
        storage.initialize().unwrap();
        let path = Path::new("test.rs");
        assert!(storage.remove_file(path).is_ok());
    }

    #[test]
    fn test_get_stats() {
        let (config, _guard) = create_test_config();
        let storage = VectorStorage::new(config).unwrap();
        storage.initialize().unwrap();
        let stats = storage.get_stats();
        assert!(stats.is_ok());
        let stats = stats.unwrap();
        assert_eq!(stats.total_chunks, 0);
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_embeddings, 0);
    }

    #[test]
    fn test_get_chunks_by_language() {
        let (config, _guard) = create_test_config();
        let storage = VectorStorage::new(config).unwrap();
        storage.initialize().unwrap();
        let chunks = storage.get_chunks_by_language(&Language::Rust);
        assert!(chunks.is_ok());
        assert_eq!(chunks.unwrap().len(), 0);
    }

    #[test]
    fn test_maintenance() {
        let (config, _guard) = create_test_config();
        let storage = VectorStorage::new(config).unwrap();
        storage.initialize().unwrap();
        assert!(storage.maintenance().is_ok());
    }

    #[test]
    fn test_storage_stats_default() {
        let stats = StorageStats::default();
        assert_eq!(stats.total_chunks, 0);
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_embeddings, 0);
        assert_eq!(stats.database_size_bytes, 0);
    }

    #[test]
    fn test_get_file_chunks() {
        let (config, _guard) = create_test_config();
        let storage = VectorStorage::new(config).unwrap();
        storage.initialize().unwrap();
        let path = Path::new("test.rs");
        let chunks = storage.get_file_chunks(path);
        assert!(chunks.is_ok());
        assert_eq!(chunks.unwrap().len(), 0);
    }

    #[test]
    fn test_reproduce_similarity_search_failure() {
        // This test reproduces the exact failure scenario from the issue
        let (config, _guard) = create_test_config();
        let storage = VectorStorage::new(config).unwrap();
        storage.initialize().unwrap();

        // Create a query vector like the real search would
        let query_embedding = vec![0.1; 384];

        // This should succeed but currently fails
        let result = storage.similarity_search(&query_embedding, 10, 0.5);

        match result {
            Ok(results) => {
                tracing::debug!("Search succeeded with {} results", results.len());
                assert_eq!(results.len(), 0); // Should be empty but not fail
            }
            Err(e) => {
                tracing::error!("Search failed with error: {e}");
                panic!("similarity_search should not fail on empty database: {e}");
            }
        }
    }

    #[tokio::test]
    async fn test_reproduce_full_search_integration() {
        use crate::search::{EmbeddingEngine, SearchQuery, SemanticSearcher};
        use std::error::Error;

        // Use proper test config for unique temp directories and avoid trying to download real embedding models
        let (config, _guard) = create_test_config();

        let storage = VectorStorage::new(config.clone()).unwrap();
        storage.initialize().unwrap();

        // Use test embedding engine instead of real one
        let embedding_engine = EmbeddingEngine::new_for_testing().await.unwrap();

        // Try to create a searcher using the test embedding engine
        match SemanticSearcher::with_embedding_engine(storage, embedding_engine, config).await {
            Ok(searcher) => {
                // Perform search with the same parameters as the failing command
                let search_query = SearchQuery {
                    text: "duckdb".to_string(),
                    limit: 10,
                    similarity_threshold: 0.5,
                    language_filter: None,
                };

                match searcher.search(&search_query).await {
                    Ok(results) => {
                        tracing::debug!(
                            "Full integration search succeeded! Found {} results",
                            results.len()
                        );
                        // The important thing is that the search succeeds without crashing
                        // The number of results doesn't matter - it could be 0 or more
                        // Test that we get results back (could be 0 or more)
                    }
                    Err(e) => {
                        tracing::error!("Full integration search failed with error: {e}");
                        tracing::error!("Error debug: {e:?}");

                        // Log the full error chain
                        let mut source = e.source();
                        let mut level = 1;
                        while let Some(err) = source {
                            tracing::error!("  Error level {level}: {err}");
                            tracing::error!("  Error level {level} debug: {err:?}");
                            source = err.source();
                            level += 1;
                        }

                        panic!("Full integration search should not fail on empty database: {e}");
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to create searcher: {e}");
                panic!("Failed to create searcher: {e}");
            }
        }
    }
}
