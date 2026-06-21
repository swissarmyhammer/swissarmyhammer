//! Preview a rename without applying edits.
//!
//! **Live LSP only** -- there is no index fallback because rename requires
//! live semantic analysis. When no live LSP is available, returns
//! `can_rename: false` immediately.
//!
//! Two-phase protocol:
//! 1. `textDocument/prepareRename` -- validates that the position is renameable.
//! 2. `textDocument/rename` -- computes the workspace-wide edits.
//!
//! The result is a preview: edits are returned but NOT applied.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::layered_context::{FileEdit, LayeredContext};

use super::get_code_actions::parse_workspace_edit;
use super::lsp_helpers::file_path_to_uri;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Options for the `get_rename_edits` operation.
#[derive(Debug, Clone)]
pub struct GetRenameEditsOptions {
    /// Path to the file containing the symbol to rename.
    pub file_path: String,
    /// Zero-based line number of the symbol.
    pub line: u32,
    /// Zero-based character offset within the line.
    pub character: u32,
    /// The new name for the symbol.
    pub new_name: String,
}

/// Result of a rename-edits preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameEditsResult {
    /// Whether the position can be renamed.
    pub can_rename: bool,
    /// The edits grouped by file. Empty when `can_rename` is false.
    pub edits: Vec<FileEdit>,
    /// Number of distinct files affected. Zero when `can_rename` is false.
    pub files_affected: usize,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Preview a rename at the given position without applying edits.
