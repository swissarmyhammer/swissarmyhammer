//! Regex search across stored chunk text in `ts_chunks`.
//!
//! Returns complete semantic blocks that match a given regex pattern.
//! Uses the `regex` crate for compilation and `rayon::par_iter` for
//! parallel matching across all loaded chunks.

use rayon::prelude::*;
use regex::Regex;
use rusqlite::Connection;

use crate::error::CodeContextError;

/// A match position within chunk text.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MatchPosition {
    /// Byte offset of the match start within the chunk text.
    pub start: usize,
    /// Byte offset of the match end within the chunk text.
    pub end: usize,
}

/// A chunk that matched the grep pattern.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GrepMatch {
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
    /// All regex match positions within `text`.
    pub matches: Vec<MatchPosition>,
}

/// Options for [`grep_code`].
#[derive(Debug, Default)]
pub struct GrepOptions {
    /// Only search chunks from files with these extensions (e.g., `["rs", "py"]`).
    pub language: Option<Vec<String>>,
    /// Only search chunks from these specific file paths.
    pub files: Option<Vec<String>>,
    /// Maximum number of matching chunks to return.
    pub max_results: Option<usize>,
}

/// Result of [`grep_code`].
#[derive(Debug, serde::Serialize)]
pub struct GrepResult {
    /// The regex pattern that was searched for.
    pub pattern: String,
    /// Chunks that matched the pattern.
    pub matches: Vec<GrepMatch>,
    /// Total number of chunks examined.
    pub total_chunks_searched: usize,
    /// Whether the result set was truncated by `max_results`.
    pub truncated: bool,
}

/// A row loaded from the `ts_chunks` table.
struct ChunkRow {
    file_path: String,
    start_line: u32,
    end_line: u32,
    symbol_path: Option<String>,
    text: String,
}

/// Search chunk text with a regex pattern.
///
/// Loads matching chunks from `ts_chunks`, runs the compiled regex in
/// parallel with rayon, and collects results.
///
/// # Errors
///
/// Returns [`CodeContextError::Pattern`] if the regex is invalid,
/// or [`CodeContextError::Database`] on SQLite failures.
pub fn grep_code(
    conn: &Connection,
    pattern: &str,
    options: &GrepOptions,
) -> Result<GrepResult, CodeContextError> {
    let re = Regex::new(pattern).map_err(|e| CodeContextError::Pattern(e.to_string()))?;

    let chunks = load_chunks(conn, options)?;
    let total_chunks_searched = chunks.len();

    let all_matches: Vec<GrepMatch> = chunks
        .par_iter()
        .filter_map(|chunk| {
            let positions: Vec<MatchPosition> = re
                .find_iter(&chunk.text)
                .map(|m| MatchPosition {
                    start: m.start(),
                    end: m.end(),
                })
                .collect();

            if positions.is_empty() {
                None
            } else {
                Some(GrepMatch {
                    file_path: chunk.file_path.clone(),
                    start_line: chunk.start_line,
                    end_line: chunk.end_line,
                    symbol_path: chunk.symbol_path.clone(),
                    text: chunk.text.clone(),
                    matches: positions,
                })
            }
        })
        .collect();

    let max = options.max_results.unwrap_or(usize::MAX);
    let truncated = all_matches.len() > max;
    let matches: Vec<GrepMatch> = all_matches.into_iter().take(max).collect();

    Ok(GrepResult {
        pattern: pattern.to_string(),
        matches,
        total_chunks_searched,
        truncated,
    })
}

