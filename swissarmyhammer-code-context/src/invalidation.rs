//! Incremental invalidation engine for LSP symbol tracking.
//!
//! The engine tracks which symbols exist in a file before and after re-extraction
//! and uses that diff to propagate edge refreshes to dependent files.
//!
//! The invalidation action produced is [`InvalidationAction::RefreshEdges`],
//! which the worker applies by flipping `lsp_indexed = 0` on the dependent file.
//! On the next worker pass the dependent is re-queried end-to-end (symbols and
//! outgoing call edges), which rewrites its edge set against the current symbol
//! universe — including the fact that a previously-referenced callee is gone.
//!
//! **Propagation terminates at one hop.** `RefreshEdges` only re-queries the
//! flagged file's outgoing edges; it does not re-run symbol extraction for that
//! file. Since edge rewriting cannot produce new deleted symbols, the cycle
//! cannot continue past the first generation.
//!
//! Full-file re-extraction (e.g. after a watcher-observed content change) is
//! handled by flipping `ts_indexed` and `lsp_indexed` to `0` directly on the
//! file row — the watcher does this in [`crate::watcher::FanoutWatcher::notify`]
//! and the worker picks the file up automatically. That path does not flow
//! through this module.

use std::collections::HashSet;

use rusqlite::Connection;

use crate::error::CodeContextError;
use crate::lsp_indexer::{symbol_kind_to_i32, FlatSymbol};

/// Maximum number of `?` bind parameters in a single prepared statement.
///
/// SQLite ships with a default `SQLITE_MAX_VARIABLE_NUMBER` of 32766 on modern
/// builds (set at compile time — older builds cap at 999). We use a
/// conservative 900 so a single chunk comfortably fits under either bound
/// while leaving room for auxiliary bind parameters (e.g. the `exclude_file`
/// in [`find_reverse_edge_files`]).
const SQLITE_IN_CHUNK_SIZE: usize = 900;

