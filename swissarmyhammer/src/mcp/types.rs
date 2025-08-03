//! Request and response types for MCP operations, along with constants

use crate::config::Config;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Constants for validation
const FILESYSTEM_MAX_ISSUE_NAME_LENGTH: usize = 200;

// Type safety wrapper types

// IssueNumber type eliminated - we now use issue names (filename without .md) as the primary identifier

/// A wrapper type for issue names to prevent mixing up different string types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct IssueName(pub String);

impl IssueName {
    /// Create a new issue name with strict validation for MCP interface
    ///
    /// Uses configurable length limit and rejects filesystem-unsafe characters.
    /// Intended for user-provided input through the MCP interface.
    /// Empty names are allowed for nameless issues, but whitespace-only strings are rejected.
    pub fn new(name: String) -> Result<Self, String> {
        let trimmed = name.trim();

        // Allow truly empty names for nameless issues, but reject whitespace-only strings
        if name.trim().is_empty() && !name.is_empty() {
            return Err("Issue name cannot be empty".to_string());
        }

        let config = Config::global();
        if trimmed.len() > config.max_issue_name_length {
            return Err(format!(
                "Issue name cannot exceed {} characters",
                config.max_issue_name_length
            ));
        }

        // Check for invalid characters - reject problematic characters for MCP interface
        if trimmed.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0']) {
            return Err("Issue name contains invalid characters".to_string());
        }

        Ok(IssueName(trimmed.to_string()))
    }

    /// Create a new issue name with relaxed validation for internal filesystem use
    ///
    /// Uses a fixed length limit and only rejects null bytes.
    /// Intended for parsing existing filenames from the filesystem.
    /// Empty names are allowed for nameless issues like 000123.md, but whitespace-only strings are rejected.
    pub fn from_filesystem(name: String) -> Result<Self, String> {
        let trimmed = name.trim();

        // Allow truly empty names for nameless issues, but reject whitespace-only strings
        if name.trim().is_empty() && !name.is_empty() {
            return Err("Issue name cannot be empty".to_string());
        }

        // For filesystem names, allow up to a fixed limit and only reject null bytes
        if trimmed.len() > FILESYSTEM_MAX_ISSUE_NAME_LENGTH {
            return Err(format!(
                "Issue name cannot exceed {FILESYSTEM_MAX_ISSUE_NAME_LENGTH} characters"
            ));
        }

        // Only reject null bytes for filesystem names
        if trimmed.contains('\0') {
            return Err("Issue name contains invalid characters".to_string());
        }

        Ok(IssueName(trimmed.to_string()))
    }

    /// Get the inner string value
    pub fn get(&self) -> &str {
        &self.0
    }

    /// Get the inner string value as a string reference
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Create from string with validation
    pub fn from_string(name: String) -> Result<Self, String> {
        Self::new(name)
    }
}

impl std::fmt::Display for IssueName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<IssueName> for String {
    fn from(issue_name: IssueName) -> Self {
        issue_name.0
    }
}

impl AsRef<str> for IssueName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Request structure for getting a prompt
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetPromptRequest {
    /// Name of the prompt to retrieve
    pub name: String,
    /// Optional arguments for template rendering
    #[serde(default)]
    pub arguments: HashMap<String, String>,
}

/// Request structure for listing prompts
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListPromptsRequest {
    /// Optional filter by category
    pub category: Option<String>,
}

/// Request to create a new issue
///
/// # Examples
///
/// Create a named issue (will create file like `000123_feature_name.md`):
/// ```ignore
/// CreateIssueRequest {
///     name: Some(IssueName("feature_name".to_string())),
///     content: "# Implement new feature\n\nDetails...".to_string(),
/// }
/// ```
///
/// Create a nameless issue (will create file like `000123.md`):
/// ```ignore
/// CreateIssueRequest {
///     name: None,
///     content: "# Quick fix needed\n\nDetails...".to_string(),
/// }
/// ```
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateIssueRequest {
    /// Name of the issue (will be used in filename) - optional
    /// When `Some(name)`, creates files like `000123_name.md`
    /// When `None`, creates files like `000123.md`
    pub name: Option<IssueName>,
    /// Markdown content of the issue
    pub content: String,
}

/// Request to mark an issue as complete
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MarkCompleteRequest {
    /// Issue name to mark as complete
    pub name: IssueName,
}

/// Request to check if all issues are complete
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AllCompleteRequest {
    // No parameters needed
}

/// Request to update an issue
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateIssueRequest {
    /// Issue name to update
    pub name: IssueName,
    /// New markdown content for the issue
    pub content: String,
    /// If true, append to existing content instead of replacing
    #[serde(default)]
    pub append: bool,
}

/// Request to work on an issue
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WorkIssueRequest {
    /// Issue name to work on
    pub name: IssueName,
}

