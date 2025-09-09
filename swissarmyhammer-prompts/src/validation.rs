//! Validation types and traits for prompt validation
//!
//! This module provides the validation framework used to check prompts
//! for correctness, completeness, and best practices.

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

/// Validation severity level
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationLevel {
    /// Error - must be fixed before prompt can be used
    Error,
    /// Warning - should be addressed but doesn't prevent usage
    Warning,
    /// Info - informational message about potential improvements
    Info,
}

impl ValidationLevel {
    /// Get the string representation of the validation level
    pub fn as_str(&self) -> &'static str {
        match self {
            ValidationLevel::Error => "error",
            ValidationLevel::Warning => "warning", 
            ValidationLevel::Info => "info",
        }
    }

    /// Check if this is an error level
    pub fn is_error(&self) -> bool {
        matches!(self, ValidationLevel::Error)
    }

    /// Check if this is a warning level
    pub fn is_warning(&self) -> bool {
        matches!(self, ValidationLevel::Warning)
    }

    /// Check if this is an info level
    pub fn is_info(&self) -> bool {
        matches!(self, ValidationLevel::Info)
    }
}

/// Represents a validation issue found during prompt validation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationIssue {
    /// Severity level of the issue
    pub level: ValidationLevel,
    /// Path to the file where the issue was found
    pub file_path: PathBuf,
    /// Title or name of the content being validated
    pub content_title: Option<String>,
    /// Line number where the issue occurs (if applicable)
    pub line: Option<usize>,
    /// Column number where the issue occurs (if applicable)  
    pub column: Option<usize>,
    /// Description of the validation issue
    pub message: String,
    /// Optional suggestion for fixing the issue
    pub suggestion: Option<String>,
}

impl ValidationIssue {
    /// Create a new validation issue
    pub fn new(
        level: ValidationLevel,
        file_path: PathBuf,
        message: String,
    ) -> Self {
        Self {
            level,
            file_path,
            content_title: None,
            line: None,
            column: None,
            message,
            suggestion: None,
        }
    }

    /// Create a new error validation issue
    pub fn error(file_path: PathBuf, message: String) -> Self {
        Self::new(ValidationLevel::Error, file_path, message)
    }

    /// Create a new warning validation issue
    pub fn warning(file_path: PathBuf, message: String) -> Self {
        Self::new(ValidationLevel::Warning, file_path, message)
    }

    /// Create a new info validation issue
    pub fn info(file_path: PathBuf, message: String) -> Self {
        Self::new(ValidationLevel::Info, file_path, message)
    }

    /// Set the content title
    pub fn with_content_title(mut self, title: String) -> Self {
        self.content_title = Some(title);
        self
    }

    /// Set the line number
    pub fn with_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    /// Set the column number
    pub fn with_column(mut self, column: usize) -> Self {
        self.column = Some(column);
        self
    }

    /// Set a suggestion for fixing the issue
    pub fn with_suggestion(mut self, suggestion: String) -> Self {
        self.suggestion = Some(suggestion);
        self
    }

    /// Format the validation issue as a human-readable string
    pub fn format(&self) -> String {
        let level = self.level.as_str().to_uppercase();
        let location = if let (Some(line), Some(column)) = (self.line, self.column) {
            format!("{}:{}:{}", self.file_path.display(), line, column)
        } else if let Some(line) = self.line {
            format!("{}:{}", self.file_path.display(), line)
        } else {
            self.file_path.display().to_string()
        };

        let mut result = format!("{}: {} - {}", level, location, self.message);
        
        if let Some(title) = &self.content_title {
            result = format!("{} (in {})", result, title);
        }

        if let Some(suggestion) = &self.suggestion {
            result = format!("{}\n  Suggestion: {}", result, suggestion);
        }

        result
    }
}

/// Trait for types that can be validated
pub trait Validatable {
    /// Validate the object and return any issues found
    fn validate(&self, source_path: Option<&Path>) -> Vec<ValidationIssue>;

    /// Check if the object is valid (has no error-level issues)
    fn is_valid(&self, source_path: Option<&Path>) -> bool {
        self.validate(source_path)
            .iter()
            .all(|issue| !issue.level.is_error())
    }

