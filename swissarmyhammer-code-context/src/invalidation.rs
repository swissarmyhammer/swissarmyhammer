//! Incremental invalidation engine (2-generation strategy).
//!
//! **Generation 0 -- ReextractFile**: When a file changes, delete its old
//! symbols and outgoing edges, then write fresh symbols/edges supplied by the
//! caller (from LSP or tree-sitter).
//!
//! **Generation 1 -- RefreshEdges**: After reextraction, diff old vs new
//! symbol IDs.  For any symbol ID that was deleted (existed before, not after),
//! find files that had edges pointing TO that symbol (reverse lookup in
//! `lsp_call_edges.callee_id`).  Those files need their edges refreshed but
//! NOT their symbols re-extracted.
//!
//! **Key rule**: `RefreshEdges` never triggers further propagation -- this
//! closes the loop and prevents cascading re-indexing.

use std::collections::HashSet;

use rusqlite::Connection;

use crate::error::CodeContextError;
use crate::lsp_indexer::{write_edges, write_symbols, CallEdge, FlatSymbol};

/// Actions that the invalidation engine can produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidationAction {
    /// Full re-extract: delete old symbols+edges, write new ones.
    ReextractFile { file_path: String },
    /// Edges-only refresh: keep symbols, re-query outgoing calls.
    RefreshEdges { file_path: String },
}

