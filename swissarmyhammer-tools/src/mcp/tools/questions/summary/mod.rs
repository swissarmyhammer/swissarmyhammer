use crate::mcp::tool_registry::{BaseToolImpl, ToolContext};
use crate::mcp::tools::questions::persistence::load_all_questions;
use chrono::Utc;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};

/// Operation metadata for summarizing questions
#[derive(Debug, Default)]
pub struct SummarizeQuestions;

static SUMMARIZE_QUESTIONS_PARAMS: &[ParamMeta] = &[ParamMeta::new("limit")
    .description("Optional maximum number of Q&A pairs to include (default: all)")
    .param_type(ParamType::Integer)];

impl Operation for SummarizeQuestions {
    fn verb(&self) -> &'static str {
        "summarize"
    }
    fn noun(&self) -> &'static str {
        "questions"
    }
    fn description(&self) -> &'static str {
        "Retrieve all persisted question/answer pairs as a YAML summary"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SUMMARIZE_QUESTIONS_PARAMS
    }
}

/// Request structure for summarize questions operation
#[derive(Debug, Deserialize, Serialize)]
pub struct QuestionSummaryRequest {
    /// Optional limit on number of entries to return (default: all)
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Execute a summarize questions operation
pub async fn execute_summary(
    arguments: serde_json::Map<String, serde_json::Value>,
    _context: &ToolContext,
) -> std::result::Result<CallToolResult, McpError> {
    // Parse arguments
    let request: QuestionSummaryRequest = BaseToolImpl::parse_arguments(arguments)?;

    tracing::debug!("Loading question/answer summary");

    // Load all questions
    let mut entries = load_all_questions()
        .map_err(|e| McpError::internal_error(format!("Failed to load questions: {}", e), None))?;

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