    /// Get only error-level validation issues
    fn get_errors(&self, source_path: Option<&Path>) -> Vec<ValidationIssue> {
        self.validate(source_path)
            .into_iter()
            .filter(|issue| issue.level.is_error())
            .collect()
    }

    /// Get only warning-level validation issues
    fn get_warnings(&self, source_path: Option<&Path>) -> Vec<ValidationIssue> {
        self.validate(source_path)
            .into_iter()
            .filter(|issue| issue.level.is_warning())
            .collect()
    }
}

/// Validation result that can contain multiple issues
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// All validation issues found
    pub issues: Vec<ValidationIssue>,
}

impl ValidationResult {
    /// Create a new validation result
    pub fn new(issues: Vec<ValidationIssue>) -> Self {
        Self { issues }
    }

    /// Create an empty validation result (no issues)
    pub fn ok() -> Self {
        Self { issues: Vec::new() }
    }

    /// Check if the validation was successful (no errors)
    pub fn is_ok(&self) -> bool {
        self.issues.iter().all(|issue| !issue.level.is_error())
    }

    /// Check if there are any errors
    pub fn has_errors(&self) -> bool {
        self.issues.iter().any(|issue| issue.level.is_error())
    }

    /// Check if there are any warnings
    pub fn has_warnings(&self) -> bool {
        self.issues.iter().any(|issue| issue.level.is_warning())
    }

    /// Get all error issues
    pub fn errors(&self) -> Vec<&ValidationIssue> {
        self.issues.iter().filter(|issue| issue.level.is_error()).collect()
    }

    /// Get all warning issues
    pub fn warnings(&self) -> Vec<&ValidationIssue> {
        self.issues.iter().filter(|issue| issue.level.is_warning()).collect()
    }

    /// Get total number of issues
    pub fn issue_count(&self) -> usize {
        self.issues.len()
    }

    /// Add an issue to the result
    pub fn add_issue(&mut self, issue: ValidationIssue) {
        self.issues.push(issue);
    }

    /// Merge another validation result into this one
    pub fn merge(&mut self, other: ValidationResult) {
        self.issues.extend(other.issues);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_level() {
        assert!(ValidationLevel::Error.is_error());
        assert!(!ValidationLevel::Error.is_warning());
        assert!(!ValidationLevel::Error.is_info());

        assert!(ValidationLevel::Warning.is_warning());
        assert!(!ValidationLevel::Warning.is_error());
        assert!(!ValidationLevel::Warning.is_info());

        assert!(ValidationLevel::Info.is_info());
        assert!(!ValidationLevel::Info.is_error());
        assert!(!ValidationLevel::Info.is_warning());
    }

    #[test]
    fn test_validation_issue_creation() {
        let issue = ValidationIssue::error(
            PathBuf::from("test.md"),
            "Missing required field".to_string(),
        )
        .with_content_title("test_prompt".to_string())
        .with_line(10)
        .with_column(5)
        .with_suggestion("Add the missing field".to_string());

        assert_eq!(issue.level, ValidationLevel::Error);
        assert_eq!(issue.file_path, PathBuf::from("test.md"));
        assert_eq!(issue.message, "Missing required field");
        assert_eq!(issue.content_title, Some("test_prompt".to_string()));
        assert_eq!(issue.line, Some(10));
        assert_eq!(issue.column, Some(5));
        assert_eq!(issue.suggestion, Some("Add the missing field".to_string()));
    }

    #[test]
    fn test_validation_result() {
        let mut result = ValidationResult::ok();
        assert!(result.is_ok());
        assert!(!result.has_errors());
        assert!(!result.has_warnings());
        assert_eq!(result.issue_count(), 0);

        let error_issue = ValidationIssue::error(
            PathBuf::from("test.md"),
            "Error message".to_string(),
        );
        let warning_issue = ValidationIssue::warning(
            PathBuf::from("test.md"),
            "Warning message".to_string(),
        );

        result.add_issue(error_issue);
        result.add_issue(warning_issue);

        assert!(!result.is_ok());
        assert!(result.has_errors());
        assert!(result.has_warnings());
        assert_eq!(result.issue_count(), 2);
        assert_eq!(result.errors().len(), 1);
        assert_eq!(result.warnings().len(), 1);
    }
}