/// Request to merge an issue
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MergeIssueRequest {
    /// Issue name to merge
    pub name: IssueName,
    /// Whether to delete the branch after merging (default: false)
    #[serde(default)]
    pub delete_branch: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_name_mcp_validation_success() {
        // Valid names should pass
        assert!(IssueName::new("valid_name".to_string()).is_ok());
        assert!(IssueName::new("feature_123".to_string()).is_ok());
        assert!(IssueName::new("fix-bug".to_string()).is_ok());
        assert!(IssueName::new("a".to_string()).is_ok()); // Minimum length
    }

    #[test]
    fn test_issue_name_mcp_validation_invalid_characters() {
        // Invalid characters should be rejected
        let invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

        for invalid_char in invalid_chars {
            let name = format!("test{invalid_char}name");
            let result = IssueName::new(name.clone());
            assert!(
                result.is_err(),
                "Should reject name with '{invalid_char}': {name}"
            );
            assert_eq!(
                result.unwrap_err(),
                "Issue name contains invalid characters"
            );
        }

        // Null byte should be rejected
        let result = IssueName::new("test\0name".to_string());
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Issue name contains invalid characters"
        );
    }

    #[test]
    fn test_issue_name_mcp_validation_length_limits() {
        // This test assumes max_issue_name_length is set to a reasonable value
        // Test at the boundary of configured max length
        let config = Config::global();

        // Create name exactly at limit
        let max_name = "a".repeat(config.max_issue_name_length);
        assert!(IssueName::new(max_name).is_ok());

        // Create name over limit
        let over_limit_name = "a".repeat(config.max_issue_name_length + 1);
        let result = IssueName::new(over_limit_name);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot exceed"));
    }

    #[test]
    fn test_issue_name_mcp_validation_trimming() {
        // Names should be trimmed
        let name = IssueName::new("  test_name  ".to_string()).unwrap();
        assert_eq!(name.get(), "test_name");
    }

    #[test]
    fn test_issue_name_filesystem_validation_success() {
        // Filesystem validation should be more permissive
        assert!(IssueName::from_filesystem("valid_name".to_string()).is_ok());
        assert!(IssueName::from_filesystem("name with spaces".to_string()).is_ok());
        assert!(IssueName::from_filesystem("name:with:colons".to_string()).is_ok());
        assert!(IssueName::from_filesystem("name\"with\"quotes".to_string()).is_ok());
    }

    #[test]
    fn test_issue_name_filesystem_validation_length_limits() {
        // Test filesystem length limit
        let max_name = "a".repeat(FILESYSTEM_MAX_ISSUE_NAME_LENGTH);
        assert!(IssueName::from_filesystem(max_name).is_ok());

        let over_limit_name = "a".repeat(FILESYSTEM_MAX_ISSUE_NAME_LENGTH + 1);
        let result = IssueName::from_filesystem(over_limit_name);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot exceed"));
    }

    #[test]
    fn test_issue_name_filesystem_validation_null_bytes() {
        // Only null bytes should be rejected for filesystem names
        let result = IssueName::from_filesystem("test\0name".to_string());
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Issue name contains invalid characters"
        );
    }

    #[test]
    fn test_issue_name_filesystem_validation_trimming() {
        // Filesystem names should also be trimmed
        let name = IssueName::from_filesystem("  test_name  ".to_string()).unwrap();
        assert_eq!(name.get(), "test_name");
    }

    #[test]
    fn test_issue_name_validation_consistency() {
        // Test the difference between the two validation methods

        // Name with colon - rejected by MCP, accepted by filesystem
        let name_with_colon = "test:name".to_string();
        assert!(IssueName::new(name_with_colon.clone()).is_err());
        assert!(IssueName::from_filesystem(name_with_colon).is_ok());

        // Name with null byte - rejected by both
        let name_with_null = "test\0name".to_string();
        assert!(IssueName::new(name_with_null.clone()).is_err());
        assert!(IssueName::from_filesystem(name_with_null).is_err());
    }

    #[test]
    fn test_issue_name_lexicographical_ordering() {
        // Test that names are ordered lexicographically when used as keys
        let names = vec![
            IssueName::from_filesystem("zebra".to_string()).unwrap(),
            IssueName::from_filesystem("apple".to_string()).unwrap(),
            IssueName::from_filesystem("banana".to_string()).unwrap(),
        ];

        let mut sorted_names = names.clone();
        sorted_names.sort_by(|a, b| a.get().cmp(b.get()));

        assert_eq!(sorted_names[0].get(), "apple");
        assert_eq!(sorted_names[1].get(), "banana");
        assert_eq!(sorted_names[2].get(), "zebra");
    }

    #[test]
    fn test_issue_name_boundary_conditions() {
        // Empty string after trimming
        let result = IssueName::new("   ".to_string());
        assert!(result.is_err());

        let result = IssueName::from_filesystem("   ".to_string());
        assert!(result.is_err());

        // Single character
        assert!(IssueName::new("a".to_string()).is_ok());
        assert!(IssueName::from_filesystem("a".to_string()).is_ok());
    }
}
