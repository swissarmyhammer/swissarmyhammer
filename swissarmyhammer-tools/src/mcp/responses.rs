//! Response creation utilities for MCP operations

use rmcp::model::*;
use swissarmyhammer::issues::{Issue, IssueInfo};

/// Create a success response for MCP tool calls
pub fn create_success_response(message: String) -> CallToolResult {
    CallToolResult {
        content: vec![Annotated::new(
            RawContent::Text(RawTextContent {
                text: message,
                meta: None,
            }),
            None,
        )],
        is_error: Some(false),
        structured_content: None,
        meta: None,
    }
}

/// Create an error response for MCP tool calls
pub fn create_error_response(message: String) -> CallToolResult {
    CallToolResult {
        content: vec![Annotated::new(
            RawContent::Text(RawTextContent {
                text: message,
                meta: None,
            }),
            None,
        )],
        is_error: Some(true),
        structured_content: None,
        meta: None,
    }
}

/// Create a standardized response for issue creation
///
/// This function creates a consistent response format with structured JSON
/// information and artifact support for issue creation operations.
///
/// # Arguments
///
/// * `issue` - The created issue object
///
/// # Returns
///
/// * `CallToolResult` - Standardized response with artifact support
pub fn create_issue_response(issue_info: &IssueInfo) -> CallToolResult {
    let response = serde_json::json!({
        "name": issue_info.issue.name,
        "file_path": issue_info.file_path.to_string_lossy(),
        "message": format!(
            "Created issue {} at {}",
            issue_info.issue.name,
            issue_info.file_path.display()
        )
    });

    CallToolResult {
        content: vec![Annotated::new(
            RawContent::Text(RawTextContent {
                text: response["message"].as_str().unwrap().to_string(),
                meta: None,
            }),
            None,
        )],
        is_error: Some(false),
        structured_content: None,
        meta: None,
    }
}

/// Create a standardized response for issue mark complete operations
pub fn create_mark_complete_response(issue: &Issue) -> CallToolResult {
    let response = serde_json::json!({
        "name": issue.name,
        "completed": true,
        "message": format!("Marked issue {} as complete", issue.name)
    });

    CallToolResult {
        content: vec![Annotated::new(
            RawContent::Text(RawTextContent {
                text: response["message"].as_str().unwrap().to_string(),
                meta: None,
            }),
            None,
        )],
        is_error: Some(false),
        structured_content: None,
        meta: None,
    }
}
