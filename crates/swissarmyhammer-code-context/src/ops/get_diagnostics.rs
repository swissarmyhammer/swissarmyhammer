//! File diagnostics (errors, warnings) via live LSP pull diagnostics.
//!
//! This operation is **live LSP only** -- there is no meaningful index fallback
//! for diagnostics, which require live analysis of the file. When no live LSP
//! is available, returns an empty result with `SourceLayer::None`.
//!
//! Uses `textDocument/diagnostic` (LSP 3.17+ pull diagnostics) via
//! [`LayeredContext::lsp_request`]. Falls back to empty if the server
//! does not support pull diagnostics.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::layered_context::{LayeredContext, LspRange, SourceLayer};
use crate::ops::lsp_helpers::file_path_to_uri;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Severity level for a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

impl DiagnosticSeverity {
    /// Convert an LSP severity integer (1=Error, 2=Warning, 3=Info, 4=Hint)
    /// to our enum. Defaults to `Hint` for unknown values.
    pub fn from_lsp(value: u64) -> Self {
        match value {
            1 => Self::Error,
            2 => Self::Warning,
            3 => Self::Info,
            4 => Self::Hint,
            _ => Self::Hint,
        }
    }

    /// Convert to the LSP severity integer.
    pub fn to_lsp(self) -> u64 {
        match self {
            Self::Error => 1,
            Self::Warning => 2,
            Self::Info => 3,
            Self::Hint => 4,
        }
    }
}

/// A single diagnostic for a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    /// The range in the file where the diagnostic applies.
    pub range: LspRange,
    /// Severity of the diagnostic.
    pub severity: DiagnosticSeverity,
    /// Human-readable message.
    pub message: String,
    /// Optional diagnostic code (e.g. "E0308").
    pub code: Option<String>,
    /// Optional source tool (e.g. "rustc", "clippy").
    pub source: Option<String>,
    /// Name of the enclosing symbol, enriched via `enrich_location`.
    pub containing_symbol: Option<String>,
}

/// Options for the `get_diagnostics` operation.
#[derive(Debug, Clone)]
pub struct GetDiagnosticsOptions {
    /// Path to the file (relative to workspace root or absolute).
    pub file_path: String,
    /// Optional severity filter -- only return diagnostics at or above this level.
    /// `None` means return all severities.
    pub severity_filter: Option<DiagnosticSeverity>,
}

/// Result of a diagnostics operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticsResult {
    /// The collected diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// Count of Error-severity diagnostics.
    pub error_count: usize,
    /// Count of Warning-severity diagnostics.
    pub warning_count: usize,
    /// Which data layer provided the result.
    pub source_layer: SourceLayer,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Get diagnostics for a file.
///
/// Uses pull diagnostics (`textDocument/diagnostic`, LSP 3.17+) via the live
/// LSP server. Returns an empty result when no live LSP is available or the
/// server does not support pull diagnostics.
///
/// # Arguments
/// * `ctx` - The layered context providing access to all data layers.
/// * `opts` - The file path and optional severity filter.
///
/// # Errors
/// Returns a `CodeContextError` if an LSP request fails unexpectedly.
pub fn get_diagnostics(
    ctx: &LayeredContext,
    opts: &GetDiagnosticsOptions,
) -> Result<DiagnosticsResult, crate::error::CodeContextError> {
    if !ctx.has_live_lsp() {
        return Ok(empty_result());
    }

    match try_pull_diagnostics(ctx, opts) {
        Ok(Some(result)) => Ok(result),
        Ok(None) | Err(_) => {
            // Pull diagnostics not supported or failed -- return empty
            Ok(empty_result())
        }
    }
}

// ---------------------------------------------------------------------------
// Pull diagnostics (LSP 3.17+)
// ---------------------------------------------------------------------------

