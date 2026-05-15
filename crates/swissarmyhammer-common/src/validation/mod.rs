//! Validation framework for checking content and workflow integrity
//!
//! This module provides a flexible validation system that can check various
//! aspects of prompts, workflows, and other content types. The framework is
//! extensible, allowing custom validators to be added.

use std::path::{Path, PathBuf};

pub mod validation_builders;

/// Represents the severity level of a validation issue
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationLevel {
    /// Critical issues that prevent normal operation
    Error,
    /// Issues that should be addressed but don't prevent operation
    Warning,
    /// Informational messages for best practices
    Info,
}

/// A single validation issue found during validation
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    /// Severity level of the issue
    pub level: ValidationLevel,
    /// File path where the issue was found
    pub file_path: PathBuf,
    /// Optional title of the content being validated
    pub content_title: Option<String>,
    /// Line number where the issue occurs
    pub line: Option<usize>,
    /// Column number where the issue occurs
    pub column: Option<usize>,
    /// Description of the issue
    pub message: String,
    /// Suggested fix for the issue
    pub suggestion: Option<String>,
}

/// Result of a validation operation
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// All issues found during validation
    pub issues: Vec<ValidationIssue>,
    /// Number of files checked
    pub files_checked: usize,
    /// Count of error-level issues
    pub errors: usize,
    /// Count of warning-level issues
    pub warnings: usize,
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidationResult {
    /// Creates a new empty validation result
    pub fn new() -> Self {
        Self {
            issues: Vec::new(),
            files_checked: 0,
            errors: 0,
            warnings: 0,
        }
    }

    /// Adds an issue to the validation result
    pub fn add_issue(&mut self, issue: ValidationIssue) {
        match issue.level {
            ValidationLevel::Error => self.errors += 1,
            ValidationLevel::Warning => self.warnings += 1,
            ValidationLevel::Info => {}
        }
        self.issues.push(issue);
    }

    /// Checks if there are any error-level issues
    pub fn has_errors(&self) -> bool {
        self.errors > 0
    }

    /// Checks if there are any warning-level issues
    pub fn has_warnings(&self) -> bool {
        self.warnings > 0
    }

    /// Merges another validation result into this one
    pub fn merge(&mut self, other: ValidationResult) {
        self.files_checked += other.files_checked;
        self.errors += other.errors;
        self.warnings += other.warnings;
        self.issues.extend(other.issues);
    }
}

/// Trait for validators that check content patterns
pub trait ContentValidator: Send + Sync {
    /// Validate content and add issues to the result
    fn validate_content(
        &self,
        content: &str,
        file_path: &Path,
        result: &mut ValidationResult,
        content_title: Option<String>,
    );

    /// Get the name of this validator
    fn name(&self) -> &str;
}

/// Configuration for validation operations
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Maximum allowed complexity for workflows (states + transitions)
    pub max_workflow_complexity: usize,
    /// Whether to validate encoding (check for BOM)
    pub check_encoding: bool,
    /// Whether to validate line endings consistency
    pub check_line_endings: bool,
    /// Whether to check for YAML typos
    pub check_yaml_typos: bool,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            max_workflow_complexity: std::env::var("SWISSARMYHAMMER_MAX_WORKFLOW_COMPLEXITY")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1000),
            check_encoding: std::env::var("SWISSARMYHAMMER_CHECK_ENCODING")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
            check_line_endings: std::env::var("SWISSARMYHAMMER_CHECK_LINE_ENDINGS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
            check_yaml_typos: std::env::var("SWISSARMYHAMMER_CHECK_YAML_TYPOS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
        }
    }
}

/// Validator for UTF-8 encoding issues
pub struct EncodingValidator;

impl ContentValidator for EncodingValidator {
    fn validate_content(
        &self,
        content: &str,
        file_path: &Path,
        result: &mut ValidationResult,
        content_title: Option<String>,
    ) {
        // Check for BOM
        if content.starts_with('\u{FEFF}') {
            result.add_issue(ValidationIssue {
                level: ValidationLevel::Warning,
                file_path: file_path.to_path_buf(),
                content_title,
                line: Some(1),
                column: Some(1),
                message: "File contains UTF-8 BOM".to_string(),
                suggestion: Some("Remove the BOM for better compatibility".to_string()),
            });
        }
    }

