//! Display objects for doctor command output
//!
//! Provides clean display objects with `Tabled` and `Serialize` derives for consistent
//! output formatting across table, JSON, and YAML formats.

use super::types::{Check, CheckStatus};
use serde::{Deserialize, Serialize};
use tabled::Tabled;

/// Basic check information for standard doctor output
#[derive(Tabled, Serialize, Deserialize, Debug, Clone)]
pub struct CheckResult {
    #[tabled(rename = "Status")]
    pub status: String,

    #[tabled(rename = "Check")]
    pub name: String,

    #[tabled(rename = "Result")]
    pub message: String,
}

/// Detailed check information for verbose doctor output
#[derive(Tabled, Serialize, Deserialize, Debug, Clone)]
pub struct VerboseCheckResult {
    #[tabled(rename = "Status")]
    pub status: String,

    #[tabled(rename = "Check")]
    pub name: String,

    #[tabled(rename = "Result")]
    pub message: String,

    #[tabled(rename = "Fix")]
    pub fix: String,

    #[tabled(rename = "Category")]
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

/// Format check status as a symbol
fn format_check_status(status: &CheckStatus) -> String {
    match status {
        CheckStatus::Ok => "✓".to_string(),
        CheckStatus::Warning => "⚠".to_string(),
        CheckStatus::Error => "✗".to_string(),
    }
}

/// Categorize check based on its name
fn categorize_check(check: &Check) -> String {
    if check.name.contains("Installation")
        || check.name.contains("PATH")
        || check.name.contains("Permission")
        || check.name.contains("Binary")
    {
        "System".to_string()
    } else if check.name.contains("Claude")
        || check.name.contains("Config")
        || check.name.contains("MCP")
    {
        "Config".to_string()
    } else if check.name.contains("Prompt")
        || check.name.contains("YAML")
        || check.name.contains("Template")
    {
        "Prompt".to_string()
    } else if check.name.contains("Workflow") || check.name.contains("workflow") {
        "Workflow".to_string()
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
        assert_eq!(result.status, "✓");
        assert_eq!(result.name, "Test Check");
        assert_eq!(result.message, "Everything is working");
    }

    #[test]
    fn test_verbose_check_result_conversion() {
        let check = create_test_check();
        let result = VerboseCheckResult::from(&check);
        assert_eq!(result.status, "✓");
        assert_eq!(result.name, "Test Check");
        assert_eq!(result.message, "Everything is working");
        assert_eq!(result.fix, "No fix needed");
        assert_eq!(result.category, "Other");
    }

    #[test]
    fn test_format_check_status() {
        assert_eq!(format_check_status(&CheckStatus::Ok), "✓");
        assert_eq!(format_check_status(&CheckStatus::Warning), "⚠");
        assert_eq!(format_check_status(&CheckStatus::Error), "✗");
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
    }

    #[test]
    fn test_categorize_check_prompt() {
        let check = Check {
            name: "Prompt Directory".to_string(),
            status: CheckStatus::Ok,
            message: "Found prompts".to_string(),
            fix: None,
        };
        assert_eq!(categorize_check(&check), "Prompt");

        let check = Check {
            name: "YAML Parsing".to_string(),
            status: CheckStatus::Warning,
            message: "Some invalid YAML".to_string(),
            fix: Some("Fix YAML syntax".to_string()),
        };
        assert_eq!(categorize_check(&check), "Prompt");
    }

    #[test]
    fn test_categorize_check_workflow() {
        let check = Check {
            name: "Workflow Directory".to_string(),
            status: CheckStatus::Ok,
            message: "Found workflows".to_string(),
            fix: None,
        };
        assert_eq!(categorize_check(&check), "Workflow");

        let check = Check {
            name: "workflow permissions".to_string(),
            status: CheckStatus::Warning,
            message: "Limited permissions".to_string(),
            fix: Some("Update permissions".to_string()),
        };
        assert_eq!(categorize_check(&check), "Workflow");
    }

    #[test]
    fn test_categorize_check_other() {
        let check = Check {
            name: "Random Check".to_string(),
            status: CheckStatus::Ok,
            message: "Some message".to_string(),
            fix: None,
        };
        assert_eq!(categorize_check(&check), "Other");
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
            status: "✓".to_string(),
            name: "Test".to_string(),
            message: "Test message".to_string(),
        };

        let json = serde_json::to_string(&result).expect("Should serialize to JSON");
        assert!(json.contains("✓"));
        assert!(json.contains("Test"));
        assert!(json.contains("Test message"));

        let deserialized: CheckResult =
            serde_json::from_str(&json).expect("Should deserialize from JSON");
        assert_eq!(deserialized.status, "✓");
        assert_eq!(deserialized.name, "Test");
        assert_eq!(deserialized.message, "Test message");
    }

    #[test]
    fn test_serialization_verbose_check_result() {
        let result = VerboseCheckResult {
            status: "⚠".to_string(),
            name: "Test".to_string(),
            message: "Test message".to_string(),
            fix: "Test fix".to_string(),
            category: "Test Category".to_string(),
        };

        let json = serde_json::to_string(&result).expect("Should serialize to JSON");
        assert!(json.contains("⚠"));
        assert!(json.contains("Test"));
        assert!(json.contains("Test message"));
        assert!(json.contains("Test fix"));
        assert!(json.contains("Test Category"));

        let deserialized: VerboseCheckResult =
            serde_json::from_str(&json).expect("Should deserialize from JSON");
        assert_eq!(deserialized.status, "⚠");
        assert_eq!(deserialized.name, "Test");
        assert_eq!(deserialized.message, "Test message");
        assert_eq!(deserialized.fix, "Test fix");
        assert_eq!(deserialized.category, "Test Category");
    }
}
