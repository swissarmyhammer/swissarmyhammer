//! Call graph traversal from a starting symbol.
//!
//! Given a symbol (by name or file:line:char), traverses `lsp_call_edges`
//! in the requested direction (inbound, outbound, or both) up to a
//! configurable depth. Returns edges with source provenance (`lsp` or
//! `treesitter`).

use std::collections::{HashSet, VecDeque};

use rusqlite::Connection;

use crate::error::CodeContextError;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Direction of call graph traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum CallGraphDirection {
    /// Follow edges from callee to caller ("who calls this?").
    Inbound,
    /// Follow edges from caller to callee ("what does this call?").
    Outbound,
    /// Both directions.
    Both,
}

/// Options for [`get_callgraph`].
#[derive(Debug, Clone)]
pub struct CallGraphOptions {
    /// Symbol identifier -- either a name (matched via `get_symbol` logic)
    /// or a `file:line:char` locator.
    pub symbol: String,
    /// Traversal direction.
    pub direction: CallGraphDirection,
    /// Maximum traversal depth (1..=5, default 2).
    pub max_depth: u32,
}

impl Default for CallGraphOptions {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            direction: CallGraphDirection::Outbound,
            max_depth: 2,
        }
    }
}

/// A node in the call graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
pub struct CallGraphNode {
    /// Symbol ID (the `lsp_symbols.id` value).
    pub symbol_id: String,
    /// Human-readable name.
    pub name: String,
    /// File containing the symbol.
    pub file_path: String,
}

/// A directed edge in the call graph.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CallGraphEdge {
    /// The calling symbol.
    pub caller: CallGraphNode,
    /// The called symbol.
    pub callee: CallGraphNode,
    /// Provenance of this edge: `"lsp"` or `"treesitter"`.
    pub source: String,
    /// BFS depth at which this edge was discovered.
    pub depth: u32,
}

/// Result of a call graph traversal.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CallGraph {
    /// The symbol from which traversal started.
    pub root: CallGraphNode,
    /// All discovered edges.
    pub edges: Vec<CallGraphEdge>,
    /// All unique nodes (including root).
    pub nodes: Vec<CallGraphNode>,
}

// ---------------------------------------------------------------------------
// Core function
// ---------------------------------------------------------------------------

