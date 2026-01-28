//! Display objects for doctor command output
//!
//! Provides clean display objects with `Serialize` derives for consistent
//! output formatting across table, JSON, and YAML formats.

use crate::types::{Check, CheckStatus};
use serde::{Deserialize, Serialize};

/// Basic check information for standard doctor output
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CheckResult {
    pub status: String,
    pub name: String,
    pub message: String,
}

/// Detailed check information for verbose doctor output
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VerboseCheckResult {
    pub status: String,
    pub name: String,
    pub message: String,
    pub fix: String,
    pub category: String,
}

impl From<&Check> for CheckResult {
    fn from(check: &Check) -> Self {
        Self {
            status: format_check_status(&check.status),
            name: check.name.clone(),
            message: check.message.clone(),
        }
    }
}

impl From<&Check> for VerboseCheckResult {
    fn from(check: &Check) -> Self {
        Self {
            status: format_check_status(&check.status),
            name: check.name.clone(),
            message: check.message.clone(),
            fix: check
                .fix
                .clone()
                .unwrap_or_else(|| "No fix available".to_string()),
            category: categorize_check(check),
        }
    }
}

/// Format check status as a symbol (without color - color is applied in table rendering)
pub fn format_check_status(status: &CheckStatus) -> String {
    match status {
        CheckStatus::Ok => "\u{2713}".to_string(),      // ✓
        CheckStatus::Warning => "\u{26A0}".to_string(), // ⚠
        CheckStatus::Error => "\u{2717}".to_string(),   // ✗
    }
}

/// Categorize check based on its name
///
/// This provides a default categorization scheme. Tools can override
/// this by implementing their own categorization logic.
pub fn categorize_check(check: &Check) -> String {
    if check.name.contains("Installation")
        || check.name.contains("PATH")
        || check.name.contains("Permission")
        || check.name.contains("Binary")
    {
        "System".to_string()
    } else if check.name.contains("Claude")
        || check.name.contains("Config")
        || check.name.contains("MCP")
        || check.name.contains("Hook")
    {
        "Config".to_string()
    } else if check.name.contains("Prompt")
        || check.name.contains("YAML")
        || check.name.contains("Template")
    {
        "Prompt".to_string()
    } else if check.name.contains("Workflow") || check.name.contains("workflow") {
        "Workflow".to_string()
    } else if check.name.contains("Validator") || check.name.contains("validator") {
        "Validator".to_string()
    } else if check.name.contains("Git") || check.name.contains("git") {
        "Git".to_string()
    } else {
        "Other".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_check() -> Check {
        Check {
            name: "Test Check".to_string(),
            status: CheckStatus::Ok,
            message: "Everything is working".to_string(),
            fix: Some("No fix needed".to_string()),
        }
    }

    fn create_warning_check() -> Check {
        Check {
            name: "Binary PATH Check".to_string(),
            status: CheckStatus::Warning,
            message: "Binary not found in PATH".to_string(),
            fix: Some("Add binary to PATH".to_string()),
        }
    }

    fn create_error_check() -> Check {
        Check {
            name: "Claude Config".to_string(),
            status: CheckStatus::Error,
            message: "Configuration file missing".to_string(),
            fix: None,
        }
    }

    #[test]
    fn test_check_result_conversion() {
        let check = create_test_check();
        let result = CheckResult::from(&check);
        assert_eq!(result.status, "\u{2713}");
        assert_eq!(result.name, "Test Check");
        assert_eq!(result.message, "Everything is working");
    }

    #[test]
    fn test_verbose_check_result_conversion() {
        let check = create_test_check();
        let result = VerboseCheckResult::from(&check);
        assert_eq!(result.status, "\u{2713}");
        assert_eq!(result.name, "Test Check");
        assert_eq!(result.message, "Everything is working");
        assert_eq!(result.fix, "No fix needed");
        assert_eq!(result.category, "Other");
    }

    #[test]
    fn test_format_check_status() {
        assert_eq!(format_check_status(&CheckStatus::Ok), "\u{2713}");
        assert_eq!(format_check_status(&CheckStatus::Warning), "\u{26A0}");
        assert_eq!(format_check_status(&CheckStatus::Error), "\u{2717}");
    }

    #[test]
    fn test_categorize_check_system() {
        let check = create_warning_check();
        assert_eq!(categorize_check(&check), "System");

        let check = Check {
            name: "Installation Check".to_string(),
            status: CheckStatus::Ok,
            message: "Installed".to_string(),
            fix: None,
        };
        assert_eq!(categorize_check(&check), "System");
    }

    #[test]
    fn test_categorize_check_config() {
        let check = create_error_check();
        assert_eq!(categorize_check(&check), "Config");

        let check = Check {
            name: "MCP Server Check".to_string(),
            status: CheckStatus::Ok,
            message: "Connected".to_string(),
            fix: None,
        };
        assert_eq!(categorize_check(&check), "Config");

        let check = Check {
            name: "Hook Setup".to_string(),
            status: CheckStatus::Ok,
            message: "Hooks installed".to_string(),
            fix: None,
        };
        assert_eq!(categorize_check(&check), "Config");
    }

    #[test]
    fn test_categorize_check_git() {
        let check = Check {
            name: "Git Repository".to_string(),
            status: CheckStatus::Ok,
            message: "Found".to_string(),
            fix: None,
        };
        assert_eq!(categorize_check(&check), "Git");
    }

    #[test]
    fn test_categorize_check_validator() {
        let check = Check {
            name: "Validator Loading".to_string(),
            status: CheckStatus::Ok,
            message: "Loaded".to_string(),
            fix: None,
        };
        assert_eq!(categorize_check(&check), "Validator");
    }

    #[test]
    fn test_verbose_check_result_no_fix() {
        let check = create_error_check();
        let result = VerboseCheckResult::from(&check);
        assert_eq!(result.fix, "No fix available");
    }

    #[test]
    fn test_serialization_check_result() {
        let result = CheckResult {
            status: "\u{2713}".to_string(),
            name: "Test".to_string(),
            message: "Test message".to_string(),
        };

        let json = serde_json::to_string(&result).expect("Should serialize to JSON");
        assert!(json.contains("Test"));
        assert!(json.contains("Test message"));

        let deserialized: CheckResult =
            serde_json::from_str(&json).expect("Should deserialize from JSON");
        assert_eq!(deserialized.name, "Test");
        assert_eq!(deserialized.message, "Test message");
    }
}
