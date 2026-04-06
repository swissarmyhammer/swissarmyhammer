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

    // Resolve the root symbol.
    let root = resolve_symbol(conn, &options.symbol)?;

    let mut visited_ids: HashSet<String> = HashSet::new();
    visited_ids.insert(root.symbol_id.clone());

    let mut all_edges: Vec<CallGraphEdge> = Vec::new();
    let mut all_nodes: Vec<CallGraphNode> = vec![root.clone()];

    // BFS frontier: (symbol_id, current_depth)
    let mut queue: VecDeque<(String, u32)> = VecDeque::new();
    queue.push_back((root.symbol_id.clone(), 0));

    while let Some((current_id, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }

        let edges = fetch_edges(conn, &current_id, options.direction, depth + 1)?;

        for edge in edges {
            // Determine the "next" node to follow.
            let next_id = match options.direction {
                CallGraphDirection::Inbound => &edge.caller.symbol_id,
                CallGraphDirection::Outbound => &edge.callee.symbol_id,
                CallGraphDirection::Both => {
                    // Follow whichever side is new.
                    if !visited_ids.contains(&edge.callee.symbol_id) {
                        &edge.callee.symbol_id
                    } else {
                        &edge.caller.symbol_id
                    }
                }
            };

            if visited_ids.insert(next_id.clone()) {
                let next_node = if next_id == &edge.caller.symbol_id {
                    edge.caller.clone()
                } else {
                    edge.callee.clone()
                };
                all_nodes.push(next_node);
                queue.push_back((next_id.clone(), depth + 1));
            }

            all_edges.push(edge);
        }
    }

    Ok(CallGraph {
        root,
        edges: all_edges,
        nodes: all_nodes,
    })
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

/// Fetch edges adjacent to a symbol in the requested direction.
fn fetch_edges(
    conn: &Connection,
    symbol_id: &str,
    direction: CallGraphDirection,
    depth: u32,
) -> Result<Vec<CallGraphEdge>, CodeContextError> {
    let mut edges = Vec::new();

    if direction == CallGraphDirection::Outbound || direction == CallGraphDirection::Both {
        // symbol is the caller -- find callees.
        let mut stmt = conn.prepare(
            "SELECT e.caller_id, c1.name, e.caller_file, \
                    e.callee_id, c2.name, e.callee_file, e.source \
             FROM lsp_call_edges e \
             JOIN lsp_symbols c1 ON c1.id = e.caller_id \
             JOIN lsp_symbols c2 ON c2.id = e.callee_id \
             WHERE e.caller_id = ?1",
        )?;

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

        for row in rows {
            edges.push(row?);
        }
    }

    if direction == CallGraphDirection::Inbound || direction == CallGraphDirection::Both {
        // symbol is the callee -- find callers.
        let mut stmt = conn.prepare(
            "SELECT e.caller_id, c1.name, e.caller_file, \
                    e.callee_id, c2.name, e.callee_file, e.source \
             FROM lsp_call_edges e \
             JOIN lsp_symbols c1 ON c1.id = e.caller_id \
             JOIN lsp_symbols c2 ON c2.id = e.callee_id \
             WHERE e.callee_id = ?1",
        )?;

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

        for row in rows {
            edges.push(row?);
        }
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
}
