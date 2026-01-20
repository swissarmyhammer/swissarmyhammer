use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::tools::questions::persistence::load_all_questions;
use async_trait::async_trait;
use chrono::Utc;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Request structure for question_summary tool
#[derive(Debug, Deserialize, Serialize)]
pub struct QuestionSummaryRequest {
    /// Optional limit on number of entries to return (default: all)
    #[serde(default)]
    pub limit: Option<usize>,
}

/// MCP tool for retrieving all question/answer pairs as YAML summary
#[derive(Default)]
pub struct QuestionSummaryTool;

impl QuestionSummaryTool {
    /// Creates a new instance of the QuestionSummaryTool
    pub fn new() -> Self {
        Self
    }
}

// No health checks needed
crate::impl_empty_doctorable!(QuestionSummaryTool);

#[async_trait]
impl McpTool for QuestionSummaryTool {
    fn name(&self) -> &'static str {
        "question_summary"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Optional maximum number of Q&A pairs to include (default: all)"
                }
            },
            "required": []
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        // Parse arguments
        let request: QuestionSummaryRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::debug!("Loading question/answer summary");

        // Load all questions
        let mut entries = load_all_questions().map_err(|e| {
            McpError::internal_error(format!("Failed to load questions: {}", e), None)
        })?;

        // Apply limit if specified (take most recent N)
        if let Some(limit) = request.limit {
            if entries.len() > limit {
                // Take last N entries (most recent), but keep them sorted oldest to newest
                let start_index = entries.len() - limit;
                entries = entries.into_iter().skip(start_index).collect();
            }
        }

        let count = entries.len();
        tracing::info!("Retrieved {} question/answer pairs", count);

        // Build YAML summary
        let now = Utc::now();
        let mut summary = format!(
            "# Question/Answer History\n# Generated: {}\n# Total Q&A Pairs: {}\n\nentries:\n",
            now.to_rfc3339(),
            count
        );

        for entry in &entries {
            summary.push_str(&format!(
                "  - timestamp: \"{}\"\n    question: \"{}\"\n    answer: \"{}\"\n\n",
                entry.timestamp,
                entry.question.replace('"', "\\\""),
                entry.answer.replace('"', "\\\"")
            ));
        }

        // Return response
        Ok(BaseToolImpl::create_success_response(
            json!({
                "summary": summary,
                "count": count
            })
            .to_string(),
        ))
    }
}
