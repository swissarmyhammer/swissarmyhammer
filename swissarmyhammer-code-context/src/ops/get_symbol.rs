//! Symbol lookup with multi-tier fuzzy matching.
//!
//! Returns the full source text of a symbol by name, along with location
//! metadata from both LSP and tree-sitter indices. The caller does not
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
//!
//! Both `lsp_symbols` and `ts_chunks` are queried and merged. When both
//! sources have a symbol at the same `(file_path, start_line)`, the result
//! carries LSP metadata (kind, detail, char positions) combined with the
//! tree-sitter source text.

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use rusqlite::Connection;
use std::collections::HashMap;

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
    /// First line of the symbol (1-indexed from TS, 0-indexed from LSP).
    pub start_line: u32,
    /// Last line of the symbol.
    pub end_line: u32,
    /// Start character position (precise from LSP, 0 from tree-sitter).
    pub start_char: u32,
    /// End character position (precise from LSP, 0 from tree-sitter).
    pub end_char: u32,
    /// Full source text of the chunk containing the symbol (empty for LSP-only symbols).
    pub text: String,
    /// Which tier produced this match.
    pub match_tier: MatchTier,
    /// Match score -- higher is better.
    pub score: i64,
    /// Symbol kind (e.g. "function", "struct"), if known from LSP.
    pub kind: Option<String>,
    /// LSP detail string, if available.
    pub detail: Option<String>,
    /// Which index produced this result: `"lsp"`, `"treesitter"`, or `"merged"`.
    pub source: String,
}

/// A symbol's definition location (used by `list_symbols` and other modules).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SymbolLocation {
    /// The symbol's short name (leaf segment).
    pub name: String,
    /// Fully qualified path (e.g. `MyStruct::new`).
    pub qualified_path: String,
    /// Symbol kind (e.g. "function", "struct"), if known.
    pub kind: Option<String>,
    /// File containing the symbol.
    pub file_path: String,
    /// Start line (0-based from LSP, 1-based from tree-sitter).
    pub start_line: u32,
    /// Start character (0-based from LSP, 0 from tree-sitter).
    pub start_char: u32,
    /// End line.
    pub end_line: u32,
    /// End character.
    pub end_char: u32,
    /// Optional detail string from the LSP server.
    pub detail: Option<String>,
    /// Which index produced this result: `"lsp"` or `"treesitter"`.
    pub source: String,
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
// Symbol kind mapping
// ---------------------------------------------------------------------------

