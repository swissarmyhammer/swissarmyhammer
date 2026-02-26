//! Unified question operations tool for MCP
//!
//! This module provides a single `question` tool that dispatches between operations:
//! - `ask question`: Ask the user a question via MCP elicitation and persist the answer
//! - `summarize questions`: Retrieve all persisted question/answer pairs as a YAML summary
//!
//! Follows the Operation pattern from `swissarmyhammer-operations`.

pub mod ask;
mod persistence;
pub mod schema;
pub mod summary;

use crate::mcp::tool_registry::{McpTool, ToolContext, ToolRegistry};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use swissarmyhammer_common::health::{Doctorable, HealthCheck};
use swissarmyhammer_operations::Operation;

use ask::AskQuestion;
use summary::SummarizeQuestions;

// Static operation instances for schema generation
static ASK_QUESTION: Lazy<AskQuestion> = Lazy::new(AskQuestion::default);
static SUMMARIZE_QUESTIONS: Lazy<SummarizeQuestions> = Lazy::new(SummarizeQuestions::default);

static QUESTION_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| {
    vec![
        &*ASK_QUESTION as &dyn Operation,
        &*SUMMARIZE_QUESTIONS as &dyn Operation,
    ]
});

/// Unified question operations tool providing ask and summarize
#[derive(Default)]
pub struct QuestionTool;

impl QuestionTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for QuestionTool {
    fn name(&self) -> &'static str {
        "question"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        schema::generate_question_mcp_schema(&QUESTION_OPERATIONS)
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let op_str = arguments.get("op").and_then(|v| v.as_str()).unwrap_or("");

        // Strip the "op" key from arguments before passing to handlers
        let mut args = arguments.clone();
        args.remove("op");

        match op_str {
            "ask question" => ask::execute_ask(args, context).await,
            "summarize questions" => summary::execute_summary(args, context).await,
            "" => {
                // Infer operation from present keys
                if arguments.contains_key("question") {
                    ask::execute_ask(args, context).await
                } else {
                    summary::execute_summary(args, context).await
                }
            }
            other => Err(McpError::invalid_params(
                format!(
                    "Unknown operation '{}'. Valid operations: 'ask question', 'summarize questions'",
                    other
                ),
                None,
            )),
        }
    }
}

impl Doctorable for QuestionTool {
    fn name(&self) -> &str {
        "Question"
    }

    fn category(&self) -> &str {
        "tools"
    }

    fn run_health_checks(&self) -> Vec<HealthCheck> {
        Vec::new()
    }

    fn is_applicable(&self) -> bool {
        true
    }
}

/// Register the unified question tool with the registry
pub fn register_questions_tools(registry: &mut ToolRegistry) {
    registry.register(QuestionTool::new());
    tracing::debug!("Registered question tool");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_registry::ToolRegistry;

    #[test]
    fn test_question_tool_name() {
        let tool = QuestionTool::new();
        assert_eq!(<QuestionTool as McpTool>::name(&tool), "question");
    }

    #[test]
    fn test_question_tool_has_description() {
        let tool = QuestionTool::new();
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_question_tool_schema_has_op_field() {
        let tool = QuestionTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["op"].is_object());

        let op_enum = schema["properties"]["op"]["enum"]
            .as_array()
            .expect("op should have enum");
        assert!(op_enum.contains(&serde_json::json!("ask question")));
        assert!(op_enum.contains(&serde_json::json!("summarize questions")));
    }

    #[test]
    fn test_question_tool_schema_has_operation_schemas() {
        let tool = QuestionTool::new();
        let schema = tool.schema();

        let op_schemas = schema["x-operation-schemas"]
            .as_array()
            .expect("should have x-operation-schemas");
        assert_eq!(op_schemas.len(), 2);
    }

    #[test]
    fn test_question_tool_schema_has_all_parameters() {
        let tool = QuestionTool::new();
        let schema = tool.schema();

        let props = schema["properties"].as_object().unwrap();
        // Ask params
        assert!(props.contains_key("question"));
        // Summary params
        assert!(props.contains_key("limit"));
    }

    #[test]
    fn test_question_tool_schema_has_examples() {
        let tool = QuestionTool::new();
        let schema = tool.schema();

        assert!(schema["examples"].is_array());
        assert_eq!(schema["examples"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_register_questions_tools() {
        let mut registry = ToolRegistry::new();
        assert_eq!(registry.len(), 0);

        register_questions_tools(&mut registry);

        assert_eq!(registry.len(), 1);
        assert!(registry.get_tool("question").is_some());
    }

    #[tokio::test]
    async fn test_question_tool_unknown_op() {
        let tool = QuestionTool::new();
        let context = crate::test_utils::create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert(
            "op".to_string(),
            serde_json::Value::String("invalid op".to_string()),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown operation"));
    }

    #[tokio::test]
    async fn test_question_tool_infer_ask_from_question_key() {
        let tool = QuestionTool::new();
        let context = crate::test_utils::create_test_context().await;

        // When "question" key is present but no "op", should infer ask
        // This will fail because no peer is available, but the error message
        // tells us it dispatched to the ask handler
        let mut args = serde_json::Map::new();
        args.insert(
            "question".to_string(),
            serde_json::Value::String("test?".to_string()),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        // The error should be about elicitation, not about unknown operation
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("licitation"),
            "Expected elicitation error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_question_tool_infer_summary_from_empty() {
        let tool = QuestionTool::new();
        let context = crate::test_utils::create_test_context().await;

        // When no keys present and no "op", should infer summary
        let args = serde_json::Map::new();

        let result = tool.execute(args, &context).await;
        // Summary should succeed (returns empty list)
        assert!(result.is_ok());
    }
}
