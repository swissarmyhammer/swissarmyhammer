//! Code actions (quickfixes, refactors) via live LSP only.
//!
//! Sends `textDocument/codeAction` to a running LSP server and optionally
//! resolves actions with `codeAction/resolve` to obtain workspace edits.
//! Returns empty when no live LSP is available (not an error).

use serde::{Deserialize, Serialize};
use serde_json::json;

use std::collections::BTreeMap;

use crate::layered_context::{FileEdit, LayeredContext, TextEdit};
use crate::ops::lsp_helpers::{file_path_to_uri, parse_lsp_range, uri_to_file_path};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Options for the `get_code_actions` operation.
#[derive(Debug, Clone)]
pub struct GetCodeActionsOptions {
    /// Path to the file (relative to workspace root).
    pub file_path: String,
    /// Zero-based start line of the range to query.
    pub start_line: u32,
    /// Zero-based start character offset.
    pub start_character: u32,
    /// Zero-based end line of the range to query.
    pub end_line: u32,
    /// Zero-based end character offset.
    pub end_character: u32,
    /// Optional filter to limit code action kinds (e.g. "quickfix", "refactor", "source").
    pub filter_kind: Option<Vec<String>>,
}

/// A single code action returned by the LSP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeAction {
    /// Human-readable title of the action.
    pub title: String,
    /// The kind of code action (e.g. "quickfix", "refactor.extract").
    pub kind: Option<String>,
    /// Workspace edits to apply, if available (after resolution).
    pub edits: Option<Vec<FileEdit>>,
    /// Whether this action is marked as preferred by the LSP server.
    pub is_preferred: bool,
}

/// Result of a code actions query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeActionsResult {
    /// The code actions available for the given range.
    pub actions: Vec<CodeAction>,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Get code actions for a range in a file.
///
/// Uses live LSP only -- code actions require real-time analysis from a
/// running language server. Returns an empty result when no live LSP is
/// available (graceful degradation, not an error).
///
/// Actions without inline edits are resolved via `codeAction/resolve` to
/// attempt to obtain workspace edits.
///
/// # Arguments
/// * `ctx` - The layered context providing access to all data layers.
/// * `opts` - The file path, range, and optional kind filter.
///
/// # Errors
/// Returns a `CodeContextError` if an LSP request fails in a way that
/// is not a graceful "no data" response.
pub fn get_code_actions(
    ctx: &LayeredContext,
    opts: &GetCodeActionsOptions,
) -> Result<CodeActionsResult, crate::error::CodeContextError> {
    if !ctx.has_live_lsp() {
        return Ok(CodeActionsResult {
            actions: Vec::new(),
        });
    }

    let uri = file_path_to_uri(&opts.file_path);

    // Build codeAction params
    let mut params = json!({
        "textDocument": { "uri": uri },
        "range": {
            "start": { "line": opts.start_line, "character": opts.start_character },
            "end": { "line": opts.end_line, "character": opts.end_character }
        },
        "context": {
            "diagnostics": []
        }
    });

    // Add kind filter if specified
    if let Some(ref kinds) = opts.filter_kind {
        params["context"]["only"] = json!(kinds);
    }

    // Atomic didOpen + codeAction request + didClose
    let response =
        ctx.lsp_request_with_document(&opts.file_path, "textDocument/codeAction", params)?;

    let response: serde_json::Value = match response {
        Some(v) if !v.is_null() => v,
        _ => {
            return Ok(CodeActionsResult {
                actions: Vec::new(),
            })
        }
    };

    let mut actions = parse_code_actions(&response);

    // Apply kind filter for actions that survived parsing
    if let Some(ref kinds) = opts.filter_kind {
        actions.retain(|a| {
            a.kind
                .as_ref()
                .is_some_and(|k| kinds.iter().any(|f| k.starts_with(f)))
        });
    }

    // Try to resolve actions that lack edits
    for action in &mut actions {
        if action.edits.is_none() {
            if let Some(resolved) = try_resolve_action(ctx, action) {
                action.edits = resolved;
            }
        }
    }

    Ok(CodeActionsResult { actions })
}

// ---------------------------------------------------------------------------
// Response parsing
// ---------------------------------------------------------------------------

