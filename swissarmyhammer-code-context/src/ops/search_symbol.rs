//! Fuzzy symbol search with optional kind filter.
//!
//! Searches across all indexed symbols (both LSP and tree-sitter) using
//! `fuzzy_matcher::skim::SkimMatcherV2` for fuzzy matching against symbol
//! names and qualified paths. Results can be filtered by symbol kind
//! (function, method, struct, etc.).

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use rusqlite::Connection;

use crate::error::CodeContextError;
use crate::ops::get_symbol::symbol_kind_name;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Options for [`search_symbol`].
#[derive(Debug, Default)]
pub struct SearchSymbolOptions {
    /// Filter by symbol kind: `"function"`, `"method"`, `"struct"`, `"class"`,
    /// `"interface"`, `"module"`, etc.
    pub kind: Option<String>,
    /// Maximum number of results to return.
    pub max_results: Option<usize>,
}

/// A single fuzzy-matched symbol result.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchSymbolMatch {
    /// The symbol's short name (leaf segment).
    pub name: String,
    /// Fully qualified path (e.g. `MyStruct::new`).
    pub qualified_path: String,
    /// Symbol kind (e.g. "function", "struct"), if known.
    pub kind: Option<String>,
    /// File containing the symbol.
    pub file_path: String,
    /// Start line of the symbol.
    pub start_line: u32,
    /// Fuzzy match score (higher is better).
    pub score: i64,
    /// Which index produced this result: `"lsp"` or `"treesitter"`.
    pub source: String,
}

// ---------------------------------------------------------------------------
// Internal row types
// ---------------------------------------------------------------------------

/// A candidate symbol loaded from the database before fuzzy matching.
struct Candidate {
    name: String,
    qualified_path: String,
    kind: Option<String>,
    file_path: String,
    start_line: u32,
    source: String,
}

// ---------------------------------------------------------------------------
// Core function
// ---------------------------------------------------------------------------

