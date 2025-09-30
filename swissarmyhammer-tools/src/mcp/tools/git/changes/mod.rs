//! Git changes tool - list files changed on a branch
//!
//! This tool provides programmatic access to git diff operations using libgit2,
//! identifying which files have been modified on a branch relative to its parent.
//!
//! ## Key Concept
//!
//! The tool determines the "scope of changes" for any branch:
//! - **Feature/Issue branches**: Files changed since diverging from the parent branch
//! - **Main/trunk branches**: All tracked files (cumulative changes)
//!
//! The distinction is based on whether a branch has a clear parent it diverged from.

use async_trait::async_trait;
use rmcp::model::CallToolResult;
use serde::{Deserialize, Serialize};

use crate::mcp::tool_registry::{McpTool, ToolContext};

/// Request structure for git changes operation
#[derive(Debug, Deserialize, Serialize)]
pub struct GitChangesRequest {
    /// Branch name to analyze
    pub branch: String,
}

/// Response structure containing changed files
#[derive(Debug, Deserialize, Serialize)]
pub struct GitChangesResponse {
    /// The analyzed branch
    pub branch: String,
    /// Parent branch (if determined), null for root branches
    pub parent_branch: Option<String>,
    /// List of file paths that have changed
    pub files: Vec<String>,
}

/// Tool for listing changed files on a git branch
#[derive(Default)]
pub struct GitChangesTool;

impl GitChangesTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for GitChangesTool {
    fn name(&self) -> &'static str {
        "git_changes"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "branch": {
                    "type": "string",
                    "description": "Branch name to analyze"
                }
            },
            "required": ["branch"]
        })
    }

    async fn execute(
        &self,
        _arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, rmcp::ErrorData> {
        Err(rmcp::ErrorData::internal_error(
            "git_changes tool not yet implemented",
            None,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_changes_tool_name() {
        let tool = GitChangesTool::new();
        assert_eq!(tool.name(), "git_changes");
    }

    #[test]
    fn test_git_changes_tool_description() {
        let tool = GitChangesTool::new();
        let description = tool.description();
        assert!(!description.is_empty());
        assert!(description.contains("Git Changes"));
    }

    #[test]
    fn test_git_changes_tool_schema() {
        let tool = GitChangesTool::new();
        let schema = tool.schema();

        assert!(schema.is_object());
        let properties = schema
            .get("properties")
            .expect("schema should have properties");
        assert!(properties.get("branch").is_some());

        let required = schema
            .get("required")
            .expect("schema should have required fields");
        assert!(required.is_array());
        let required_array = required.as_array().expect("required should be an array");
        assert_eq!(required_array.len(), 1);
        assert_eq!(required_array[0], "branch");
    }

    #[tokio::test]
    async fn test_git_changes_tool_execute_stub() {
        let tool = GitChangesTool::new();
        let mut arguments = serde_json::Map::new();
        arguments.insert("branch".to_string(), serde_json::json!("main"));

        let context = crate::test_utils::create_test_context().await;

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.message.contains("not yet implemented"));
    }

    #[test]
    fn test_git_changes_request_serialization() {
        let request = GitChangesRequest {
            branch: "main".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("main"));

        let deserialized: GitChangesRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.branch, "main");
    }

    #[test]
    fn test_git_changes_response_serialization() {
        let response = GitChangesResponse {
            branch: "issue/test".to_string(),
            parent_branch: Some("main".to_string()),
            files: vec!["src/main.rs".to_string(), "README.md".to_string()],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("issue/test"));
        assert!(json.contains("main"));
        assert!(json.contains("src/main.rs"));

        let deserialized: GitChangesResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.branch, "issue/test");
        assert_eq!(deserialized.parent_branch, Some("main".to_string()));
        assert_eq!(deserialized.files.len(), 2);
    }
}
