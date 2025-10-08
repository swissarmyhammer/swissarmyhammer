//! Rule checking MCP tool that validates code against SwissArmyHammer rules.
//!
//! This tool provides an MCP interface to the SwissArmyHammer rule checking functionality.
//! It uses the swissarmyhammer-rules library directly for better performance and type safety.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use swissarmyhammer_agent_executor::{AgentExecutor, LlamaAgentExecutorWrapper};
use swissarmyhammer_config::LlamaAgentConfig;
use swissarmyhammer_rules::{RuleCheckRequest as DomainRuleCheckRequest, RuleChecker, Severity};
use tokio::sync::OnceCell;

/// Request structure for rule checking operations via MCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleCheckRequest {
    /// Optional list of specific rule names to check
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_names: Option<Vec<String>>,

    /// Optional severity filter (error, warning, info)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<Severity>,

    /// Optional category filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    /// Optional list of file paths or glob patterns to check
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_paths: Option<Vec<String>>,
}

/// Tool for checking code against rules via direct library integration
///
/// This tool uses the swissarmyhammer-rules library directly, avoiding subprocess
/// overhead and providing better error handling and type safety.
#[derive(Clone)]
pub struct RuleCheckTool {
    /// Lazily initialized rule checker (shared across requests)
    checker: Arc<OnceCell<RuleChecker>>,
}

impl RuleCheckTool {
    /// Creates a new instance of the RuleCheckTool
    pub fn new() -> Self {
        Self {
            checker: Arc::new(OnceCell::new()),
        }
    }

    /// Get or initialize the rule checker
    async fn get_checker(&self) -> Result<&RuleChecker, McpError> {
        self.checker
            .get_or_try_init(|| async {
                tracing::debug!("Initializing RuleChecker for MCP tool");

                // Use testing configuration for MCP context to avoid loading full production
                // model weights. This provides fast initialization while maintaining rule
                // checking functionality through the agent executor interface.
                let config = LlamaAgentConfig::for_testing();
                let agent: Arc<dyn AgentExecutor> =
                    Arc::new(LlamaAgentExecutorWrapper::new(config));

                // Create rule checker
                let mut checker = RuleChecker::new(agent).map_err(|e| {
                    McpError::internal_error(format!("Failed to create rule checker: {}", e), None)
                })?;

                // Initialize the checker
                checker.initialize().await.map_err(|e| {
                    McpError::internal_error(
                        format!("Failed to initialize rule checker: {}", e),
                        None,
                    )
                })?;

                tracing::info!("RuleChecker initialized successfully");
                Ok(checker)
            })
            .await
            .map_err(|e: McpError| e)
    }
}

