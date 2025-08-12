//! Response creation utilities for MCP operations

use rmcp::model::*;
use swissarmyhammer::issues::{Issue, IssueInfo};

/// Create a success response for MCP tool calls
pub fn create_success_response(message: String) -> CallToolResult {
    CallToolResult {
        content: vec![Annotated::new(
            RawContent::Text(RawTextContent { text: message }),
            None,
        )],
        is_error: Some(false),
    }
}

/// Create an error response for MCP tool calls
pub fn create_error_response(message: String) -> CallToolResult {
    CallToolResult {
        content: vec![Annotated::new(
            RawContent::Text(RawTextContent { text: message }),
            None,
        )],
        is_error: Some(true),
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
            }),
            None,
        )],
        is_error: Some(false),
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
            }),
            None,
        )],
        is_error: Some(false),
    }
}

/// Create a standardized response for issue update operations
pub fn create_update_response(issue_info: &IssueInfo) -> CallToolResult {
    let response = serde_json::json!({
        "name": issue_info.issue.name,
        "file_path": issue_info.file_path.to_string_lossy(),
        "message": format!("Updated issue {}", issue_info.issue.name)
    });

    CallToolResult {
        content: vec![Annotated::new(
            RawContent::Text(RawTextContent {
                text: response["message"].as_str().unwrap().to_string(),
            }),
            None,
        )],
        is_error: Some(false),
    }
}

/// Create a standardized response for issue all complete check
pub fn create_all_complete_response(total_issues: usize, pending_count: usize) -> CallToolResult {
    let all_complete = pending_count == 0;
    let response = serde_json::json!({
        "all_complete": all_complete,
        "total_issues": total_issues,
        "pending_count": pending_count,
        "completed_count": total_issues - pending_count,
        "message": format!(
            "All issues complete: {}. {} of {} issues pending",
            all_complete, pending_count, total_issues
        )
    });

    CallToolResult {
        content: vec![Annotated::new(
            RawContent::Text(RawTextContent {
                text: response["message"].as_str().unwrap().to_string(),
            }),
            None,
        )],
        is_error: Some(false),
    }
}

/// Create a standardized response for current issue check
pub fn create_current_issue_response(
    current_branch: &str,
    issue_name: Option<&str>,
) -> CallToolResult {
    let response = if let Some(name) = issue_name {
        serde_json::json!({
            "current_branch": current_branch,
            "issue_name": name,
            "on_issue_branch": true,
            "message": format!("Currently working on issue: {}", name)
        })
    } else {
        serde_json::json!({
            "current_branch": current_branch,
            "issue_name": null,
            "on_issue_branch": false,
            "message": format!("Not on an issue branch. Current branch: {}", current_branch)
        })
    };

    CallToolResult {
        content: vec![Annotated::new(
            RawContent::Text(RawTextContent {
                text: response["message"].as_str().unwrap().to_string(),
            }),
            None,
        )],
        is_error: Some(false),
    }
}

/// Create a standardized response for issue work operations
pub fn create_work_response(issue: &Issue, branch_name: &str) -> CallToolResult {
    let response = serde_json::json!({
        "issue_name": issue.name,
        "branch_name": branch_name,
        "message": format!("Switched to work branch: {}", branch_name)
    });

    CallToolResult {
        content: vec![Annotated::new(
            RawContent::Text(RawTextContent {
                text: response["message"].as_str().unwrap().to_string(),
            }),
            None,
        )],
        is_error: Some(false),
    }
}
