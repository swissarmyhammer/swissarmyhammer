//! Display objects for validate command output
//!
//! Provides clean display objects with `Tabled` and `Serialize` derives for consistent
//! output formatting across table, JSON, and YAML formats.

use serde::{Deserialize, Serialize};
use swissarmyhammer::validation::{ValidationIssue, ValidationLevel};
use tabled::Tabled;

/// Basic validation result for standard output
#[derive(Tabled, Serialize, Deserialize, Debug, Clone)]
pub struct ValidationResult {
    #[tabled(rename = "Status")]
    pub status: String,

    #[tabled(rename = "File")]
    pub file: String,

    #[tabled(rename = "Result")]
    pub result: String,
}

/// Detailed validation result for verbose output
#[derive(Tabled, Serialize, Deserialize, Debug, Clone)]
pub struct VerboseValidationResult {
    #[tabled(rename = "Status")]
    pub status: String,

    #[tabled(rename = "File")]
    pub file: String,

    #[tabled(rename = "Result")]
    pub result: String,

    #[tabled(rename = "Fix")]
    pub fix: String,

    #[tabled(rename = "Type")]
    pub file_type: String,
}

impl From<&ValidationIssue> for ValidationResult {
    fn from(issue: &ValidationIssue) -> Self {
        Self {
            status: format_validation_status(&issue.level),
            file: format_file_display(&issue.file_path, &issue.content_title),
            result: issue.message.clone(),
        }
    }
}

impl From<&ValidationIssue> for VerboseValidationResult {
    fn from(issue: &ValidationIssue) -> Self {
        Self {
            status: format_validation_status(&issue.level),
            file: format_file_display(&issue.file_path, &issue.content_title),
            result: issue.message.clone(),
            fix: issue
                .suggestion
                .clone()
                .unwrap_or_else(|| "No fix available".to_string()),
            file_type: determine_file_type(&issue.file_path, &issue.content_title),
        }
    }
}

/// Format validation status as a symbol
fn format_validation_status(level: &ValidationLevel) -> String {
    match level {
        ValidationLevel::Info => "✅".to_string(),
        ValidationLevel::Warning => "⚠️".to_string(),
        ValidationLevel::Error => "❌".to_string(),
    }
}

/// Format file display name, preferring content title over file path
fn format_file_display(file_path: &std::path::Path, content_title: &Option<String>) -> String {
    if let Some(title) = content_title {
        if !title.is_empty() {
            return title.clone();
        }
    }

    // Use file name or path as fallback
    if let Some(file_name) = file_path.file_name() {
        file_name.to_string_lossy().to_string()
    } else {
        file_path.display().to_string()
    }
}

/// Determine file type based on path and content
fn determine_file_type(file_path: &std::path::Path, content_title: &Option<String>) -> String {
    let path_str = file_path.to_string_lossy();

    if path_str.starts_with("workflow:") {
        "Workflow".to_string()
    } else if path_str.ends_with(".md") || path_str.ends_with(".liquid") {
        "Prompt".to_string()
    } else if path_str.ends_with(".toml") {
        "Config".to_string()
    } else if path_str.contains("MCP Tools")
        || content_title.as_ref().is_some_and(|t| t.contains("Tool"))
    {
        "Tool".to_string()
    } else {
        "Other".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_issue() -> ValidationIssue {
        ValidationIssue {
            level: ValidationLevel::Warning,
            file_path: PathBuf::from("test.md"),
            content_title: Some("Test Prompt".to_string()),
            line: Some(10),
            column: Some(5),
            message: "Test warning message".to_string(),
            suggestion: Some("Test fix suggestion".to_string()),
        }
    }

    #[test]
    fn test_validation_result_conversion() {
        let issue = create_test_issue();
        let result = ValidationResult::from(&issue);

        assert_eq!(result.status, "⚠️");
        assert_eq!(result.file, "Test Prompt");
        assert_eq!(result.result, "Test warning message");
    }

    #[test]
    fn test_verbose_validation_result_conversion() {
        let issue = create_test_issue();
        let result = VerboseValidationResult::from(&issue);

        assert_eq!(result.status, "⚠️");
        assert_eq!(result.file, "Test Prompt");
        assert_eq!(result.result, "Test warning message");
        assert_eq!(result.fix, "Test fix suggestion");
        assert_eq!(result.file_type, "Prompt");
    }

    #[test]
    fn test_format_validation_status() {
        assert_eq!(format_validation_status(&ValidationLevel::Info), "✅");
        assert_eq!(format_validation_status(&ValidationLevel::Warning), "⚠️");
        assert_eq!(format_validation_status(&ValidationLevel::Error), "❌");
    }

    #[test]
    fn test_format_file_display_with_title() {
        let path = PathBuf::from("some/path/file.md");
        let title = Some("My Prompt".to_string());
        assert_eq!(format_file_display(&path, &title), "My Prompt");
    }

    #[test]
    fn test_format_file_display_without_title() {
        let path = PathBuf::from("some/path/file.md");
        let title = None;
        assert_eq!(format_file_display(&path, &title), "file.md");
    }

    #[test]
    fn test_format_file_display_empty_title() {
        let path = PathBuf::from("some/path/file.md");
        let title = Some("".to_string());
        assert_eq!(format_file_display(&path, &title), "file.md");
    }

    #[test]
    fn test_determine_file_type_workflow() {
        let path = PathBuf::from("workflow:builtin:test-workflow");
        let title = None;
        assert_eq!(determine_file_type(&path, &title), "Workflow");
    }

    #[test]
    fn test_determine_file_type_prompt() {
        let path = PathBuf::from("prompt.md");
        let title = None;
        assert_eq!(determine_file_type(&path, &title), "Prompt");

        let path = PathBuf::from("template.liquid");
        let title = None;
        assert_eq!(determine_file_type(&path, &title), "Prompt");
    }

    #[test]
    fn test_determine_file_type_config() {
        let path = PathBuf::from("sah.toml");
        let title = None;
        assert_eq!(determine_file_type(&path, &title), "Config");
    }

    #[test]
    fn test_determine_file_type_tool() {
        let path = PathBuf::from("MCP Tools");
        let title = None;
        assert_eq!(determine_file_type(&path, &title), "Tool");

        let path = PathBuf::from("some/path");
        let title = Some("Tool Validation".to_string());
        assert_eq!(determine_file_type(&path, &title), "Tool");
    }

    #[test]
    fn test_determine_file_type_other() {
        let path = PathBuf::from("some/unknown/file.txt");
        let title = None;
        assert_eq!(determine_file_type(&path, &title), "Other");
    }

    #[test]
    fn test_verbose_result_no_fix() {
        let mut issue = create_test_issue();
        issue.suggestion = None;
        let result = VerboseValidationResult::from(&issue);
        assert_eq!(result.fix, "No fix available");
    }

    #[test]
    fn test_serialization() {
        let result = ValidationResult {
            status: "✅".to_string(),
            file: "test.md".to_string(),
            result: "All good".to_string(),
        };

        let json = serde_json::to_string(&result).expect("Should serialize to JSON");
        assert!(json.contains("✅"));
        assert!(json.contains("test.md"));
        assert!(json.contains("All good"));

        let deserialized: ValidationResult =
            serde_json::from_str(&json).expect("Should deserialize from JSON");
        assert_eq!(deserialized.status, "✅");
        assert_eq!(deserialized.file, "test.md");
        assert_eq!(deserialized.result, "All good");
    }
}