/// Traverse the call graph starting from the given symbol.
///
/// Resolves the starting symbol from `lsp_symbols` by name or
/// `file:line:char` locator, then performs a BFS over `lsp_call_edges`
/// in the requested direction up to `max_depth` hops.
///
/// # Errors
///
/// Returns [`CodeContextError::Database`] on SQLite failures.
/// Returns [`CodeContextError::Pattern`] if the symbol cannot be resolved.
pub fn get_callgraph(
    conn: &Connection,
    options: &CallGraphOptions,
) -> Result<CallGraph, CodeContextError> {
    let max_depth = options.max_depth.clamp(1, 5);
    let root = resolve_symbol(conn, &options.symbol)?;

    let mut visited_ids: HashSet<String> = HashSet::new();
    visited_ids.insert(root.symbol_id.clone());

    let mut all_edges: Vec<CallGraphEdge> = Vec::new();
    let mut all_nodes: Vec<CallGraphNode> = vec![root.clone()];

    let mut queue: VecDeque<(String, u32)> = VecDeque::new();
    queue.push_back((root.symbol_id.clone(), 0));

    while let Some((current_id, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        let edges = fetch_edges(conn, &current_id, options.direction, depth + 1)?;
        for edge in edges {
            expand_edge(
                &edge,
                options.direction,
                &mut visited_ids,
                &mut all_nodes,
                &mut queue,
                depth,
            );
            all_edges.push(edge);
        }
    }

    Ok(CallGraph {
        root,
        edges: all_edges,
        nodes: all_nodes,
    })
}

/// Process a single edge during BFS: determine the next node, add it if new,
/// and enqueue it for further traversal.
fn expand_edge(
    edge: &CallGraphEdge,
    direction: CallGraphDirection,
    visited: &mut HashSet<String>,
    nodes: &mut Vec<CallGraphNode>,
    queue: &mut VecDeque<(String, u32)>,
    depth: u32,
) {
    let next_id = match direction {
        CallGraphDirection::Inbound => &edge.caller.symbol_id,
        CallGraphDirection::Outbound => &edge.callee.symbol_id,
        CallGraphDirection::Both => {
            if !visited.contains(&edge.callee.symbol_id) {
                &edge.callee.symbol_id
            } else {
                &edge.caller.symbol_id
            }
        }
    };

    if visited.insert(next_id.clone()) {
        let next_node = if next_id == &edge.caller.symbol_id {
            edge.caller.clone()
        } else {
            edge.callee.clone()
        };
        nodes.push(next_node);
        queue.push_back((next_id.clone(), depth + 1));
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve a symbol identifier to a [`CallGraphNode`].
///
/// Accepts either a symbol name (matched by name or qualified path suffix)
/// or a `file:line:char` locator string.
fn resolve_symbol(conn: &Connection, symbol: &str) -> Result<CallGraphNode, CodeContextError> {
    // Try file:line:char format first.
    if let Some(node) = try_resolve_by_location(conn, symbol)? {
        return Ok(node);
    }

    // Fall back to name/suffix match.
    resolve_by_name(conn, symbol)
}

/// Try to resolve a `file:line:char` locator.
///
/// Returns `Ok(None)` if the string doesn't look like a locator.
fn try_resolve_by_location(
    conn: &Connection,
    symbol: &str,
) -> Result<Option<CallGraphNode>, CodeContextError> {
    let parts: Vec<&str> = symbol.rsplitn(3, ':').collect();
    if parts.len() != 3 {
        return Ok(None);
    }

    let (char_str, line_str, file_path) = (parts[0], parts[1], parts[2]);
    let line: u32 = match line_str.parse() {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let char_pos: u32 = match char_str.parse() {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    let mut stmt = conn.prepare(
        "SELECT id, name, file_path FROM lsp_symbols \
         WHERE file_path = ?1 AND start_line <= ?2 AND end_line >= ?2 \
         AND start_char <= ?3 \
         ORDER BY (end_line - start_line) ASC, (end_char - start_char) ASC \
         LIMIT 1",
    )?;

    let result = stmt.query_row(rusqlite::params![file_path, line, char_pos], |row| {
        Ok(CallGraphNode {
            symbol_id: row.get(0)?,
            name: row.get(1)?,
            file_path: row.get(2)?,
        })
    });

    match result {
        Ok(node) => Ok(Some(node)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Resolve by name or qualified path suffix.
fn resolve_by_name(conn: &Connection, name: &str) -> Result<CallGraphNode, CodeContextError> {
    let mut stmt = conn.prepare("SELECT id, name, file_path FROM lsp_symbols")?;

    let suffix = format!("::{}", name);

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    for row in rows {
        let (id, sym_name, file_path) = row?;
        // Extract qualified path from id: "lsp:{file_path}:{qualified_path}"
        let prefix = format!("lsp:{}:", file_path);
        let qpath = id.strip_prefix(&prefix).unwrap_or(&id);

        if sym_name == name || qpath == name || qpath.ends_with(&suffix) {
            return Ok(CallGraphNode {
                symbol_id: id,
                name: sym_name,
                file_path,
            });
        }
    }

    Err(CodeContextError::Pattern(format!(
        "symbol not found: {}",
        name
    )))
}

/// Which side of a call edge to match against.
enum EdgeSide {
    Caller,
    Callee,
}

/// Query call edges where `symbol_id` matches the specified side.
fn query_edges_by_side(
    conn: &Connection,
    side: EdgeSide,
    symbol_id: &str,
    depth: u32,
) -> Result<Vec<CallGraphEdge>, CodeContextError> {
    let filter = match side {
        EdgeSide::Caller => "e.caller_id = ?1",
        EdgeSide::Callee => "e.callee_id = ?1",
    };
    let sql = format!(
        "SELECT e.caller_id, c1.name, e.caller_file, \
                e.callee_id, c2.name, e.callee_file, e.source \
         FROM lsp_call_edges e \
         JOIN lsp_symbols c1 ON c1.id = e.caller_id \
         JOIN lsp_symbols c2 ON c2.id = e.callee_id \
         WHERE {filter}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([symbol_id], |row| {
        Ok(CallGraphEdge {
            caller: CallGraphNode {
                symbol_id: row.get(0)?,
                name: row.get(1)?,
                file_path: row.get(2)?,
            },
            callee: CallGraphNode {
                symbol_id: row.get(3)?,
                name: row.get(4)?,
                file_path: row.get(5)?,
            },
            source: row.get(6)?,
            depth,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Fetch edges adjacent to a symbol in the requested direction.
fn fetch_edges(
    conn: &Connection,
    symbol_id: &str,
    direction: CallGraphDirection,
    depth: u32,
) -> Result<Vec<CallGraphEdge>, CodeContextError> {
    let mut edges = Vec::new();

    if direction == CallGraphDirection::Outbound || direction == CallGraphDirection::Both {
        edges.extend(query_edges_by_side(
            conn,
            EdgeSide::Caller,
            symbol_id,
            depth,
        )?);
    }
    if direction == CallGraphDirection::Inbound || direction == CallGraphDirection::Both {
        edges.extend(query_edges_by_side(
            conn,
            EdgeSide::Callee,
            symbol_id,
            depth,
        )?);
    }

    Ok(edges)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{insert_file_simple as insert_file, test_db};

    /// Insert an LSP symbol.
    fn insert_symbol(conn: &Connection, id: &str, name: &str, kind: i32, file_path: &str) {
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char)
             VALUES (?1, ?2, ?3, ?4, 0, 0, 10, 0)",
            rusqlite::params![id, name, kind, file_path],
        )
        .unwrap();
    }

    /// Insert a call edge.
    fn insert_edge(conn: &Connection, caller_id: &str, callee_id: &str, source: &str) {
        // Derive file paths from the symbol IDs (format: "lsp:{file}:{qpath}").
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

    /// Seed a simple A -> B -> C chain.
    ///
    /// All edges use `lsp` source unless overridden.
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
    fn test_outbound_depth_2_chain() {
        let conn = test_db();
        seed_chain(&conn);

        let result = get_callgraph(
            &conn,
            &CallGraphOptions {
                symbol: "func_a".to_string(),
                direction: CallGraphDirection::Outbound,
                max_depth: 2,
            },
        )
        .unwrap();

        assert_eq!(result.root.name, "func_a");
        assert_eq!(result.edges.len(), 2, "A->B and B->C");
        assert_eq!(result.nodes.len(), 3, "A, B, C");

        // Verify A->B edge at depth 1.
        let ab = result
            .edges
            .iter()
            .find(|e| e.caller.name == "func_a" && e.callee.name == "func_b")
            .expect("missing A->B edge");
        assert_eq!(ab.depth, 1);

        // Verify B->C edge at depth 2.
        let bc = result
            .edges
            .iter()
            .find(|e| e.caller.name == "func_b" && e.callee.name == "func_c")
            .expect("missing B->C edge");
        assert_eq!(bc.depth, 2);
    }

    #[test]
    fn test_outbound_depth_1_limits() {
        let conn = test_db();
        seed_chain(&conn);

        let result = get_callgraph(
            &conn,
            &CallGraphOptions {
                symbol: "func_a".to_string(),
                direction: CallGraphDirection::Outbound,
                max_depth: 1,
            },
        )
        .unwrap();

        assert_eq!(result.edges.len(), 1, "only A->B at depth 1");
        assert_eq!(result.edges[0].callee.name, "func_b");
    }

    #[test]
    fn test_inbound_depth_1() {
        let conn = test_db();
        seed_chain(&conn);

        let result = get_callgraph(
            &conn,
            &CallGraphOptions {
                symbol: "func_c".to_string(),
                direction: CallGraphDirection::Inbound,
                max_depth: 1,
            },
        )
        .unwrap();

        assert_eq!(result.root.name, "func_c");
        assert_eq!(result.edges.len(), 1, "only B->C");
        assert_eq!(result.edges[0].caller.name, "func_b");
        assert_eq!(result.edges[0].callee.name, "func_c");
    }

    #[test]
    fn test_inbound_depth_2() {
        let conn = test_db();
        seed_chain(&conn);

        let result = get_callgraph(
            &conn,
            &CallGraphOptions {
                symbol: "func_c".to_string(),
                direction: CallGraphDirection::Inbound,
                max_depth: 2,
            },
        )
        .unwrap();

        assert_eq!(result.edges.len(), 2, "B->C and A->B");
        assert_eq!(result.nodes.len(), 3);
    }

    #[test]
    fn test_both_direction() {
        let conn = test_db();
        seed_chain(&conn);

        let result = get_callgraph(
            &conn,
            &CallGraphOptions {
                symbol: "func_b".to_string(),
                direction: CallGraphDirection::Both,
                max_depth: 1,
            },
        )
        .unwrap();

        assert_eq!(result.root.name, "func_b");
        // B calls C (outbound) and A calls B (inbound).
        assert_eq!(result.edges.len(), 2);
        assert_eq!(result.nodes.len(), 3);
    }

    #[test]
    fn test_mixed_provenance() {
        let conn = test_db();
        insert_file(&conn, "src/x.rs");
        insert_file(&conn, "src/y.rs");
        insert_file(&conn, "src/z.rs");

        insert_symbol(&conn, "lsp:src/x.rs:fn_x", "fn_x", 12, "src/x.rs");
        insert_symbol(&conn, "lsp:src/y.rs:fn_y", "fn_y", 12, "src/y.rs");
        insert_symbol(&conn, "lsp:src/z.rs:fn_z", "fn_z", 12, "src/z.rs");

        insert_edge(&conn, "lsp:src/x.rs:fn_x", "lsp:src/y.rs:fn_y", "lsp");
        insert_edge(
            &conn,
            "lsp:src/x.rs:fn_x",
            "lsp:src/z.rs:fn_z",
            "treesitter",
        );

        let result = get_callgraph(
            &conn,
            &CallGraphOptions {
                symbol: "fn_x".to_string(),
                direction: CallGraphDirection::Outbound,
                max_depth: 1,
            },
        )
        .unwrap();

        assert_eq!(result.edges.len(), 2);

        let lsp_edge = result
            .edges
            .iter()
            .find(|e| e.callee.name == "fn_y")
            .unwrap();
        assert_eq!(lsp_edge.source, "lsp");

        let ts_edge = result
            .edges
            .iter()
            .find(|e| e.callee.name == "fn_z")
            .unwrap();
        assert_eq!(ts_edge.source, "treesitter");
    }

    #[test]
    fn test_no_edges() {
        let conn = test_db();
        insert_file(&conn, "src/lonely.rs");
        insert_symbol(
            &conn,
            "lsp:src/lonely.rs:lonely",
            "lonely",
            12,
            "src/lonely.rs",
        );

        let result = get_callgraph(
            &conn,
            &CallGraphOptions {
                symbol: "lonely".to_string(),
                direction: CallGraphDirection::Outbound,
                max_depth: 2,
            },
        )
        .unwrap();

        assert_eq!(result.root.name, "lonely");
        assert!(result.edges.is_empty());
        assert_eq!(result.nodes.len(), 1); // just the root
    }

    #[test]
    fn test_symbol_not_found() {
        let conn = test_db();

        let result = get_callgraph(
            &conn,
            &CallGraphOptions {
                symbol: "nonexistent".to_string(),
                direction: CallGraphDirection::Outbound,
                max_depth: 2,
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_default_options() {
        let opts = CallGraphOptions::default();

        assert!(opts.symbol.is_empty(), "default symbol should be empty");
        assert_eq!(
            opts.direction,
            CallGraphDirection::Outbound,
            "default direction should be Outbound"
        );
        assert_eq!(opts.max_depth, 2, "default max_depth should be 2");
    }

    /// Insert a symbol with explicit line/char positions for location-resolution tests.
    #[allow(clippy::too_many_arguments)]
    fn insert_symbol_at(
        conn: &Connection,
        id: &str,
        name: &str,
        kind: i32,
        file_path: &str,
        start_line: i32,
        start_char: i32,
        end_line: i32,
        end_char: i32,
    ) {
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![id, name, kind, file_path, start_line, start_char, end_line, end_char],
        )
        .unwrap();
    }

    #[test]
    fn test_resolve_by_file_line_char_finds_symbol() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs");

        // Symbol spans lines 10..20, chars 0..50.
        insert_symbol_at(
            &conn,
            "lsp:src/main.rs:process",
            "process",
            12,
            "src/main.rs",
            10,
            0,
            20,
            50,
        );

        // Query at line 15, char 5 -- inside the symbol's range.
        let result = get_callgraph(
            &conn,
            &CallGraphOptions {
                symbol: "src/main.rs:15:5".to_string(),
                direction: CallGraphDirection::Outbound,
                max_depth: 1,
            },
        )
        .unwrap();

        assert_eq!(result.root.name, "process");
        assert_eq!(result.root.symbol_id, "lsp:src/main.rs:process");
        assert_eq!(result.root.file_path, "src/main.rs");
    }

    #[test]
    fn test_resolve_by_file_line_char_picks_narrowest() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs");

        // Outer symbol spans lines 5..30.
        insert_symbol_at(
            &conn,
            "lsp:src/lib.rs:MyStruct::impl",
            "impl",
            5,
            "src/lib.rs",
            5,
            0,
            30,
            0,
        );

        // Inner symbol spans lines 10..15 -- narrower.
        insert_symbol_at(
            &conn,
            "lsp:src/lib.rs:MyStruct::new",
            "new",
            12,
            "src/lib.rs",
            10,
            0,
            15,
            40,
        );

        // Query at line 12, char 5 -- inside both, but the narrower one should win.
        let result = get_callgraph(
            &conn,
            &CallGraphOptions {
                symbol: "src/lib.rs:12:5".to_string(),
                direction: CallGraphDirection::Outbound,
                max_depth: 1,
            },
        )
        .unwrap();

        assert_eq!(
            result.root.name, "new",
            "should resolve to the narrowest enclosing symbol"
        );
    }

    #[test]
    fn test_resolve_by_file_line_char_no_symbol_at_location() {
        let conn = test_db();
        insert_file(&conn, "src/empty.rs");

        // Insert a symbol at lines 100..110, but query line 50.
        insert_symbol_at(
            &conn,
            "lsp:src/empty.rs:func_x",
            "func_x",
            12,
            "src/empty.rs",
            100,
            0,
            110,
            0,
        );

        // "src/empty.rs:50:0" -- no symbol covers line 50.
        // try_resolve_by_location returns Ok(None), then resolve_by_name
        // tries matching "src/empty.rs:50:0" as a name, which also fails.
        let result = get_callgraph(
            &conn,
            &CallGraphOptions {
                symbol: "src/empty.rs:50:0".to_string(),
                direction: CallGraphDirection::Outbound,
                max_depth: 1,
            },
        );

        assert!(
            result.is_err(),
            "should fail when no symbol at location and name doesn't match"
        );
    }

    #[test]
    fn test_resolve_by_file_line_char_invalid_non_numeric() {
        let conn = test_db();

        // "src/foo.rs:abc:xyz" has the right colon-count but non-numeric parts.
        // try_resolve_by_location returns Ok(None) for non-numeric line/char.
        let result = get_callgraph(
            &conn,
            &CallGraphOptions {
                symbol: "src/foo.rs:abc:xyz".to_string(),
                direction: CallGraphDirection::Outbound,
                max_depth: 1,
            },
        );

        // Falls through to name-match which also fails.
        assert!(
            result.is_err(),
            "non-numeric line:char should not crash, just fail to resolve"
        );
    }

    #[test]
    fn test_resolve_by_file_line_char_invalid_char_only() {
        let conn = test_db();

        // "src/foo.rs:10:notanumber" -- line is numeric but char is not.
        let result = get_callgraph(
            &conn,
            &CallGraphOptions {
                symbol: "src/foo.rs:10:notanumber".to_string(),
                direction: CallGraphDirection::Outbound,
                max_depth: 1,
            },
        );

        assert!(
            result.is_err(),
            "non-numeric char should not crash, just fail to resolve"
        );
    }

    #[test]
    fn test_fetch_edges_both_returns_inbound_and_outbound() {
        let conn = test_db();
        insert_file(&conn, "src/p.rs");
        insert_file(&conn, "src/q.rs");
        insert_file(&conn, "src/r.rs");

        insert_symbol(&conn, "lsp:src/p.rs:fn_p", "fn_p", 12, "src/p.rs");
        insert_symbol(&conn, "lsp:src/q.rs:fn_q", "fn_q", 12, "src/q.rs");
        insert_symbol(&conn, "lsp:src/r.rs:fn_r", "fn_r", 12, "src/r.rs");

        // fn_p -> fn_q -> fn_r
        insert_edge(&conn, "lsp:src/p.rs:fn_p", "lsp:src/q.rs:fn_q", "lsp");
        insert_edge(&conn, "lsp:src/q.rs:fn_q", "lsp:src/r.rs:fn_r", "lsp");

        // fetch_edges for fn_q with Both should return:
        // - outbound: fn_q -> fn_r (fn_q is caller)
        // - inbound:  fn_p -> fn_q (fn_q is callee)
        let edges = fetch_edges(&conn, "lsp:src/q.rs:fn_q", CallGraphDirection::Both, 1).unwrap();

        assert_eq!(
            edges.len(),
            2,
            "Both should return inbound + outbound edges"
        );

        let outbound = edges
            .iter()
            .find(|e| e.caller.name == "fn_q" && e.callee.name == "fn_r")
            .expect("missing outbound edge fn_q -> fn_r");
        assert_eq!(outbound.depth, 1);

        let inbound = edges
            .iter()
            .find(|e| e.caller.name == "fn_p" && e.callee.name == "fn_q")
            .expect("missing inbound edge fn_p -> fn_q");
        assert_eq!(inbound.depth, 1);
    }

    #[test]
    fn test_fetch_edges_both_self_loop() {
        let conn = test_db();
        insert_file(&conn, "src/s.rs");

        insert_symbol(&conn, "lsp:src/s.rs:fn_s", "fn_s", 12, "src/s.rs");

        // Self-loop: fn_s calls itself.
        insert_edge(&conn, "lsp:src/s.rs:fn_s", "lsp:src/s.rs:fn_s", "lsp");

        // With Both, the self-loop appears on both the outbound and inbound
        // queries, so fetch_edges returns it twice (once per direction query).
        let edges = fetch_edges(&conn, "lsp:src/s.rs:fn_s", CallGraphDirection::Both, 1).unwrap();

        assert_eq!(
            edges.len(),
            2,
            "self-loop should appear in both outbound and inbound results"
        );
        for edge in &edges {
            assert_eq!(edge.caller.name, "fn_s");
            assert_eq!(edge.callee.name, "fn_s");
        }
    }
}