///
/// Sends `textDocument/prepareRename` to validate, then `textDocument/rename`
/// to compute edits. Returns `can_rename: false` when no live LSP is
/// available or the position is not renameable.
///
/// # Arguments
/// * `ctx` - The layered context providing access to all data layers.
/// * `opts` - File path, position, and the desired new name.
///
/// # Errors
/// Returns a `CodeContextError` if an LSP request fails in a way that
/// is not a graceful "no data" response.
pub fn get_rename_edits(
    ctx: &LayeredContext,
    opts: &GetRenameEditsOptions,
) -> Result<RenameEditsResult, crate::error::CodeContextError> {
    if !ctx.has_live_lsp() {
        return Ok(RenameEditsResult {
            can_rename: false,
            edits: Vec::new(),
            files_affected: 0,
        });
    }

    let uri = file_path_to_uri(&opts.file_path);

    let not_renameable = || RenameEditsResult {
        can_rename: false,
        edits: Vec::new(),
        files_affected: 0,
    };

    // The two-phase prepareRename + rename sequence is an ordered batch run as
    // ONE atomic exchange under a single client lock — locally on an in-process
    // session, or routed to the leader's session on a follower
    // (`lsp_multi_request_batch` picks the path). Both phases are sent
    // unconditionally; a position that is not renameable makes prepareRename
    // return null and rename return null/empty edits, which the parsing below
    // collapses to `can_rename: false` — the same observable result as skipping
    // rename, with the whole exchange kept atomic and routable.
    let position = json!({ "line": opts.line, "character": opts.character });
    let steps = vec![
        (
            "textDocument/prepareRename".to_string(),
            json!({ "textDocument": { "uri": uri }, "position": position }),
        ),
        (
            "textDocument/rename".to_string(),
            json!({
                "textDocument": { "uri": uri },
                "position": position,
                "newName": opts.new_name
            }),
        ),
    ];

    let responses = match ctx.lsp_multi_request_batch(&opts.file_path, steps)? {
        Some(responses) => responses,
        // No live layer (no session, no router): degrade to not renameable.
        None => return Ok(not_renameable()),
    };

    // Phase 1: prepareRename — a null result means the position is not
    // renameable. The batch results are the bare (already-unwrapped) LSP
    // results, in step order.
    let prepare_response = responses
        .first()
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    if prepare_response.is_null() {
        return Ok(not_renameable());
    }

    // Phase 2: rename — compute the edits from the second step's result.
    let rename_response = responses.get(1).cloned().unwrap_or(serde_json::Value::Null);
    if rename_response.is_null() {
        return Ok(not_renameable());
    }

    let edits = parse_workspace_edit(&rename_response);
    let files_affected = edits.len();

    Ok(RenameEditsResult {
        can_rename: !edits.is_empty(),
        edits,
        files_affected,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layered_context::{LspRange, TextEdit};
    use crate::test_fixtures::test_db;

    // --- Follower multi-step router path ---

    #[test]
    fn test_follower_multi_router_returns_real_rename_edits() {
        // A follower (no in-process session) is wired with a MultiLspRouter that
        // stands in for the leader: it runs prepareRename + rename under one lock
        // and returns the leader's ordered envelopes. get_rename_edits must route
        // its batch through it (not short-circuit to can_rename:false) and parse
        // the rename edits — proving the multi-step seam carries the live result.
        use std::sync::{Arc, Mutex};
        let conn = test_db();
        let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let seen_for_router = Arc::clone(&seen);
        let ctx = LayeredContext::with_multi_lsp_router(
            &conn,
            Box::new(move |_file_path, steps| {
                *seen_for_router.lock().unwrap() = steps.iter().map(|(m, _)| m.clone()).collect();
                // Step 0: prepareRename → a renameable range. Step 1: rename →
                // workspace edits. Both wrapped in JSON-RPC envelopes, as a real
                // leader returns.
                Ok(Some(vec![
                    serde_json::json!({
                        "jsonrpc": "2.0", "id": 1,
                        "result": {
                            "start": { "line": 5, "character": 4 },
                            "end": { "line": 5, "character": 12 }
                        }
                    }),
                    serde_json::json!({
                        "jsonrpc": "2.0", "id": 2,
                        "result": {
                            "changes": {
                                "file:///src/main.rs": [{
                                    "range": {
                                        "start": { "line": 5, "character": 4 },
                                        "end": { "line": 5, "character": 12 }
                                    },
                                    "newText": "renamed"
                                }]
                            }
                        }
                    }),
                ]))
            }),
        );

        let opts = GetRenameEditsOptions {
            file_path: "src/main.rs".to_string(),
            line: 5,
            character: 4,
            new_name: "renamed".to_string(),
        };

        let result = get_rename_edits(&ctx, &opts).unwrap();
        assert!(
            result.can_rename,
            "follower must get the leader's live rename"
        );
        assert_eq!(result.files_affected, 1);
        assert_eq!(result.edits[0].file_path, "/src/main.rs");
        assert_eq!(result.edits[0].text_edits[0].new_text, "renamed");
        assert_eq!(
            *seen.lock().unwrap(),
            vec![
                "textDocument/prepareRename".to_string(),
                "textDocument/rename".to_string()
            ],
            "the batch must carry prepareRename then rename, in order"
        );
    }

    #[test]
    fn test_follower_multi_router_null_prepare_is_not_renameable() {
        // prepareRename returns null → not renameable; the op must report
        // can_rename:false even though a router was present.
        let conn = test_db();
        let ctx = LayeredContext::with_multi_lsp_router(
            &conn,
            Box::new(|_file_path, _steps| {
                Ok(Some(vec![
                    serde_json::json!({ "jsonrpc": "2.0", "id": 1, "result": null }),
                    serde_json::json!({ "jsonrpc": "2.0", "id": 2, "result": null }),
                ]))
            }),
        );
        let opts = GetRenameEditsOptions {
            file_path: "src/main.rs".to_string(),
            line: 5,
            character: 4,
            new_name: "renamed".to_string(),
        };
        let result = get_rename_edits(&ctx, &opts).unwrap();
        assert!(!result.can_rename);
        assert!(result.edits.is_empty());
    }

    // --- can_rename: false when no live LSP ---

    #[test]
    fn test_no_live_lsp_returns_can_rename_false() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, None);

        let opts = GetRenameEditsOptions {
            file_path: "src/main.rs".to_string(),
            line: 5,
            character: 3,
            new_name: "new_name".to_string(),
        };

        let result = get_rename_edits(&ctx, &opts).unwrap();
        assert!(!result.can_rename);
        assert!(result.edits.is_empty());
        assert_eq!(result.files_affected, 0);
    }

    // --- WorkspaceEdit parsing (via get_code_actions::parse_workspace_edit) ---

    #[test]
    fn test_parse_workspace_edit_empty() {
        let response = serde_json::json!({});
        let edits = parse_workspace_edit(&response);
        assert!(edits.is_empty());
    }

    #[test]
    fn test_parse_workspace_edit_changes_format() {
        let response = serde_json::json!({
            "changes": {
                "file:///src/main.rs": [
                    {
                        "range": {
                            "start": { "line": 5, "character": 4 },
                            "end": { "line": 5, "character": 12 }
                        },
                        "newText": "new_name"
                    },
                    {
                        "range": {
                            "start": { "line": 10, "character": 8 },
                            "end": { "line": 10, "character": 16 }
                        },
                        "newText": "new_name"
                    }
                ],
                "file:///src/lib.rs": [
                    {
                        "range": {
                            "start": { "line": 20, "character": 0 },
                            "end": { "line": 20, "character": 8 }
                        },
                        "newText": "new_name"
                    }
                ]
            }
        });

        let edits = parse_workspace_edit(&response);
        assert_eq!(edits.len(), 2, "should have two file groups");

        let main = edits
            .iter()
            .find(|e| e.file_path == "/src/main.rs")
            .unwrap();
        assert_eq!(main.text_edits.len(), 2);
        assert_eq!(main.text_edits[0].new_text, "new_name");
        assert_eq!(main.text_edits[0].range.start_line, 5);
        assert_eq!(main.text_edits[0].range.start_character, 4);

        let lib = edits.iter().find(|e| e.file_path == "/src/lib.rs").unwrap();
        assert_eq!(lib.text_edits.len(), 1);
        assert_eq!(lib.text_edits[0].range.start_line, 20);
    }

    #[test]
    fn test_parse_workspace_edit_document_changes_format() {
        let response = serde_json::json!({
            "documentChanges": [
                {
                    "textDocument": {
                        "uri": "file:///src/main.rs",
                        "version": 1
                    },
                    "edits": [
                        {
                            "range": {
                                "start": { "line": 3, "character": 7 },
                                "end": { "line": 3, "character": 15 }
                            },
                            "newText": "renamed"
                        }
                    ]
                },
                {
                    "textDocument": {
                        "uri": "file:///src/other.rs",
                        "version": 2
                    },
                    "edits": [
                        {
                            "range": {
                                "start": { "line": 0, "character": 4 },
                                "end": { "line": 0, "character": 12 }
                            },
                            "newText": "renamed"
                        }
                    ]
                }
            ]
        });

        let edits = parse_workspace_edit(&response);
        assert_eq!(edits.len(), 2);

        assert_eq!(edits[0].file_path, "/src/main.rs");
        assert_eq!(edits[0].text_edits.len(), 1);
        assert_eq!(edits[0].text_edits[0].new_text, "renamed");
        assert_eq!(edits[0].text_edits[0].range.start_line, 3);

        assert_eq!(edits[1].file_path, "/src/other.rs");
    }

    #[test]
    fn test_parse_workspace_edit_null_response() {
        let response = serde_json::json!(null);
        let edits = parse_workspace_edit(&response);
        assert!(edits.is_empty());
    }

    #[test]
    fn test_files_affected_count() {
        let response = serde_json::json!({
            "changes": {
                "file:///a.rs": [
                    {
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 0, "character": 3 }
                        },
                        "newText": "x"
                    }
                ],
                "file:///b.rs": [
                    {
                        "range": {
                            "start": { "line": 1, "character": 0 },
                            "end": { "line": 1, "character": 3 }
                        },
                        "newText": "x"
                    }
                ],
                "file:///c.rs": [
                    {
                        "range": {
                            "start": { "line": 2, "character": 0 },
                            "end": { "line": 2, "character": 3 }
                        },
                        "newText": "x"
                    }
                ]
            }
        });

        let edits = parse_workspace_edit(&response);
        assert_eq!(edits.len(), 3);
    }

    // --- SharedLspClient present but containing None ---

    #[test]
    fn test_shared_lsp_client_with_none_returns_can_rename_false() {
        // When a SharedLspClient exists but wraps None (LSP process not
        // connected), lsp_multi_request_with_document returns Ok(None) and
        // the unwrap_or_else path produces can_rename: false.
        let conn = test_db();
        let ctx = LayeredContext::new(
            &conn,
            Some(crate::layered_context::SharedLspSession::new(
                std::sync::Arc::new(std::sync::Mutex::new(None)),
                "rust",
            )),
        );

        let opts = GetRenameEditsOptions {
            file_path: "src/main.rs".to_string(),
            line: 10,
            character: 5,
            new_name: "renamed".to_string(),
        };

        let result = get_rename_edits(&ctx, &opts).unwrap();
        assert!(!result.can_rename);
        assert!(result.edits.is_empty());
        assert_eq!(result.files_affected, 0);
    }

    // --- WorkspaceEdit parsing: precedence and edge cases ---

    #[test]
    fn test_parse_workspace_edit_document_changes_takes_precedence_over_changes() {
        // LSP spec: when both `documentChanges` and `changes` are present,
        // `documentChanges` takes precedence. Verify that edits come only
        // from the documentChanges branch.
        let response = serde_json::json!({
            "documentChanges": [
                {
                    "textDocument": { "uri": "file:///src/preferred.rs", "version": 1 },
                    "edits": [
                        {
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": 0, "character": 5 }
                            },
                            "newText": "from_doc_changes"
                        }
                    ]
                }
            ],
            "changes": {
                "file:///src/ignored.rs": [
                    {
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 0, "character": 5 }
                        },
                        "newText": "from_changes"
                    }
                ]
            }
        });

        let edits = parse_workspace_edit(&response);
        assert_eq!(edits.len(), 1, "only documentChanges should be used");
        assert_eq!(edits[0].file_path, "/src/preferred.rs");
        assert_eq!(edits[0].text_edits[0].new_text, "from_doc_changes");
    }

    #[test]
    fn test_parse_workspace_edit_document_changes_missing_uri_skipped() {
        // An entry in documentChanges without a textDocument.uri should be
        // skipped gracefully.
        let response = serde_json::json!({
            "documentChanges": [
                {
                    "textDocument": {},
                    "edits": [
                        {
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": 0, "character": 3 }
                            },
                            "newText": "abc"
                        }
                    ]
                },
                {
                    "textDocument": { "uri": "file:///src/valid.rs", "version": 1 },
                    "edits": [
                        {
                            "range": {
                                "start": { "line": 1, "character": 0 },
                                "end": { "line": 1, "character": 4 }
                            },
                            "newText": "good"
                        }
                    ]
                }
            ]
        });

        let edits = parse_workspace_edit(&response);
        assert_eq!(edits.len(), 1, "entry without URI should be skipped");
        assert_eq!(edits[0].file_path, "/src/valid.rs");
    }

    #[test]
    fn test_parse_workspace_edit_document_changes_missing_edits_skipped() {
        // An entry in documentChanges with no `edits` array should be skipped.
        let response = serde_json::json!({
            "documentChanges": [
                {
                    "textDocument": { "uri": "file:///src/no_edits.rs", "version": 1 }
                },
                {
                    "textDocument": { "uri": "file:///src/has_edits.rs", "version": 1 },
                    "edits": [
                        {
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": 0, "character": 2 }
                            },
                            "newText": "ok"
                        }
                    ]
                }
            ]
        });

        let edits = parse_workspace_edit(&response);
        assert_eq!(edits.len(), 1, "entry without edits should be skipped");
        assert_eq!(edits[0].file_path, "/src/has_edits.rs");
    }

    #[test]
    fn test_parse_workspace_edit_changes_empty_edits_array_omitted() {
        // A file URI in `changes` that maps to an empty array should not
        // produce a FileEdit entry.
        let response = serde_json::json!({
            "changes": {
                "file:///src/empty.rs": [],
                "file:///src/real.rs": [
                    {
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 0, "character": 4 }
                        },
                        "newText": "data"
                    }
                ]
            }
        });

        let edits = parse_workspace_edit(&response);
        assert_eq!(
            edits.len(),
            1,
            "empty edits array should produce no FileEdit"
        );
        assert_eq!(edits[0].file_path, "/src/real.rs");
    }

    #[test]
    fn test_parse_workspace_edit_malformed_text_edit_skipped() {
        // TextEdits missing `range` or `newText` should be silently skipped.
        let response = serde_json::json!({
            "changes": {
                "file:///src/main.rs": [
                    {
                        "newText": "no range"
                    },
                    {
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 0, "character": 3 }
                        }
                    },
                    {
                        "range": {
                            "start": { "line": 1, "character": 0 },
                            "end": { "line": 1, "character": 5 }
                        },
                        "newText": "valid"
                    }
                ]
            }
        });

        let edits = parse_workspace_edit(&response);
        assert_eq!(edits.len(), 1);
        assert_eq!(
            edits[0].text_edits.len(),
            1,
            "only the well-formed text edit should survive"
        );
        assert_eq!(edits[0].text_edits[0].new_text, "valid");
    }

    // --- RenameEditsResult serialization ---

    #[test]
    fn test_result_can_rename_false_serialization() {
        let result = RenameEditsResult {
            can_rename: false,
            edits: Vec::new(),
            files_affected: 0,
        };

        let json = serde_json::to_string(&result).unwrap();
        let roundtrip: RenameEditsResult = serde_json::from_str(&json).unwrap();
        assert!(!roundtrip.can_rename);
        assert!(roundtrip.edits.is_empty());
        assert_eq!(roundtrip.files_affected, 0);
    }

    #[test]
    fn test_result_serializable() {
        let result = RenameEditsResult {
            can_rename: true,
            edits: vec![FileEdit {
                file_path: "/src/main.rs".to_string(),
                text_edits: vec![TextEdit {
                    range: LspRange {
                        start_line: 1,
                        start_character: 0,
                        end_line: 1,
                        end_character: 5,
                    },
                    new_text: "new_name".to_string(),
                }],
            }],
            files_affected: 1,
        };

        let json = serde_json::to_string(&result).unwrap();
        let roundtrip: RenameEditsResult = serde_json::from_str(&json).unwrap();
        assert!(roundtrip.can_rename);
        assert_eq!(roundtrip.files_affected, 1);
        assert_eq!(roundtrip.edits[0].file_path, "/src/main.rs");
        assert_eq!(roundtrip.edits[0].text_edits[0].new_text, "new_name");
    }
}
