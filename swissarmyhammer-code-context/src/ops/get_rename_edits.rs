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

    // Hold the mutex for the entire prepareRename + rename + didClose sequence
    let result = ctx.lsp_multi_request_with_document(&opts.file_path, |rpc| {
        // Phase 1: prepareRename -- validate the position is renameable
        let prepare_response = rpc.send_request(
            "textDocument/prepareRename",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": opts.line, "character": opts.character }
            }),
        )?;

        // null means the position is not renameable
        if prepare_response.is_null()
            || prepare_response
                .get("result")
                .is_some_and(|v| v.is_null())
        {
            return Ok(not_renameable());
        }

        // Phase 2: rename -- compute the edits
        let rename_response = rpc.send_request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": opts.line, "character": opts.character },
                "newName": opts.new_name
            }),
        )?;

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
    })?;

    Ok(result.unwrap_or_else(not_renameable))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layered_context::{LspRange, TextEdit};
    use crate::test_fixtures::test_db;
    use rusqlite::Connection;

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
