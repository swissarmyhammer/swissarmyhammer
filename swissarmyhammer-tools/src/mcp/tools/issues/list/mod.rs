//! Issue list tool for MCP operations
//!
//! This module provides the ListIssuesTool for listing existing issues through the MCP protocol.

use crate::mcp::shared_utils::McpErrorHandler;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use serde::{Deserialize, Serialize};
use swissarmyhammer::issues::Issue;

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
    fn format_as_table(issues: &[Issue]) -> String {
        if issues.is_empty() {
            return "No issues found.".to_string();
        }

        let active_issues: Vec<_> = issues.iter().filter(|i| !i.completed).collect();
        let completed_issues: Vec<_> = issues.iter().filter(|i| i.completed).collect();

        let total_issues = issues.len();
        let completed_count = completed_issues.len();
        let active_count = active_issues.len();
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
            for issue in active_issues {
                result.push_str(&format!("  🔄 {}\n", issue.name));
            }
        }

        if completed_count > 0 {
            result.push('\n');
            result.push_str("Recently Completed:\n");
            let mut sorted_completed = completed_issues;
            sorted_completed.sort_by(|a, b| b.created_at.cmp(&a.created_at));

            for issue in sorted_completed.iter().take(5) {
                result.push_str(&format!("  ✅ {}\n", issue.name));
            }
        }

        result
    }

    /// Format issues as markdown
    fn format_as_markdown(issues: &[Issue]) -> String {
        let mut result = String::from("# Issues\n\n");

        if issues.is_empty() {
            result.push_str("No issues found.\n");
            return result;
        }

        for issue in issues {
            let status = if issue.completed { "✅" } else { "🔄" };
            result.push_str(&format!("## {} - {}\n\n", status, issue.name));
            result.push_str(&format!(
                "- **Status**: {}\n",
                if issue.completed {
                    "Completed"
                } else {
                    "Active"
                }
            ));
            result.push_str(&format!(
                "- **Created**: {}\n",
                issue.created_at.format("%Y-%m-%d")
            ));
            result.push_str(&format!("- **File**: {}\n\n", issue.file_path.display()));

            if !issue.content.is_empty() {
                result.push_str("### Content\n\n");
                result.push_str(&issue.content);
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

        // Apply rate limiting for issue listing
        context
            .rate_limiter
            .check_rate_limit("unknown", "issue_list", 1)
            .map_err(|e| {
                tracing::warn!("Rate limit exceeded for issue listing: {}", e);
                McpError::invalid_params(e.to_string(), None)
            })?;

        tracing::debug!(
            "Listing issues with filters: show_completed={:?}, show_active={:?}, format={:?}",
            request.show_completed,
            request.show_active,
            request.format
        );

        let issue_storage = context.issue_storage.read().await;
        let all_issues = issue_storage
            .list_issues()
            .await
            .map_err(|e| McpErrorHandler::handle_error(e, "list issues"))?;

        let show_completed = request.show_completed.unwrap_or(false);
        let show_active = request.show_active.unwrap_or(true);
        let format = request.format.unwrap_or_else(|| "table".to_string());

        // Filter issues based on criteria
        let filtered_issues: Vec<_> = all_issues
            .into_iter()
            .filter(|issue| {
                if show_completed && show_active {
                    true // show all
                } else if show_completed {
                    issue.completed
                } else if show_active {
                    !issue.completed
                } else {
                    true // default: show all
                }
            })
            .collect();

        let response = match format.as_str() {
            "json" => serde_json::to_string_pretty(&filtered_issues).map_err(|e| {
                McpError::internal_error(format!("Failed to serialize issues: {e}"), None)
            })?,
            "markdown" => Self::format_as_markdown(&filtered_issues),
            _ => Self::format_as_table(&filtered_issues),
        };

        tracing::info!("Listed {} issues", filtered_issues.len());
        Ok(BaseToolImpl::create_success_response(&response))
    }
}
