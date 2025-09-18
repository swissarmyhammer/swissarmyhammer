//! Issue completion checking tool for MCP operations
//!
//! This module provides the AllCompleteIssueTool for checking if all issues are completed.

use crate::mcp::responses::create_success_response;
use crate::mcp::shared_utils::McpErrorHandler;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::types::AllCompleteRequest;
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;

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

    fn cli_name(&self) -> &'static str {
        "status"
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

        // Get all issues with extended info for completion status
        let all_issue_infos = match issue_storage.list_issues_info().await {
            Ok(issue_infos) => issue_infos,
            Err(e) => {
                return Err(McpErrorHandler::handle_error(
                    swissarmyhammer_common::SwissArmyHammerError::Other {
                        message: e.to_string(),
                    },
                    "list issues for completion check",
                ))
            }
        };

        // Separate active and completed issues
        let mut active_issues = Vec::new();
        let mut completed_issues = Vec::new();

        for issue_info in all_issue_infos {
            if issue_info.completed {
                completed_issues.push(issue_info.issue);
            } else {
                active_issues.push(issue_info.issue);
            }
        }

        // Calculate statistics
        let total_issues = active_issues.len() + completed_issues.len();
        let completed_count = completed_issues.len();
        let active_count = active_issues.len();
        let all_complete = active_count == 0 && total_issues > 0;

        // Generate simple response text
        let response_text = if total_issues == 0 {
            "No issues found".to_string()
        } else if all_complete {
            format!("All {} issues complete", total_issues)
        } else {
            format!("{} of {} issues complete", completed_count, total_issues)
        };

        Ok(create_success_response(response_text))
    }
}