/// Parse a `textDocument/codeAction` response into a list of code actions.
///
/// The LSP response is an array where each element is either:
/// - A `Command` object (`{ title, command, arguments }`) -- no edits
/// - A `CodeAction` object (`{ title, kind, edit, isPreferred, ... }`)
pub fn parse_code_actions(response: &serde_json::Value) -> Vec<CodeAction> {
    let arr = match response.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };

    arr.iter().filter_map(parse_single_action).collect()
}

/// Parse a single element of the codeAction response array.
///
/// Returns `None` if the element cannot be interpreted as a code action.
fn parse_single_action(value: &serde_json::Value) -> Option<CodeAction> {
    let title = value.get("title")?.as_str()?.to_string();

    // Distinguish Command vs CodeAction by presence of "command" field as a string
    // (Commands have `command` as a string method name; CodeActions may have a nested
    // `command` object but also have `kind`).
    let is_command_only =
        value.get("command").is_some_and(|c| c.is_string()) && value.get("kind").is_none();

    if is_command_only {
        // Command variant -- no edits available
        return Some(CodeAction {
            title,
            kind: None,
            edits: None,
            is_preferred: false,
        });
    }

    // CodeAction variant
    let kind = value.get("kind").and_then(|k| k.as_str()).map(String::from);
    let is_preferred = value
        .get("isPreferred")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let edits = value
        .get("edit")
        .map(parse_workspace_edit)
        .filter(|e: &Vec<FileEdit>| !e.is_empty());

    Some(CodeAction {
        title,
        kind,
        edits,
        is_preferred,
    })
}

// ---------------------------------------------------------------------------
// WorkspaceEdit parsing
// ---------------------------------------------------------------------------

/// Parse a WorkspaceEdit into a list of FileEdits.
///
/// Handles two LSP workspace edit formats:
/// - `documentChanges`: `TextDocumentEdit[]` -- preferred per LSP spec
/// - `changes`: `{ uri: TextEdit[] }` map -- fallback
///
/// If both are present, `documentChanges` takes precedence per the LSP spec.
pub fn parse_workspace_edit(edit: &serde_json::Value) -> Vec<FileEdit> {
    // Prefer documentChanges over changes (LSP spec precedence)
    if let Some(doc_changes) = edit.get("documentChanges").and_then(|v| v.as_array()) {
        return parse_document_changes(doc_changes);
    }

    if let Some(changes) = edit.get("changes").and_then(|v| v.as_object()) {
        return parse_changes(changes);
    }

    Vec::new()
}

/// Parse the `documentChanges` format: `TextDocumentEdit[]`.
///
/// Each element has `textDocument.uri` and an `edits` array of `TextEdit`.
fn parse_document_changes(doc_changes: &[serde_json::Value]) -> Vec<FileEdit> {
    let mut result = Vec::new();

    for entry in doc_changes {
        let uri = entry
            .get("textDocument")
            .and_then(|td| td.get("uri"))
            .and_then(|u| u.as_str());

        let edits_arr = entry.get("edits").and_then(|e| e.as_array());

        if let (Some(uri), Some(edits)) = (uri, edits_arr) {
            let file_path = uri_to_file_path(uri);
            let text_edits: Vec<TextEdit> = edits.iter().filter_map(parse_text_edit).collect();
            if !text_edits.is_empty() {
                result.push(FileEdit {
                    file_path,
                    text_edits,
                });
            }
        }
    }

    result
}

/// Parse the `changes` format: `{ [uri]: TextEdit[] }`.
fn parse_changes(changes: &serde_json::Map<String, serde_json::Value>) -> Vec<FileEdit> {
    // Use BTreeMap for deterministic ordering by file path
    let mut grouped: BTreeMap<String, Vec<TextEdit>> = BTreeMap::new();

    for (uri, edits_val) in changes {
        let file_path = uri_to_file_path(uri);
        if let Some(edits_arr) = edits_val.as_array() {
            let text_edits: Vec<TextEdit> = edits_arr.iter().filter_map(parse_text_edit).collect();
            grouped.entry(file_path).or_default().extend(text_edits);
        }
    }

    grouped
        .into_iter()
        .filter(|(_, edits): &(String, Vec<TextEdit>)| !edits.is_empty())
        .map(|(file_path, text_edits)| FileEdit {
            file_path,
            text_edits,
        })
        .collect()
}

