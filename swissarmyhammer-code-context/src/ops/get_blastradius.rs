//! Blast radius analysis for a file or symbol.
//!
//! Starting from a file (optionally narrowed to a symbol), finds all
//! transitive inbound callers ("who calls this?") and aggregates the
//! impact per hop level.

use std::collections::HashSet;

use rusqlite::Connection;

use crate::error::CodeContextError;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Options for [`get_blastradius`].
#[derive(Debug, Clone)]
pub struct BlastRadiusOptions {
    /// File path to analyze.
    pub file_path: String,
    /// Optional symbol name within the file to narrow the starting set.
    pub symbol: Option<String>,
    /// Maximum number of hops to follow (1..=10, default 3).
    pub max_hops: u32,
}

impl Default for BlastRadiusOptions {
    fn default() -> Self {
        Self {
            file_path: String::new(),
            symbol: None,
            max_hops: 3,
        }
    }
}

/// A symbol affected by a change.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AffectedSymbol {
    /// Symbol ID (the `lsp_symbols.id` value).
    pub symbol_id: String,
    /// Human-readable name.
    pub name: String,
    /// File containing the symbol.
    pub file_path: String,
    /// Provenance of the edge that led here: `"lsp"` or `"treesitter"`.
    pub source: String,
}

/// Impact at a single hop distance.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HopLevel {
    /// Hop distance from the starting symbol(s).
    pub hop: u32,
    /// Symbols discovered at this hop.
    pub symbols: Vec<AffectedSymbol>,
    /// Number of distinct files affected at this hop.
    pub affected_files: usize,
}

/// Result of a blast radius analysis.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BlastRadius {
    /// Starting symbol IDs.
    pub roots: Vec<String>,
    /// Impact broken down by hop level.
    pub hops: Vec<HopLevel>,
    /// Total number of affected symbols (across all hops).
    pub total_affected_symbols: usize,
    /// Total number of affected files (across all hops).
    pub total_affected_files: usize,
}

// ---------------------------------------------------------------------------
// Core function
// ---------------------------------------------------------------------------