/// Attempt to get diagnostics via `textDocument/diagnostic` (pull model).
///
/// Opens the document, requests diagnostics, then closes the document
/// atomically under a single mutex hold to prevent interleaving with the
/// indexing worker.
/// Returns `None` if the server responds with an error (likely unsupported).
fn try_pull_diagnostics(
    ctx: &LayeredContext,
    opts: &GetDiagnosticsOptions,
) -> Result<Option<DiagnosticsResult>, crate::error::CodeContextError> {
    let uri = file_path_to_uri(&opts.file_path);

    let response = ctx.lsp_request_with_document(
        &opts.file_path,
        "textDocument/diagnostic",
        json!({
            "textDocument": { "uri": uri }
        }),
    )?;

    let response = match response {
        Some(v) if !v.is_null() => v,
        _ => return Ok(None),
    };

    // Check for error response (server doesn't support pull diagnostics)
    if response.get("error").is_some() {
        return Ok(None);
    }

    // Parse the result -- could be in "result" or directly at top level
    let result_val = response.get("result").unwrap_or(&response);

    let raw_diagnostics = parse_diagnostics_from_result(result_val);
    let diagnostics =
        enrich_and_filter(ctx, &opts.file_path, raw_diagnostics, opts.severity_filter);

    let error_count = diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .count();
    let warning_count = diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Warning)
        .count();

    Ok(Some(DiagnosticsResult {
        diagnostics,
        error_count,
        warning_count,
        source_layer: SourceLayer::LiveLsp,
    }))
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/// Parse diagnostics from a `textDocument/diagnostic` response result.
///
/// The result contains either:
/// - `{ "kind": "full", "items": [...] }` (DocumentDiagnosticReport)
/// - `{ "items": [...] }` (simplified)
/// - A direct array of diagnostics
pub fn parse_diagnostics_from_result(result: &serde_json::Value) -> Vec<Diagnostic> {
    // Try "items" array first (standard pull diagnostics response)
    let items = if let Some(items) = result.get("items").and_then(|v| v.as_array()) {
        items.clone()
    } else if let Some(arr) = result.as_array() {
        arr.clone()
    } else {
        return Vec::new();
    };

    items.iter().filter_map(parse_single_diagnostic).collect()
}

/// Parse diagnostics from a `publishDiagnostics` notification.
///
/// The notification params contain `{ "uri": "...", "diagnostics": [...] }`.
pub fn parse_publish_diagnostics(params: &serde_json::Value) -> Vec<Diagnostic> {
    let items = match params.get("diagnostics").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    items.iter().filter_map(parse_single_diagnostic).collect()
}

/// Parse a single LSP diagnostic object into our `Diagnostic` type.
fn parse_single_diagnostic(item: &serde_json::Value) -> Option<Diagnostic> {
    let range_val = item.get("range")?;
    let start = range_val.get("start")?;
    let end = range_val.get("end")?;

    let range = LspRange {
        start_line: start.get("line")?.as_u64()? as u32,
        start_character: start.get("character")?.as_u64()? as u32,
        end_line: end.get("line")?.as_u64()? as u32,
        end_character: end.get("character")?.as_u64()? as u32,
    };

    let severity = item
        .get("severity")
        .and_then(|v| v.as_u64())
        .map(DiagnosticSeverity::from_lsp)
        .unwrap_or(DiagnosticSeverity::Hint);

    let message = item.get("message")?.as_str()?.to_string();

    let code = item.get("code").and_then(|v| {
        // Code can be a string or integer
        if let Some(s) = v.as_str() {
            Some(s.to_string())
        } else {
            v.as_u64().map(|n| n.to_string())
        }
    });

    let source = item
        .get("source")
        .and_then(|v| v.as_str())
        .map(String::from);

    Some(Diagnostic {
        range,
        severity,
        message,
        code,
        source,
        containing_symbol: None, // enriched later
    })
}

// ---------------------------------------------------------------------------
// Enrichment and filtering
// ---------------------------------------------------------------------------

/// Filter by severity and enrich each diagnostic with its enclosing symbol.
fn enrich_and_filter(
    ctx: &LayeredContext,
    file_path: &str,
    diagnostics: Vec<Diagnostic>,
    severity_filter: Option<DiagnosticSeverity>,
) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .filter(|d| passes_severity_filter(d.severity, severity_filter))
        .map(|mut d| {
            let enrichment = ctx.enrich_location(file_path, &d.range);
            d.containing_symbol = enrichment.symbol.map(|s| s.name);
            d
        })
        .collect()
}

/// Check if a diagnostic severity passes the given filter.
///
/// Severity ordering: Error(1) > Warning(2) > Info(3) > Hint(4).
/// A filter of `Warning` means only Error and Warning pass.
pub fn passes_severity_filter(
    severity: DiagnosticSeverity,
    filter: Option<DiagnosticSeverity>,
) -> bool {
    match filter {
        None => true,
        Some(threshold) => severity.to_lsp() <= threshold.to_lsp(),
    }
}

