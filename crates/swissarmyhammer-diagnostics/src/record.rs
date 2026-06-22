//! The model-free diagnostics report types and the pure `lsp_types` mapping.
//!
//! These types are derived state: they are produced from live LSP diagnostics
//! and serialized for transport, but never persisted to disk. The mapping
//! [`map`] is pure — it turns a single [`lsp_types::Diagnostic`] plus the file
//! path into a [`DiagnosticRecord`] with no I/O and no enrichment.

use serde::{Deserialize, Serialize};
use swissarmyhammer_lsp::DiagnosticSeverity;

/// A half-open text range within a file, in zero-based LSP line/character
/// coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    /// Zero-based start line.
    pub start_line: u32,
    /// Zero-based start character (UTF-16 code unit offset).
    pub start_character: u32,
    /// Zero-based end line.
    pub end_line: u32,
    /// Zero-based end character (UTF-16 code unit offset).
    pub end_character: u32,
}

impl From<lsp_types::Range> for Range {
    fn from(r: lsp_types::Range) -> Self {
        Range {
            start_line: r.start.line,
            start_character: r.start.character,
            end_line: r.end.line,
            end_character: r.end.character,
        }
    }
}

/// A single diagnostic for a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticRecord {
    /// The file the diagnostic applies to (as supplied to [`map`]).
    pub path: String,
    /// The range in the file where the diagnostic applies.
    pub range: Range,
    /// Severity of the diagnostic.
    pub severity: DiagnosticSeverity,
    /// Human-readable message.
    pub message: String,
    /// Optional diagnostic code (e.g. `"E0308"`).
    pub code: Option<String>,
    /// Optional source tool (e.g. `"rustc"`, `"clippy"`).
    pub source: Option<String>,
    /// Name of the enclosing symbol. Populated by an enriching consumer; the
    /// pure [`map`] always leaves this `None`.
    pub containing_symbol: Option<String>,
}

/// Counts of the most actionable diagnostic severities in a report.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    /// Number of `Error`-severity diagnostics.
    pub errors: usize,
    /// Number of `Warning`-severity diagnostics.
    pub warnings: usize,
}

/// A complete diagnostics report for a set of diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticsReport {
    /// The diagnostics in the report.
    pub diagnostics: Vec<DiagnosticRecord>,
    /// Error/warning counts over `diagnostics`.
    pub counts: Counts,
}

impl Counts {
    /// Compute error/warning counts over a slice of records.
    pub fn from_records(records: &[DiagnosticRecord]) -> Self {
        let mut counts = Counts::default();
        for record in records {
            match record.severity {
                DiagnosticSeverity::Error => counts.errors += 1,
                DiagnosticSeverity::Warning => counts.warnings += 1,
                _ => {}
            }
        }
        counts
    }
}

impl DiagnosticsReport {
    /// Build a report from records, computing the counts.
    pub fn new(diagnostics: Vec<DiagnosticRecord>) -> Self {
        let counts = Counts::from_records(&diagnostics);
        DiagnosticsReport {
            diagnostics,
            counts,
        }
    }
}

