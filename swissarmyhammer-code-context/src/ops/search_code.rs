//! Semantic similarity search across stored chunk embeddings.
//!
//! Loads chunk embeddings from `ts_chunks`, computes cosine similarity
//! against a pre-computed query embedding, and returns the top-k results.

use rusqlite::Connection;

use crate::error::CodeContextError;

/// A chunk that matched the semantic search query.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchCodeMatch {
    /// Path of the file containing this chunk.
    pub file_path: String,
    /// First line of the chunk (1-indexed).
    pub start_line: u32,
    /// Last line of the chunk (1-indexed).
    pub end_line: u32,
    /// Qualified symbol path for this chunk, if available.
    pub symbol_path: Option<String>,
    /// Full text of the matched chunk.
    pub text: String,
    /// Cosine similarity score (0.0 to 1.0).
    pub similarity: f32,
}

/// Options for [`search_code`].
#[derive(Debug)]
pub struct SearchCodeOptions {
    /// Maximum number of results to return.
    pub top_k: usize,
    /// Minimum cosine similarity threshold.
    pub min_similarity: f32,
    /// Only search chunks from files with these extensions.
    pub language: Option<Vec<String>>,
    /// Only search chunks matching this file path pattern.
    pub file_pattern: Option<String>,
}

impl Default for SearchCodeOptions {
    fn default() -> Self {
        Self {
            top_k: 10,
            min_similarity: 0.7,
            language: None,
            file_pattern: None,
        }
    }
}

/// Result of [`search_code`].
#[derive(Debug, serde::Serialize)]
pub struct SearchCodeResult {
    /// Chunks ranked by cosine similarity (descending).
    pub matches: Vec<SearchCodeMatch>,
    /// Total number of chunks with embeddings that were searched.
    pub total_chunks_searched: usize,
    /// Whether the result set was truncated by `top_k`.
    pub truncated: bool,
}

/// A row loaded from `ts_chunks` with its embedding.
struct EmbeddingRow {
    file_path: String,
    start_line: u32,
    end_line: u32,
    symbol_path: Option<String>,
    text: String,
    embedding: Vec<f32>,
}

/// Compute cosine similarity between two vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

