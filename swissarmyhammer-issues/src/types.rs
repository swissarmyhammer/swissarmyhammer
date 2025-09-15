//! Core types for issue management

use crate::config::Config;
use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
    pub fn new(name: String) -> Result<Self> {
        Self::new_with_config(name, Config::global())
    }

    /// Create a new issue name with custom config (for testing)
    pub fn new_with_config(name: String, config: &Config) -> Result<Self> {
        let trimmed = name.trim();

        // Allow truly empty names for nameless issues, but reject whitespace-only strings
        if name.trim().is_empty() && !name.is_empty() {
            return Err(Error::other("Issue name cannot be empty"));
        }

        if trimmed.len() > config.max_issue_name_length {
            return Err(Error::other(format!(
                "Issue name cannot exceed {} characters",
                config.max_issue_name_length
            )));
        }

        // Check for invalid characters - reject problematic characters for MCP interface
        if trimmed.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0']) {
            return Err(Error::other("Issue name contains invalid characters"));
        }

        Ok(IssueName(trimmed.to_string()))
    }

    /// Create a new issue name with relaxed validation for internal filesystem use
    ///
    /// Uses configurable length limit and allows filesystem-safe characters.
    /// Intended for filesystem operations and internal use.
    pub fn new_internal(name: String) -> Result<Self> {
        Self::new_internal_with_config(name, Config::global())
    }

    /// Create a new issue name with relaxed validation for internal filesystem use with custom config
    pub fn new_internal_with_config(name: String, config: &Config) -> Result<Self> {
        let trimmed = name.trim();

        if trimmed.is_empty() {
            return Err(Error::other("Issue name cannot be empty"));
        }

        if trimmed.len() > config.max_issue_name_length {
            return Err(Error::other(format!(
                "Issue name cannot exceed {} characters",
                config.max_issue_name_length
            )));
        }

        // More permissive validation for filesystem operations
        if trimmed.contains(['\0', '/']) {
            return Err(Error::other("Issue name contains invalid characters"));
        }

        Ok(IssueName(trimmed.to_string()))
    }

    /// Create a new issue name with relaxed validation for internal filesystem use
    ///
    /// Uses configurable length limit and only rejects null bytes.
    /// Intended for parsing existing filenames from the filesystem.
    /// Empty names are allowed for nameless issues like 000123.md, but whitespace-only strings are rejected.
    pub fn from_filesystem(name: String) -> Result<Self> {
        Self::from_filesystem_with_config(name, Config::global())
    }

    /// Create a new issue name from filesystem with custom config
    pub fn from_filesystem_with_config(name: String, config: &Config) -> Result<Self> {
        let trimmed = name.trim();

        // Allow truly empty names for nameless issues, but reject whitespace-only strings
        if name.trim().is_empty() && !name.is_empty() {
            return Err(Error::other("Issue name cannot be empty"));
        }

        // For filesystem names, allow up to configurable limit and only reject null bytes
        if trimmed.len() > config.max_issue_name_length {
            return Err(Error::other(format!(
                "Issue name cannot exceed {} characters",
                config.max_issue_name_length
            )));
        }

        // Only reject null bytes for filesystem names
        if trimmed.contains('\0') {
            return Err(Error::other("Issue name contains invalid characters"));
        }

        Ok(IssueName(trimmed.to_string()))
    }

    /// Get the inner string value (alias for as_str)
    pub fn get(&self) -> &str {
        &self.0
    }

    /// Create from string with validation (alias for new)
    pub fn from_string(name: String) -> Result<Self> {
        Self::new(name)
    }

    /// Get the inner string value
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert into the inner string value
    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for IssueName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for IssueName {
    fn from(name: String) -> Self {
        IssueName(name)
    }
}

impl From<&str> for IssueName {
    fn from(name: &str) -> Self {
        IssueName(name.to_string())
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

/// Represents an issue in the tracking system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Issue {
    /// The primary identifier - issue name derived from filename (without .md extension)
    pub name: String,
    /// The full content of the issue markdown file
    pub content: String,
}

impl Issue {
    /// Check if this issue is completed based on file path location
    pub fn is_completed(&self, file_path: &Path, completed_dir: &Path) -> bool {
        file_path
            .parent()
            .map(|parent| parent == completed_dir)
            .unwrap_or(false)
    }

    /// Get the file path for this issue based on its location (completed or active)
    pub fn get_file_path(&self, base_dir: &Path, completed: bool) -> PathBuf {
        let dir = if completed {
            base_dir.join("complete")
        } else {
            base_dir.to_path_buf()
        };
        dir.join(format!("{}.md", self.name))
    }

