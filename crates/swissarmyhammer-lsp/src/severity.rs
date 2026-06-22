//! The one canonical diagnostic severity enum.
//!
//! Both `swissarmyhammer-code-context` (which renders diagnostics) and
//! `swissarmyhammer-diagnostics` (the model-free diagnostics core) need to name
//! diagnostic severity. Defining it here — in the LSP crate both already depend
//! on — keeps it as a single canonical type rather than two competing enums,
//! while staying free of any dependency cycle (neither downstream crate has to
//! depend on the other).

use serde::{Deserialize, Serialize};

/// Severity level for a diagnostic.
///
/// Mirrors the LSP severity scale (1=Error, 2=Warning, 3=Info, 4=Hint). This is
/// the single canonical severity type for the workspace; downstream crates
/// re-export it rather than defining their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    /// An error that prevents the program from building/running correctly.
    Error,
    /// A warning about a potential problem.
    Warning,
    /// Informational diagnostic.
    Info,
    /// A hint, the lowest severity.
    Hint,
}

impl DiagnosticSeverity {
    /// Convert an LSP severity integer (1=Error, 2=Warning, 3=Info, 4=Hint)
    /// to this enum. Defaults to `Hint` for unknown values.
    pub fn from_lsp(value: u64) -> Self {
        match value {
            1 => Self::Error,
            2 => Self::Warning,
            3 => Self::Info,
            4 => Self::Hint,
            _ => Self::Hint,
        }
    }

    /// Convert to the LSP severity integer (1=Error, 2=Warning, 3=Info, 4=Hint).
    pub fn to_lsp(self) -> u64 {
        match self {
            Self::Error => 1,
            Self::Warning => 2,
            Self::Info => 3,
            Self::Hint => 4,
        }
    }

    /// Map a typed [`lsp_types::DiagnosticSeverity`] onto this enum, defaulting
    /// any unrecognized value to `Hint`.
    pub fn from_lsp_types(severity: lsp_types::DiagnosticSeverity) -> Self {
        match severity {
            lsp_types::DiagnosticSeverity::ERROR => Self::Error,
            lsp_types::DiagnosticSeverity::WARNING => Self::Warning,
            lsp_types::DiagnosticSeverity::INFORMATION => Self::Info,
            lsp_types::DiagnosticSeverity::HINT => Self::Hint,
            _ => Self::Hint,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_lsp_maps_known_values_and_roundtrips() {
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
    fn from_lsp_unknown_defaults_to_hint() {
        assert_eq!(DiagnosticSeverity::from_lsp(99), DiagnosticSeverity::Hint);
    }

    #[test]
    fn from_lsp_types_maps_each_severity() {
        assert_eq!(
            DiagnosticSeverity::from_lsp_types(lsp_types::DiagnosticSeverity::ERROR),
            DiagnosticSeverity::Error
        );
        assert_eq!(
            DiagnosticSeverity::from_lsp_types(lsp_types::DiagnosticSeverity::WARNING),
            DiagnosticSeverity::Warning
        );
        assert_eq!(
            DiagnosticSeverity::from_lsp_types(lsp_types::DiagnosticSeverity::INFORMATION),
            DiagnosticSeverity::Info
        );
        assert_eq!(
            DiagnosticSeverity::from_lsp_types(lsp_types::DiagnosticSeverity::HINT),
            DiagnosticSeverity::Hint
        );
    }
}
