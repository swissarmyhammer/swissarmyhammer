//! Diagnostics configuration.
//!
//! This is derived state, never persisted to disk. It carries the knobs the two
//! consumers (the `diagnostics` MCP tool and the inline-on-edit fold-in) share:
//! which severities to report, how long to let diagnostics settle before
//! reporting, a cap on the number of records per report, and a per-language
//! enable/disable override.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use swissarmyhammer_lsp::DiagnosticSeverity;

/// How long to wait after the last diagnostic change before reporting, so a
/// burst of edits/republishes settles into one report. Kept short for
/// responsiveness.
pub const DEFAULT_SETTLE_WINDOW: Duration = Duration::from_millis(300);

/// Hard backstop: how long to keep waiting for diagnostics to settle before
/// giving up and reporting `Pending`. Generous on purpose — a few seconds spent
/// settling in-tool beats forcing the model to take another turn — but bounded
/// so a pathologically never-quiescing server cannot block forever.
pub const DEFAULT_SETTLE_HARD_TIMEOUT: Duration = Duration::from_secs(5);

/// Default cap on the number of diagnostic records included in a single report.
pub const DEFAULT_PER_REPORT_CAP: usize = 100;

/// Configuration for producing diagnostics reports.
///
/// Defaults: report errors and warnings, a short settle window, a per-report
/// cap, and every detected language enabled. Nothing here is persisted — it is
/// rebuilt each run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticsConfig {
    /// Severities to include in a report. Defaults to `Error` + `Warning`.
    pub severities: Vec<DiagnosticSeverity>,
    /// How long to let diagnostics settle before reporting.
    pub settle_window: Duration,
    /// Hard backstop after which an un-settled stream reports `Pending` instead
    /// of blocking indefinitely.
    pub settle_hard_timeout: Duration,
    /// Maximum number of records per report.
    pub per_report_cap: usize,
    /// Per-language enable/disable overrides, keyed by LSP language id.
    ///
    /// A language absent from the map is enabled (the default is "all detected
    /// languages enabled"); an entry maps a language id to an explicit
    /// enabled flag. An empty map means every detected language is enabled.
    pub per_language_enabled: BTreeMap<String, bool>,
}

impl Default for DiagnosticsConfig {
    fn default() -> Self {
        DiagnosticsConfig {
            severities: vec![DiagnosticSeverity::Error, DiagnosticSeverity::Warning],
            settle_window: DEFAULT_SETTLE_WINDOW,
            settle_hard_timeout: DEFAULT_SETTLE_HARD_TIMEOUT,
            per_report_cap: DEFAULT_PER_REPORT_CAP,
            per_language_enabled: BTreeMap::new(),
        }
    }
}

impl DiagnosticsConfig {
    /// Whether `severity` should be included given this config.
    pub fn includes_severity(&self, severity: DiagnosticSeverity) -> bool {
        self.severities.contains(&severity)
    }

    /// Whether diagnostics are enabled for `language_id`.
    ///
    /// A language with no explicit override is enabled (defaults to "all
    /// detected languages").
    pub fn language_enabled(&self, language_id: &str) -> bool {
        self.per_language_enabled
            .get(language_id)
            .copied()
            .unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_reports_errors_and_warnings_only() {
        let config = DiagnosticsConfig::default();
        assert!(config.includes_severity(DiagnosticSeverity::Error));
        assert!(config.includes_severity(DiagnosticSeverity::Warning));
        assert!(!config.includes_severity(DiagnosticSeverity::Info));
        assert!(!config.includes_severity(DiagnosticSeverity::Hint));
    }

    #[test]
    fn default_settle_window_is_short() {
        let config = DiagnosticsConfig::default();
        assert_eq!(config.settle_window, DEFAULT_SETTLE_WINDOW);
        assert!(config.settle_window <= Duration::from_secs(1));
    }

    #[test]
    fn default_hard_timeout_is_a_generous_backstop() {
        let config = DiagnosticsConfig::default();
        assert_eq!(config.settle_hard_timeout, DEFAULT_SETTLE_HARD_TIMEOUT);
        // Generous backstop: comfortably longer than the (short) settle window,
        // but still bounded.
        assert!(config.settle_hard_timeout > config.settle_window);
        assert!(config.settle_hard_timeout >= Duration::from_secs(1));
    }

    #[test]
    fn default_is_capped() {
        let config = DiagnosticsConfig::default();
        assert_eq!(config.per_report_cap, DEFAULT_PER_REPORT_CAP);
        assert!(config.per_report_cap > 0);
    }

    #[test]
    fn default_enables_all_languages() {
        let config = DiagnosticsConfig::default();
        assert!(config.per_language_enabled.is_empty());
        assert!(config.language_enabled("rust"));
        assert!(config.language_enabled("python"));
    }

    #[test]
    fn explicit_language_override_disables() {
        let mut config = DiagnosticsConfig::default();
        config
            .per_language_enabled
            .insert("python".to_string(), false);
        assert!(!config.language_enabled("python"));
        assert!(config.language_enabled("rust"));
    }

    #[test]
    fn config_serde_round_trip() {
        let config = DiagnosticsConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let roundtrip: DiagnosticsConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip, config);
    }
}