    /// Get the creation time from file metadata
    pub fn get_created_at(file_path: &Path) -> DateTime<Utc> {
        file_path
            .metadata()
            .and_then(|m| m.created())
            .or_else(|_| file_path.metadata().and_then(|m| m.modified()))
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(|_| Utc::now())
    }
}

/// Extended issue information that includes derived metadata
#[derive(Debug, Clone)]
pub struct IssueInfo {
    /// The core issue data
    pub issue: Issue,
    /// Whether this issue is completed (in completed directory)
    pub completed: bool,
    /// Full path to the issue file
    pub file_path: PathBuf,
    /// When this issue was created
    pub created_at: DateTime<Utc>,
}

impl IssueInfo {
    /// Create issue info from an issue and its file path
    pub fn from_issue_and_path(issue: Issue, file_path: PathBuf, completed_dir: &Path) -> Self {
        let completed = issue.is_completed(&file_path, completed_dir);
        let created_at = Issue::get_created_at(&file_path);

        Self {
            issue,
            completed,
            file_path,
            created_at,
        }
    }
}

/// Represents the current state of the issue system
#[derive(Debug, Clone)]
pub struct IssueState {
    /// Path to the issues directory
    pub issues_dir: PathBuf,
    /// Path to the completed issues directory
    pub completed_dir: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_name_validation() {
        // Valid names
        assert!(IssueName::new("valid_name".to_string()).is_ok());
        assert!(IssueName::new("123".to_string()).is_ok());
        assert!(IssueName::new("".to_string()).is_ok()); // Empty allowed

        // Invalid characters
        assert!(IssueName::new("invalid/name".to_string()).is_err());
        assert!(IssueName::new("invalid\\name".to_string()).is_err());
        assert!(IssueName::new("invalid:name".to_string()).is_err());

        // Whitespace-only strings
        assert!(IssueName::new("   ".to_string()).is_err());
        assert!(IssueName::new("\t\n".to_string()).is_err());
    }

    #[test]
    fn test_issue_name_from_filesystem() {
        // Valid filesystem names
        assert!(IssueName::from_filesystem("valid_name".to_string()).is_ok());
        assert!(IssueName::from_filesystem("name-with-dashes".to_string()).is_ok());
        assert!(IssueName::from_filesystem("".to_string()).is_ok()); // Empty allowed

        // Invalid filesystem names
        assert!(IssueName::from_filesystem("invalid\0name".to_string()).is_err());

        // Whitespace-only strings still rejected
        assert!(IssueName::from_filesystem("   ".to_string()).is_err());
    }

    #[test]
    fn test_issue_name_internal() {
        // Valid internal names
        assert!(IssueName::new_internal("valid_name".to_string()).is_ok());
        assert!(IssueName::new_internal("name.with.dots".to_string()).is_ok());

        // Invalid internal names
        assert!(IssueName::new_internal("".to_string()).is_err()); // Empty not allowed for internal
        assert!(IssueName::new_internal("invalid/name".to_string()).is_err());
        assert!(IssueName::new_internal("invalid\0name".to_string()).is_err());
    }

    #[test]
    fn test_issue_name_conversions() {
        let name = IssueName::new("test_name".to_string()).unwrap();

        // Test various conversion methods
        assert_eq!(name.as_str(), "test_name");
        assert_eq!(name.get(), "test_name");
        assert_eq!(name.to_string(), "test_name");
        assert_eq!(String::from(name.clone()), "test_name");
        assert_eq!(name.into_string(), "test_name");
    }

    #[test]
    fn test_issue_creation() {
        let issue = Issue {
            name: "test_issue".to_string(),
            content: "# Test Issue\n\nContent".to_string(),
        };

        assert_eq!(issue.name, "test_issue");
        assert_eq!(issue.content, "# Test Issue\n\nContent");
    }

    #[test]
    fn test_issue_file_path_generation() {
        let issue = Issue {
            name: "test_issue".to_string(),
            content: "content".to_string(),
        };

        let base_dir = PathBuf::from("/test/issues");

        // Active issue path
        let active_path = issue.get_file_path(&base_dir, false);
        assert_eq!(active_path, PathBuf::from("/test/issues/test_issue.md"));

        // Completed issue path
        let completed_path = issue.get_file_path(&base_dir, true);
        assert_eq!(
            completed_path,
            PathBuf::from("/test/issues/complete/test_issue.md")
        );
    }

    #[test]
    fn test_issue_completion_check() {
        let issue = Issue {
            name: "test_issue".to_string(),
            content: "content".to_string(),
        };

        let completed_dir = PathBuf::from("/test/issues/complete");

        // Active issue file
        let active_path = PathBuf::from("/test/issues/test_issue.md");
        assert!(!issue.is_completed(&active_path, &completed_dir));

        // Completed issue file
        let completed_path = PathBuf::from("/test/issues/complete/test_issue.md");
        assert!(issue.is_completed(&completed_path, &completed_dir));
    }
}