/// Parse a single LSP `TextEdit` JSON value into our [`TextEdit`] type.
fn parse_text_edit(edit: &serde_json::Value) -> Option<TextEdit> {
    let range = parse_lsp_range(edit.get("range")?)?;
    let new_text = edit.get("newText")?.as_str()?;

    Some(TextEdit {
        range,
        new_text: new_text.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

/// Try to resolve a code action via `codeAction/resolve` to obtain edits.
///
/// Only attempts resolution if the action has a kind (indicating it's a
/// CodeAction, not a Command). Returns None if resolution fails or yields
/// no edits.
fn try_resolve_action(ctx: &LayeredContext, action: &CodeAction) -> Option<Option<Vec<FileEdit>>> {
    // Only resolve CodeAction variants (those with a kind)
    action.kind.as_ref()?;

    let resolve_params = json!({
        "title": action.title,
        "kind": action.kind,
    });

    let response = ctx.lsp_request("codeAction/resolve", resolve_params).ok()?;
    let response = response?;

    if response.is_null() {
        return None;
    }

    let edits = response
        .get("edit")
        .map(parse_workspace_edit)
        .unwrap_or_default();
    if edits.is_empty() {
        None
    } else {
        Some(Some(edits))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layered_context::LspRange;
    use crate::test_fixtures::test_db;

    // --- No live LSP returns empty, not error ---

    #[test]
    fn test_no_live_lsp_returns_empty() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, None);
        let opts = GetCodeActionsOptions {
            file_path: "src/main.rs".to_string(),
            start_line: 0,
            start_character: 0,
            end_line: 0,
            end_character: 10,
            filter_kind: None,
        };
        let result = get_code_actions(&ctx, &opts).unwrap();
        assert!(result.actions.is_empty());
    }

    // --- parse_code_actions: Command variant ---

    #[test]
    fn test_parse_command_variant() {
        let response = serde_json::json!([
            {
                "title": "Run test",
                "command": "rust-analyzer.runSingle",
                "arguments": [{ "label": "test foo" }]
            }
        ]);
        let actions = parse_code_actions(&response);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].title, "Run test");
        assert!(actions[0].kind.is_none());
        assert!(actions[0].edits.is_none());
        assert!(!actions[0].is_preferred);
    }

    // --- parse_code_actions: CodeAction variant ---

    #[test]
    fn test_parse_code_action_variant() {
        let response = serde_json::json!([
            {
                "title": "Add missing import",
                "kind": "quickfix",
                "isPreferred": true,
                "edit": {
                    "changes": {
                        "file:///src/main.rs": [
                            {
                                "range": {
                                    "start": { "line": 0, "character": 0 },
                                    "end": { "line": 0, "character": 0 }
                                },
                                "newText": "use std::io;\n"
                            }
                        ]
                    }
                }
            }
        ]);
        let actions = parse_code_actions(&response);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].title, "Add missing import");
        assert_eq!(actions[0].kind.as_deref(), Some("quickfix"));
        assert!(actions[0].is_preferred);

        let edits = actions[0].edits.as_ref().unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].file_path, "/src/main.rs");
        assert_eq!(edits[0].text_edits.len(), 1);
        assert_eq!(edits[0].text_edits[0].new_text, "use std::io;\n");
    }

    // --- parse_code_actions: mixed Command and CodeAction ---

    #[test]
    fn test_parse_mixed_command_and_code_action() {
        let response = serde_json::json!([
            {
                "title": "Run test",
                "command": "runSingle",
                "arguments": []
            },
            {
                "title": "Extract to function",
                "kind": "refactor.extract",
                "edit": {
                    "documentChanges": [
                        {
                            "textDocument": { "uri": "file:///src/lib.rs", "version": 1 },
                            "edits": [
                                {
                                    "range": {
                                        "start": { "line": 10, "character": 0 },
                                        "end": { "line": 15, "character": 1 }
                                    },
                                    "newText": "extracted_fn();\n"
                                }
                            ]
                        }
                    ]
                }
            }
        ]);
        let actions = parse_code_actions(&response);
        assert_eq!(actions.len(), 2);

        // First is a Command
        assert_eq!(actions[0].title, "Run test");
        assert!(actions[0].kind.is_none());
        assert!(actions[0].edits.is_none());

        // Second is a CodeAction with documentChanges
        assert_eq!(actions[1].title, "Extract to function");
        assert_eq!(actions[1].kind.as_deref(), Some("refactor.extract"));
        let edits = actions[1].edits.as_ref().unwrap();
        assert_eq!(edits[0].file_path, "/src/lib.rs");
        assert_eq!(edits[0].text_edits[0].new_text, "extracted_fn();\n");
    }

    // --- filter_kind filtering ---

    #[test]
    fn test_filter_kind() {
        let response = serde_json::json!([
            {
                "title": "Quick fix A",
                "kind": "quickfix",
                "isPreferred": false
            },
            {
                "title": "Refactor B",
                "kind": "refactor.extract",
                "isPreferred": false
            },
            {
                "title": "Source organize",
                "kind": "source.organizeImports",
                "isPreferred": false
            }
        ]);
        let mut actions = parse_code_actions(&response);

        // Filter to quickfix only
        let kinds = ["quickfix".to_string()];
        actions.retain(|a| {
            a.kind
                .as_ref()
                .is_some_and(|k| kinds.iter().any(|f| k.starts_with(f)))
        });

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].title, "Quick fix A");
    }

    #[test]
    fn test_filter_kind_prefix_matching() {
        let response = serde_json::json!([
            {
                "title": "Extract function",
                "kind": "refactor.extract",
                "isPreferred": false
            },
            {
                "title": "Inline variable",
                "kind": "refactor.inline",
                "isPreferred": false
            },
            {
                "title": "Quick fix",
                "kind": "quickfix",
                "isPreferred": false
            }
        ]);
        let mut actions = parse_code_actions(&response);

        // Filter to all refactor kinds (prefix match)
        let kinds = ["refactor".to_string()];
        actions.retain(|a| {
            a.kind
                .as_ref()
                .is_some_and(|k| kinds.iter().any(|f| k.starts_with(f)))
        });

        assert_eq!(actions.len(), 2);
        assert!(actions
            .iter()
            .all(|a| a.kind.as_ref().unwrap().starts_with("refactor")));
    }

    // --- WorkspaceEdit parsing ---

    #[test]
    fn test_parse_workspace_edit_changes_format() {
        let edit = serde_json::json!({
            "changes": {
                "file:///src/a.rs": [
                    {
                        "range": {
                            "start": { "line": 1, "character": 0 },
                            "end": { "line": 1, "character": 5 }
                        },
                        "newText": "hello"
                    }
                ],
                "file:///src/b.rs": [
                    {
                        "range": {
                            "start": { "line": 10, "character": 0 },
                            "end": { "line": 10, "character": 0 }
                        },
                        "newText": "world\n"
                    }
                ]
            }
        });

        let file_edits = parse_workspace_edit(&edit);
        assert_eq!(file_edits.len(), 2);

        let a_edit = file_edits
            .iter()
            .find(|e| e.file_path.contains("a.rs"))
            .unwrap();
        assert_eq!(a_edit.text_edits.len(), 1);
        assert_eq!(a_edit.text_edits[0].new_text, "hello");
        assert_eq!(a_edit.text_edits[0].range.start_line, 1);

        let b_edit = file_edits
            .iter()
            .find(|e| e.file_path.contains("b.rs"))
            .unwrap();
        assert_eq!(b_edit.text_edits[0].new_text, "world\n");
    }

    #[test]
    fn test_parse_workspace_edit_document_changes_format() {
        let edit = serde_json::json!({
            "documentChanges": [
                {
                    "textDocument": { "uri": "file:///src/main.rs", "version": 1 },
                    "edits": [
                        {
                            "range": {
                                "start": { "line": 5, "character": 0 },
                                "end": { "line": 5, "character": 10 }
                            },
                            "newText": "new_content"
                        }
                    ]
                }
            ]
        });

        let file_edits = parse_workspace_edit(&edit);
        assert_eq!(file_edits.len(), 1);
        assert_eq!(file_edits[0].file_path, "/src/main.rs");
        assert_eq!(file_edits[0].text_edits[0].new_text, "new_content");
    }

    #[test]
    fn test_parse_workspace_edit_empty() {
        let edit = serde_json::json!({});
        assert!(parse_workspace_edit(&edit).is_empty());
    }

    // --- Empty response ---

    #[test]
    fn test_parse_code_actions_empty_array() {
        let response = serde_json::json!([]);
        let actions = parse_code_actions(&response);
        assert!(actions.is_empty());
    }

    #[test]
    fn test_parse_code_actions_null() {
        let response = serde_json::json!(null);
        let actions = parse_code_actions(&response);
        assert!(actions.is_empty());
    }

    // --- Result serialization ---

    #[test]
    fn test_result_serializable() {
        let result = CodeActionsResult {
            actions: vec![
                CodeAction {
                    title: "Fix import".to_string(),
                    kind: Some("quickfix".to_string()),
                    edits: Some(vec![FileEdit {
                        file_path: "/src/main.rs".to_string(),
                        text_edits: vec![TextEdit {
                            range: LspRange {
                                start_line: 0,
                                start_character: 0,
                                end_line: 0,
                                end_character: 0,
                            },
                            new_text: "use std::io;\n".to_string(),
                        }],
                    }]),
                    is_preferred: true,
                },
                CodeAction {
                    title: "Run test".to_string(),
                    kind: None,
                    edits: None,
                    is_preferred: false,
                },
            ],
        };

        let json = serde_json::to_string(&result).unwrap();
        let roundtrip: CodeActionsResult = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.actions.len(), 2);
        assert_eq!(roundtrip.actions[0].title, "Fix import");
        assert!(roundtrip.actions[0].is_preferred);
        assert_eq!(roundtrip.actions[1].title, "Run test");
    }

    // --- parse_code_actions: non-array responses ---

    #[test]
    fn test_parse_code_actions_object_returns_empty() {
        let response = serde_json::json!({"title": "not an array"});
        assert!(parse_code_actions(&response).is_empty());
    }

    #[test]
    fn test_parse_code_actions_string_returns_empty() {
        let response = serde_json::json!("just a string");
        assert!(parse_code_actions(&response).is_empty());
    }

    #[test]
    fn test_parse_code_actions_number_returns_empty() {
        let response = serde_json::json!(42);
        assert!(parse_code_actions(&response).is_empty());
    }

    // --- parse_single_action: malformed entries ---

    #[test]
    fn test_parse_action_missing_title_skipped() {
        let response = serde_json::json!([
            {
                "kind": "quickfix",
                "edit": {}
            }
        ]);
        let actions = parse_code_actions(&response);
        assert!(
            actions.is_empty(),
            "actions without title should be skipped"
        );
    }

    #[test]
    fn test_parse_action_title_not_string_skipped() {
        let response = serde_json::json!([
            {
                "title": 123,
                "kind": "quickfix"
            }
        ]);
        let actions = parse_code_actions(&response);
        assert!(actions.is_empty(), "non-string title should be skipped");
    }

    #[test]
    fn test_parse_action_null_element_skipped() {
        let response = serde_json::json!([null, {"title": "Valid", "kind": "quickfix"}]);
        let actions = parse_code_actions(&response);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].title, "Valid");
    }

    // --- CodeAction with kind but no edit ---

    #[test]
    fn test_parse_code_action_no_edit_field() {
        let response = serde_json::json!([
            {
                "title": "Organize imports",
                "kind": "source.organizeImports",
                "isPreferred": false
            }
        ]);
        let actions = parse_code_actions(&response);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].kind.as_deref(), Some("source.organizeImports"));
        assert!(
            actions[0].edits.is_none(),
            "no edit field means edits is None"
        );
    }

    // --- CodeAction with empty workspace edit (should filter to None) ---

    #[test]
    fn test_parse_code_action_with_empty_edit() {
        let response = serde_json::json!([
            {
                "title": "No-op action",
                "kind": "quickfix",
                "edit": {}
            }
        ]);
        let actions = parse_code_actions(&response);
        assert_eq!(actions.len(), 1);
        assert!(
            actions[0].edits.is_none(),
            "empty workspace edit should be filtered to None"
        );
    }

    #[test]
    fn test_parse_code_action_with_empty_changes_map() {
        let response = serde_json::json!([
            {
                "title": "Empty changes",
                "kind": "refactor",
                "edit": {
                    "changes": {}
                }
            }
        ]);
        let actions = parse_code_actions(&response);
        assert_eq!(actions.len(), 1);
        assert!(
            actions[0].edits.is_none(),
            "empty changes map should produce None edits"
        );
    }

    #[test]
    fn test_parse_code_action_with_empty_document_changes() {
        let response = serde_json::json!([
            {
                "title": "Empty doc changes",
                "kind": "refactor",
                "edit": {
                    "documentChanges": []
                }
            }
        ]);
        let actions = parse_code_actions(&response);
        assert_eq!(actions.len(), 1);
        assert!(
            actions[0].edits.is_none(),
            "empty documentChanges array should produce None edits"
        );
    }

    // --- is_preferred defaults ---

    #[test]
    fn test_is_preferred_defaults_false_when_absent() {
        let response = serde_json::json!([
            {
                "title": "Some action",
                "kind": "quickfix"
            }
        ]);
        let actions = parse_code_actions(&response);
        assert!(!actions[0].is_preferred);
    }

    #[test]
    fn test_is_preferred_true() {
        let response = serde_json::json!([
            {
                "title": "Preferred action",
                "kind": "quickfix",
                "isPreferred": true
            }
        ]);
        let actions = parse_code_actions(&response);
        assert!(actions[0].is_preferred);
    }

    // --- parse_workspace_edit: documentChanges precedence over changes ---

    #[test]
    fn test_parse_workspace_edit_document_changes_takes_precedence() {
        let edit = serde_json::json!({
            "documentChanges": [
                {
                    "textDocument": { "uri": "file:///src/from_doc_changes.rs", "version": 1 },
                    "edits": [
                        {
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": 0, "character": 0 }
                            },
                            "newText": "from documentChanges"
                        }
                    ]
                }
            ],
            "changes": {
                "file:///src/from_changes.rs": [
                    {
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 0, "character": 0 }
                        },
                        "newText": "from changes"
                    }
                ]
            }
        });

        let file_edits = parse_workspace_edit(&edit);
        assert_eq!(file_edits.len(), 1);
        assert!(
            file_edits[0].file_path.contains("from_doc_changes"),
            "documentChanges should take precedence over changes"
        );
        assert_eq!(file_edits[0].text_edits[0].new_text, "from documentChanges");
    }

    // --- parse_workspace_edit: multiple edits in one file ---

    #[test]
    fn test_parse_workspace_edit_multiple_edits_in_one_file() {
        let edit = serde_json::json!({
            "changes": {
                "file:///src/multi.rs": [
                    {
                        "range": {
                            "start": { "line": 1, "character": 0 },
                            "end": { "line": 1, "character": 5 }
                        },
                        "newText": "first"
                    },
                    {
                        "range": {
                            "start": { "line": 10, "character": 0 },
                            "end": { "line": 10, "character": 3 }
                        },
                        "newText": "second"
                    }
                ]
            }
        });

        let file_edits = parse_workspace_edit(&edit);
        assert_eq!(file_edits.len(), 1);
        assert_eq!(file_edits[0].text_edits.len(), 2);
        assert_eq!(file_edits[0].text_edits[0].new_text, "first");
        assert_eq!(file_edits[0].text_edits[1].new_text, "second");
    }

    // --- parse_workspace_edit: multiple files in documentChanges ---

    #[test]
    fn test_parse_workspace_edit_multiple_files_document_changes() {
        let edit = serde_json::json!({
            "documentChanges": [
                {
                    "textDocument": { "uri": "file:///src/a.rs", "version": 1 },
                    "edits": [
                        {
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": 0, "character": 0 }
                            },
                            "newText": "edit_a"
                        }
                    ]
                },
                {
                    "textDocument": { "uri": "file:///src/b.rs", "version": 2 },
                    "edits": [
                        {
                            "range": {
                                "start": { "line": 5, "character": 0 },
                                "end": { "line": 5, "character": 0 }
                            },
                            "newText": "edit_b"
                        }
                    ]
                }
            ]
        });

        let file_edits = parse_workspace_edit(&edit);
        assert_eq!(file_edits.len(), 2);
    }

    // --- parse_text_edit: malformed edits ---

    #[test]
    fn test_parse_workspace_edit_text_edit_missing_new_text() {
        let edit = serde_json::json!({
            "changes": {
                "file:///src/bad.rs": [
                    {
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 0, "character": 0 }
                        }
                    }
                ]
            }
        });

        let file_edits = parse_workspace_edit(&edit);
        assert!(
            file_edits.is_empty(),
            "text edit missing newText should be skipped, leaving no valid edits"
        );
    }

    #[test]
    fn test_parse_workspace_edit_text_edit_missing_range() {
        let edit = serde_json::json!({
            "changes": {
                "file:///src/bad.rs": [
                    {
                        "newText": "orphan text"
                    }
                ]
            }
        });

        let file_edits = parse_workspace_edit(&edit);
        assert!(
            file_edits.is_empty(),
            "text edit missing range should be skipped"
        );
    }

    #[test]
    fn test_parse_workspace_edit_text_edit_malformed_range() {
        let edit = serde_json::json!({
            "changes": {
                "file:///src/bad.rs": [
                    {
                        "range": {
                            "start": { "line": 0 }
                        },
                        "newText": "bad range"
                    }
                ]
            }
        });

        let file_edits = parse_workspace_edit(&edit);
        assert!(
            file_edits.is_empty(),
            "text edit with incomplete range should be skipped"
        );
    }

    // --- documentChanges: missing textDocument or edits ---

    #[test]
    fn test_parse_document_changes_missing_text_document() {
        let edit = serde_json::json!({
            "documentChanges": [
                {
                    "edits": [
                        {
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": 0, "character": 0 }
                            },
                            "newText": "orphan"
                        }
                    ]
                }
            ]
        });

        let file_edits = parse_workspace_edit(&edit);
        assert!(
            file_edits.is_empty(),
            "documentChange missing textDocument should be skipped"
        );
    }

    #[test]
    fn test_parse_document_changes_missing_edits_array() {
        let edit = serde_json::json!({
            "documentChanges": [
                {
                    "textDocument": { "uri": "file:///src/main.rs", "version": 1 }
                }
            ]
        });

        let file_edits = parse_workspace_edit(&edit);
        assert!(
            file_edits.is_empty(),
            "documentChange missing edits array should be skipped"
        );
    }

    // --- CodeAction with command object (not string) should still parse as CodeAction ---

    #[test]
    fn test_parse_code_action_with_nested_command_object() {
        let response = serde_json::json!([
            {
                "title": "Fix with command object",
                "kind": "quickfix",
                "command": {
                    "title": "Apply fix",
                    "command": "editor.action.fixAll",
                    "arguments": []
                },
                "isPreferred": false
            }
        ]);
        let actions = parse_code_actions(&response);
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0].kind.as_deref(),
            Some("quickfix"),
            "action with kind + command object should be parsed as CodeAction, not Command"
        );
    }

    // --- Range values are correctly parsed ---

    #[test]
    fn test_parse_workspace_edit_range_values() {
        let edit = serde_json::json!({
            "changes": {
                "file:///src/range.rs": [
                    {
                        "range": {
                            "start": { "line": 3, "character": 7 },
                            "end": { "line": 5, "character": 12 }
                        },
                        "newText": "replaced"
                    }
                ]
            }
        });

        let file_edits = parse_workspace_edit(&edit);
        assert_eq!(file_edits.len(), 1);
        let te = &file_edits[0].text_edits[0];
        assert_eq!(te.range.start_line, 3);
        assert_eq!(te.range.start_character, 7);
        assert_eq!(te.range.end_line, 5);
        assert_eq!(te.range.end_character, 12);
    }

    // --- changes format: edits value not an array ---

    #[test]
    fn test_parse_workspace_edit_changes_value_not_array() {
        let edit = serde_json::json!({
            "changes": {
                "file:///src/bad.rs": "not an array"
            }
        });

        let file_edits = parse_workspace_edit(&edit);
        assert!(
            file_edits.is_empty(),
            "non-array edits value should be skipped"
        );
    }

    // --- Multiple valid and invalid actions in one response ---

    #[test]
    fn test_parse_code_actions_mixed_valid_invalid() {
        let response = serde_json::json!([
            { "title": "Good command", "command": "do.thing" },
            { "no_title": true },
            null,
            {
                "title": "Good action",
                "kind": "source.organizeImports",
                "isPreferred": true,
                "edit": {
                    "changes": {
                        "file:///src/main.rs": [
                            {
                                "range": {
                                    "start": { "line": 0, "character": 0 },
                                    "end": { "line": 0, "character": 0 }
                                },
                                "newText": "import"
                            }
                        ]
                    }
                }
            },
            42
        ]);
        let actions = parse_code_actions(&response);
        assert_eq!(actions.len(), 2, "only valid actions should survive");
        assert_eq!(actions[0].title, "Good command");
        assert_eq!(actions[1].title, "Good action");
        assert_eq!(actions[1].kind.as_deref(), Some("source.organizeImports"));
        assert!(actions[1].is_preferred);
    }
}
