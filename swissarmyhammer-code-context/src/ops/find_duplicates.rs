//! Find code in a file that is duplicated elsewhere in the codebase.
//!
//! For each chunk in the target file(s), finds semantically similar chunks
//! in other files using embedding cosine similarity. Results are grouped
//! by source chunk — each group answers "this piece of your file looks
//! like these places elsewhere."

use rusqlite::Connection;

use crate::error::CodeContextError;
use crate::ops::search_code::cosine_similarity;

/// A chunk location with its text.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChunkRef {
    /// Path of the file containing this chunk.
    pub file_path: String,
    /// First line of the chunk (1-indexed).
    pub start_line: u32,
    /// Last line of the chunk (1-indexed).
    pub end_line: u32,
    /// Qualified symbol path, if available.
    pub symbol_path: Option<String>,
    /// Full text of the chunk.
    pub text: String,
}

/// A match: another chunk that looks like the source chunk.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DuplicateMatch {
    /// The similar chunk found elsewhere.
    pub chunk: ChunkRef,
    /// Cosine similarity to the source chunk.
    pub similarity: f32,
}

/// A source chunk and all the places it's duplicated.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DuplicateGroup {
    /// The chunk from the target file.
    pub source: ChunkRef,
    /// Similar chunks found in other files, sorted by similarity (descending).
    pub duplicates: Vec<DuplicateMatch>,
}

/// Options for [`find_duplicates`].
#[derive(Debug)]
pub struct FindDuplicatesOptions {
    /// Minimum cosine similarity to report (default: 0.85).
    pub min_similarity: f32,
    /// Minimum chunk size in bytes to consider (default: 100).
    pub min_chunk_bytes: usize,
    /// Maximum number of duplicates per source chunk (default: 5).
    pub max_per_chunk: usize,
}

impl Default for FindDuplicatesOptions {
    fn default() -> Self {
        Self {
            min_similarity: 0.85,
            min_chunk_bytes: 100,
            max_per_chunk: 5,
        }
    }
}

/// Result of [`find_duplicates`].
#[derive(Debug, serde::Serialize)]
pub struct FindDuplicatesResult {
    /// The file(s) that were analyzed.
    pub file: String,
    /// Groups of duplicates, one per source chunk that has matches.
    /// Only chunks with at least one duplicate are included.
    pub groups: Vec<DuplicateGroup>,
    /// Total chunks in the target file(s).
    pub source_chunks: usize,
    /// Total chunks compared against (from other files).
    pub compared_chunks: usize,
}

/// An embedded chunk row from the database.
struct EmbeddedChunk {
    file_path: String,
    start_line: u32,
    end_line: u32,
    symbol_path: Option<String>,
    text: String,
    embedding: Vec<f32>,
}

impl EmbeddedChunk {
    fn to_chunk_ref(&self) -> ChunkRef {
        ChunkRef {
            file_path: self.file_path.clone(),
            start_line: self.start_line,
            end_line: self.end_line,
            symbol_path: self.symbol_path.clone(),
            text: self.text.clone(),
        }
    }
}

