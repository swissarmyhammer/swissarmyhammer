//! Error types for the rules system
//!
//! This module defines error types specific to rule checking and validation,
//! including `RuleError` for different failure modes and `RuleViolation` for
//! representing rule violations with fail-fast behavior.
//!
//! ## Logging Contract
//!
//! When a rule violation is detected, the rule checker logs it immediately
//! with appropriate formatting and severity level. The violation is then
//! converted to `SwissArmyHammerError::RuleViolation` which signals to
//! upper layers (CLI, commands) that they should NOT log it again to
//! avoid duplicate output in the user's terminal.

use crate::Severity;
use std::fmt;
use std::path::PathBuf;
use swissarmyhammer_common::SwissArmyHammerError;

/// Represents a violation of a rule during checking
///
/// This is a special error type used for fail-fast behavior when a rule
/// violation is detected during checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleViolation {
    /// Name of the rule that was violated
    pub rule_name: String,

    /// Path to the file where the violation occurred
    pub file_path: PathBuf,

    /// Severity level of the violation
    pub severity: Severity,

    /// Full LLM response message describing the violation
    pub message: String,
}

impl RuleViolation {
    /// Create a new rule violation
    pub fn new(rule_name: String, file_path: PathBuf, severity: Severity, message: String) -> Self {
        Self {
            rule_name,
            file_path,
            severity,
            message,
        }
    }

    /// Format the violation as a compact single-line summary
    ///
    /// This is used for error messages where a brief summary is needed.
    /// The full details are available via the Display trait.
    pub fn compact_format(&self) -> String {
        format!(
            "Rule '{}' violated in {} (severity: {})",
            self.rule_name,
            self.file_path.display(),
            self.severity
        )
    }
}

impl fmt::Display for RuleViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Violation\nRule: {}\nFile: {}\nSeverity: {}\nMessage: {}",
            self.rule_name,
            self.file_path.display(),
            self.severity,
            self.message
        )
    }
}

/// Error types for rule operations
///
/// Represents different failure modes in the rules system, from loading
/// and validation errors to runtime checking errors and rule violations.
#[derive(Debug)]
pub enum RuleError {
    /// Error loading a rule file
    LoadError(String),

    /// Rule validation failed
    ValidationError(String),

    /// Error occurred during rule checking
    CheckError(String),

    /// LLM agent execution failed
    AgentError(String),

    /// Language detection failed
    LanguageDetectionError(String),

    /// Glob pattern expansion failed
    GlobExpansionError(String),

    /// Cache operation failed
    CacheError(String),

    /// Rule violation found (for fail-fast behavior)
    Violation(RuleViolation),
}

impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuleError::LoadError(msg) => write!(f, "Failed to load rule: {}", msg),
            RuleError::ValidationError(msg) => write!(f, "Rule validation failed: {}", msg),
            RuleError::CheckError(msg) => write!(f, "Error during rule checking: {}", msg),
            RuleError::AgentError(msg) => write!(f, "LLM agent error: {}", msg),
            RuleError::LanguageDetectionError(msg) => {
                write!(f, "Language detection failed: {}", msg)
            }
            RuleError::GlobExpansionError(msg) => {
                write!(f, "Glob pattern expansion failed: {}", msg)
            }
            RuleError::CacheError(msg) => write!(f, "Cache operation failed: {}", msg),
            RuleError::Violation(violation) => write!(f, "{}", violation),
        }
    }
}

impl std::error::Error for RuleError {}

/// Conversion from RuleError to SwissArmyHammerError
impl From<RuleError> for SwissArmyHammerError {
    fn from(error: RuleError) -> Self {
        match error {
            RuleError::Violation(violation) => {
                SwissArmyHammerError::RuleViolation(violation.compact_format())
            }
            _ => SwissArmyHammerError::other(error.to_string()),
        }
    }
}

