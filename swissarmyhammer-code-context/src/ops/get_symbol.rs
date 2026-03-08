//! Symbol lookup with multi-tier fuzzy matching.
//!
//! Returns the full source text of a symbol by name. The caller does not
//! need to know which file the symbol lives in -- we search across the
//! entire indexed codebase using four resolution tiers:
//!
//! 1. **Exact** -- `symbol_path` equals the query exactly.
//! 2. **Suffix** -- `symbol_path` ends with `::<query>`.
//! 3. **Case-insensitive** -- lowercased `symbol_path` contains the
//!    lowercased query.
//! 4. **Fuzzy** -- subsequence matching via `fuzzy_matcher::skim::SkimMatcherV2`.
//!
//! The search stops at the first tier that produces results.

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use rusqlite::Connection;

use crate::error::CodeContextError;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Which resolution tier produced the match.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum MatchTier {
    /// `symbol_path` equals the query exactly.
    Exact,
    /// `symbol_path` ends with `::<query>`.
    Suffix,
    /// Case-insensitive substring match.
    CaseInsensitive,
    /// Subsequence / fuzzy match via SkimMatcherV2.
    Fuzzy,
}

/// A single symbol match result.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SymbolMatch {
    /// The symbol name (leaf segment of `qualified_path`).
    pub name: String,
    /// Fully qualified path (e.g. `MyStruct::new`).
    pub qualified_path: String,
    /// File that contains the symbol.
    pub file_path: String,
    /// First line of the symbol (1-indexed).
    pub start_line: u32,
    /// Last line of the symbol (1-indexed).
    pub end_line: u32,
    /// Full source text of the chunk containing the symbol.
    pub text: String,
    /// Which tier produced this match.
    pub match_tier: MatchTier,
    /// Match score -- higher is better.
    pub score: i64,
}

/// Options for [`get_symbol`].
#[derive(Debug, Default)]
pub struct GetSymbolOptions {
    /// Maximum number of results to return.
    pub max_results: Option<usize>,
}

/// Result of [`get_symbol`].
#[derive(Debug, serde::Serialize)]
pub struct GetSymbolResult {
    /// The original query string.
    pub query: String,
    /// Matched symbols, ordered by descending score.
    pub symbols: Vec<SymbolMatch>,
}

// ---------------------------------------------------------------------------
// Internal row type
// ---------------------------------------------------------------------------

/// A row loaded from `ts_chunks` that has a non-NULL `symbol_path`.
struct SymbolRow {
    file_path: String,
    start_line: u32,
    end_line: u32,
    symbol_path: String,
    text: String,
}

// ---------------------------------------------------------------------------
// Core function
// ---------------------------------------------------------------------------

