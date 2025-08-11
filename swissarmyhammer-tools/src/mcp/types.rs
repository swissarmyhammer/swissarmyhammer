//! Request and response types for MCP operations, along with constants

// IssueName is used via full path and re-exported below
use serde::Deserialize;
use std::collections::HashMap;

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
///     name: Some(swissarmyhammer::issues::IssueName("feature_name".to_string())),
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
    pub name: Option<swissarmyhammer::issues::IssueName>,
    /// Markdown content of the issue
    pub content: String,
}

/// Request to mark an issue as complete
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MarkCompleteRequest {
    /// Issue name to mark as complete
    pub name: swissarmyhammer::issues::IssueName,
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
    pub name: swissarmyhammer::issues::IssueName,
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
    pub name: swissarmyhammer::issues::IssueName,
}

/// Request to merge an issue
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MergeIssueRequest {
    /// Issue name to merge
    pub name: swissarmyhammer::issues::IssueName,
    /// Whether to delete the branch after merging (default: false)
    #[serde(default)]
    pub delete_branch: bool,
}

// Re-export IssueName for convenience
pub use swissarmyhammer::issues::IssueName;