/// Fuzzy search across all symbols, with optional kind filter.
///
/// Uses `SkimMatcherV2` for fuzzy matching against symbol names and
/// qualified paths. If `kind` is specified, only symbols matching that
/// kind are included in results.
///
/// Results are sorted by descending score.
///
/// # Errors
///
/// Returns [`CodeContextError::Database`] on SQLite failures.
pub fn search_symbol(
    conn: &Connection,
    query: &str,
    options: &SearchSymbolOptions,
) -> Result<Vec<SearchSymbolMatch>, CodeContextError> {
    let candidates = load_candidates(conn)?;
    let matcher = SkimMatcherV2::default();
    let max = options.max_results.unwrap_or(usize::MAX);

    let mut matches: Vec<SearchSymbolMatch> = candidates
        .into_iter()
        .filter(|c| {
            // Apply kind filter if specified
            if let Some(ref kind_filter) = options.kind {
                match &c.kind {
                    Some(k) => k == kind_filter,
                    None => false,
                }
            } else {
                true
            }
        })
        .filter_map(|c| {
            // Try matching against qualified_path first (gives better context),
            // fall back to name
            let score = matcher
                .fuzzy_match(&c.qualified_path, query)
                .or_else(|| matcher.fuzzy_match(&c.name, query));

            score.map(|s| SearchSymbolMatch {
                name: c.name,
                qualified_path: c.qualified_path,
                kind: c.kind,
                file_path: c.file_path,
                start_line: c.start_line,
                score: s,
                source: c.source,
            })
        })
        .collect();

    matches.sort_by(|a, b| b.score.cmp(&a.score));
    matches.truncate(max);

    Ok(matches)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the qualified path from an `lsp_symbols.id` field.
fn qualified_path_from_id(id: &str, file_path: &str) -> String {
    let prefix = format!("lsp:{}:", file_path);
    if let Some(qpath) = id.strip_prefix(&prefix) {
        qpath.to_string()
    } else {
        id.to_string()
    }
}

/// Load all symbol candidates from both LSP and tree-sitter tables.
///
/// Deduplicates by `(file_path, start_line)`, preferring LSP.
fn load_candidates(conn: &Connection) -> Result<Vec<Candidate>, CodeContextError> {
    use std::collections::HashMap;

    let mut seen: HashMap<(String, u32), Candidate> = HashMap::new();

    // LSP symbols
    {
        let mut stmt =
            conn.prepare("SELECT id, name, kind, file_path, start_line FROM lsp_symbols")?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?, // id
                row.get::<_, String>(1)?, // name
                row.get::<_, i32>(2)?,    // kind
                row.get::<_, String>(3)?, // file_path
                row.get::<_, u32>(4)?,    // start_line
            ))
        })?;

        for row in rows {
            let (id, name, kind, file_path, start_line) = row?;
            let qpath = qualified_path_from_id(&id, &file_path);
            let key = (file_path.clone(), start_line);

            seen.insert(
                key,
                Candidate {
                    name,
                    qualified_path: qpath,
                    kind: symbol_kind_name(kind).map(|s| s.to_string()),
                    file_path,
                    start_line,
                    source: "lsp".to_string(),
                },
            );
        }
    }

    // Tree-sitter symbols
    {
        let mut stmt = conn.prepare(
            "SELECT file_path, start_line, symbol_path \
             FROM ts_chunks WHERE symbol_path IS NOT NULL",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?, // file_path
                row.get::<_, u32>(1)?,    // start_line
                row.get::<_, String>(2)?, // symbol_path
            ))
        })?;

        for row in rows {
            let (file_path, start_line, symbol_path) = row?;
            let key = (file_path.clone(), start_line);

            // Only insert if LSP hasn't already provided this location
            seen.entry(key).or_insert_with(|| {
                let name = symbol_path
                    .rsplit("::")
                    .next()
                    .unwrap_or(&symbol_path)
                    .to_string();
                Candidate {
                    name,
                    qualified_path: symbol_path,
                    kind: None,
                    file_path,
                    start_line,
                    source: "treesitter".to_string(),
                }
            });
        }
    }

    Ok(seen.into_values().collect())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{insert_file_simple as insert_file, test_db};

    /// Insert an LSP symbol (simplified: start_line only, char/end defaults to 0).
    fn insert_lsp_symbol(
        conn: &Connection,
        id: &str,
        name: &str,
        kind: i32,
        file_path: &str,
        start_line: u32,
    ) {
        crate::test_fixtures::insert_lsp_symbol(
            conn,
            id,
            name,
            kind,
            file_path,
            start_line as i32,
            0,
            start_line as i32,
            0,
            None,
        );
    }

    /// Insert a ts_chunks row with a required symbol_path.
    fn insert_ts_chunk(
        conn: &Connection,
        file_path: &str,
        start_line: u32,
        end_line: u32,
        symbol_path: &str,
    ) {
        crate::test_fixtures::insert_ts_chunk(
            conn,
            file_path,
            start_line as i32,
            end_line as i32,
            "source text",
            Some(symbol_path),
        );
    }

    /// Seed the database with standard test fixtures.
    fn seed_fixtures(conn: &Connection) {
        insert_file(conn, "src/lib.rs");
        insert_file(conn, "src/auth.rs");

        // LSP symbols
        insert_lsp_symbol(
            conn,
            "lsp:src/lib.rs:MyStruct",
            "MyStruct",
            23,
            "src/lib.rs",
            0,
        );
        insert_lsp_symbol(
            conn,
            "lsp:src/lib.rs:MyStruct::new",
            "new",
            12,
            "src/lib.rs",
            5,
        );
        insert_lsp_symbol(
            conn,
            "lsp:src/lib.rs:MyStruct::authenticate",
            "authenticate",
            6,
            "src/lib.rs",
            10,
        );
        insert_lsp_symbol(
            conn,
            "lsp:src/auth.rs:AuthService",
            "AuthService",
            5,
            "src/auth.rs",
            0,
        );
        insert_lsp_symbol(
            conn,
            "lsp:src/auth.rs:AuthService::validate",
            "validate",
            6,
            "src/auth.rs",
            15,
        );

        // Tree-sitter chunks
        insert_ts_chunk(conn, "src/lib.rs", 0, 20, "MyStruct");
        insert_ts_chunk(conn, "src/lib.rs", 5, 8, "MyStruct::new");
        insert_ts_chunk(conn, "src/lib.rs", 10, 15, "MyStruct::authenticate");
        insert_ts_chunk(conn, "src/auth.rs", 0, 30, "AuthService");
        insert_ts_chunk(conn, "src/auth.rs", 15, 20, "AuthService::validate");
    }

    #[test]
    fn test_search_symbol_fuzzy() {
        let conn = test_db();
        seed_fixtures(&conn);

        let results = search_symbol(&conn, "auth", &SearchSymbolOptions::default()).unwrap();

        assert!(
            results.len() >= 2,
            "expected at least authenticate + AuthService, got {}",
            results.len()
        );

        let paths: Vec<&str> = results.iter().map(|s| s.qualified_path.as_str()).collect();
        assert!(
            paths.contains(&"MyStruct::authenticate"),
            "expected authenticate in results: {:?}",
            paths
        );
        assert!(
            paths.contains(&"AuthService") || paths.contains(&"AuthService::validate"),
            "expected an AuthService symbol in results: {:?}",
            paths
        );
    }

    #[test]
    fn test_search_symbol_kind_filter() {
        let conn = test_db();
        seed_fixtures(&conn);

        let opts = SearchSymbolOptions {
            kind: Some("function".to_string()),
            ..Default::default()
        };
        let results = search_symbol(&conn, "new", &opts).unwrap();

        // Only MyStruct::new is kind=function (12)
        assert!(!results.is_empty(), "expected at least one function match");
        for r in &results {
            assert_eq!(
                r.kind,
                Some("function".to_string()),
                "all results should be functions, got {:?}",
                r.kind
            );
        }
    }

    #[test]
    fn test_search_symbol_no_match() {
        let conn = test_db();
        seed_fixtures(&conn);

        let results =
            search_symbol(&conn, "zzzznonexistent", &SearchSymbolOptions::default()).unwrap();

        assert!(results.is_empty());
    }

    #[test]
    fn test_search_symbol_max_results() {
        let conn = test_db();
        seed_fixtures(&conn);

        let opts = SearchSymbolOptions {
            max_results: Some(1),
            ..Default::default()
        };
        let results = search_symbol(&conn, "a", &opts).unwrap();

        assert!(results.len() <= 1);
    }
}