    fn name(&self) -> &str {
        "EncodingValidator"
    }
}

/// Validator for line ending consistency
pub struct LineEndingValidator;

impl ContentValidator for LineEndingValidator {
    fn validate_content(
        &self,
        content: &str,
        file_path: &Path,
        result: &mut ValidationResult,
        content_title: Option<String>,
    ) {
        let has_crlf = content.contains("\r\n");
        // Check for LF that are not part of CRLF
        let content_without_crlf = content.replace("\r\n", "");
        let has_lf_only = content_without_crlf.contains('\n');

        if has_crlf && has_lf_only {
            result.add_issue(ValidationIssue {
                level: ValidationLevel::Warning,
                file_path: file_path.to_path_buf(),
                content_title,
                line: None,
                column: None,
                message: "Mixed line endings detected (both CRLF and LF)".to_string(),
                suggestion: Some("Use consistent line endings throughout the file".to_string()),
            });
        }
    }

    fn name(&self) -> &str {
        "LineEndingValidator"
    }
}

/// Validator for common typos in YAML fields
pub struct YamlTypoValidator {
    typo_map: Vec<(&'static str, &'static str)>,
}

impl Default for YamlTypoValidator {
    fn default() -> Self {
        Self {
            typo_map: vec![
                ("titel", "title"),
                ("descripton", "description"),
                ("argumnets", "arguments"),
                ("requried", "required"),
                ("catagory", "category"),
                ("tage", "tags"),
                ("defualt", "default"),
            ],
        }
    }
}

impl ContentValidator for YamlTypoValidator {
    fn validate_content(
        &self,
        content: &str,
        file_path: &Path,
        result: &mut ValidationResult,
        content_title: Option<String>,
    ) {
        for (line_num, line) in content.lines().enumerate() {
            for (typo, correct) in &self.typo_map {
                // Check for whole word matches to avoid false positives like "tage" in "staged"
                let words: Vec<&str> = line.split_whitespace().collect();
                for word in words {
                    // Remove common punctuation to check the actual word
                    let clean_word = word.trim_matches(
                        &[
                            ',', '.', ':', ';', '!', '?', '"', '\'', '(', ')', '[', ']', '{', '}',
                        ][..],
                    );
                    if clean_word == *typo {
                        result.add_issue(ValidationIssue {
                            level: ValidationLevel::Warning,
                            file_path: file_path.to_path_buf(),
                            content_title: content_title.clone(),
                            line: Some(line_num + 1),
                            column: None,
                            message: format!("Possible typo: '{typo}' should be '{correct}'"),
                            suggestion: Some(format!("Replace '{typo}' with '{correct}'")),
                        });
                    }
                }
            }
        }
    }

    fn name(&self) -> &str {
        "YamlTypoValidator"
    }
}

/// Trait for types that can validate themselves
///
/// This trait allows types to implement their own validation logic
/// and return a list of validation issues. It's designed to move
/// validation logic closer to the types being validated.
pub trait Validatable {
    /// Validate this instance and return any issues found
    ///
    /// # Arguments
    /// * `source_path` - Optional path to the source file for better error reporting
    ///
    /// # Returns
    /// A vector of validation issues. An empty vector indicates successful validation.
    fn validate(&self, source_path: Option<&std::path::Path>) -> Vec<ValidationIssue>;
}

/// Manager for running multiple validators
pub struct ValidationManager {
    validators: Vec<Box<dyn ContentValidator>>,
    config: ValidationConfig,
}

impl ValidationManager {
    /// Creates a new validation manager with default validators
    pub fn new(config: ValidationConfig) -> Self {
        let mut manager = Self {
            validators: Vec::new(),
            config,
        };

        // Add default validators based on config
        if manager.config.check_encoding {
            manager.add_validator(Box::new(EncodingValidator));
        }
        if manager.config.check_line_endings {
            manager.add_validator(Box::new(LineEndingValidator));
        }
        if manager.config.check_yaml_typos {
            manager.add_validator(Box::new(YamlTypoValidator::default()));
        }

        manager
    }