/// Create an empty diagnostics result for when no data is available.
fn empty_result() -> DiagnosticsResult {
    DiagnosticsResult {
        diagnostics: Vec::new(),
        error_count: 0,
        warning_count: 0,
        source_layer: SourceLayer::None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::test_db;

    // --- publishDiagnostics notification parsing ---

    #[test]
    fn test_parse_publish_diagnostics_basic() {
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

        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
        assert_eq!(diagnostics[0].message, "mismatched types");
        assert_eq!(diagnostics[0].code.as_deref(), Some("E0308"));
        assert_eq!(diagnostics[0].source.as_deref(), Some("rustc"));
        assert_eq!(diagnostics[0].range.start_line, 5);
        assert_eq!(diagnostics[0].range.start_character, 10);

        assert_eq!(diagnostics[1].severity, DiagnosticSeverity::Warning);
        assert_eq!(diagnostics[1].message, "unused variable");
        assert_eq!(diagnostics[1].source.as_deref(), Some("clippy"));
    }

    #[test]
    fn test_parse_publish_diagnostics_empty() {
        let params = json!({
            "uri": "file:///src/main.rs",
            "diagnostics": []
        });
        let diagnostics = parse_publish_diagnostics(&params);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_parse_publish_diagnostics_missing_diagnostics_key() {
        let params = json!({ "uri": "file:///src/main.rs" });
        let diagnostics = parse_publish_diagnostics(&params);
        assert!(diagnostics.is_empty());
    }

    // --- pull diagnostics response parsing ---

    #[test]
    fn test_parse_diagnostics_from_result_full_report() {
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
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
    }

    #[test]
    fn test_parse_diagnostics_from_result_direct_array() {
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
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Warning);
    }

    #[test]
    fn test_parse_diagnostics_numeric_code() {
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
        assert_eq!(diagnostics[0].code.as_deref(), Some("42"));
    }

    #[test]
    fn test_parse_diagnostics_missing_severity_defaults_to_hint() {
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
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Hint);
    }

    // --- severity filter tests ---

    #[test]
    fn test_severity_filter_none_passes_all() {
        assert!(passes_severity_filter(DiagnosticSeverity::Error, None));
        assert!(passes_severity_filter(DiagnosticSeverity::Warning, None));
        assert!(passes_severity_filter(DiagnosticSeverity::Info, None));
        assert!(passes_severity_filter(DiagnosticSeverity::Hint, None));
    }

    #[test]
    fn test_severity_filter_error_only() {
        let filter = Some(DiagnosticSeverity::Error);
        assert!(passes_severity_filter(DiagnosticSeverity::Error, filter));
        assert!(!passes_severity_filter(DiagnosticSeverity::Warning, filter));
        assert!(!passes_severity_filter(DiagnosticSeverity::Info, filter));
        assert!(!passes_severity_filter(DiagnosticSeverity::Hint, filter));
    }

    #[test]
    fn test_severity_filter_warning_and_above() {
        let filter = Some(DiagnosticSeverity::Warning);
        assert!(passes_severity_filter(DiagnosticSeverity::Error, filter));
        assert!(passes_severity_filter(DiagnosticSeverity::Warning, filter));
        assert!(!passes_severity_filter(DiagnosticSeverity::Info, filter));
        assert!(!passes_severity_filter(DiagnosticSeverity::Hint, filter));
    }

    #[test]
    fn test_severity_filter_info_and_above() {
        let filter = Some(DiagnosticSeverity::Info);
        assert!(passes_severity_filter(DiagnosticSeverity::Error, filter));
        assert!(passes_severity_filter(DiagnosticSeverity::Warning, filter));
        assert!(passes_severity_filter(DiagnosticSeverity::Info, filter));
        assert!(!passes_severity_filter(DiagnosticSeverity::Hint, filter));
    }

    // --- enrichment maps diagnostic range to enclosing symbol ---

    #[test]
    fn test_enrichment_maps_diagnostic_to_symbol() {
        let conn = test_db();
        conn.execute(
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed)
             VALUES ('src/main.rs', X'DEADBEEF', 1024, 1000, 0, 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, detail, file_path, start_line, start_char, end_line, end_char)
             VALUES ('sym1', 'process_data', 12, NULL, 'src/main.rs', 5, 0, 25, 1)",
            [],
        )
        .unwrap();

        let ctx = LayeredContext::new(&conn, None);

        // A diagnostic at line 10 falls within the process_data function (lines 5-25)
        let raw = vec![Diagnostic {
            range: LspRange {
                start_line: 10,
                start_character: 5,
                end_line: 10,
                end_character: 20,
            },
            severity: DiagnosticSeverity::Error,
            message: "type mismatch".to_string(),
            code: Some("E0308".to_string()),
            source: Some("rustc".to_string()),
            containing_symbol: None,
        }];

        let enriched = enrich_and_filter(&ctx, "src/main.rs", raw, None);
        assert_eq!(enriched.len(), 1);
        assert_eq!(
            enriched[0].containing_symbol.as_deref(),
            Some("process_data")
        );
    }

    // --- empty result when no live LSP ---

    #[test]
    fn test_no_live_lsp_returns_empty() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, None);
        let opts = GetDiagnosticsOptions {
            file_path: "src/main.rs".to_string(),
            severity_filter: None,
        };
        let result = get_diagnostics(&ctx, &opts).unwrap();
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.error_count, 0);
        assert_eq!(result.warning_count, 0);
        assert_eq!(result.source_layer, SourceLayer::None);
    }

    // --- DiagnosticSeverity conversion ---

    #[test]
    fn test_severity_roundtrip() {
        for (lsp, expected) in [
            (1, DiagnosticSeverity::Error),
            (2, DiagnosticSeverity::Warning),
            (3, DiagnosticSeverity::Info),
            (4, DiagnosticSeverity::Hint),
        ] {
            let severity = DiagnosticSeverity::from_lsp(lsp);
            assert_eq!(severity, expected);
            assert_eq!(severity.to_lsp(), lsp);
        }
    }

    #[test]
    fn test_severity_unknown_defaults_to_hint() {
        assert_eq!(DiagnosticSeverity::from_lsp(99), DiagnosticSeverity::Hint);
    }

    // --- DiagnosticsResult serialization ---

    #[test]
    fn test_diagnostics_result_serializable() {
        let result = DiagnosticsResult {
            diagnostics: vec![Diagnostic {
                range: LspRange {
                    start_line: 1,
                    start_character: 0,
                    end_line: 1,
                    end_character: 10,
                },
                severity: DiagnosticSeverity::Error,
                message: "test error".to_string(),
                code: Some("E0001".to_string()),
                source: Some("rustc".to_string()),
                containing_symbol: Some("main".to_string()),
            }],
            error_count: 1,
            warning_count: 0,
            source_layer: SourceLayer::LiveLsp,
        };
        let json = serde_json::to_string(&result).unwrap();
        let roundtrip: DiagnosticsResult = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.diagnostics.len(), 1);
        assert_eq!(roundtrip.error_count, 1);
        assert_eq!(roundtrip.source_layer, SourceLayer::LiveLsp);
    }

    // --- parse_diagnostics_from_result: non-array, non-object-with-items fallback ---

    #[test]
    fn test_parse_diagnostics_from_result_plain_string_returns_empty() {
        let result = json!("not a diagnostic at all");
        assert!(parse_diagnostics_from_result(&result).is_empty());
    }

    #[test]
    fn test_parse_diagnostics_from_result_number_returns_empty() {
        let result = json!(42);
        assert!(parse_diagnostics_from_result(&result).is_empty());
    }

    #[test]
    fn test_parse_diagnostics_from_result_null_returns_empty() {
        let result = json!(null);
        assert!(parse_diagnostics_from_result(&result).is_empty());
    }

    #[test]
    fn test_parse_diagnostics_from_result_bool_returns_empty() {
        let result = json!(true);
        assert!(parse_diagnostics_from_result(&result).is_empty());
    }

    #[test]
    fn test_parse_diagnostics_from_result_object_without_items_returns_empty() {
        let result = json!({ "kind": "full", "resultId": "abc" });
        assert!(parse_diagnostics_from_result(&result).is_empty());
    }

    // --- parse_diagnostics_from_result: empty containers ---

    #[test]
    fn test_parse_diagnostics_from_result_empty_items_array() {
        let result = json!({ "items": [] });
        assert!(parse_diagnostics_from_result(&result).is_empty());
    }

    #[test]
    fn test_parse_diagnostics_from_result_empty_direct_array() {
        let result = json!([]);
        assert!(parse_diagnostics_from_result(&result).is_empty());
    }

    // --- parse_single_diagnostic edge cases (exercised through parse_diagnostics_from_result) ---

    #[test]
    fn test_parse_diagnostics_skips_item_missing_range() {
        let result = json!({
            "items": [
                {
                    "severity": 1,
                    "message": "no range here"
                }
            ]
        });
        assert!(parse_diagnostics_from_result(&result).is_empty());
    }

    #[test]
    fn test_parse_diagnostics_skips_item_missing_message() {
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
    fn test_parse_diagnostics_skips_item_with_incomplete_range() {
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
    fn test_parse_diagnostics_mixed_valid_and_invalid_items() {
        let result = json!({
            "items": [
                {
                    "severity": 1,
                    "message": "missing range - should be skipped"
                },
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
                    // missing "message" - should be skipped
                }
            ]
        });
        let diagnostics = parse_diagnostics_from_result(&result);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, "valid warning");
    }

    #[test]
    fn test_parse_diagnostics_code_as_non_string_non_number_yields_none() {
        // code is a boolean -- not a string or number, so it should be None
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
    fn test_parse_diagnostics_source_as_non_string_yields_none() {
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

    // --- parse_publish_diagnostics: mixed valid and invalid ---

    #[test]
    fn test_parse_publish_diagnostics_mixed_valid_and_invalid() {
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

    // --- enrich_and_filter: severity filtering ---

    #[test]
    fn test_enrich_and_filter_applies_severity_filter() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, None);

        let raw = vec![
            Diagnostic {
                range: LspRange {
                    start_line: 0,
                    start_character: 0,
                    end_line: 0,
                    end_character: 5,
                },
                severity: DiagnosticSeverity::Error,
                message: "error".to_string(),
                code: None,
                source: None,
                containing_symbol: None,
            },
            Diagnostic {
                range: LspRange {
                    start_line: 1,
                    start_character: 0,
                    end_line: 1,
                    end_character: 5,
                },
                severity: DiagnosticSeverity::Warning,
                message: "warning".to_string(),
                code: None,
                source: None,
                containing_symbol: None,
            },
            Diagnostic {
                range: LspRange {
                    start_line: 2,
                    start_character: 0,
                    end_line: 2,
                    end_character: 5,
                },
                severity: DiagnosticSeverity::Info,
                message: "info".to_string(),
                code: None,
                source: None,
                containing_symbol: None,
            },
            Diagnostic {
                range: LspRange {
                    start_line: 3,
                    start_character: 0,
                    end_line: 3,
                    end_character: 5,
                },
                severity: DiagnosticSeverity::Hint,
                message: "hint".to_string(),
                code: None,
                source: None,
                containing_symbol: None,
            },
        ];

        let filtered =
            enrich_and_filter(&ctx, "src/test.rs", raw, Some(DiagnosticSeverity::Warning));
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].message, "error");
        assert_eq!(filtered[1].message, "warning");
    }

    // --- parse_diagnostics_from_result: direct array with multiple diagnostics ---

    #[test]
    fn test_parse_diagnostics_from_result_direct_array_multiple() {
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
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
        assert_eq!(diagnostics[0].message, "first error");
        assert_eq!(diagnostics[0].code, None);
        assert_eq!(diagnostics[1].severity, DiagnosticSeverity::Info);
        assert_eq!(diagnostics[1].code.as_deref(), Some("W001"));
        assert_eq!(diagnostics[1].source.as_deref(), Some("mypy"));
        assert_eq!(diagnostics[1].range.start_line, 10);
        assert_eq!(diagnostics[1].range.start_character, 4);
    }

    // --- error_count and warning_count ---

    #[test]
    fn test_counts_computed_correctly() {
        let diagnostics = [
            Diagnostic {
                range: LspRange {
                    start_line: 0,
                    start_character: 0,
                    end_line: 0,
                    end_character: 5,
                },
                severity: DiagnosticSeverity::Error,
                message: "err1".to_string(),
                code: None,
                source: None,
                containing_symbol: None,
            },
            Diagnostic {
                range: LspRange {
                    start_line: 1,
                    start_character: 0,
                    end_line: 1,
                    end_character: 5,
                },
                severity: DiagnosticSeverity::Error,
                message: "err2".to_string(),
                code: None,
                source: None,
                containing_symbol: None,
            },
            Diagnostic {
                range: LspRange {
                    start_line: 2,
                    start_character: 0,
                    end_line: 2,
                    end_character: 5,
                },
                severity: DiagnosticSeverity::Warning,
                message: "warn1".to_string(),
                code: None,
                source: None,
                containing_symbol: None,
            },
            Diagnostic {
                range: LspRange {
                    start_line: 3,
                    start_character: 0,
                    end_line: 3,
                    end_character: 5,
                },
                severity: DiagnosticSeverity::Info,
                message: "info1".to_string(),
                code: None,
                source: None,
                containing_symbol: None,
            },
        ];

        let error_count = diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Error)
            .count();
        let warning_count = diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
            .count();

        assert_eq!(error_count, 2);
        assert_eq!(warning_count, 1);
    }
}