/// Compute the blast radius for a file or symbol.
///
/// Finds all symbols in the given file (optionally filtered by name),
/// then follows inbound call edges transitively up to `max_hops` levels.
/// Returns per-hop impact summaries.
///
/// # Errors
///
/// Returns [`CodeContextError::Database`] on SQLite failures.
/// Returns [`CodeContextError::Pattern`] if no symbols are found for the
/// given file/symbol combination.
pub fn get_blastradius(
    conn: &Connection,
    options: &BlastRadiusOptions,
) -> Result<BlastRadius, CodeContextError> {
    let max_hops = options.max_hops.clamp(1, 10);

    // Find starting symbols in the target file.
    let roots = find_roots(conn, &options.file_path, options.symbol.as_deref())?;

    if roots.is_empty() {
        return Err(CodeContextError::Pattern(format!(
            "no symbols found in file '{}' matching '{}'",
            options.file_path,
            options.symbol.as_deref().unwrap_or("*"),
        )));
    }

    let root_ids: Vec<String> = roots.iter().map(|r| r.0.clone()).collect();

    let mut visited: HashSet<String> = HashSet::new();
    for id in &root_ids {
        visited.insert(id.clone());
    }

    let mut hops: Vec<HopLevel> = Vec::new();
    let mut all_affected_files: HashSet<String> = HashSet::new();
    let mut total_symbols = 0usize;

    // BFS frontier: symbol IDs at the current hop.
    let mut frontier: Vec<String> = root_ids.clone();

    for hop in 1..=max_hops {
        let mut next_frontier: Vec<String> = Vec::new();
        let mut hop_symbols: Vec<AffectedSymbol> = Vec::new();
        let mut hop_files: HashSet<String> = HashSet::new();

        for symbol_id in &frontier {
            let callers = find_inbound_callers(conn, symbol_id)?;
            for (caller_id, caller_name, caller_file, source) in callers {
                if visited.insert(caller_id.clone()) {
                    hop_files.insert(caller_file.clone());
                    all_affected_files.insert(caller_file.clone());
                    hop_symbols.push(AffectedSymbol {
                        symbol_id: caller_id.clone(),
                        name: caller_name,
                        file_path: caller_file,
                        source,
                    });
                    next_frontier.push(caller_id);
                }
            }
        }

        if hop_symbols.is_empty() {
            break; // No more callers to discover.
        }

        total_symbols += hop_symbols.len();

        hops.push(HopLevel {
            hop,
            affected_files: hop_files.len(),
            symbols: hop_symbols,
        });

        frontier = next_frontier;
    }

    Ok(BlastRadius {
        roots: root_ids,
        hops,
        total_affected_symbols: total_symbols,
        total_affected_files: all_affected_files.len(),
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find all symbols in a file, optionally filtered by name.
///
/// Returns `(symbol_id, name)` pairs.
fn find_roots(
    conn: &Connection,
    file_path: &str,
    symbol_name: Option<&str>,
) -> Result<Vec<(String, String)>, CodeContextError> {
    let mut stmt = conn.prepare(
        "SELECT id, name FROM lsp_symbols WHERE file_path = ?1",
    )?;

    let rows = stmt.query_map([file_path], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut results = Vec::new();
    for row in rows {
        let (id, name) = row?;
        if let Some(filter) = symbol_name {
            // Match by exact name or suffix.
            let prefix = format!("lsp:{}:", file_path);
            let qpath = id.strip_prefix(&prefix).unwrap_or(&id);
            let suffix = format!("::{}", filter);
            if name != filter && qpath != filter && !qpath.ends_with(&suffix) {
                continue;
            }
        }
        results.push((id, name));
    }

    Ok(results)
}

/// Find all callers of a symbol (inbound edges).
///
/// Returns `(caller_id, caller_name, caller_file, source)`.
fn find_inbound_callers(
    conn: &Connection,
    callee_id: &str,
) -> Result<Vec<(String, String, String, String)>, CodeContextError> {
    let mut stmt = conn.prepare(
        "SELECT e.caller_id, s.name, e.caller_file, e.source \
         FROM lsp_call_edges e \
         JOIN lsp_symbols s ON s.id = e.caller_id \
         WHERE e.callee_id = ?1",
    )?;

    let rows = stmt.query_map([callee_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
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

    /// Insert an `indexed_files` row.
    fn insert_file(conn: &Connection, path: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO indexed_files (file_path, content_hash, file_size, last_seen_at)
             VALUES (?1, X'DEADBEEF', 1024, 1000)",
            [path],
        )
        .unwrap();
    }

    /// Insert an LSP symbol.
    fn insert_symbol(conn: &Connection, id: &str, name: &str, kind: i32, file_path: &str) {
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char)
             VALUES (?1, ?2, ?3, ?4, 0, 0, 10, 0)",
            rusqlite::params![id, name, kind, file_path],
        )
        .unwrap();
    }

    /// Insert a call edge with source provenance.
    fn insert_edge(conn: &Connection, caller_id: &str, callee_id: &str, source: &str) {
        let caller_file = caller_id
            .strip_prefix("lsp:")
            .and_then(|s| s.split(':').next())
            .unwrap_or("");
        let callee_file = callee_id
            .strip_prefix("lsp:")
            .and_then(|s| s.split(':').next())
            .unwrap_or("");

        conn.execute(
            "INSERT INTO lsp_call_edges (caller_id, callee_id, caller_file, callee_file, from_ranges, source)
             VALUES (?1, ?2, ?3, ?4, '[]', ?5)",
            rusqlite::params![caller_id, callee_id, caller_file, callee_file, source],
        )
        .unwrap();
    }

    /// Seed an A -> B -> C chain (A calls B, B calls C).
    fn seed_chain(conn: &Connection) {
        insert_file(conn, "src/a.rs");
        insert_file(conn, "src/b.rs");
        insert_file(conn, "src/c.rs");

        insert_symbol(conn, "lsp:src/a.rs:func_a", "func_a", 12, "src/a.rs");
        insert_symbol(conn, "lsp:src/b.rs:func_b", "func_b", 12, "src/b.rs");
        insert_symbol(conn, "lsp:src/c.rs:func_c", "func_c", 12, "src/c.rs");

        insert_edge(conn, "lsp:src/a.rs:func_a", "lsp:src/b.rs:func_b", "lsp");
        insert_edge(conn, "lsp:src/b.rs:func_b", "lsp:src/c.rs:func_c", "lsp");
    }

    #[test]
    fn test_blast_radius_single_hop() {
        let conn = test_db();
        seed_chain(&conn);

        let result = get_blastradius(
            &conn,
            &BlastRadiusOptions {
                file_path: "src/c.rs".to_string(),
                symbol: Some("func_c".to_string()),
                max_hops: 1,
            },
        )
        .unwrap();

        assert_eq!(result.hops.len(), 1);
        assert_eq!(result.hops[0].hop, 1);
        assert_eq!(result.hops[0].symbols.len(), 1);
        assert_eq!(result.hops[0].symbols[0].name, "func_b");
        assert_eq!(result.total_affected_symbols, 1);
        assert_eq!(result.total_affected_files, 1);
    }

    #[test]
    fn test_blast_radius_two_hops() {
        let conn = test_db();
        seed_chain(&conn);

        let result = get_blastradius(
            &conn,
            &BlastRadiusOptions {
                file_path: "src/c.rs".to_string(),
                symbol: Some("func_c".to_string()),
                max_hops: 2,
            },
        )
        .unwrap();

        assert_eq!(result.hops.len(), 2);

        // Hop 1: B calls C.
        assert_eq!(result.hops[0].hop, 1);
        assert_eq!(result.hops[0].symbols.len(), 1);
        assert_eq!(result.hops[0].symbols[0].name, "func_b");

        // Hop 2: A calls B.
        assert_eq!(result.hops[1].hop, 2);
        assert_eq!(result.hops[1].symbols.len(), 1);
        assert_eq!(result.hops[1].symbols[0].name, "func_a");

        assert_eq!(result.total_affected_symbols, 2);
        assert_eq!(result.total_affected_files, 2);
    }

    #[test]
    fn test_blast_radius_file_only() {
        let conn = test_db();
        seed_chain(&conn);

        // No symbol filter -- should use all symbols in src/c.rs.
        let result = get_blastradius(
            &conn,
            &BlastRadiusOptions {
                file_path: "src/c.rs".to_string(),
                symbol: None,
                max_hops: 3,
            },
        )
        .unwrap();

        assert_eq!(result.hops.len(), 2);
        assert_eq!(result.total_affected_symbols, 2);
    }

    #[test]
    fn test_blast_radius_no_callers() {
        let conn = test_db();
        seed_chain(&conn);

        // func_a is the top of the chain -- nobody calls it.
        let result = get_blastradius(
            &conn,
            &BlastRadiusOptions {
                file_path: "src/a.rs".to_string(),
                symbol: Some("func_a".to_string()),
                max_hops: 5,
            },
        )
        .unwrap();

        assert!(result.hops.is_empty());
        assert_eq!(result.total_affected_symbols, 0);
        assert_eq!(result.total_affected_files, 0);
    }

    #[test]
    fn test_blast_radius_mixed_provenance() {
        let conn = test_db();
        insert_file(&conn, "src/x.rs");
        insert_file(&conn, "src/y.rs");
        insert_file(&conn, "src/z.rs");

        insert_symbol(&conn, "lsp:src/x.rs:fn_x", "fn_x", 12, "src/x.rs");
        insert_symbol(&conn, "lsp:src/y.rs:fn_y", "fn_y", 12, "src/y.rs");
        insert_symbol(&conn, "lsp:src/z.rs:fn_z", "fn_z", 12, "src/z.rs");

        // fn_y -> fn_x via lsp, fn_z -> fn_x via treesitter.
        insert_edge(&conn, "lsp:src/y.rs:fn_y", "lsp:src/x.rs:fn_x", "lsp");
        insert_edge(
            &conn,
            "lsp:src/z.rs:fn_z",
            "lsp:src/x.rs:fn_x",
            "treesitter",
        );

        let result = get_blastradius(
            &conn,
            &BlastRadiusOptions {
                file_path: "src/x.rs".to_string(),
                symbol: Some("fn_x".to_string()),
                max_hops: 1,
            },
        )
        .unwrap();

        assert_eq!(result.hops.len(), 1);
        assert_eq!(result.hops[0].symbols.len(), 2);

        let lsp_sym = result.hops[0]
            .symbols
            .iter()
            .find(|s| s.name == "fn_y")
            .unwrap();
        assert_eq!(lsp_sym.source, "lsp");

        let ts_sym = result.hops[0]
            .symbols
            .iter()
            .find(|s| s.name == "fn_z")
            .unwrap();
        assert_eq!(ts_sym.source, "treesitter");
    }

    #[test]
    fn test_blast_radius_symbol_not_found() {
        let conn = test_db();
        insert_file(&conn, "src/empty.rs");
        // File exists in indexed_files but has no symbols.

        let result = get_blastradius(
            &conn,
            &BlastRadiusOptions {
                file_path: "src/empty.rs".to_string(),
                symbol: Some("nonexistent".to_string()),
                max_hops: 3,
            },
        );

        assert!(result.is_err());
    }
}