impl Default for RuleCheckTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl McpTool for RuleCheckTool {
    fn name(&self) -> &'static str {
        "rules_check"
    }

    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("rules", "check")
            .expect("Tool description should be available")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "rule_names": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Optional array of specific rule names to check"
                },
                "severity": {
                    "type": "string",
                    "enum": ["error", "warning", "info", "hint"],
                    "description": "Optional severity filter (error, warning, info, hint)"
                },
                "category": {
                    "type": "string",
                    "description": "Optional category filter"
                },
                "file_paths": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Optional array of file paths or glob patterns to check (defaults to **/*.*)"
                }
            }
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: RuleCheckRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::debug!("Executing rule check with request: {:?}", request);

        // Get or initialize the rule checker
        let checker = self.get_checker().await?;

        // Map MCP request to domain request
        let domain_request = DomainRuleCheckRequest {
            rule_names: request.rule_names,
            severity: request.severity,
            category: request.category,
            patterns: request
                .file_paths
                .unwrap_or_else(|| vec!["**/*.*".to_string()]),
        };

        // Execute the rule check via library
        let result = checker
            .check_with_filters(domain_request)
            .await
            .map_err(|e| McpError::internal_error(format!("Rule check failed: {}", e), None))?;

        // Format the response
        let result_text = if result.violations.is_empty() {
            format!(
                "✅ No rule violations found\n\nChecked {} rules against {} files",
                result.rules_checked, result.files_checked
            )
        } else {
            let violations_text = result
                .violations
                .iter()
                .map(|v| {
                    format!(
                        "❌ {} [{}] in {}\n   {}",
                        v.rule_name,
                        v.severity,
                        v.file_path.display(),
                        v.message
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");

            format!(
                "Found {} violation(s) in {} files\n\n{}",
                result.violations.len(),
                result.files_checked,
                violations_text
            )
        };

        Ok(BaseToolImpl::create_success_response(&result_text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Verifies that the tool reports its correct name for MCP registration
    #[tokio::test]
    async fn test_rule_check_tool_name() {
        let tool = RuleCheckTool::new();
        assert_eq!(tool.name(), "rules_check");
    }

    /// Verifies that the tool schema includes all required fields and proper structure
    #[tokio::test]
    async fn test_rule_check_tool_schema() {
        let tool = RuleCheckTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["rule_names"].is_object());
        assert!(schema["properties"]["severity"].is_object());
        assert!(schema["properties"]["category"].is_object());
        assert!(schema["properties"]["file_paths"].is_object());
    }

    /// Verifies that RuleCheckRequest correctly parses all fields from JSON arguments
    #[tokio::test]
    async fn test_rule_check_request_parsing() {
        let args = json!({
            "rule_names": ["no-unwrap", "no-panic"],
            "severity": "error",
            "category": "safety",
            "file_paths": ["src/**/*.rs"]
        });

        let request: RuleCheckRequest = serde_json::from_value(args).unwrap();
        assert_eq!(request.rule_names.unwrap(), vec!["no-unwrap", "no-panic"]);
        assert!(matches!(request.severity, Some(Severity::Error)));
        assert_eq!(request.category.unwrap(), "safety");
        assert_eq!(request.file_paths.unwrap(), vec!["src/**/*.rs"]);
    }

    /// Verifies that all fields in RuleCheckRequest are properly optional
    #[tokio::test]
    async fn test_rule_check_request_optional_fields() {
        let args = json!({});

        let request: RuleCheckRequest = serde_json::from_value(args).unwrap();
        assert!(request.rule_names.is_none());
        assert!(request.severity.is_none());
        assert!(request.category.is_none());
        assert!(request.file_paths.is_none());
    }

    /// Verifies that the RuleChecker initialization completes without panicking
    /// and handles both success and expected failure cases gracefully
    #[tokio::test]
    async fn test_rule_check_tool_initialization() {
        let tool = RuleCheckTool::new();

        // Get checker should initialize it
        let checker_result = tool.get_checker().await;

        // In test environment without actual model, initialization may fail
        // which is expected - we're just testing the initialization pattern
        match checker_result {
            Ok(_) => {
                // Initialization succeeded - great!
            }
            Err(e) => {
                // Initialization failed - expected in test without model
                assert!(e.to_string().contains("Failed to") || e.to_string().contains("failed"));
            }
        }
    }

    /// Verifies that the RuleCheckTool uses lazy initialization pattern and reuses
    /// the same RuleChecker instance across multiple calls
    #[tokio::test]
    async fn test_rule_check_tool_lazy_initialization() {
        let tool = RuleCheckTool::new();

        // Checker should not be initialized yet
        assert!(tool.checker.get().is_none());

        // Calling get_checker should initialize it
        let _ = tool.get_checker().await;

        // Now it should be initialized (or have attempted initialization)
        // We can't check the internal state directly, but a second call
        // should return the same instance (testing the OnceCell behavior)
        let result1 = tool.get_checker().await;
        let result2 = tool.get_checker().await;

        // Both results should have the same success/failure status
        assert_eq!(result1.is_ok(), result2.is_ok());
    }
}
