//! Issue list tool for MCP operations
//!
//! This module provides the ListIssuesTool for listing existing issues through the MCP protocol.

use crate::mcp::shared_utils::McpErrorHandler;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::{Deserialize, Serialize};
use swissarmyhammer_issues::{Issue, IssueInfo};

/// Request structure for listing issues
#[derive(Debug, Deserialize, Serialize)]
pub struct ListIssuesRequest {
    /// Include completed issues in the list
    pub show_completed: Option<bool>,
    /// Include active issues in the list
    pub show_active: Option<bool>,
    /// Output format (table, json, markdown)
    pub format: Option<String>,
}

/// Tool for listing issues
#[derive(Default)]
pub struct ListIssuesTool;

impl ListIssuesTool {
    /// Creates a new instance of the ListIssuesTool
    pub fn new() -> Self {
        Self
    }

    /// Format issues as a table
    fn format_as_table(issue_infos: &[IssueInfo]) -> String {
        if issue_infos.is_empty() {
            return "No issues found.".to_string();
        }

        let active_issue_infos: Vec<_> = issue_infos.iter().filter(|i| !i.completed).collect();
        let completed_issue_infos: Vec<_> = issue_infos.iter().filter(|i| i.completed).collect();

        let total_issues = issue_infos.len();
        let completed_count = completed_issue_infos.len();
        let active_count = active_issue_infos.len();
        let completion_percentage = if total_issues > 0 {
            (completed_count * 100) / total_issues
        } else {
            0
        };

        let mut result = String::new();
        result.push_str(&format!("📊 Issues: {total_issues} total\n"));
        result.push_str(&format!(
            "✅ Completed: {completed_count} ({completion_percentage}%)\n"
        ));
        result.push_str(&format!("🔄 Active: {active_count}\n"));

        if active_count > 0 {
            result.push('\n');
            result.push_str("Active Issues:\n");
            for issue_info in active_issue_infos {
                result.push_str(&format!("  🔄 {}\n", issue_info.issue.name));
            }
        }

        if completed_count > 0 {
            result.push('\n');
            result.push_str("Recently Completed:\n");
            let mut sorted_completed = completed_issue_infos;
            sorted_completed.sort_by(|a, b| b.created_at.cmp(&a.created_at));

            for issue_info in sorted_completed.iter().take(5) {
                result.push_str(&format!("  ✅ {}\n", issue_info.issue.name));
            }
        }

        result
    }

    /// Format issues as markdown
    fn format_as_markdown(issue_infos: &[IssueInfo]) -> String {
        let mut result = String::from("# Issues\n\n");

        if issue_infos.is_empty() {
            result.push_str("No issues found.\n");
            return result;
        }

        for issue_info in issue_infos {
            let status = if issue_info.completed { "✅" } else { "🔄" };
            result.push_str(&format!("## {} - {}\n\n", status, issue_info.issue.name));
            result.push_str(&format!(
                "- **Status**: {}\n",
                if issue_info.completed {
                    "Completed"
                } else {
                    "Active"
                }
            ));
            result.push_str(&format!(
                "- **Created**: {}\n",
                issue_info.created_at.format("%Y-%m-%d")
            ));
            result.push_str(&format!(
                "- **File**: {}\n\n",
                issue_info.file_path.display()
            ));

            if !issue_info.issue.content.is_empty() {
                result.push_str("### Content\n\n");
                result.push_str(&issue_info.issue.content);
                result.push_str("\n\n");
            }
            result.push_str("---\n\n");
        }

        result
    }
}

#[async_trait]
impl McpTool for ListIssuesTool {
    fn name(&self) -> &'static str {
        "issue_list"
    }

    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("issues", "list")
            .unwrap_or("List all available issues with their status and metadata")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "show_completed": {
                    "type": "boolean",
                    "description": "Include completed issues in the list",
                    "default": false
                },
                "show_active": {
                    "type": "boolean",
                    "description": "Include active issues in the list",
                    "default": true
                },
                "format": {
                    "type": "string",
                    "description": "Output format - table, json, or markdown",
                    "default": "table",
                    "enum": ["table", "json", "markdown"]
                }
            },
            "required": []
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: ListIssuesRequest = BaseToolImpl::parse_arguments(arguments)?;



        tracing::debug!(
            "Listing issues with filters: show_completed={:?}, show_active={:?}, format={:?}",
            request.show_completed,
            request.show_active,
            request.format
        );

        let issue_storage = context.issue_storage.read().await;
        let all_issue_infos = issue_storage.list_issues_info().await.map_err(|e| {
            McpErrorHandler::handle_error(
                swissarmyhammer_common::SwissArmyHammerError::Other {
                    message: e.to_string(),
                },
                "list issues",
            )
        })?;

        let show_completed = request.show_completed.unwrap_or(false);
        let show_active = request.show_active.unwrap_or(true);
        let format = request.format.unwrap_or_else(|| "table".to_string());

        // Filter issues based on criteria
        let filtered_issue_infos: Vec<_> = all_issue_infos
            .into_iter()
            .filter(|issue_info| {
                if show_completed && show_active {
                    true // show all
                } else if show_completed {
                    issue_info.completed
                } else if show_active {
                    !issue_info.completed
                } else {
                    true // default: show all
                }
            })
            .collect();

        let response = match format.as_str() {
            "json" => {
                // Convert IssueInfo to Issue for JSON serialization
                let issues_for_json: Vec<Issue> = filtered_issue_infos
                    .iter()
                    .map(|info| info.issue.clone())
                    .collect();
                serde_json::to_string_pretty(&issues_for_json).map_err(|e| {
                    McpError::internal_error(format!("Failed to serialize issues: {e}"), None)
                })?
            }
            "markdown" => Self::format_as_markdown(&filtered_issue_infos),
            _ => Self::format_as_table(&filtered_issue_infos),
        };

        tracing::info!("Listed {} issues", filtered_issue_infos.len());
        Ok(BaseToolImpl::create_success_response(&response))
    }
}