/// Collect the current symbol IDs for a file from the DB.
///
/// Returns a `Vec<String>` of all `lsp_symbols.id` values where
/// `file_path` matches.
pub fn get_symbol_ids(conn: &Connection, file_path: &str) -> Result<Vec<String>, CodeContextError> {
    let mut stmt = conn.prepare_cached("SELECT id FROM lsp_symbols WHERE file_path = ?1")?;
    let ids: Vec<String> = stmt
        .query_map([file_path], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

/// Collect the current symbol IDs for a file from the DB as a `HashSet`.
///
/// Convenience wrapper around [`get_symbol_ids`] for fast set operations.
pub fn get_symbol_id_set(
    conn: &Connection,
    file_path: &str,
) -> Result<HashSet<String>, CodeContextError> {
    Ok(get_symbol_ids(conn, file_path)?.into_iter().collect())
}

/// Find files that have outgoing edges pointing to any of the given callee
/// symbol IDs (reverse lookup: "who calls these symbols?").
///
/// Excludes `exclude_file` from the results so the triggering file is not
/// included in its own propagation list.
///
/// Uses a dynamically built `IN` clause since rusqlite does not support
/// native array binding.
pub fn find_reverse_edge_files(
    conn: &Connection,
    callee_ids: &[String],
    exclude_file: &str,
) -> Result<Vec<String>, CodeContextError> {
    if callee_ids.is_empty() {
        return Ok(Vec::new());
    }

    // Build "?,?,?" placeholders
    let placeholders: Vec<&str> = callee_ids.iter().map(|_| "?").collect();
    let sql = format!(
        "SELECT DISTINCT caller_file FROM lsp_call_edges \
         WHERE callee_id IN ({}) AND caller_file != ?",
        placeholders.join(", ")
    );

    let mut stmt = conn.prepare(&sql)?;

    // Bind callee IDs (1-based) then the exclude file
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = callee_ids
        .iter()
        .map(|id| Box::new(id.clone()) as Box<dyn rusqlite::types::ToSql>)
        .collect();
    params.push(Box::new(exclude_file.to_string()));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let files: Vec<String> = stmt
        .query_map(param_refs.as_slice(), |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(files)
}

/// Execute a ReextractFile: snapshot old symbol IDs, write new symbols and
/// edges, then compute 1-hop propagation actions.
///
/// Steps:
/// 1. Snapshot old symbol IDs for the file.
/// 2. Call `write_symbols` (delete-then-insert) and `write_edges`.
/// 3. Compute new symbol ID set from `new_symbols`.
/// 4. Find deleted IDs: old - new.
/// 5. Reverse-lookup files with edges to deleted IDs.
/// 6. Return `RefreshEdges` for each affected file (excluding self).
///
/// **Important**: The old symbol IDs and reverse-edge files must be captured
/// *before* `write_symbols` runs, because CASCADE deletes on `lsp_symbols`
/// will also remove the edges we need to query.
pub fn reextract_file(
    conn: &Connection,
    file_path: &str,
    new_symbols: &[FlatSymbol],
    new_edges: &[CallEdge],
) -> Result<Vec<InvalidationAction>, CodeContextError> {
    // 1. Snapshot old symbol IDs
    let old_ids = get_symbol_id_set(conn, file_path)?;

    // 2. Compute new symbol IDs from the incoming symbols
    let new_ids: HashSet<String> = new_symbols.iter().map(|s| s.id.clone()).collect();

    // 3. Find deleted IDs: old - new
    let deleted_ids: Vec<String> = old_ids.difference(&new_ids).cloned().collect();

    // 4. Find files with reverse edges to deleted symbols BEFORE we delete them
    let affected_files = find_reverse_edge_files(conn, &deleted_ids, file_path)?;

    // 5. Now write (delete old + insert new)
    write_symbols(conn, file_path, new_symbols)?;
    write_edges(conn, file_path, new_edges)?;

    // 6. Build RefreshEdges actions
    let actions = affected_files
        .into_iter()
        .map(|fp| InvalidationAction::RefreshEdges { file_path: fp })
        .collect();

    Ok(actions)
}

/// Execute a RefreshEdges: delete old outgoing edges for the file, write new
/// ones.
///
/// Does **not** trigger further propagation (closes the loop).
pub fn refresh_edges(
    conn: &Connection,
    file_path: &str,
    new_edges: &[CallEdge],
) -> Result<(), CodeContextError> {
    write_edges(conn, file_path, new_edges)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use lsp_types::SymbolKind;

    /// Open an in-memory DB with the full schema.
    fn open_memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        db::configure_connection(&conn).unwrap();
        db::create_schema(&conn).unwrap();
        conn
    }

    /// Insert a file row so foreign-key constraints are satisfied.
    fn seed_file(conn: &Connection, path: &str) {
        conn.execute(
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at)
             VALUES (?1, X'00112233', 1024, 1000)",
            [path],
        )
        .unwrap();
    }

    /// Helper to build a `FlatSymbol` with minimal boilerplate.
    fn make_symbol(file_path: &str, qualified_path: &str) -> FlatSymbol {
        let id = format!("lsp:{file_path}:{qualified_path}");
        let name = qualified_path
            .rsplit("::")
            .next()
            .unwrap_or(qualified_path)
            .to_string();
        FlatSymbol {
            id,
            name,
            kind: SymbolKind::FUNCTION,
            file_path: file_path.to_string(),
            qualified_path: qualified_path.to_string(),
            start_line: 0,
            start_char: 0,
            end_line: 5,
            end_char: 1,
            detail: None,
        }
    }

    /// Helper to build a `CallEdge`.
    fn make_edge(
        caller_file: &str,
        caller_qpath: &str,
        callee_file: &str,
        callee_qpath: &str,
    ) -> CallEdge {
        CallEdge {
            caller_id: format!("lsp:{caller_file}:{caller_qpath}"),
            callee_id: format!("lsp:{callee_file}:{callee_qpath}"),
            caller_file: caller_file.to_string(),
            callee_file: callee_file.to_string(),
            from_ranges: "[]".to_string(),
            source: "lsp".to_string(),
        }
    }

    #[test]
    fn test_get_symbol_ids() {
        let conn = open_memory_db();
        seed_file(&conn, "src/f.rs");

        let sym_a = make_symbol("src/f.rs", "A");
        let sym_b = make_symbol("src/f.rs", "B");
        write_symbols(&conn, "src/f.rs", &[sym_a, sym_b]).unwrap();

        let mut ids = get_symbol_ids(&conn, "src/f.rs").unwrap();
        ids.sort();

        assert_eq!(ids, vec!["lsp:src/f.rs:A", "lsp:src/f.rs:B"]);
    }

    #[test]
    fn test_find_reverse_edge_files() {
        let conn = open_memory_db();
        seed_file(&conn, "src/a.rs");
        seed_file(&conn, "src/b.rs");
        seed_file(&conn, "src/c.rs");

        // Symbols: A in a.rs, B_sym in b.rs, C in c.rs
        let sym_a = make_symbol("src/a.rs", "A");
        let sym_b = make_symbol("src/b.rs", "B_sym");
        let sym_c = make_symbol("src/c.rs", "C");
        write_symbols(&conn, "src/a.rs", &[sym_a]).unwrap();
        write_symbols(&conn, "src/b.rs", &[sym_b]).unwrap();
        write_symbols(&conn, "src/c.rs", &[sym_c]).unwrap();

        // Edges: A -> B_sym, C -> B_sym
        let edge_ab = make_edge("src/a.rs", "A", "src/b.rs", "B_sym");
        let edge_cb = make_edge("src/c.rs", "C", "src/b.rs", "B_sym");
        write_edges(&conn, "src/a.rs", &[edge_ab]).unwrap();
        write_edges(&conn, "src/c.rs", &[edge_cb]).unwrap();

        // Reverse lookup: who calls B_sym?
        let callee_ids = vec!["lsp:src/b.rs:B_sym".to_string()];
        let mut files = find_reverse_edge_files(&conn, &callee_ids, "src/b.rs").unwrap();
        files.sort();

        assert_eq!(files, vec!["src/a.rs", "src/c.rs"]);
    }

    #[test]
    fn test_reextract_triggers_refresh_for_affected_files() {
        let conn = open_memory_db();
        seed_file(&conn, "src/f.rs");
        seed_file(&conn, "src/g.rs");

        // File F has symbol A, file G has symbol foo
        let sym_a = make_symbol("src/f.rs", "A");
        let sym_foo = make_symbol("src/g.rs", "foo");
        write_symbols(&conn, "src/f.rs", &[sym_a]).unwrap();
        write_symbols(&conn, "src/g.rs", &[sym_foo]).unwrap();

        // Edge: foo -> A (G calls F's symbol A)
        let edge = make_edge("src/g.rs", "foo", "src/f.rs", "A");
        write_edges(&conn, "src/g.rs", &[edge]).unwrap();

        // Reextract F with NO symbols (A is deleted)
        let actions = reextract_file(&conn, "src/f.rs", &[], &[]).unwrap();

        assert_eq!(
            actions,
            vec![InvalidationAction::RefreshEdges {
                file_path: "src/g.rs".to_string()
            }]
        );
    }

    #[test]
    fn test_refresh_edges_no_propagation() {
        let conn = open_memory_db();
        seed_file(&conn, "src/f.rs");
        seed_file(&conn, "src/g.rs");

        // Set up symbols
        let sym_a = make_symbol("src/f.rs", "A");
        let sym_foo = make_symbol("src/g.rs", "foo");
        write_symbols(&conn, "src/f.rs", &[sym_a]).unwrap();
        write_symbols(&conn, "src/g.rs", &[sym_foo]).unwrap();

        // Call refresh_edges on G with new edges
        let edge = make_edge("src/g.rs", "foo", "src/f.rs", "A");
        let result = refresh_edges(&conn, "src/g.rs", &[edge]);

        // Should succeed with no further actions
        assert!(result.is_ok());

        // Verify edge was written
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lsp_call_edges WHERE caller_file = 'src/g.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_reextract_with_renamed_symbol() {
        let conn = open_memory_db();
        seed_file(&conn, "src/f.rs");
        seed_file(&conn, "src/g.rs");

        // F has old_name, G calls old_name
        let sym_old = make_symbol("src/f.rs", "old_name");
        let sym_foo = make_symbol("src/g.rs", "foo");
        write_symbols(&conn, "src/f.rs", &[sym_old]).unwrap();
        write_symbols(&conn, "src/g.rs", &[sym_foo]).unwrap();

        let edge = make_edge("src/g.rs", "foo", "src/f.rs", "old_name");
        write_edges(&conn, "src/g.rs", &[edge]).unwrap();

        // Reextract F with new_name (old_name is gone -> rename)
        let sym_new = make_symbol("src/f.rs", "new_name");
        let actions = reextract_file(&conn, "src/f.rs", &[sym_new], &[]).unwrap();

        // Should trigger RefreshEdges for G because old_name was deleted
        assert_eq!(
            actions,
            vec![InvalidationAction::RefreshEdges {
                file_path: "src/g.rs".to_string()
            }]
        );
    }

    #[test]
    fn test_reextract_no_changes() {
        let conn = open_memory_db();
        seed_file(&conn, "src/f.rs");
        seed_file(&conn, "src/g.rs");

        // F has symbol A, G calls A
        let sym_a = make_symbol("src/f.rs", "A");
        let sym_foo = make_symbol("src/g.rs", "foo");
        write_symbols(&conn, "src/f.rs", std::slice::from_ref(&sym_a)).unwrap();
        write_symbols(&conn, "src/g.rs", &[sym_foo]).unwrap();

        let edge = make_edge("src/g.rs", "foo", "src/f.rs", "A");
        write_edges(&conn, "src/g.rs", &[edge]).unwrap();

        // Reextract F with same symbol A -> no deletions -> no RefreshEdges
        let actions = reextract_file(&conn, "src/f.rs", &[sym_a], &[]).unwrap();

        assert!(actions.is_empty());
    }
}
