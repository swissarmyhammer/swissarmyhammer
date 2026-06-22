//! The one diagnostics parser and the in-process fan-out payload.
//!
//! Both the push model (`textDocument/publishDiagnostics` notifications) and the
//! pull model (`textDocument/diagnostic` request results) arrive as raw
//! JSON-RPC payloads. This module owns the single, lenient parser that turns
//! those payloads into [`lsp_types::Diagnostic`] records, so there is exactly
//! one place that maps the LSP wire shape onto typed diagnostics. The
//! [`LspSession`](crate::session::LspSession) feeds the result of both models
//! into the same per-uri cache and the same [`DiagnosticUpdate`] broadcast.
//!
//! The parser is deliberately lenient: a malformed item (missing range,
//! missing message, an un-parseable position) is skipped rather than failing
//! the whole batch, matching how editors tolerate partial server output.

use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range};
use serde_json::Value;

/// One per-uri diagnostics update, broadcast to in-process subscribers.
///
/// Carries the *latest* full set of diagnostics for `uri` — diagnostics are
/// published as a complete replacement for a document, not as a delta, so each
/// update fully describes the current state of `uri`.
#[derive(Debug, Clone)]
pub struct DiagnosticUpdate {
    /// The document URI the diagnostics apply to (e.g. `file:///src/main.rs`).
    pub uri: String,
    /// The latest complete set of diagnostics for `uri`.
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse a `textDocument/publishDiagnostics` notification's params.
///
/// The params object has the shape `{ "uri": "...", "diagnostics": [...] }`.
/// Returns the parsed diagnostics; malformed individual items are skipped.
pub fn parse_publish_diagnostics(params: &Value) -> Vec<Diagnostic> {
    match params.get("diagnostics").and_then(|v| v.as_array()) {
        Some(items) => items.iter().filter_map(parse_single_diagnostic).collect(),
        None => Vec::new(),
    }
}

/// Parse a `textDocument/diagnostic` (pull) response result.
///
/// The result is one of:
/// - `{ "kind": "full", "items": [...] }` (a `DocumentDiagnosticReport`),
/// - `{ "items": [...] }` (simplified), or
/// - a bare array of diagnostics.
///
/// Anything else yields an empty vector. Malformed individual items are
/// skipped.
pub fn parse_diagnostics_from_result(result: &Value) -> Vec<Diagnostic> {
    let items = if let Some(items) = result.get("items").and_then(|v| v.as_array()) {
        items
    } else if let Some(arr) = result.as_array() {
        arr
    } else {
        return Vec::new();
    };

    items.iter().filter_map(parse_single_diagnostic).collect()
}

/// Parse a single raw LSP diagnostic object into an [`lsp_types::Diagnostic`].
///
/// Returns `None` when the object lacks a usable range or message. A missing
/// severity defaults to [`DiagnosticSeverity::HINT`], and a `code` that is
/// neither a string nor an integer is dropped (left `None`) rather than
/// rejecting the diagnostic.
fn parse_single_diagnostic(item: &Value) -> Option<Diagnostic> {
    let range = parse_range(item.get("range")?)?;
    let message = item.get("message")?.as_str()?.to_string();

    let severity = item
        .get("severity")
        .and_then(|v| v.as_u64())
        .map(severity_from_lsp)
        .unwrap_or(DiagnosticSeverity::HINT);

    let code = item.get("code").and_then(parse_code);

    let source = item
        .get("source")
        .and_then(|v| v.as_str())
        .map(String::from);

    Some(Diagnostic {
        range,
        severity: Some(severity),
        code,
        source,
        message,
        ..Diagnostic::default()
    })
}

/// Parse an LSP `Range` object, requiring complete start/end positions.
fn parse_range(range: &Value) -> Option<Range> {
    Some(Range {
        start: parse_position(range.get("start")?)?,
        end: parse_position(range.get("end")?)?,
    })
}

/// Parse an LSP `Position` object, requiring both `line` and `character`.
fn parse_position(pos: &Value) -> Option<Position> {
    Some(Position {
        line: pos.get("line")?.as_u64()? as u32,
        character: pos.get("character")?.as_u64()? as u32,
    })
}

/// Parse an LSP diagnostic `code`, which may be a string or an integer.
///
/// A boolean, object, or other shape yields `None` so the enclosing diagnostic
/// is kept without a code rather than discarded.
fn parse_code(value: &Value) -> Option<NumberOrString> {
    if let Some(s) = value.as_str() {
        Some(NumberOrString::String(s.to_string()))
    } else {
        value
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .map(NumberOrString::Number)
    }
}

/// Map an LSP severity integer (1=Error, 2=Warning, 3=Info, 4=Hint) to a
/// [`DiagnosticSeverity`], defaulting unknown values to [`DiagnosticSeverity::HINT`].
fn severity_from_lsp(value: u64) -> DiagnosticSeverity {
    match value {
        1 => DiagnosticSeverity::ERROR,
        2 => DiagnosticSeverity::WARNING,
        3 => DiagnosticSeverity::INFORMATION,
        4 => DiagnosticSeverity::HINT,
        _ => DiagnosticSeverity::HINT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- publishDiagnostics notification parsing ---

    #[test]
    fn parse_publish_diagnostics_basic() {
        let params = json!({
            "uri": "file:///src/main.rs",
            "diagnostics": [
                {
                    "range": {
                        "start": { "line": 5, "character": 10 },
                        "end": { "line": 5, "character": 20 }
                    },
                    "severity": 1,
                    "message": "mismatched types",
                    "code": "E0308",
                    "source": "rustc"
                },
                {
                    "range": {
                        "start": { "line": 12, "character": 0 },
                        "end": { "line": 12, "character": 15 }
                    },
                    "severity": 2,
                    "message": "unused variable",
                    "code": "unused_variables",
                    "source": "clippy"
                }
            ]
        });

        let diagnostics = parse_publish_diagnostics(&params);
        assert_eq!(diagnostics.len(), 2);

        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diagnostics[0].message, "mismatched types");
        assert_eq!(
            diagnostics[0].code,
            Some(NumberOrString::String("E0308".to_string()))
        );
        assert_eq!(diagnostics[0].source.as_deref(), Some("rustc"));
        assert_eq!(diagnostics[0].range.start.line, 5);
        assert_eq!(diagnostics[0].range.start.character, 10);

        assert_eq!(diagnostics[1].severity, Some(DiagnosticSeverity::WARNING));
        assert_eq!(diagnostics[1].message, "unused variable");
        assert_eq!(diagnostics[1].source.as_deref(), Some("clippy"));
    }

    #[test]
    fn parse_publish_diagnostics_empty() {
        let params = json!({
            "uri": "file:///src/main.rs",
            "diagnostics": []
        });
        assert!(parse_publish_diagnostics(&params).is_empty());
    }

    #[test]
    fn parse_publish_diagnostics_missing_diagnostics_key() {
        let params = json!({ "uri": "file:///src/main.rs" });
        assert!(parse_publish_diagnostics(&params).is_empty());
    }

    #[test]
    fn parse_publish_diagnostics_mixed_valid_and_invalid() {
        let params = json!({
            "uri": "file:///src/lib.rs",
            "diagnostics": [
                {
                    "range": {
                        "start": { "line": 1, "character": 0 },
                        "end": { "line": 1, "character": 10 }
                    },
                    "severity": 1,
                    "message": "valid error"
                },
                {
                    "message": "missing range - skipped"
                },
                {
                    "range": {
                        "start": { "line": 2, "character": 0 },
                        "end": { "line": 2, "character": 5 }
                    },
                    "severity": 3,
                    "message": "valid info"
                }
            ]
        });
        let diagnostics = parse_publish_diagnostics(&params);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].message, "valid error");
        assert_eq!(diagnostics[1].message, "valid info");
    }

    // --- pull diagnostics response parsing ---

    #[test]
    fn parse_diagnostics_from_result_full_report() {
        let result = json!({
            "kind": "full",
            "items": [
                {
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 5 }
                    },
                    "severity": 1,
                    "message": "syntax error"
                }
            ]
        });
        let diagnostics = parse_diagnostics_from_result(&result);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, "syntax error");
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
    }

    #[test]
    fn parse_diagnostics_from_result_direct_array() {
        let result = json!([
            {
                "range": {
                    "start": { "line": 3, "character": 4 },
                    "end": { "line": 3, "character": 10 }
                },
                "severity": 2,
                "message": "deprecated function"
            }
        ]);
        let diagnostics = parse_diagnostics_from_result(&result);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::WARNING));
    }

    #[test]
    fn parse_diagnostics_numeric_code() {
        let result = json!({
            "items": [
                {
                    "range": {
                        "start": { "line": 1, "character": 0 },
                        "end": { "line": 1, "character": 10 }
                    },
                    "severity": 1,
                    "message": "error",
                    "code": 42
                }
            ]
        });
        let diagnostics = parse_diagnostics_from_result(&result);
        assert_eq!(diagnostics[0].code, Some(NumberOrString::Number(42)));
    }

    #[test]
    fn parse_diagnostics_missing_severity_defaults_to_hint() {
        let result = json!({
            "items": [
                {
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 5 }
                    },
                    "message": "info message"
                }
            ]
        });
        let diagnostics = parse_diagnostics_from_result(&result);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::HINT));
    }

    #[test]
    fn parse_diagnostics_from_result_plain_string_returns_empty() {
        assert!(parse_diagnostics_from_result(&json!("not a diagnostic")).is_empty());
    }

    #[test]
    fn parse_diagnostics_from_result_number_returns_empty() {
        assert!(parse_diagnostics_from_result(&json!(42)).is_empty());
    }

    #[test]
    fn parse_diagnostics_from_result_null_returns_empty() {
        assert!(parse_diagnostics_from_result(&json!(null)).is_empty());
    }

    #[test]
    fn parse_diagnostics_from_result_bool_returns_empty() {
        assert!(parse_diagnostics_from_result(&json!(true)).is_empty());
    }

    #[test]
    fn parse_diagnostics_from_result_object_without_items_returns_empty() {
        let result = json!({ "kind": "full", "resultId": "abc" });
        assert!(parse_diagnostics_from_result(&result).is_empty());
    }

    #[test]
    fn parse_diagnostics_from_result_empty_items_array() {
        assert!(parse_diagnostics_from_result(&json!({ "items": [] })).is_empty());
    }

    #[test]
    fn parse_diagnostics_from_result_empty_direct_array() {
        assert!(parse_diagnostics_from_result(&json!([])).is_empty());
    }

    #[test]
    fn parse_diagnostics_skips_item_missing_range() {
        let result = json!({
            "items": [ { "severity": 1, "message": "no range here" } ]
        });
        assert!(parse_diagnostics_from_result(&result).is_empty());
    }

    #[test]
    fn parse_diagnostics_skips_item_missing_message() {
        let result = json!({
            "items": [
                {
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 5 }
                    },
                    "severity": 1
                }
            ]
        });
        assert!(parse_diagnostics_from_result(&result).is_empty());
    }

    #[test]
    fn parse_diagnostics_skips_item_with_incomplete_range() {
        // range.start is missing "character"
        let result = json!({
            "items": [
                {
                    "range": {
                        "start": { "line": 0 },
                        "end": { "line": 0, "character": 5 }
                    },
                    "severity": 1,
                    "message": "bad range"
                }
            ]
        });
        assert!(parse_diagnostics_from_result(&result).is_empty());
    }

    #[test]
    fn parse_diagnostics_mixed_valid_and_invalid_items() {
        let result = json!({
            "items": [
                { "severity": 1, "message": "missing range - should be skipped" },
                {
                    "range": {
                        "start": { "line": 5, "character": 0 },
                        "end": { "line": 5, "character": 10 }
                    },
                    "severity": 2,
                    "message": "valid warning"
                },
                {
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 1 }
                    },
                    "severity": 1
                }
            ]
        });
        let diagnostics = parse_diagnostics_from_result(&result);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, "valid warning");
    }

    #[test]
    fn parse_diagnostics_code_as_non_string_non_number_yields_none() {
        // code is a boolean -- neither string nor integer, so dropped to None
        // while keeping the diagnostic.
        let result = json!({
            "items": [
                {
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 5 }
                    },
                    "severity": 1,
                    "message": "test",
                    "code": true
                }
            ]
        });
        let diagnostics = parse_diagnostics_from_result(&result);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].code.is_none());
    }

    #[test]
    fn parse_diagnostics_source_as_non_string_yields_none() {
        let result = json!({
            "items": [
                {
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 5 }
                    },
                    "severity": 1,
                    "message": "test",
                    "source": 123
                }
            ]
        });
        let diagnostics = parse_diagnostics_from_result(&result);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].source.is_none());
    }

    #[test]
    fn parse_diagnostics_from_result_direct_array_multiple() {
        let result = json!([
            {
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 5 }
                },
                "severity": 1,
                "message": "first error"
            },
            {
                "range": {
                    "start": { "line": 10, "character": 4 },
                    "end": { "line": 10, "character": 20 }
                },
                "severity": 3,
                "message": "some info",
                "code": "W001",
                "source": "mypy"
            }
        ]);
        let diagnostics = parse_diagnostics_from_result(&result);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diagnostics[0].message, "first error");
        assert_eq!(diagnostics[0].code, None);
        assert_eq!(
            diagnostics[1].severity,
            Some(DiagnosticSeverity::INFORMATION)
        );
        assert_eq!(
            diagnostics[1].code,
            Some(NumberOrString::String("W001".to_string()))
        );
        assert_eq!(diagnostics[1].source.as_deref(), Some("mypy"));
        assert_eq!(diagnostics[1].range.start.line, 10);
        assert_eq!(diagnostics[1].range.start.character, 4);
    }

    #[test]
    fn severity_from_lsp_maps_known_values() {
        assert_eq!(severity_from_lsp(1), DiagnosticSeverity::ERROR);
        assert_eq!(severity_from_lsp(2), DiagnosticSeverity::WARNING);
        assert_eq!(severity_from_lsp(3), DiagnosticSeverity::INFORMATION);
        assert_eq!(severity_from_lsp(4), DiagnosticSeverity::HINT);
    }

    #[test]
    fn severity_from_lsp_unknown_defaults_to_hint() {
        assert_eq!(severity_from_lsp(99), DiagnosticSeverity::HINT);
    }
}