    /// Adds a custom validator
    pub fn add_validator(&mut self, validator: Box<dyn ContentValidator>) {
        self.validators.push(validator);
    }

    /// Validates content using all registered validators
    pub fn validate_content(
        &self,
        content: &str,
        file_path: &Path,
        content_title: Option<String>,
    ) -> ValidationResult {
        let mut result = ValidationResult::new();
        result.files_checked = 1;

        for validator in &self.validators {
            validator.validate_content(content, file_path, &mut result, content_title.clone());
        }

        result
    }

    /// Gets the current validation configuration
    pub fn config(&self) -> &ValidationConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_result_creation() {
        let result = ValidationResult::new();
        assert_eq!(result.files_checked, 0);
        assert_eq!(result.errors, 0);
        assert_eq!(result.warnings, 0);
        assert!(!result.has_errors());
        assert!(!result.has_warnings());
    }

    #[test]
    fn test_validation_result_add_error() {
        let mut result = ValidationResult::new();
        let issue = ValidationIssue {
            level: ValidationLevel::Error,
            file_path: PathBuf::from("test.md"),
            content_title: Some("Test Content".to_string()),
            line: Some(1),
            column: Some(1),
            message: "Test error".to_string(),
            suggestion: None,
        };

        result.add_issue(issue);
        assert_eq!(result.errors, 1);
        assert_eq!(result.warnings, 0);
        assert!(result.has_errors());
        assert!(!result.has_warnings());
    }

    #[test]
    fn test_encoding_validator() {
        let validator = EncodingValidator;
        let mut result = ValidationResult::new();

        // Test with BOM
        let content_with_bom = "\u{FEFF}Hello World";
        validator.validate_content(content_with_bom, Path::new("test.txt"), &mut result, None);
        assert_eq!(result.warnings, 1);
        assert!(result.issues[0].message.contains("BOM"));

        // Test without BOM
        let mut result2 = ValidationResult::new();
        let content_no_bom = "Hello World";
        validator.validate_content(content_no_bom, Path::new("test.txt"), &mut result2, None);
        assert_eq!(result2.warnings, 0);
    }

    #[test]
    fn test_line_ending_validator() {
        let validator = LineEndingValidator;
        let mut result = ValidationResult::new();

        // Test with mixed line endings
        let mixed_content = "Line 1\r\nLine 2\nLine 3\r\n";
        validator.validate_content(mixed_content, Path::new("test.txt"), &mut result, None);
        assert_eq!(result.warnings, 1);
        assert!(result.issues[0].message.contains("Mixed line endings"));

        // Test with consistent line endings
        let mut result2 = ValidationResult::new();
        let consistent_content = "Line 1\nLine 2\nLine 3\n";
        validator.validate_content(
            consistent_content,
            Path::new("test.txt"),
            &mut result2,
            None,
        );
        assert_eq!(result2.warnings, 0);
    }

    #[test]
    fn test_yaml_typo_validator() {
        let validator = YamlTypoValidator::default();
        let mut result = ValidationResult::new();

        // Test with typo
        let yaml_with_typo = "titel: My Title\ndescripton: My description";
        validator.validate_content(yaml_with_typo, Path::new("test.yaml"), &mut result, None);
        assert_eq!(result.warnings, 2);
        assert!(result.issues[0].message.contains("titel"));
        assert!(result.issues[1].message.contains("descripton"));

        // Test that substrings don't match (like "tage" in "staged")
        let mut result2 = ValidationResult::new();
        let content_with_substring = "These files are staged and ready";
        validator.validate_content(
            content_with_substring,
            Path::new("test.txt"),
            &mut result2,
            None,
        );
        assert_eq!(
            result2.warnings, 0,
            "Should not match 'tage' within 'staged'"
        );
    }

    #[test]
    fn test_validation_manager() {
        let config = ValidationConfig::default();
        let manager = ValidationManager::new(config);

        let content = "\u{FEFF}Line 1\r\nLine 2\ntitel: Test";
        let result = manager.validate_content(
            content,
            Path::new("test.txt"),
            Some("Test Content".to_string()),
        );

        // Should have warnings from multiple validators
        assert!(result.warnings > 0);
        assert_eq!(result.files_checked, 1);
    }
}