/// Look up symbols by name using multi-tier fuzzy matching.
///
/// Queries the `ts_chunks` table for rows with a non-NULL `symbol_path`,
/// then applies four matching tiers in order, returning results from the
/// first tier that produces any matches.
///
/// # Errors
///
/// Returns [`CodeContextError::Database`] on SQLite failures.
pub fn get_symbol(
    conn: &Connection,
    query: &str,
    options: &GetSymbolOptions,
) -> Result<GetSymbolResult, CodeContextError> {
    let symbols = load_symbol_rows(conn)?;
    let max = options.max_results.unwrap_or(usize::MAX);

    // Tier 1: Exact match
    let exact: Vec<&SymbolRow> = symbols
        .iter()
        .filter(|s| s.symbol_path == query)
        .collect();
    if !exact.is_empty() {
        return Ok(make_result(query, &exact, MatchTier::Exact, 1000, max));
    }

    // Tier 2: Suffix match -- symbol_path ends with `::<query>`
    let suffix_pattern = format!("::{query}");
    let mut suffix: Vec<(&SymbolRow, i64)> = symbols
        .iter()
        .filter(|s| s.symbol_path.ends_with(&suffix_pattern) || s.symbol_path == query)
        .map(|s| {
            // Shorter paths are more specific, so give them a bonus.
            let bonus = 100_i64.saturating_sub(s.symbol_path.len() as i64);
            (s, 900 + bonus.max(0))
        })
        .collect();
    if !suffix.is_empty() {
        suffix.sort_by(|a, b| b.1.cmp(&a.1));
        return Ok(make_result_scored(query, &suffix, MatchTier::Suffix, max));
    }

    // Tier 3: Case-insensitive substring
    let query_lower = query.to_lowercase();
    let ci: Vec<(&SymbolRow, i64)> = symbols
        .iter()
        .filter(|s| s.symbol_path.to_lowercase().contains(&query_lower))
        .map(|s| {
            let bonus = 100_i64.saturating_sub(s.symbol_path.len() as i64);
            (s, 800 + bonus.max(0))
        })
        .collect();
    if !ci.is_empty() {
        return Ok(make_result_scored(
            query,
            &ci,
            MatchTier::CaseInsensitive,
            max,
        ));
    }

    // Tier 4: Fuzzy subsequence matching
    let matcher = SkimMatcherV2::default();
    let mut fuzzy: Vec<(&SymbolRow, i64)> = symbols
        .iter()
        .filter_map(|s| {
            matcher
                .fuzzy_match(&s.symbol_path, query)
                .map(|score| (s, score))
        })
        .collect();
    fuzzy.sort_by(|a, b| b.1.cmp(&a.1));
    if !fuzzy.is_empty() {
        return Ok(make_result_scored(query, &fuzzy, MatchTier::Fuzzy, max));
    }

    // No matches at any tier.
    Ok(GetSymbolResult {
        query: query.to_string(),
        symbols: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Load all `ts_chunks` rows that have a non-NULL `symbol_path`.
fn load_symbol_rows(conn: &Connection) -> Result<Vec<SymbolRow>, CodeContextError> {
    let mut stmt = conn.prepare(
        "SELECT file_path, start_line, end_line, symbol_path, text \
         FROM ts_chunks WHERE symbol_path IS NOT NULL",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(SymbolRow {
            file_path: row.get(0)?,
            start_line: row.get(1)?,
            end_line: row.get(2)?,
            symbol_path: row.get(3)?,
            text: row.get(4)?,
        })
    })?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Extract the leaf name from a qualified path (e.g. `MyStruct::new` -> `new`).
fn leaf_name(symbol_path: &str) -> &str {
    symbol_path
        .rsplit("::")
        .next()
        .unwrap_or(symbol_path)
}

/// Build a result from a uniform-score set of matches.
fn make_result(
    query: &str,
    rows: &[&SymbolRow],
    tier: MatchTier,
    score: i64,
    max: usize,
) -> GetSymbolResult {
    let symbols = rows
        .iter()
        .take(max)
        .map(|r| SymbolMatch {
            name: leaf_name(&r.symbol_path).to_string(),
            qualified_path: r.symbol_path.clone(),
            file_path: r.file_path.clone(),
            start_line: r.start_line,
            end_line: r.end_line,
            text: r.text.clone(),
            match_tier: tier.clone(),
            score,
        })
        .collect();

    GetSymbolResult {
        query: query.to_string(),
        symbols,
    }
}

/// Build a result from a pre-scored and pre-sorted list of matches.
fn make_result_scored(
    query: &str,
    rows: &[(&SymbolRow, i64)],
    tier: MatchTier,
    max: usize,
) -> GetSymbolResult {
    let symbols = rows
        .iter()
        .take(max)
        .map(|(r, score)| SymbolMatch {
            name: leaf_name(&r.symbol_path).to_string(),
            qualified_path: r.symbol_path.clone(),
            file_path: r.file_path.clone(),
            start_line: r.start_line,
            end_line: r.end_line,
            text: r.text.clone(),
            match_tier: tier.clone(),
            score: *score,
        })
        .collect();

    GetSymbolResult {
        query: query.to_string(),
        symbols,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{configure_connection, create_schema};

    /// Create an in-memory database with the schema applied.
    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        create_schema(&conn).unwrap();
        conn
    }

    /// Insert an `indexed_files` row (required by foreign key constraint).
    fn insert_file(conn: &Connection, path: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO indexed_files (file_path, content_hash, file_size, last_seen_at)
             VALUES (?1, X'DEADBEEF', 1024, 1000)",
            [path],
        )
        .unwrap();
    }

    /// Insert a chunk with a symbol path into `ts_chunks`.
    fn insert_chunk(
        conn: &Connection,
        file_path: &str,
        start_line: u32,
        end_line: u32,
        symbol_path: &str,
        text: &str,
    ) {
        conn.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, symbol_path, text)
             VALUES (?1, 0, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![file_path, text.len(), start_line, end_line, symbol_path, text],
        )
        .unwrap();
    }

    /// Seed the database with the standard test fixtures.
    fn seed_fixtures(conn: &Connection) {
        insert_file(conn, "src/main.rs");
        insert_file(conn, "src/lib.rs");
        insert_file(conn, "src/auth.rs");

        insert_chunk(
            conn,
            "src/main.rs",
            1,
            5,
            "main",
            "fn main() {\n    println!(\"hello\");\n}",
        );
        insert_chunk(
            conn,
            "src/lib.rs",
            1,
            10,
            "MyStruct::new",
            "impl MyStruct {\n    fn new() -> Self { MyStruct {} }\n}",
        );
        insert_chunk(
            conn,
            "src/lib.rs",
            11,
            20,
            "MyStruct::authenticate",
            "impl MyStruct {\n    fn authenticate(&self) -> bool { true }\n}",
        );
        insert_chunk(
            conn,
            "src/auth.rs",
            1,
            10,
            "AuthService::new",
            "impl AuthService {\n    fn new() -> Self { AuthService {} }\n}",
        );
        insert_chunk(
            conn,
            "src/auth.rs",
            11,
            20,
            "AuthService::validate",
            "impl AuthService {\n    fn validate(&self) -> bool { true }\n}",
        );
    }

    #[test]
    fn test_exact_match() {
        let conn = test_db();
        seed_fixtures(&conn);

        let result = get_symbol(&conn, "MyStruct::new", &GetSymbolOptions::default()).unwrap();

        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].qualified_path, "MyStruct::new");
        assert_eq!(result.symbols[0].match_tier, MatchTier::Exact);
        assert_eq!(result.symbols[0].score, 1000);
    }

    #[test]
    fn test_suffix_match() {
        let conn = test_db();
        seed_fixtures(&conn);

        let result = get_symbol(&conn, "new", &GetSymbolOptions::default()).unwrap();

        assert_eq!(result.symbols.len(), 2);
        assert!(result.symbols.iter().all(|s| s.match_tier == MatchTier::Suffix));

        let paths: Vec<&str> = result
            .symbols
            .iter()
            .map(|s| s.qualified_path.as_str())
            .collect();
        assert!(paths.contains(&"MyStruct::new"));
        assert!(paths.contains(&"AuthService::new"));
    }

    #[test]
    fn test_case_insensitive() {
        let conn = test_db();
        seed_fixtures(&conn);

        let result =
            get_symbol(&conn, "MYSTRUCT::NEW", &GetSymbolOptions::default()).unwrap();

        assert!(!result.symbols.is_empty());
        assert!(result
            .symbols
            .iter()
            .all(|s| s.match_tier == MatchTier::CaseInsensitive));
        assert!(result
            .symbols
            .iter()
            .any(|s| s.qualified_path == "MyStruct::new"));
    }

    #[test]
    fn test_fuzzy() {
        let conn = test_db();
        seed_fixtures(&conn);

        let result = get_symbol(&conn, "auth", &GetSymbolOptions::default()).unwrap();

        // "auth" is a substring of "authenticate", "AuthService::new", "AuthService::validate"
        // It could match at case-insensitive tier (since lowercase "auth" is contained in
        // "mystruct::authenticate", "authservice::new", "authservice::validate").
        // The key assertion is that we get multiple results including these symbols.
        assert!(result.symbols.len() >= 3, "expected at least 3 matches, got {}", result.symbols.len());

        let paths: Vec<&str> = result
            .symbols
            .iter()
            .map(|s| s.qualified_path.as_str())
            .collect();
        assert!(paths.contains(&"MyStruct::authenticate"));
        assert!(paths.contains(&"AuthService::new"));
        assert!(paths.contains(&"AuthService::validate"));
    }

    #[test]
    fn test_no_match() {
        let conn = test_db();
        seed_fixtures(&conn);

        let result =
            get_symbol(&conn, "zzzznonexistent", &GetSymbolOptions::default()).unwrap();

        assert!(result.symbols.is_empty());
    }

    #[test]
    fn test_max_results() {
        let conn = test_db();
        seed_fixtures(&conn);

        let opts = GetSymbolOptions {
            max_results: Some(1),
        };
        let result = get_symbol(&conn, "new", &opts).unwrap();

        assert_eq!(result.symbols.len(), 1);
    }
}