/// Conversion from SwissArmyHammerError to RuleError
impl From<SwissArmyHammerError> for RuleError {
    fn from(error: SwissArmyHammerError) -> Self {
        RuleError::LoadError(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_rule_violation_new() {
        let violation = RuleViolation::new(
            "no-hardcoded-secrets".to_string(),
            PathBuf::from("src/main.rs"),
            Severity::Error,
            "Found API key".to_string(),
        );

        assert_eq!(violation.rule_name, "no-hardcoded-secrets");
        assert_eq!(violation.file_path, Path::new("src/main.rs"));
        assert_eq!(violation.severity, Severity::Error);
        assert_eq!(violation.message, "Found API key");
    }

    #[test]
    fn test_rule_violation_display() {
        let violation = RuleViolation::new(
            "no-hardcoded-secrets".to_string(),
            PathBuf::from("src/main.rs"),
            Severity::Error,
            "Found API key at line 42".to_string(),
        );

        let display = violation.to_string();
        assert!(display.contains("no-hardcoded-secrets"));
        assert!(display.contains("src/main.rs"));
        assert!(display.contains("error"));
        assert!(display.contains("Found API key at line 42"));
    }

    #[test]
    fn test_rule_violation_clone() {
        let violation = RuleViolation::new(
            "test-rule".to_string(),
            PathBuf::from("test.rs"),
            Severity::Warning,
            "Test message".to_string(),
        );

        let cloned = violation.clone();
        assert_eq!(violation, cloned);
    }

    #[test]
    fn test_rule_error_load_error() {
        let error = RuleError::LoadError("File not found".to_string());
        let display = error.to_string();
        assert!(display.contains("Failed to load rule"));
        assert!(display.contains("File not found"));
    }

    #[test]
    fn test_rule_error_validation_error() {
        let error = RuleError::ValidationError("Missing required field".to_string());
        let display = error.to_string();
        assert!(display.contains("Rule validation failed"));
        assert!(display.contains("Missing required field"));
    }

    #[test]
    fn test_rule_error_check_error() {
        let error = RuleError::CheckError("Failed to read file".to_string());
        let display = error.to_string();
        assert!(display.contains("Error during rule checking"));
        assert!(display.contains("Failed to read file"));
    }

    #[test]
    fn test_rule_error_agent_error() {
        let error = RuleError::AgentError("API timeout".to_string());
        let display = error.to_string();
        assert!(display.contains("LLM agent error"));
        assert!(display.contains("API timeout"));
    }

    #[test]
    fn test_rule_error_language_detection_error() {
        let error = RuleError::LanguageDetectionError("Unknown extension".to_string());
        let display = error.to_string();
        assert!(display.contains("Language detection failed"));
        assert!(display.contains("Unknown extension"));
    }

    #[test]
    fn test_rule_error_glob_expansion_error() {
        let error = RuleError::GlobExpansionError("Invalid pattern".to_string());
        let display = error.to_string();
        assert!(display.contains("Glob pattern expansion failed"));
        assert!(display.contains("Invalid pattern"));
    }

    #[test]
    fn test_rule_error_violation() {
        let violation = RuleViolation::new(
            "test-rule".to_string(),
            PathBuf::from("test.rs"),
            Severity::Error,
            "Test violation".to_string(),
        );
        let error = RuleError::Violation(violation.clone());
        let display = error.to_string();
        assert!(display.contains("test-rule"));
        assert!(display.contains("test.rs"));
        assert!(display.contains("Test violation"));
    }

    #[test]
    fn test_rule_error_implements_error_trait() {
        let error = RuleError::LoadError("test".to_string());
        let _: &dyn std::error::Error = &error;
    }

    #[test]
    fn test_conversion_to_swiss_army_hammer_error() {
        let rule_error = RuleError::ValidationError("test error".to_string());
        let sah_error: SwissArmyHammerError = rule_error.into();
        assert!(sah_error.to_string().contains("test error"));
    }

    #[test]
    fn test_conversion_from_swiss_army_hammer_error() {
        let sah_error = SwissArmyHammerError::other("test error".to_string());
        let rule_error: RuleError = sah_error.into();
        match rule_error {
            RuleError::LoadError(msg) => assert!(msg.contains("test error")),
            _ => panic!("Expected LoadError variant"),
        }
    }

    #[test]
    fn test_rule_violation_equality() {
        let v1 = RuleViolation::new(
            "rule1".to_string(),
            PathBuf::from("file.rs"),
            Severity::Error,
            "message".to_string(),
        );
        let v2 = RuleViolation::new(
            "rule1".to_string(),
            PathBuf::from("file.rs"),
            Severity::Error,
            "message".to_string(),
        );
        let v3 = RuleViolation::new(
            "rule2".to_string(),
            PathBuf::from("file.rs"),
            Severity::Error,
            "message".to_string(),
        );

        assert_eq!(v1, v2);
        assert_ne!(v1, v3);
    }

    #[test]
    fn test_rule_violation_with_different_severities() {
        let error = RuleViolation::new(
            "rule".to_string(),
            PathBuf::from("file.rs"),
            Severity::Error,
            "msg".to_string(),
        );
        let warning = RuleViolation::new(
            "rule".to_string(),
            PathBuf::from("file.rs"),
            Severity::Warning,
            "msg".to_string(),
        );
        let info = RuleViolation::new(
            "rule".to_string(),
            PathBuf::from("file.rs"),
            Severity::Info,
            "msg".to_string(),
        );
        let hint = RuleViolation::new(
            "rule".to_string(),
            PathBuf::from("file.rs"),
            Severity::Hint,
            "msg".to_string(),
        );

        assert_eq!(error.severity, Severity::Error);
        assert_eq!(warning.severity, Severity::Warning);
        assert_eq!(info.severity, Severity::Info);
        assert_eq!(hint.severity, Severity::Hint);
    }

    #[test]
    fn test_rule_violation_converts_to_swiss_army_hammer_rule_violation() {
        let violation = RuleViolation::new(
            "test-rule".to_string(),
            PathBuf::from("test.rs"),
            Severity::Error,
            "Test violation message".to_string(),
        );
        let rule_error = RuleError::Violation(violation);
        let sah_error: SwissArmyHammerError = rule_error.into();

        match sah_error {
            SwissArmyHammerError::RuleViolation(msg) => {
                // Now uses compact format (single line summary)
                assert!(msg.contains("test-rule"));
                assert!(msg.contains("test.rs"));
                assert!(msg.contains("error"));
                assert!(!msg.contains('\n'), "Should use compact single-line format");
            }
            _ => panic!("Expected RuleViolation variant, got {:?}", sah_error),
        }
    }

    #[test]
    fn test_non_violation_rule_errors_convert_to_other() {
        let load_error = RuleError::LoadError("load failed".to_string());
        let sah_error: SwissArmyHammerError = load_error.into();
        match sah_error {
            SwissArmyHammerError::Other { message } => {
                assert!(message.contains("load failed"));
            }
            _ => panic!("Expected Other variant for non-violation errors"),
        }

        let check_error = RuleError::CheckError("check failed".to_string());
        let sah_error: SwissArmyHammerError = check_error.into();
        match sah_error {
            SwissArmyHammerError::Other { message } => {
                assert!(message.contains("check failed"));
            }
            _ => panic!("Expected Other variant for non-violation errors"),
        }
    }

    #[test]
    fn test_rule_violation_compact_format() {
        let violation = RuleViolation::new(
            "no-mocks".to_string(),
            PathBuf::from("swissarmyhammer-rules/tests/partials_test.rs"),
            Severity::Error,
            "Mock object detected - MockPartialLoader simulates real PartialLoader behavior"
                .to_string(),
        );

        let compact = violation.compact_format();
        assert!(compact.contains("no-mocks"));
        assert!(compact.contains("partials_test.rs"));
        assert!(compact.contains("error"));
        assert!(
            !compact.contains('\n'),
            "Compact format should be single line"
        );
    }

    #[test]
    fn test_rule_violation_compact_format_vs_display() {
        let violation = RuleViolation::new(
            "test-rule".to_string(),
            PathBuf::from("test.rs"),
            Severity::Warning,
            "Detailed violation message".to_string(),
        );

        let compact = violation.compact_format();
        let display = violation.to_string();

        // Compact format should be one line
        assert!(!compact.contains('\n'));

        // Display format is multi-line
        assert!(display.contains('\n'));

        // Both should contain the rule name
        assert!(compact.contains("test-rule"));
        assert!(display.contains("test-rule"));
    }

    #[test]
    fn test_rule_violation_conversion_uses_compact_format() {
        let violation = RuleViolation::new(
            "no-hardcoded-secrets".to_string(),
            PathBuf::from("src/config.rs"),
            Severity::Error,
            "Found hardcoded API key on line 42".to_string(),
        );

        let rule_error = RuleError::Violation(violation);
        let sah_error: SwissArmyHammerError = rule_error.into();

        match sah_error {
            SwissArmyHammerError::RuleViolation(msg) => {
                // Should use compact format (single line)
                assert!(!msg.contains('\n'), "Error message should be single line");
                assert!(msg.contains("no-hardcoded-secrets"));
                assert!(msg.contains("src/config.rs"));
                assert!(msg.contains("error"));
                // Should NOT contain the detailed message in the top-level error
                assert!(!msg.contains("Found hardcoded API key"));
            }
            _ => panic!("Expected RuleViolation variant"),
        }
    }
}