/// Deserialize an embedding blob (little-endian f32 array) into a Vec<f32>.
fn deserialize_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// Serialize an f32 slice into a little-endian byte blob.
pub fn serialize_embedding(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Search chunk embeddings by cosine similarity against a query embedding.
///
/// The query embedding must be pre-computed by the caller using the same
/// embedding model that produced the chunk embeddings.
///
/// # Errors
///
/// Returns [`CodeContextError::Database`] on SQLite failures.
pub fn search_code(
    conn: &Connection,
    query_embedding: &[f32],
    options: &SearchCodeOptions,
) -> Result<SearchCodeResult, CodeContextError> {
    let rows = load_embedded_chunks(conn, options)?;
    let total_chunks_searched = rows.len();

    let mut scored: Vec<(f32, &EmbeddingRow)> = rows
        .iter()
        .map(|row| (cosine_similarity(query_embedding, &row.embedding), row))
        .filter(|(sim, _)| *sim >= options.min_similarity)
        .collect();

    // Sort descending by similarity
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let truncated = scored.len() > options.top_k;
    let matches: Vec<SearchCodeMatch> = scored
        .into_iter()
        .take(options.top_k)
        .map(|(sim, row)| SearchCodeMatch {
            file_path: row.file_path.clone(),
            start_line: row.start_line,
            end_line: row.end_line,
            symbol_path: row.symbol_path.clone(),
            text: row.text.clone(),
            similarity: sim,
        })
        .collect();

    Ok(SearchCodeResult {
        matches,
        total_chunks_searched,
        truncated,
    })
}

/// Load chunk rows that have embeddings from `ts_chunks`.
fn load_embedded_chunks(
    conn: &Connection,
    options: &SearchCodeOptions,
) -> Result<Vec<EmbeddingRow>, CodeContextError> {
    let mut sql = String::from(
        "SELECT file_path, start_line, end_line, symbol_path, text, embedding FROM ts_chunks WHERE embedding IS NOT NULL",
    );

    if let Some(ref langs) = options.language {
        if !langs.is_empty() {
            let like_clauses: Vec<String> = langs
                .iter()
                .map(|ext| format!("file_path LIKE '%.{ext}'"))
                .collect();
            sql.push_str(&format!(" AND ({})", like_clauses.join(" OR ")));
        }
    }

    if let Some(ref pattern) = options.file_pattern {
        sql.push_str(&format!(" AND file_path LIKE '%{pattern}%'"));
    }

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        let blob: Vec<u8> = row.get(5)?;
        Ok(EmbeddingRow {
            file_path: row.get(0)?,
            start_line: row.get(1)?,
            end_line: row.get(2)?,
            symbol_path: row.get(3)?,
            text: row.get(4)?,
            embedding: deserialize_embedding(&blob),
        })
    })?;

    let mut chunks = Vec::new();
    for row in rows {
        chunks.push(row?);
    }
    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{configure_connection, create_schema};

    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        create_schema(&conn).unwrap();
        conn
    }

    fn insert_file(conn: &Connection, path: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO indexed_files (file_path, content_hash, file_size, last_seen_at)
             VALUES (?1, X'DEADBEEF', 1024, 1000)",
            [path],
        )
        .unwrap();
    }

    fn insert_chunk_with_embedding(
        conn: &Connection,
        file_path: &str,
        start_line: u32,
        end_line: u32,
        symbol_path: Option<&str>,
        text: &str,
        embedding: &[f32],
    ) {
        let blob = serialize_embedding(embedding);
        conn.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, symbol_path, text, embedding)
             VALUES (?1, 0, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![file_path, text.len(), start_line, end_line, symbol_path, text, blob],
        )
        .unwrap();
    }

    fn insert_chunk_without_embedding(
        conn: &Connection,
        file_path: &str,
        start_line: u32,
        end_line: u32,
        text: &str,
    ) {
        conn.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, text)
             VALUES (?1, 0, ?2, ?3, ?4, ?5)",
            rusqlite::params![file_path, text.len(), start_line, end_line, text],
        )
        .unwrap();
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_empty() {
        let sim = cosine_similarity(&[], &[]);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_cosine_similarity_mismatched_lengths() {
        let sim = cosine_similarity(&[1.0, 2.0], &[1.0]);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let original = vec![1.0f32, -2.5, std::f32::consts::PI, 0.0];
        let blob = serialize_embedding(&original);
        let recovered = deserialize_embedding(&blob);
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_search_code_ranking() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs");

        // Query vector points in the direction [1, 0, 0]
        let query = vec![1.0, 0.0, 0.0];

        // Chunk A: very similar to query
        insert_chunk_with_embedding(
            &conn,
            "src/main.rs",
            1,
            5,
            Some("main"),
            "fn main() {}",
            &[0.9, 0.1, 0.0],
        );

        // Chunk B: somewhat similar
        insert_chunk_with_embedding(
            &conn,
            "src/main.rs",
            6,
            10,
            Some("helper"),
            "fn helper() {}",
            &[0.5, 0.5, 0.0],
        );

        // Chunk C: not similar (orthogonal)
        insert_chunk_with_embedding(
            &conn,
            "src/main.rs",
            11,
            15,
            None,
            "const X: i32 = 1;",
            &[0.0, 0.0, 1.0],
        );

        let result = search_code(&conn, &query, &SearchCodeOptions::default()).unwrap();

        // Only chunks above min_similarity=0.7 should match
        // A has cosine ~0.994, B has cosine ~0.707, C has cosine 0.0
        assert_eq!(result.matches.len(), 2);
        assert_eq!(result.matches[0].symbol_path.as_deref(), Some("main"));
        assert!(result.matches[0].similarity > result.matches[1].similarity);
        assert_eq!(result.total_chunks_searched, 3);
    }

    #[test]
    fn test_search_code_skips_null_embeddings() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs");

        let query = vec![1.0, 0.0];

        insert_chunk_with_embedding(
            &conn,
            "src/lib.rs",
            1,
            3,
            None,
            "fn has_embedding() {}",
            &[1.0, 0.0],
        );
        insert_chunk_without_embedding(&conn, "src/lib.rs", 4, 6, "fn no_embedding() {}");

        let result = search_code(&conn, &query, &SearchCodeOptions::default()).unwrap();

        // Only the chunk with an embedding should be searched
        assert_eq!(result.total_chunks_searched, 1);
        assert_eq!(result.matches.len(), 1);
    }

    #[test]
    fn test_search_code_top_k() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs");

        let query = vec![1.0, 0.0];

        // Insert 5 chunks all similar to query
        for i in 0..5 {
            let text = format!("fn func_{i}() {{}}");
            insert_chunk_with_embedding(
                &conn,
                "src/lib.rs",
                i * 3 + 1,
                i * 3 + 3,
                None,
                &text,
                &[1.0 - (i as f32 * 0.01), 0.1],
            );
        }

        let opts = SearchCodeOptions {
            top_k: 2,
            min_similarity: 0.0,
            ..Default::default()
        };
        let result = search_code(&conn, &query, &opts).unwrap();

        assert_eq!(result.matches.len(), 2);
        assert!(result.truncated);
        assert_eq!(result.total_chunks_searched, 5);
    }

    #[test]
    fn test_search_code_min_similarity_filter() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs");

        let query = vec![1.0, 0.0];

        // High similarity
        insert_chunk_with_embedding(
            &conn,
            "src/lib.rs",
            1,
            3,
            None,
            "fn close() {}",
            &[0.99, 0.1],
        );
        // Low similarity
        insert_chunk_with_embedding(&conn, "src/lib.rs", 4, 6, None, "fn far() {}", &[0.1, 0.99]);

        let opts = SearchCodeOptions {
            min_similarity: 0.9,
            ..Default::default()
        };
        let result = search_code(&conn, &query, &opts).unwrap();

        assert_eq!(result.matches.len(), 1);
        assert!(result.matches[0].text.contains("close"));
    }

    #[test]
    fn test_search_code_empty_index() {
        let conn = test_db();
        let query = vec![1.0, 0.0, 0.0];

        let result = search_code(&conn, &query, &SearchCodeOptions::default()).unwrap();
        assert!(result.matches.is_empty());
        assert_eq!(result.total_chunks_searched, 0);
        assert!(!result.truncated);
    }
}