/// Map an LSP `SymbolKind` integer to a human-readable name.
///
/// Covers the most common kinds; returns `None` for unknown values.
pub fn symbol_kind_name(kind: i32) -> Option<&'static str> {
    match kind {
        1 => Some("file"),
        2 => Some("module"),
        3 => Some("namespace"),
        5 => Some("class"),
        6 => Some("method"),
        8 => Some("field"),
        9 => Some("constructor"),
        10 => Some("enum"),
        11 => Some("interface"),
        12 => Some("function"),
        13 => Some("variable"),
        14 => Some("constant"),
        22 => Some("enum_member"),
        23 => Some("struct"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Internal row types
// ---------------------------------------------------------------------------

/// A row loaded from `ts_chunks` that has a non-NULL `symbol_path`.
struct TsRow {
    file_path: String,
    start_line: u32,
    end_line: u32,
    symbol_path: String,
    text: String,
}

/// A row loaded from `lsp_symbols`.
struct LspRow {
    file_path: String,
    start_line: u32,
    end_line: u32,
    start_char: u32,
    end_char: u32,
    symbol_path: String,
    name: String,
    kind: Option<String>,
    detail: Option<String>,
}

/// Merged symbol row combining data from both sources.
struct MergedRow {
    file_path: String,
    start_line: u32,
    end_line: u32,
    start_char: u32,
    end_char: u32,
    symbol_path: String,
    name: String,
    text: String,
    kind: Option<String>,
    detail: Option<String>,
    source: String,
}

// ---------------------------------------------------------------------------
// Core function
// ---------------------------------------------------------------------------

/// Look up symbols by name using multi-tier fuzzy matching.
///
/// Queries both `ts_chunks` and `lsp_symbols` tables, then applies four
/// matching tiers in order, returning results from the first tier that
/// produces any matches. When both sources have a symbol at the same
/// `(file_path, start_line)`, the result carries LSP metadata (kind,
/// detail, char positions) combined with tree-sitter source text.
///
/// # Errors
///
/// Returns [`CodeContextError::Database`] on SQLite failures.
pub fn get_symbol(
    conn: &Connection,
    query: &str,
    options: &GetSymbolOptions,
) -> Result<GetSymbolResult, CodeContextError> {
    let symbols = load_merged_rows(conn)?;
    let max = options.max_results.unwrap_or(usize::MAX);

    // Tier 1: Exact match
    let exact: Vec<&MergedRow> = symbols
        .iter()
        .filter(|s| s.symbol_path == query)
        .collect();
    if !exact.is_empty() {
        return Ok(make_result(query, &exact, MatchTier::Exact, 1000, max));
    }

    // Tier 2: Suffix match -- symbol_path ends with `::<query>`
    let suffix_pattern = format!("::{query}");
    let mut suffix: Vec<(&MergedRow, i64)> = symbols
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
    let ci: Vec<(&MergedRow, i64)> = symbols
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
    let mut fuzzy: Vec<(&MergedRow, i64)> = symbols
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
fn load_ts_rows(conn: &Connection) -> Result<Vec<TsRow>, CodeContextError> {
    let mut stmt = conn.prepare(
        "SELECT file_path, start_line, end_line, symbol_path, text \
         FROM ts_chunks WHERE symbol_path IS NOT NULL",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(TsRow {
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

/// Load all `lsp_symbols` rows.
fn load_lsp_rows(conn: &Connection) -> Result<Vec<LspRow>, CodeContextError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, kind, file_path, start_line, start_char, end_line, end_char, detail \
         FROM lsp_symbols",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,  // id
            row.get::<_, String>(1)?,  // name
            row.get::<_, i32>(2)?,     // kind
            row.get::<_, String>(3)?,  // file_path
            row.get::<_, u32>(4)?,     // start_line
            row.get::<_, u32>(5)?,     // start_char
            row.get::<_, u32>(6)?,     // end_line
            row.get::<_, u32>(7)?,     // end_char
            row.get::<_, Option<String>>(8)?, // detail
        ))
    })?;

    let mut out = Vec::new();
    for row in rows {
        let (id, name, kind, file_path, start_line, start_char, end_line, end_char, detail) =
            row?;
        let qpath = qualified_path_from_id(&id, &file_path);
        out.push(LspRow {
            file_path,
            start_line,
            end_line,
            start_char,
            end_char,
            symbol_path: qpath,
            name,
            kind: symbol_kind_name(kind).map(|s| s.to_string()),
            detail,
        });
    }
    Ok(out)
}

/// Load and merge rows from both `ts_chunks` and `lsp_symbols`.
///
/// Deduplicates by `(file_path, start_line)`. When both sources have an
/// entry at the same location, the merged result carries LSP metadata
/// (kind, detail, char positions) and tree-sitter source text.
fn load_merged_rows(conn: &Connection) -> Result<Vec<MergedRow>, CodeContextError> {
    let ts_rows = load_ts_rows(conn)?;
    let lsp_rows = load_lsp_rows(conn)?;

    // Key: (file_path, start_line) -> MergedRow
    let mut seen: HashMap<(String, u32), MergedRow> = HashMap::new();

    // Insert TS rows first (they have source text)
    for ts in ts_rows {
        let key = (ts.file_path.clone(), ts.start_line);
        seen.insert(
            key,
            MergedRow {
                name: leaf_name(&ts.symbol_path).to_string(),
                symbol_path: ts.symbol_path,
                file_path: ts.file_path,
                start_line: ts.start_line,
                end_line: ts.end_line,
                start_char: 0,
                end_char: 0,
                text: ts.text,
                kind: None,
                detail: None,
                source: "treesitter".to_string(),
            },
        );
    }

    // Merge LSP rows on top -- add metadata, or insert if no TS entry exists
    for lsp in lsp_rows {
        let key = (lsp.file_path.clone(), lsp.start_line);
        if let Some(existing) = seen.get_mut(&key) {
            // Merge: keep TS text, add LSP metadata
            existing.kind = lsp.kind;
            existing.detail = lsp.detail;
            existing.start_char = lsp.start_char;
            existing.end_char = lsp.end_char;
            // Prefer LSP qualified path and name (more accurate)
            existing.symbol_path = lsp.symbol_path;
            existing.name = lsp.name;
            existing.source = "merged".to_string();
        } else {
            // LSP-only symbol (no source text)
            seen.insert(
                key,
                MergedRow {
                    name: lsp.name,
                    symbol_path: lsp.symbol_path,
                    file_path: lsp.file_path,
                    start_line: lsp.start_line,
                    end_line: lsp.end_line,
                    start_char: lsp.start_char,
                    end_char: lsp.end_char,
                    text: String::new(),
                    kind: lsp.kind,
                    detail: lsp.detail,
                    source: "lsp".to_string(),
                },
            );
        }
    }

    Ok(seen.into_values().collect())
}

/// Extract the leaf name from a qualified path (e.g. `MyStruct::new` -> `new`).
fn leaf_name(symbol_path: &str) -> &str {
    symbol_path
        .rsplit("::")
        .next()
        .unwrap_or(symbol_path)
}

/// Extract the qualified path from an `lsp_symbols.id` field.
///
/// The ID format is `{source}:{file_path}:{qualified_path}` where source
/// is either `lsp` (real LSP symbols) or `ts` (synthetic symbols from
/// `ensure_ts_symbols`). We strip the prefix to get the qualified path.
fn qualified_path_from_id(id: &str, file_path: &str) -> String {
    // Try both lsp: and ts: prefixes
    for tag in &["lsp", "ts"] {
        let prefix = format!("{}:{}:", tag, file_path);
        if let Some(qpath) = id.strip_prefix(&prefix) {
            return qpath.to_string();
        }
    }
    // Fallback: return raw id
    id.to_string()
}

/// Build a result from a uniform-score set of matches.
fn make_result(
    query: &str,
    rows: &[&MergedRow],
    tier: MatchTier,
    score: i64,
    max: usize,
) -> GetSymbolResult {
    let symbols = rows
        .iter()
        .take(max)
        .map(|r| SymbolMatch {
            name: r.name.clone(),
            qualified_path: r.symbol_path.clone(),
            file_path: r.file_path.clone(),
            start_line: r.start_line,
            end_line: r.end_line,
            start_char: r.start_char,
            end_char: r.end_char,
            text: r.text.clone(),
            match_tier: tier.clone(),
            score,
            kind: r.kind.clone(),
            detail: r.detail.clone(),
            source: r.source.clone(),
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
    rows: &[(&MergedRow, i64)],
    tier: MatchTier,
    max: usize,
) -> GetSymbolResult {
    let symbols = rows
        .iter()
        .take(max)
        .map(|(r, score)| SymbolMatch {
            name: r.name.clone(),
            qualified_path: r.symbol_path.clone(),
            file_path: r.file_path.clone(),
            start_line: r.start_line,
            end_line: r.end_line,
            start_char: r.start_char,
            end_char: r.end_char,
            text: r.text.clone(),
            match_tier: tier.clone(),
            score: *score,
            kind: r.kind.clone(),
            detail: r.detail.clone(),
            source: r.source.clone(),
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

    /// Insert an LSP symbol.
    fn insert_lsp_symbol(
        conn: &Connection,
        id: &str,
        name: &str,
        kind: i32,
        file_path: &str,
        start_line: u32,
        start_char: u32,
        end_line: u32,
        end_char: u32,
        detail: Option<&str>,
    ) {
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char, detail)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![id, name, kind, file_path, start_line, start_char, end_line, end_char, detail],
        )
        .unwrap();
    }

    /// Seed the database with the standard test fixtures (TS only, no LSP).
    fn seed_ts_fixtures(conn: &Connection) {
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

    /// Seed the database with both TS and LSP fixtures at overlapping locations.
    fn seed_merged_fixtures(conn: &Connection) {
        insert_file(conn, "src/lib.rs");
        insert_file(conn, "src/auth.rs");

        // TS chunks
        insert_chunk(
            conn,
            "src/lib.rs",
            0,
            20,
            "MyStruct",
            "pub struct MyStruct { field: u32 }",
        );
        insert_chunk(
            conn,
            "src/lib.rs",
            5,
            8,
            "MyStruct::new",
            "fn new() -> Self { MyStruct { field: 0 } }",
        );
        insert_chunk(
            conn,
            "src/auth.rs",
            0,
            30,
            "AuthService",
            "pub struct AuthService { secret: String }",
        );

        // LSP symbols at same locations
        insert_lsp_symbol(
            conn,
            "lsp:src/lib.rs:MyStruct",
            "MyStruct",
            23, // struct
            "src/lib.rs",
            0, 0, 20, 1,
            None,
        );
        insert_lsp_symbol(
            conn,
            "lsp:src/lib.rs:MyStruct::new",
            "new",
            12, // function
            "src/lib.rs",
            5, 4, 8, 5,
            Some("fn() -> MyStruct"),
        );
        insert_lsp_symbol(
            conn,
            "lsp:src/auth.rs:AuthService",
            "AuthService",
            5, // class
            "src/auth.rs",
            0, 0, 30, 1,
            None,
        );

        // LSP-only symbol (no TS chunk at this location)
        insert_lsp_symbol(
            conn,
            "lsp:src/auth.rs:AuthService::validate",
            "validate",
            6, // method
            "src/auth.rs",
            15, 4, 20, 5,
            Some("fn(&self, token: &str) -> bool"),
        );
    }

    #[test]
    fn test_exact_match() {
        let conn = test_db();
        seed_ts_fixtures(&conn);

        let result = get_symbol(&conn, "MyStruct::new", &GetSymbolOptions::default()).unwrap();

        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].qualified_path, "MyStruct::new");
        assert_eq!(result.symbols[0].match_tier, MatchTier::Exact);
        assert_eq!(result.symbols[0].score, 1000);
    }

    #[test]
    fn test_suffix_match() {
        let conn = test_db();
        seed_ts_fixtures(&conn);

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
        seed_ts_fixtures(&conn);

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
        seed_ts_fixtures(&conn);

        let result = get_symbol(&conn, "auth", &GetSymbolOptions::default()).unwrap();

        // "auth" is a substring of "authenticate", "AuthService::new", "AuthService::validate"
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
        seed_ts_fixtures(&conn);

        let result =
            get_symbol(&conn, "zzzznonexistent", &GetSymbolOptions::default()).unwrap();

        assert!(result.symbols.is_empty());
    }

    #[test]
    fn test_max_results() {
        let conn = test_db();
        seed_ts_fixtures(&conn);

        let opts = GetSymbolOptions {
            max_results: Some(1),
        };
        let result = get_symbol(&conn, "new", &opts).unwrap();

        assert_eq!(result.symbols.len(), 1);
    }

    #[test]
    fn test_lsp_symbols_included() {
        let conn = test_db();
        seed_merged_fixtures(&conn);

        // LSP-only symbol (AuthService::validate has no TS chunk)
        let result = get_symbol(&conn, "AuthService::validate", &GetSymbolOptions::default()).unwrap();

        assert!(!result.symbols.is_empty(), "expected LSP-only symbol to be found");
        let sym = &result.symbols[0];
        assert_eq!(sym.qualified_path, "AuthService::validate");
        assert_eq!(sym.source, "lsp");
        assert_eq!(sym.kind, Some("method".to_string()));
        assert_eq!(sym.detail, Some("fn(&self, token: &str) -> bool".to_string()));
        assert_eq!(sym.start_char, 4);
        assert_eq!(sym.end_char, 5);
        // LSP-only symbols have empty text
        assert!(sym.text.is_empty());
    }

    #[test]
    fn test_merged_lsp_metadata_with_ts_text() {
        let conn = test_db();
        seed_merged_fixtures(&conn);

        // MyStruct::new exists in both TS and LSP at (src/lib.rs, 5)
        let result = get_symbol(&conn, "MyStruct::new", &GetSymbolOptions::default()).unwrap();

        assert_eq!(result.symbols.len(), 1);
        let sym = &result.symbols[0];
        assert_eq!(sym.qualified_path, "MyStruct::new");
        assert_eq!(sym.source, "merged");
        // LSP metadata present
        assert_eq!(sym.kind, Some("function".to_string()));
        assert_eq!(sym.detail, Some("fn() -> MyStruct".to_string()));
        assert_eq!(sym.start_char, 4);
        assert_eq!(sym.end_char, 5);
        // TS source text present
        assert!(!sym.text.is_empty());
        assert!(sym.text.contains("fn new()"));
    }

    #[test]
    fn test_dedup_prefers_lsp_metadata() {
        let conn = test_db();
        seed_merged_fixtures(&conn);

        // MyStruct exists in both TS and LSP at (src/lib.rs, 0)
        let result = get_symbol(&conn, "MyStruct", &GetSymbolOptions::default()).unwrap();

        let mystruct = result.symbols.iter().find(|s| s.file_path == "src/lib.rs" && s.start_line == 0);
        assert!(mystruct.is_some(), "expected merged MyStruct result");
        let sym = mystruct.unwrap();
        assert_eq!(sym.source, "merged");
        assert_eq!(sym.kind, Some("struct".to_string()));
        // Has TS source text
        assert!(!sym.text.is_empty());
    }

    #[test]
    fn test_all_four_tiers_with_lsp() {
        let conn = test_db();
        seed_merged_fixtures(&conn);

        // Tier 1: Exact
        let result = get_symbol(&conn, "MyStruct", &GetSymbolOptions::default()).unwrap();
        assert!(!result.symbols.is_empty());
        // At least one should have LSP metadata
        assert!(result.symbols.iter().any(|s| s.kind.is_some()));

        // Tier 2: Suffix
        let result = get_symbol(&conn, "new", &GetSymbolOptions::default()).unwrap();
        assert!(!result.symbols.is_empty());
        assert!(result.symbols.iter().all(|s| s.match_tier == MatchTier::Suffix));

        // Tier 3: Case-insensitive
        let result = get_symbol(&conn, "AUTHSERVICE", &GetSymbolOptions::default()).unwrap();
        assert!(!result.symbols.is_empty());
        assert!(result.symbols.iter().all(|s| s.match_tier == MatchTier::CaseInsensitive));

        // Tier 4: Fuzzy
        let result = get_symbol(&conn, "vldt", &GetSymbolOptions::default()).unwrap();
        assert!(!result.symbols.is_empty());
        assert!(result.symbols.iter().all(|s| s.match_tier == MatchTier::Fuzzy));
    }

    #[test]
    fn test_symbol_kind_name_mapping() {
        assert_eq!(symbol_kind_name(12), Some("function"));
        assert_eq!(symbol_kind_name(23), Some("struct"));
        assert_eq!(symbol_kind_name(6), Some("method"));
        assert_eq!(symbol_kind_name(5), Some("class"));
        assert_eq!(symbol_kind_name(999), None);
    }
}