/// Load chunk rows from `ts_chunks`, applying optional language and file filters.
fn load_chunks(
    conn: &Connection,
    options: &GrepOptions,
) -> Result<Vec<ChunkRow>, CodeContextError> {
    let mut sql =
        String::from("SELECT file_path, start_line, end_line, symbol_path, text FROM ts_chunks");
    let mut conditions: Vec<String> = Vec::new();

    // Build language filter (match by file extension)
    if let Some(ref langs) = options.language {
        if !langs.is_empty() {
            let like_clauses: Vec<String> = langs
                .iter()
                .map(|ext| format!("file_path LIKE '%.{ext}'"))
                .collect();
            conditions.push(format!("({})", like_clauses.join(" OR ")));
        }
    }

    // Build file path filter
    if let Some(ref files) = options.files {
        if !files.is_empty() {
            let placeholders: Vec<String> = files.iter().map(|f| format!("'{f}'")).collect();
            conditions.push(format!("file_path IN ({})", placeholders.join(", ")));
        }
    }

    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        Ok(ChunkRow {
            file_path: row.get(0)?,
            start_line: row.get(1)?,
            end_line: row.get(2)?,
            symbol_path: row.get(3)?,
            text: row.get(4)?,
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
    use crate::test_fixtures::{insert_file_simple as insert_file, test_db};

    /// Insert a chunk into ts_chunks (parameter order: symbol_path before text).
    fn insert_chunk(
        conn: &Connection,
        file_path: &str,
        start_line: u32,
        end_line: u32,
        symbol_path: Option<&str>,
        text: &str,
    ) {
        crate::test_fixtures::insert_ts_chunk(
            conn,
            file_path,
            start_line as i32,
            end_line as i32,
            text,
            symbol_path,
        );
    }

    #[test]
    fn test_grep_basic() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs");
        insert_chunk(
            &conn,
            "src/main.rs",
            1,
            5,
            Some("main"),
            "fn main() {\n    println!(\"hello\");\n}",
        );
        insert_chunk(
            &conn,
            "src/main.rs",
            6,
            10,
            Some("helper"),
            "fn helper(x: i32) -> i32 {\n    x + 1\n}",
        );
        insert_chunk(
            &conn,
            "src/main.rs",
            11,
            15,
            None,
            "const MAX: usize = 100;",
        );

        let result = grep_code(&conn, r"fn\s+\w+", &GrepOptions::default()).unwrap();

        assert_eq!(result.pattern, r"fn\s+\w+");
        assert_eq!(result.matches.len(), 2);
        assert_eq!(result.total_chunks_searched, 3);
        assert!(!result.truncated);

        // Both function chunks should match
        let paths: Vec<&str> = result
            .matches
            .iter()
            .map(|m| m.file_path.as_str())
            .collect();
        assert!(paths.iter().all(|p| *p == "src/main.rs"));

        // Each match should have at least one position
        for m in &result.matches {
            assert!(!m.matches.is_empty());
        }
    }

    #[test]
    fn test_grep_max_results() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs");

        // Insert 5 chunks that all match
        for i in 0..5 {
            let text = format!("fn func_{i}() {{}}\n");
            insert_chunk(&conn, "src/lib.rs", i * 3 + 1, i * 3 + 3, None, &text);
        }

        let opts = GrepOptions {
            max_results: Some(2),
            ..Default::default()
        };
        let result = grep_code(&conn, r"fn\s+\w+", &opts).unwrap();

        assert_eq!(result.matches.len(), 2);
        assert!(result.truncated);
        assert_eq!(result.total_chunks_searched, 5);
    }

    #[test]
    fn test_grep_language_filter() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs");
        insert_file(&conn, "src/utils.py");

        insert_chunk(
            &conn,
            "src/main.rs",
            1,
            3,
            None,
            "fn hello() { println!(\"hi\"); }",
        );
        insert_chunk(
            &conn,
            "src/utils.py",
            1,
            3,
            None,
            "def hello():\n    print(\"hi\")",
        );

        let opts = GrepOptions {
            language: Some(vec!["rs".to_string()]),
            ..Default::default()
        };
        let result = grep_code(&conn, "hello", &opts).unwrap();

        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].file_path, "src/main.rs");
        // Only searched the filtered chunks
        assert_eq!(result.total_chunks_searched, 1);
    }

    #[test]
    fn test_grep_no_matches() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs");
        insert_chunk(&conn, "src/main.rs", 1, 3, None, "let x = 42;");

        let result = grep_code(
            &conn,
            "this_will_never_match_anything",
            &GrepOptions::default(),
        )
        .unwrap();

        assert!(result.matches.is_empty());
        assert_eq!(result.total_chunks_searched, 1);
        assert!(!result.truncated);
    }

    #[test]
    fn test_grep_invalid_pattern() {
        let conn = test_db();

        let result = grep_code(&conn, "[invalid", &GrepOptions::default());
        assert!(result.is_err());

        match result {
            Err(CodeContextError::Pattern(msg)) => {
                assert!(
                    msg.contains("unclosed"),
                    "expected unclosed bracket error: {msg}"
                );
            }
            other => panic!("expected Pattern error, got: {other:?}"),
        }
    }

    #[test]
    fn test_grep_match_positions() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs");

        let text = "fn alpha() {}\nfn beta() {}";
        insert_chunk(&conn, "src/lib.rs", 1, 2, None, text);

        let result = grep_code(&conn, r"fn \w+", &GrepOptions::default()).unwrap();

        assert_eq!(result.matches.len(), 1);
        let m = &result.matches[0];
        assert_eq!(m.matches.len(), 2);

        // First match: "fn alpha"
        let pos0 = &m.matches[0];
        assert_eq!(&text[pos0.start..pos0.end], "fn alpha");

        // Second match: "fn beta"
        let pos1 = &m.matches[1];
        assert_eq!(&text[pos1.start..pos1.end], "fn beta");
    }
}
