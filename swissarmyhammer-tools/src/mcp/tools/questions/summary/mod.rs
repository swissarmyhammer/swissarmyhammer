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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::questions::persistence::save_question_answer;
    use serial_test::serial;
    use tempfile::TempDir;

    /// RAII guard to restore working directory when dropped
    struct DirGuard(std::path::PathBuf);
    impl Drop for DirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }

    fn setup_test_env() -> (TempDir, DirGuard) {
        let original_dir = std::env::current_dir().unwrap();
        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();
        (temp_dir, DirGuard(original_dir))
    }

    #[test]
    fn test_summarize_questions_operation_metadata() {
        let op = SummarizeQuestions;
        assert_eq!(op.verb(), "summarize");
        assert_eq!(op.noun(), "questions");
        assert_eq!(op.op_string(), "summarize questions");
        assert!(!op.description().is_empty());
        assert_eq!(op.parameters().len(), 1);
    }

    #[tokio::test]
    #[serial(cwd)]
    async fn test_execute_summary_empty() {
        let (_temp, _guard) = setup_test_env();
        let ctx = crate::test_utils::create_test_context().await;

        let args = serde_json::Map::new();
        let result = execute_summary(args, &ctx).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        let content = response
            .content
            .first()
            .and_then(|c| c.raw.as_text())
            .map(|t| t.text.as_str())
            .unwrap_or("");
        let json: serde_json::Value = serde_json::from_str(content).unwrap();
        assert_eq!(json["count"], 0);
        assert!(json["summary"]
            .as_str()
            .unwrap()
            .contains("Total Q&A Pairs: 0"));
    }

    #[tokio::test]
    #[serial(cwd)]
    async fn test_execute_summary_with_questions() {
        let (_temp, _guard) = setup_test_env();
        let ctx = crate::test_utils::create_test_context().await;

        // Save some questions
        save_question_answer("Question 1?", "Answer 1").unwrap();
        save_question_answer("Question 2?", "Answer 2").unwrap();

        let args = serde_json::Map::new();
        let result = execute_summary(args, &ctx).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        let content = response
            .content
            .first()
            .and_then(|c| c.raw.as_text())
            .map(|t| t.text.as_str())
            .unwrap_or("");
        let json: serde_json::Value = serde_json::from_str(content).unwrap();
        assert_eq!(json["count"], 2);
        let summary = json["summary"].as_str().unwrap();
        assert!(summary.contains("Question 1?"));
        assert!(summary.contains("Answer 1"));
        assert!(summary.contains("Question 2?"));
        assert!(summary.contains("Answer 2"));
    }

    #[tokio::test]
    #[serial(cwd)]
    async fn test_execute_summary_with_limit() {
        let (_temp, _guard) = setup_test_env();
        let ctx = crate::test_utils::create_test_context().await;

        // Save 3 questions
        save_question_answer("Q1?", "A1").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        save_question_answer("Q2?", "A2").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        save_question_answer("Q3?", "A3").unwrap();

        // Request only last 2
        let mut args = serde_json::Map::new();
        args.insert("limit".to_string(), serde_json::json!(2));
        let result = execute_summary(args, &ctx).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        let content = response
            .content
            .first()
            .and_then(|c| c.raw.as_text())
            .map(|t| t.text.as_str())
            .unwrap_or("");
        let json: serde_json::Value = serde_json::from_str(content).unwrap();
        // Only 2 entries should be returned (most recent)
        assert_eq!(json["count"], 2);
        let summary = json["summary"].as_str().unwrap();
        // Q2 and Q3 should be in summary, Q1 should not
        assert!(summary.contains("Q2?"));
        assert!(summary.contains("Q3?"));
        assert!(!summary.contains("Q1?"));
    }

    #[tokio::test]
    #[serial(cwd)]
    async fn test_execute_summary_limit_larger_than_total() {
        let (_temp, _guard) = setup_test_env();
        let ctx = crate::test_utils::create_test_context().await;

        save_question_answer("Only question?", "Only answer").unwrap();

        // Request more than available
        let mut args = serde_json::Map::new();
        args.insert("limit".to_string(), serde_json::json!(100));
        let result = execute_summary(args, &ctx).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        let content = response
            .content
            .first()
            .and_then(|c| c.raw.as_text())
            .map(|t| t.text.as_str())
            .unwrap_or("");
        let json: serde_json::Value = serde_json::from_str(content).unwrap();
        // Should return all available (only 1)
        assert_eq!(json["count"], 1);
    }

    #[test]
    fn test_question_summary_request_defaults() {
        let req: QuestionSummaryRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(req.limit, None);
    }

    #[test]
    fn test_question_summary_request_with_limit() {
        let req: QuestionSummaryRequest = serde_json::from_str(r#"{"limit": 5}"#).unwrap();
        assert_eq!(req.limit, Some(5));
    }
}