/// Deserialize an embedding blob (little-endian f32 array).
fn deserialize_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// Find chunks in `file` that are duplicated elsewhere in the codebase.
///
/// For each chunk in the target file, compares its embedding against all
/// chunks in other files. Returns groups where the source chunk has at
/// least one match above `min_similarity`.
///
/// # Errors
///
/// Returns [`CodeContextError::Database`] on SQLite failures.
pub fn find_duplicates(
    conn: &Connection,
    file: &str,
    options: &FindDuplicatesOptions,
) -> Result<FindDuplicatesResult, CodeContextError> {
    let all_chunks = load_embedded_chunks(conn, options.min_chunk_bytes)?;

    let mut source_chunks_list = Vec::new();
    let mut other_chunks = Vec::new();

    for chunk in &all_chunks {
        if chunk.file_path == file {
            source_chunks_list.push(chunk);
        } else {
            other_chunks.push(chunk);
        }
    }

    let source_chunks = source_chunks_list.len();
    let compared_chunks = other_chunks.len();

    let mut groups = Vec::new();

    for src in &source_chunks_list {
        let mut matches: Vec<DuplicateMatch> = other_chunks
            .iter()
            .filter_map(|other| {
                let sim = cosine_similarity(&src.embedding, &other.embedding);
                if sim >= options.min_similarity {
                    Some(DuplicateMatch {
                        chunk: other.to_chunk_ref(),
                        similarity: sim,
                    })
                } else {
                    None
                }
            })
            .collect();

        if matches.is_empty() {
            continue;
        }

        // Sort descending by similarity
        matches.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        matches.truncate(options.max_per_chunk);

        groups.push(DuplicateGroup {
            source: src.to_chunk_ref(),
            duplicates: matches,
        });
    }

    // Sort groups by highest match similarity (most duplicated first)
    groups.sort_by(|a, b| {
        let a_best = a.duplicates.first().map(|d| d.similarity).unwrap_or(0.0);
        let b_best = b.duplicates.first().map(|d| d.similarity).unwrap_or(0.0);
        b_best
            .partial_cmp(&a_best)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(FindDuplicatesResult {
        file: file.to_string(),
        groups,
        source_chunks,
        compared_chunks,
    })
}

/// Load embedded chunks from `ts_chunks`, filtering by minimum size.
fn load_embedded_chunks(
    conn: &Connection,
    min_chunk_bytes: usize,
) -> Result<Vec<EmbeddedChunk>, CodeContextError> {
    let mut stmt = conn.prepare(
        "SELECT file_path, start_line, end_line, symbol_path, text, embedding
         FROM ts_chunks
         WHERE embedding IS NOT NULL AND LENGTH(text) >= ?1",
    )?;

    let rows = stmt.query_map([min_chunk_bytes as i64], |row| {
        let blob: Vec<u8> = row.get(5)?;
        Ok(EmbeddedChunk {
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
    use crate::ops::search_code::serialize_embedding;

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

    fn insert_chunk(
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

    #[test]
    fn test_finds_duplicate_in_other_file() {
        let conn = test_db();
        insert_file(&conn, "src/handler.rs");
        insert_file(&conn, "src/legacy.rs");

        let text = "fn validate_input(req: &Request) -> Result<(), Error> { check_fields(req)?; Ok(()) }";
        // Nearly identical embeddings = near-duplicate code
        insert_chunk(&conn, "src/handler.rs", 10, 15, Some("validate_input"), text, &[0.9, 0.1, 0.0]);
        insert_chunk(&conn, "src/legacy.rs", 20, 25, Some("check_input"), text, &[0.89, 0.11, 0.01]);

        let opts = FindDuplicatesOptions {
            min_chunk_bytes: 10,
            ..Default::default()
        };
        let result = find_duplicates(&conn, "src/handler.rs", &opts).unwrap();

        assert_eq!(result.file, "src/handler.rs");
        assert_eq!(result.source_chunks, 1);
        assert_eq!(result.compared_chunks, 1);
        assert_eq!(result.groups.len(), 1);

        let group = &result.groups[0];
        assert_eq!(group.source.file_path, "src/handler.rs");
        assert_eq!(group.source.symbol_path.as_deref(), Some("validate_input"));
        assert_eq!(group.duplicates.len(), 1);
        assert_eq!(group.duplicates[0].chunk.file_path, "src/legacy.rs");
        assert!(group.duplicates[0].similarity > 0.99);
    }

    #[test]
    fn test_no_duplicates_for_unique_code() {
        let conn = test_db();
        insert_file(&conn, "src/unique.rs");
        insert_file(&conn, "src/other.rs");

        let text_a = "fn unique_function() { let x = very_specific_logic(); transform(x); return special_result; }";
        let text_b = "fn completely_different() { database_query(); network_call(); render_template(); return html; }";
        // Orthogonal embeddings = unrelated code
        insert_chunk(&conn, "src/unique.rs", 1, 5, None, text_a, &[1.0, 0.0, 0.0]);
        insert_chunk(&conn, "src/other.rs", 1, 5, None, text_b, &[0.0, 1.0, 0.0]);

        let opts = FindDuplicatesOptions {
            min_chunk_bytes: 10,
            ..Default::default()
        };
        let result = find_duplicates(&conn, "src/unique.rs", &opts).unwrap();

        assert!(result.groups.is_empty());
    }

    #[test]
    fn test_multiple_duplicates_per_chunk() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs");
        insert_file(&conn, "src/copy1.rs");
        insert_file(&conn, "src/copy2.rs");

        let text = "fn process_data(data: &[u8]) -> Vec<u8> { parse(data); transform(data); serialize(data) }";
        insert_chunk(&conn, "src/main.rs", 1, 5, Some("process_data"), text, &[1.0, 0.0, 0.0]);
        insert_chunk(&conn, "src/copy1.rs", 1, 5, None, text, &[0.99, 0.01, 0.0]);
        insert_chunk(&conn, "src/copy2.rs", 10, 15, None, text, &[0.98, 0.02, 0.0]);

        let opts = FindDuplicatesOptions {
            min_chunk_bytes: 10,
            ..Default::default()
        };
        let result = find_duplicates(&conn, "src/main.rs", &opts).unwrap();

        assert_eq!(result.groups.len(), 1);
        assert_eq!(result.groups[0].duplicates.len(), 2);
        // Sorted by similarity descending
        assert!(result.groups[0].duplicates[0].similarity >= result.groups[0].duplicates[1].similarity);
    }

    #[test]
    fn test_max_per_chunk_limits_duplicates() {
        let conn = test_db();
        insert_file(&conn, "src/source.rs");

        let text = "fn repeated_pattern() { setup(); execute(); teardown(); report_results(); finish(); }";
        insert_chunk(&conn, "src/source.rs", 1, 5, None, text, &[1.0, 0.0, 0.0]);

        // Create 10 copies in other files
        for i in 0..10 {
            let path = format!("src/copy_{i}.rs");
            insert_file(&conn, &path);
            insert_chunk(&conn, &path, 1, 5, None, text, &[1.0 - (i as f32 * 0.001), 0.01, 0.0]);
        }

        let opts = FindDuplicatesOptions {
            min_chunk_bytes: 10,
            max_per_chunk: 3,
            ..Default::default()
        };
        let result = find_duplicates(&conn, "src/source.rs", &opts).unwrap();

        assert_eq!(result.groups.len(), 1);
        assert_eq!(result.groups[0].duplicates.len(), 3);
    }

    #[test]
    fn test_min_chunk_bytes_filters_small_chunks() {
        let conn = test_db();
        insert_file(&conn, "src/a.rs");
        insert_file(&conn, "src/b.rs");

        // Small chunks that would match but are below threshold
        insert_chunk(&conn, "src/a.rs", 1, 1, None, "let x = 1;", &[1.0, 0.0]);
        insert_chunk(&conn, "src/b.rs", 1, 1, None, "let x = 1;", &[1.0, 0.0]);

        let opts = FindDuplicatesOptions {
            min_chunk_bytes: 100,
            min_similarity: 0.5,
            ..Default::default()
        };
        let result = find_duplicates(&conn, "src/a.rs", &opts).unwrap();

        assert_eq!(result.source_chunks, 0);
        assert!(result.groups.is_empty());
    }

    #[test]
    fn test_file_not_in_index_returns_empty() {
        let conn = test_db();
        insert_file(&conn, "src/other.rs");
        let text = "fn something() { let result = compute_value(); process(result); return output_data; }";
        insert_chunk(&conn, "src/other.rs", 1, 3, None, text, &[1.0, 0.0]);

        let opts = FindDuplicatesOptions {
            min_chunk_bytes: 10,
            ..Default::default()
        };
        let result = find_duplicates(&conn, "src/nonexistent.rs", &opts).unwrap();

        assert_eq!(result.source_chunks, 0);
        assert!(result.groups.is_empty());
    }

    #[test]
    fn test_empty_index() {
        let conn = test_db();
        let result = find_duplicates(&conn, "src/any.rs", &FindDuplicatesOptions::default()).unwrap();

        assert!(result.groups.is_empty());
        assert_eq!(result.source_chunks, 0);
        assert_eq!(result.compared_chunks, 0);
    }

    #[test]
    fn test_groups_sorted_by_best_match() {
        let conn = test_db();
        insert_file(&conn, "src/target.rs");
        insert_file(&conn, "src/other.rs");

        let text_a = "fn low_match_function() { some_logic(); more_logic(); even_more(); final_step(); done(); }";
        let text_b = "fn high_match_function() { identical_logic(); same_stuff(); matching_code(); result(); }";

        // Chunk A has a weaker match
        insert_chunk(&conn, "src/target.rs", 1, 5, Some("low_match"), text_a, &[1.0, 0.0, 0.0]);
        insert_chunk(&conn, "src/other.rs", 1, 5, None, text_a, &[0.87, 0.3, 0.0]);

        // Chunk B has a stronger match
        insert_chunk(&conn, "src/target.rs", 10, 15, Some("high_match"), text_b, &[0.0, 1.0, 0.0]);
        insert_chunk(&conn, "src/other.rs", 10, 15, None, text_b, &[0.01, 0.99, 0.0]);

        let opts = FindDuplicatesOptions {
            min_similarity: 0.85,
            min_chunk_bytes: 10,
            ..Default::default()
        };
        let result = find_duplicates(&conn, "src/target.rs", &opts).unwrap();

        assert_eq!(result.groups.len(), 2);
        // high_match group should come first (higher similarity)
        let best_sim_0 = result.groups[0].duplicates[0].similarity;
        let best_sim_1 = result.groups[1].duplicates[0].similarity;
        assert!(best_sim_0 >= best_sim_1);
    }
}
