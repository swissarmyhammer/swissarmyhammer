//! Issue completion checking tool for MCP operations
//!
//! This module provides the AllCompleteIssueTool for checking if all issues are completed.

use crate::mcp::responses::create_success_response;
use crate::mcp::shared_utils::McpErrorHandler;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::types::AllCompleteRequest;
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;

/// Tool for checking if all issues are complete
#[derive(Default)]
pub struct AllCompleteIssueTool;

impl AllCompleteIssueTool {
    /// Creates a new instance of the AllCompleteIssueTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for AllCompleteIssueTool {
    fn name(&self) -> &'static str {
        "issue_all_complete"
    }

    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("issues", "all_complete")
            .expect("Tool description should be available")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let _request: AllCompleteRequest = BaseToolImpl::parse_arguments(arguments)?;

        let issue_storage = context.issue_storage.read().await;

        // Get all issues with comprehensive error handling
        let all_issues = match issue_storage.list_issues().await {
            Ok(issues) => issues,
            Err(e) => {
                return Err(McpErrorHandler::handle_error(
                    e,
                    "list issues for completion check",
                ))
            }
        };

        // Separate active and completed issues
        let mut active_issues = Vec::new();
        let mut completed_issues = Vec::new();

        for issue in all_issues {
            if issue.completed {
                completed_issues.push(issue);
            } else {
                active_issues.push(issue);
            }
        }

        // Calculate statistics
        let total_issues = active_issues.len() + completed_issues.len();
        let completed_count = completed_issues.len();
        let active_count = active_issues.len();
        let all_complete = active_count == 0 && total_issues > 0;

        let completion_percentage = if total_issues > 0 {
            (completed_count * 100) / total_issues
        } else {
            0
        };

        // Generate comprehensive response text
        let response_text = if total_issues == 0 {
            "📋 No issues found in the project\n\n✨ The project has no tracked issues. You can create issues using the `issue_create` tool.".to_string()
        } else if all_complete {
            format!(
                "🎉 All issues are complete!\n\n📊 Project Status:\n• Total Issues: {}\n• Completed: {} (100%)\n• Active: 0\n\n✅ Completed Issues:\n{}",
                total_issues,
                completed_count,
                completed_issues.iter()
                    .map(|issue| format!("• {}", issue.name))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else {
            let active_list = active_issues
                .iter()
                .map(|issue| format!("• {}", issue.name))
                .collect::<Vec<_>>()
                .join("\n");

            let completed_list = if completed_count > 0 {
                completed_issues
                    .iter()
                    .map(|issue| format!("• {}", issue.name))
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                "  (none)".to_string()
            };

            format!(
                "⏳ Project has active issues ({completion_percentage}% complete)\n\n📊 Project Status:\n• Total Issues: {total_issues}\n• Completed: {completed_count} ({completion_percentage}%)\n• Active: {active_count}\n\n🔄 Active Issues:\n{active_list}\n\n✅ Completed Issues:\n{completed_list}"
            )
        };

        Ok(create_success_response(response_text))
    }
}