/// Map a single [`lsp_types::Diagnostic`] and its file path into a
/// [`DiagnosticRecord`].
///
/// Pure: no I/O, no enrichment. A missing severity defaults to
/// [`DiagnosticSeverity::Hint`]; a numeric `code` is rendered to its decimal
/// string, and `containing_symbol` is always left `None` for a caller to fill
/// in.
pub fn map(diagnostic: &lsp_types::Diagnostic, path: impl Into<String>) -> DiagnosticRecord {
    let code = diagnostic.code.as_ref().map(|c| match c {
        lsp_types::NumberOrString::Number(n) => n.to_string(),
        lsp_types::NumberOrString::String(s) => s.clone(),
    });

    let severity = diagnostic
        .severity
        .map(DiagnosticSeverity::from_lsp_types)
        .unwrap_or(DiagnosticSeverity::Hint);

    DiagnosticRecord {
        path: path.into(),
        range: diagnostic.range.into(),
        severity,
        message: diagnostic.message.clone(),
        code,
        source: diagnostic.source.clone(),
        containing_symbol: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{NumberOrString, Position};

    fn lsp_diag(
        severity: Option<lsp_types::DiagnosticSeverity>,
        message: &str,
        code: Option<NumberOrString>,
        source: Option<&str>,
    ) -> lsp_types::Diagnostic {
        lsp_types::Diagnostic {
            range: lsp_types::Range {
                start: Position {
                    line: 5,
                    character: 10,
                },
                end: Position {
                    line: 5,
                    character: 20,
                },
            },
            severity,
            code,
            source: source.map(String::from),
            message: message.to_string(),
            ..lsp_types::Diagnostic::default()
        }
    }

    #[test]
    fn map_carries_path_range_severity_message_code_source() {
        let d = lsp_diag(
            Some(lsp_types::DiagnosticSeverity::ERROR),
            "mismatched types",
            Some(NumberOrString::String("E0308".to_string())),
            Some("rustc"),
        );
        let record = map(&d, "src/main.rs");
        assert_eq!(record.path, "src/main.rs");
        assert_eq!(record.severity, DiagnosticSeverity::Error);
        assert_eq!(record.message, "mismatched types");
        assert_eq!(record.code.as_deref(), Some("E0308"));
        assert_eq!(record.source.as_deref(), Some("rustc"));
        assert_eq!(
            record.range,
            Range {
                start_line: 5,
                start_character: 10,
                end_line: 5,
                end_character: 20,
            }
        );
        assert_eq!(record.containing_symbol, None);
    }

    #[test]
    fn map_warning_severity() {
        let d = lsp_diag(
            Some(lsp_types::DiagnosticSeverity::WARNING),
            "unused variable",
            None,
            Some("clippy"),
        );
        assert_eq!(map(&d, "lib.rs").severity, DiagnosticSeverity::Warning);
    }

    #[test]
    fn map_missing_severity_defaults_to_hint() {
        let d = lsp_diag(None, "no severity", None, None);
        assert_eq!(map(&d, "lib.rs").severity, DiagnosticSeverity::Hint);
    }

    #[test]
    fn map_numeric_code_renders_to_string() {
        let d = lsp_diag(
            Some(lsp_types::DiagnosticSeverity::ERROR),
            "numeric code",
            Some(NumberOrString::Number(42)),
            None,
        );
        assert_eq!(map(&d, "lib.rs").code.as_deref(), Some("42"));
    }

    #[test]
    fn map_no_code_no_source_yields_none() {
        let d = lsp_diag(
            Some(lsp_types::DiagnosticSeverity::INFORMATION),
            "bare",
            None,
            None,
        );
        let record = map(&d, "lib.rs");
        assert_eq!(record.code, None);
        assert_eq!(record.source, None);
        assert_eq!(record.severity, DiagnosticSeverity::Info);
    }

    #[test]
    fn counts_tally_errors_and_warnings_only() {
        let records = vec![
            map(
                &lsp_diag(Some(lsp_types::DiagnosticSeverity::ERROR), "e1", None, None),
                "f",
            ),
            map(
                &lsp_diag(Some(lsp_types::DiagnosticSeverity::ERROR), "e2", None, None),
                "f",
            ),
            map(
                &lsp_diag(
                    Some(lsp_types::DiagnosticSeverity::WARNING),
                    "w1",
                    None,
                    None,
                ),
                "f",
            ),
            map(
                &lsp_diag(
                    Some(lsp_types::DiagnosticSeverity::INFORMATION),
                    "i1",
                    None,
                    None,
                ),
                "f",
            ),
            map(
                &lsp_diag(Some(lsp_types::DiagnosticSeverity::HINT), "h1", None, None),
                "f",
            ),
        ];
        let counts = Counts::from_records(&records);
        assert_eq!(counts.errors, 2);
        assert_eq!(counts.warnings, 1);
    }

    #[test]
    fn report_new_computes_counts() {
        let records = vec![
            map(
                &lsp_diag(Some(lsp_types::DiagnosticSeverity::ERROR), "e", None, None),
                "f",
            ),
            map(
                &lsp_diag(
                    Some(lsp_types::DiagnosticSeverity::WARNING),
                    "w",
                    None,
                    None,
                ),
                "f",
            ),
        ];
        let report = DiagnosticsReport::new(records);
        assert_eq!(report.counts.errors, 1);
        assert_eq!(report.counts.warnings, 1);
        assert_eq!(report.diagnostics.len(), 2);
    }

    #[test]
    fn report_serde_round_trip() {
        let report = DiagnosticsReport::new(vec![DiagnosticRecord {
            path: "src/main.rs".to_string(),
            range: Range {
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
        }]);

        let json = serde_json::to_string(&report).unwrap();
        let roundtrip: DiagnosticsReport = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip, report);
    }
}