/// Actions that the invalidation engine can produce.
///
/// Only one variant exists today. Additional variants may be added if a future
/// action cannot be expressed as an edges-only refresh — but the propagation
/// invariant (no cascading re-extracts) must be preserved so the algorithm
/// terminates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidationAction {
    /// The listed file's outgoing call edges need to be refreshed because at
    /// least one of its callees was deleted from the triggering file.
    ///
    /// The worker applies this by flipping `lsp_indexed = 0` on the file; the
    /// next pass re-queries the LSP call hierarchy and replaces the file's
    /// edge rows with fresh data. Symbol rows are left untouched.
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
/// Chunks the input through [`SQLITE_IN_CHUNK_SIZE`] so the dynamic `IN`
/// clause never exceeds SQLite's bind-parameter limit even on pathological
/// symbol counts. Results are deduplicated across chunks.
pub fn find_reverse_edge_files(
    conn: &Connection,
    callee_ids: &[String],
    exclude_file: &str,
) -> Result<Vec<String>, CodeContextError> {
    if callee_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut found: HashSet<String> = HashSet::new();

    for chunk in callee_ids.chunks(SQLITE_IN_CHUNK_SIZE) {
        let placeholders: Vec<&str> = chunk.iter().map(|_| "?").collect();
        let sql = format!(
            "SELECT DISTINCT caller_file FROM lsp_call_edges \
             WHERE callee_id IN ({}) AND caller_file != ?",
            placeholders.join(", ")
        );

        let mut stmt = conn.prepare(&sql)?;

        // Bind callee IDs followed by the exclude file.
        let params =
            rusqlite::params_from_iter(chunk.iter().map(|s| s.as_str()).chain([exclude_file]));

        let rows = stmt
            .query_map(params, |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        found.extend(rows);
    }

    Ok(found.into_iter().collect())
}

/// Re-extract only the symbols for a file, preserving any outgoing call edges
/// whose caller symbol still exists.
///
/// This is the LSP worker's symbol-pass primitive. It is paired with a
/// separate call-hierarchy pass that rewrites outgoing edges; the two passes
/// together keep a file's symbol and edge state aligned.
///
/// Steps (in order, all under the caller's transaction/connection):
///
/// 1. Snapshot old symbol IDs for `file_path`.
/// 2. Compute the new-symbol ID set from `new_symbols`.
/// 3. Diff: `deleted_ids = old - new`.
/// 4. Reverse-lookup dependent files that had outgoing edges to any deleted
///    symbol (must be done **before** step 5 because the `lsp_symbols`
///    CASCADE on step 5 would otherwise remove the rows we are querying).
/// 5. `DELETE FROM lsp_symbols WHERE id IN (deleted_ids)`. CASCADE cleans up
///    edges that depend on these rows as caller or callee.
/// 6. Upsert `new_symbols` via `INSERT ... ON CONFLICT DO UPDATE` — this
///    updates existing rows in place (preserving their outgoing edges) and
///    inserts new ones.
/// 7. Return one [`InvalidationAction::RefreshEdges`] per dependent file.
///
/// Using `INSERT OR REPLACE` here would be incorrect: SQLite implements it as
/// DELETE + INSERT, which would fire the `lsp_call_edges` CASCADE on every row
/// — wiping the edges we are trying to preserve.
pub fn reextract_symbols(
    conn: &Connection,
    file_path: &str,
    new_symbols: &[FlatSymbol],
) -> Result<Vec<InvalidationAction>, CodeContextError> {
    // 1. Snapshot old symbol IDs.
    let old_ids = get_symbol_id_set(conn, file_path)?;

    // 2. Compute new symbol IDs from the incoming symbols.
    let new_ids: HashSet<String> = new_symbols.iter().map(|s| s.id.clone()).collect();

    // 3. Find deleted IDs: old - new.
    let deleted_ids: Vec<String> = old_ids.difference(&new_ids).cloned().collect();

    // 4. Find files with reverse edges to deleted symbols BEFORE we delete them.
    let affected_files = find_reverse_edge_files(conn, &deleted_ids, file_path)?;

    // 5. Delete only the rows that actually disappeared. CASCADE cleans up
    //    edges where they appear as caller or callee.
    delete_symbols_by_id(conn, &deleted_ids)?;

    // 6. UPSERT via ON CONFLICT: update existing rows in place (preserves
    //    their outgoing/inbound edges) and insert new ones.
    upsert_symbols(conn, new_symbols)?;

    // 7. Build RefreshEdges actions.
    let actions = affected_files
        .into_iter()
        .map(|fp| InvalidationAction::RefreshEdges { file_path: fp })
        .collect();

    Ok(actions)
}

/// Delete a batch of `lsp_symbols` rows by primary key.
///
/// Chunks the input through [`SQLITE_IN_CHUNK_SIZE`] so the generated `IN`
/// clause never exceeds SQLite's bind-parameter limit.
fn delete_symbols_by_id(conn: &Connection, ids: &[String]) -> Result<(), CodeContextError> {
    if ids.is_empty() {
        return Ok(());
    }

    for chunk in ids.chunks(SQLITE_IN_CHUNK_SIZE) {
        let placeholders: Vec<&str> = chunk.iter().map(|_| "?").collect();
        let sql = format!(
            "DELETE FROM lsp_symbols WHERE id IN ({})",
            placeholders.join(", ")
        );
        let mut stmt = conn.prepare(&sql)?;
        stmt.execute(rusqlite::params_from_iter(chunk.iter().map(|s| s.as_str())))?;
    }

    Ok(())
}

/// Insert new symbols and update existing rows in place using `ON CONFLICT`.
///
/// Unlike `INSERT OR REPLACE` (which SQLite implements as DELETE + INSERT and
/// therefore cascades away edges on every conflicting row), `ON CONFLICT DO
/// UPDATE` modifies the row in place without firing CASCADE. That is
/// essential for the symbols-only re-extract path — we must keep the edges
/// of unchanged symbols intact.
///
/// **Trigger note**: any future trigger attached to `lsp_symbols` must account
/// for both paths. `ON CONFLICT DO UPDATE` fires `AFTER UPDATE` triggers per
/// conflicting row (and `AFTER INSERT` per new row), while `INSERT OR REPLACE`
/// fires `AFTER DELETE` + `AFTER INSERT`. Do not assume the two paths are
/// interchangeable when adding trigger-based invariants.
fn upsert_symbols(conn: &Connection, symbols: &[FlatSymbol]) -> Result<(), CodeContextError> {
    if symbols.is_empty() {
        return Ok(());
    }

    let mut stmt = conn.prepare_cached(
        "INSERT INTO lsp_symbols \
         (id, name, kind, file_path, start_line, start_char, end_line, end_char, detail) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
         ON CONFLICT(id) DO UPDATE SET \
            name = excluded.name, \
            kind = excluded.kind, \
            file_path = excluded.file_path, \
            start_line = excluded.start_line, \
            start_char = excluded.start_char, \
            end_line = excluded.end_line, \
            end_char = excluded.end_char, \
            detail = excluded.detail",
    )?;

    for sym in symbols {
        stmt.execute(rusqlite::params![
            sym.id,
            sym.name,
            symbol_kind_to_i32(sym.kind),
            sym.file_path,
            sym.start_line,
            sym.start_char,
            sym.end_line,
            sym.end_char,
            sym.detail,
        ])?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::lsp_indexer::{write_edges, write_symbols, CallEdge};
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
    fn test_find_reverse_edge_files_chunks_over_limit() {
        // Pathological: more deleted callee IDs than fit in one bind-parameter
        // batch. The function must chunk the IN clause and still return the
        // complete, de-duplicated set of caller files.
        let conn = open_memory_db();
        seed_file(&conn, "src/callee.rs");
        seed_file(&conn, "src/caller.rs");

        // Build (SQLITE_IN_CHUNK_SIZE + 50) callee symbols in src/callee.rs
        // plus a single caller symbol in src/caller.rs that calls all of them.
        let total = SQLITE_IN_CHUNK_SIZE + 50;
        let mut callees: Vec<FlatSymbol> = Vec::with_capacity(total);
        let mut callee_ids: Vec<String> = Vec::with_capacity(total);
        for i in 0..total {
            let qpath = format!("callee_{i}");
            let sym = make_symbol("src/callee.rs", &qpath);
            callee_ids.push(sym.id.clone());
            callees.push(sym);
        }
        write_symbols(&conn, "src/callee.rs", &callees).unwrap();

        let caller_sym = make_symbol("src/caller.rs", "caller_fn");
        write_symbols(&conn, "src/caller.rs", &[caller_sym]).unwrap();

        let edges: Vec<CallEdge> = (0..total)
            .map(|i| {
                make_edge(
                    "src/caller.rs",
                    "caller_fn",
                    "src/callee.rs",
                    &format!("callee_{i}"),
                )
            })
            .collect();
        write_edges(&conn, "src/caller.rs", &edges).unwrap();

        let files = find_reverse_edge_files(&conn, &callee_ids, "src/callee.rs").unwrap();
        assert_eq!(files, vec!["src/caller.rs"]);
    }

    // ── reextract_symbols tests ────────────────────────────────────────
    //
    // The symbols-only variant must preserve existing outgoing edges for the
    // target file while still propagating `RefreshEdges` actions to files
    // that called into deleted/renamed symbols.

    #[test]
    fn test_reextract_symbols_preserves_outgoing_edges() {
        // When re-extracting only symbols, the caller file's own outgoing
        // edges must not be touched — they were produced by a separate
        // call-hierarchy request and are still valid until a full re-extract.
        let conn = open_memory_db();
        seed_file(&conn, "src/f.rs");
        seed_file(&conn, "src/g.rs");

        // F calls G's symbol g_sym — edge lives on F (caller_file = src/f.rs)
        let sym_a = make_symbol("src/f.rs", "A");
        let sym_g = make_symbol("src/g.rs", "g_sym");
        write_symbols(&conn, "src/f.rs", std::slice::from_ref(&sym_a)).unwrap();
        write_symbols(&conn, "src/g.rs", &[sym_g]).unwrap();

        let edge = make_edge("src/f.rs", "A", "src/g.rs", "g_sym");
        write_edges(&conn, "src/f.rs", &[edge]).unwrap();

        // Re-extract F's symbols only (same symbol set).
        let actions = reextract_symbols(&conn, "src/f.rs", &[sym_a]).unwrap();
        assert!(
            actions.is_empty(),
            "no symbol churn should produce no actions"
        );

        // F's outgoing edge must still be present.
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lsp_call_edges WHERE caller_file = 'src/f.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 1,
            "outgoing edge on F must survive symbols-only re-extract"
        );
    }

    #[test]
    fn test_reextract_symbols_deleted_symbol_triggers_refresh() {
        // Classic rename/delete: F loses a symbol that G was calling.
        // reextract_symbols should emit a RefreshEdges action for G.
        let conn = open_memory_db();
        seed_file(&conn, "src/f.rs");
        seed_file(&conn, "src/g.rs");

        let sym_old = make_symbol("src/f.rs", "old_name");
        let sym_foo = make_symbol("src/g.rs", "foo");
        write_symbols(&conn, "src/f.rs", &[sym_old]).unwrap();
        write_symbols(&conn, "src/g.rs", &[sym_foo]).unwrap();

        // G calls F::old_name
        let edge = make_edge("src/g.rs", "foo", "src/f.rs", "old_name");
        write_edges(&conn, "src/g.rs", &[edge]).unwrap();

        // F is re-extracted with a renamed symbol
        let sym_new = make_symbol("src/f.rs", "new_name");
        let actions = reextract_symbols(&conn, "src/f.rs", &[sym_new]).unwrap();

        assert_eq!(
            actions,
            vec![InvalidationAction::RefreshEdges {
                file_path: "src/g.rs".to_string()
            }]
        );
    }

    #[test]
    fn test_reextract_symbols_caller_rename_cascade_deletes_own_edges() {
        // When a caller symbol in F is renamed, the CASCADE constraint on
        // lsp_call_edges.caller_id removes F's outgoing edges for that
        // caller. This is correct: the edge's caller_id no longer resolves.
        // We document the behaviour here so callers of reextract_symbols
        // know they may need to re-collect call hierarchy afterwards.
        let conn = open_memory_db();
        seed_file(&conn, "src/f.rs");
        seed_file(&conn, "src/g.rs");

        let sym_a = make_symbol("src/f.rs", "A");
        let sym_g = make_symbol("src/g.rs", "g_sym");
        write_symbols(&conn, "src/f.rs", &[sym_a]).unwrap();
        write_symbols(&conn, "src/g.rs", &[sym_g]).unwrap();

        // F::A calls G::g_sym
        let edge = make_edge("src/f.rs", "A", "src/g.rs", "g_sym");
        write_edges(&conn, "src/f.rs", &[edge]).unwrap();

        // Rename F's A -> A_renamed (A disappears from F)
        let sym_renamed = make_symbol("src/f.rs", "A_renamed");
        let _actions = reextract_symbols(&conn, "src/f.rs", &[sym_renamed]).unwrap();

        // F's edge (caller_id = lsp:src/f.rs:A) cascaded away because the
        // caller symbol was deleted.
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lsp_call_edges WHERE caller_file = 'src/f.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 0,
            "cascade removes edges whose caller symbol was deleted"
        );
    }

    #[test]
    fn test_reextract_symbols_no_deleted_symbols_no_actions() {
        // Adding new symbols without removing any should not emit actions.
        let conn = open_memory_db();
        seed_file(&conn, "src/f.rs");
        seed_file(&conn, "src/g.rs");

        let sym_a = make_symbol("src/f.rs", "A");
        let sym_foo = make_symbol("src/g.rs", "foo");
        write_symbols(&conn, "src/f.rs", std::slice::from_ref(&sym_a)).unwrap();
        write_symbols(&conn, "src/g.rs", &[sym_foo]).unwrap();

        let edge = make_edge("src/g.rs", "foo", "src/f.rs", "A");
        write_edges(&conn, "src/g.rs", &[edge]).unwrap();

        // Re-extract F with an additional symbol B. A still exists.
        let sym_b = make_symbol("src/f.rs", "B");
        let actions = reextract_symbols(&conn, "src/f.rs", &[sym_a, sym_b]).unwrap();

        assert!(actions.is_empty());

        // G's edge to F::A is still intact.
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
    fn test_reextract_symbols_empty_new_symbols_propagates_to_all_callers() {
        // When a file is re-extracted with zero symbols (e.g. temporary parse
        // failure or file emptied), every dependent file must be flagged for
        // edge refresh.
        let conn = open_memory_db();
        seed_file(&conn, "src/f.rs");
        seed_file(&conn, "src/g.rs");
        seed_file(&conn, "src/h.rs");

        let sym_a = make_symbol("src/f.rs", "A");
        let sym_foo = make_symbol("src/g.rs", "foo");
        let sym_bar = make_symbol("src/h.rs", "bar");
        write_symbols(&conn, "src/f.rs", &[sym_a]).unwrap();
        write_symbols(&conn, "src/g.rs", &[sym_foo]).unwrap();
        write_symbols(&conn, "src/h.rs", &[sym_bar]).unwrap();

        write_edges(
            &conn,
            "src/g.rs",
            &[make_edge("src/g.rs", "foo", "src/f.rs", "A")],
        )
        .unwrap();
        write_edges(
            &conn,
            "src/h.rs",
            &[make_edge("src/h.rs", "bar", "src/f.rs", "A")],
        )
        .unwrap();

        // F becomes empty of symbols.
        let mut actions = reextract_symbols(&conn, "src/f.rs", &[]).unwrap();
        actions.sort_by(|a, b| match (a, b) {
            (
                InvalidationAction::RefreshEdges { file_path: a },
                InvalidationAction::RefreshEdges { file_path: b },
            ) => a.cmp(b),
        });

        assert_eq!(
            actions,
            vec![
                InvalidationAction::RefreshEdges {
                    file_path: "src/g.rs".to_string()
                },
                InvalidationAction::RefreshEdges {
                    file_path: "src/h.rs".to_string()
                },
            ]
        );
    }
}
