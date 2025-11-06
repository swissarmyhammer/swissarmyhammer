use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::tools::questions::persistence::save_question_answer;
use async_trait::async_trait;
use rmcp::model::{CallToolResult, CreateElicitationRequestParam, ElicitationSchema};
use rmcp::ErrorData as McpError;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Request structure for question_ask tool
#[derive(Debug, Deserialize, Serialize)]
pub struct QuestionAskRequest {
    /// The question to ask the user
    pub question: String,
}

/// MCP tool for asking users questions via elicitation
#[derive(Default)]
pub struct QuestionAskTool;

impl QuestionAskTool {
    /// Creates a new instance of the QuestionAskTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for QuestionAskTool {
    fn name(&self) -> &'static str {
        "question_ask"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The question to ask the user"
                }
            },
            "required": ["question"]
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        // Parse arguments
        let request: QuestionAskRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::debug!("Asking user question via elicitation");

        // Validate question
        McpValidation::validate_not_empty(&request.question, "question")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate question"))?;

        // Check if peer is available for elicitation
        let peer = context.peer.as_ref().ok_or_else(|| {
            McpError::invalid_request(
                "Elicitation not available. This tool requires MCP client support for elicitation (MCP protocol 2025-06-18 or later).",
                None
            )
        })?;

        // Create elicitation schema - simple string input
        let question_text = request.question.clone();
        let elicitation_schema = ElicitationSchema::builder()
            .required_string_with("answer", move |s| s.description(question_text))
            .build_unchecked();

        // Send elicitation request to client
        tracing::info!("Sending elicitation request: {}", request.question);
        let elicitation_request = CreateElicitationRequestParam {
            message: request.question.clone(),
            requested_schema: elicitation_schema,
        };

        // This blocks until the user responds
        let elicitation_result =
            peer.create_elicitation(elicitation_request)
                .await
                .map_err(|e| {
                    McpError::internal_error(format!("Elicitation request failed: {}", e), None)
                })?;

        // Check if user accepted, declined, or cancelled
        match elicitation_result.action {
            rmcp::model::ElicitationAction::Accept => {
                // Extract answer from response
                let answer = elicitation_result
                    .content
                    .as_ref()
                    .and_then(|content| content.get("answer"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        McpError::invalid_request(
                            "No answer provided in elicitation response",
                            None,
                        )
                    })?;

                tracing::info!("User answered: {}", answer);

                // Save question/answer to file
                let file_path = save_question_answer(&request.question, answer).map_err(|e| {
                    McpError::internal_error(format!("Failed to save question/answer: {}", e), None)
                })?;

                // Return success response
                Ok(BaseToolImpl::create_success_response(
                    json!({
                        "answer": answer,
                        "saved_to": file_path.display().to_string()
                    })
                    .to_string(),
                ))
            }
            rmcp::model::ElicitationAction::Decline | rmcp::model::ElicitationAction::Cancel => {
                Err(McpError::invalid_request(
                    "User declined or cancelled the question",
                    None,
                ))
            }
        }
    }
}
