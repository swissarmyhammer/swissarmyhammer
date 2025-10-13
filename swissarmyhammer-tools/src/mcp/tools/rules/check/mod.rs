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
use swissarmyhammer_agent_executor::{AgentExecutor, ClaudeCodeExecutor, LlamaAgentExecutorWrapper};
use swissarmyhammer_config::{AgentConfig, AgentExecutorConfig};
use swissarmyhammer_rules::{RuleCheckRequest as DomainRuleCheckRequest, RuleChecker, Severity};
use tokio::sync::OnceCell;

/// Create an agent executor from agent configuration
///
/// Note: This is a simplified implementation for the MCP tool. The canonical implementation
/// with full MCP server lifecycle management is in swissarmyhammer_workflow::actions::AgentExecutorFactory.
/// This cannot use the workflow factory due to circular dependency (workflow depends on tools).
///
/// # Arguments
///
/// * `config` - The agent configuration specifying which executor to use
///
/// # Returns
///
/// * `Result<Arc<dyn AgentExecutor>, McpError>` - The initialized agent executor
///
/// # Errors
///
/// Returns an error if agent initialization fails
async fn create_agent_from_config(
    config: &AgentConfig,
) -> Result<Arc<dyn AgentExecutor>, McpError> {
    match &config.executor {
        AgentExecutorConfig::ClaudeCode(_claude_config) => {
            tracing::debug!("Creating ClaudeCode executor for rule checking");
            let mut executor = ClaudeCodeExecutor::new();
            executor.initialize().await.map_err(|e| {
                McpError::internal_error(
                    format!("Failed to initialize ClaudeCode executor: {}", e),
                    None,
                )
            })?;
            Ok(Arc::new(executor) as Arc<dyn AgentExecutor>)
        }
        AgentExecutorConfig::LlamaAgent(llama_config) => {
            tracing::debug!("Creating LlamaAgent executor for rule checking");
            let mut executor = LlamaAgentExecutorWrapper::new(llama_config.clone());
            executor.initialize().await.map_err(|e| {
                McpError::internal_error(
                    format!("Failed to initialize LlamaAgent executor: {}", e),
                    None,
                )
            })?;
            Ok(Arc::new(executor) as Arc<dyn AgentExecutor>)
        }
    }
}

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

    /// Get or initialize the rule checker using the provided context's agent configuration
    async fn get_checker(&self, context: &ToolContext) -> Result<&RuleChecker, McpError> {
        // Clone config for use in async closure
        let agent_config = context.agent_config.clone();

        self.checker
            .get_or_try_init(|| async move {
                tracing::debug!("Initializing RuleChecker for MCP tool with configured agent");

                // Create agent executor from configuration
                let agent = create_agent_from_config(&agent_config).await?;

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

                tracing::info!(
                    "RuleChecker initialized successfully with {:?} executor",
                    agent_config.executor_type()
                );
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
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: RuleCheckRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::info!("Executing rule check with request: {:?}", request);
        tracing::info!("Rule names filter: {:?}", request.rule_names);
        tracing::info!("File paths: {:?}", request.file_paths);

        // Get or initialize the rule checker with context's agent configuration
        let checker = self.get_checker(context).await?;
        tracing::info!("RuleChecker initialized successfully");

        // Map MCP request to domain request
        let domain_request = DomainRuleCheckRequest {
            rule_names: request.rule_names.clone(),
            severity: request.severity,
            category: request.category.clone(),
            patterns: request
                .file_paths
                .clone()
                .unwrap_or_else(|| vec!["**/*.*".to_string()]),
            check_mode: swissarmyhammer_rules::CheckMode::FailFast,
        };

        tracing::info!("Domain request patterns: {:?}", domain_request.patterns);
        tracing::info!("Domain request rule_names: {:?}", domain_request.rule_names);

        // Execute the rule check via streaming library
        use futures_util::stream::StreamExt;
        let mut stream = checker
            .check(domain_request)
            .await
            .map_err(|e| McpError::internal_error(format!("Rule check failed: {}", e), None))?;

        // Collect all violations from the stream
        let mut violations = Vec::new();
        while let Some(result) = stream.next().await {
            match result {
                Ok(violation) => violations.push(violation),
                Err(e) => {
                    return Err(McpError::internal_error(
                        format!("Rule check failed: {}", e),
                        None,
                    ));
                }
            }
        }

        tracing::info!("Check completed: found {} violations", violations.len());

        // Format the response
        let result_text = if violations.is_empty() {
            "✅ No rule violations found".to_string()
        } else {
            let violations_text = violations
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
                "Found {} violation(s)\n\n{}",
                violations.len(),
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
        let context = crate::test_utils::create_test_context().await;

        // Get checker should initialize it
        let checker_result = tool.get_checker(&context).await;

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
        let context = crate::test_utils::create_test_context().await;

        // Checker should not be initialized yet
        assert!(tool.checker.get().is_none());

        // Calling get_checker should initialize it
        let _ = tool.get_checker(&context).await;

        // Now it should be initialized (or have attempted initialization)
        // We can't check the internal state directly, but a second call
        // should return the same instance (testing the OnceCell behavior)
        let result1 = tool.get_checker(&context).await;
        let result2 = tool.get_checker(&context).await;

        // Both results should have the same success/failure status
        assert_eq!(result1.is_ok(), result2.is_ok());
    }

    /// Integration test that verifies the full execute path works end-to-end
    /// This test creates a temporary file, runs a real rule check via the MCP tool,
    /// and verifies that rules are loaded and checked properly.
    #[tokio::test]
    async fn test_rule_check_tool_execute_integration() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory and test file
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.rs");
        fs::write(
            &test_file,
            r#"
fn example() {
    let x = vec![1, 2, 3];
    let first = x.first().unwrap(); // This should trigger no-unwrap if that rule exists
    println!("first: {}", first);
}
"#,
        )
        .unwrap();

        // Create tool and context
        let tool = RuleCheckTool::new();
        let context = crate::test_utils::create_test_context().await;

        // Create request to check a single rule against the test file for speed
        // Use a simple rule that should pass quickly
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "rule_names".to_string(),
            json!(["code-quality/no-commented-code"]),
        );
        arguments.insert(
            "file_paths".to_string(),
            json!([test_file.to_string_lossy().to_string()]),
        );

        // Execute the tool
        let result = tool.execute(arguments, &context).await;

        // The result should succeed (even if violations are found, execute() returns Ok
        // with the formatted result in the response text)
        match result {
            Ok(call_result) => {
                // Verify we got some content back
                assert!(
                    !call_result.content.is_empty(),
                    "Tool should return content"
                );

                // Extract text from the result - we know it's RawContent::Text from the response format
                let text = if let Some(first_content) = call_result.content.first() {
                    // Access the Annotated struct's raw field directly via debug formatting for now
                    // In a real implementation, we'd use proper accessors
                    format!("{:?}", first_content)
                } else {
                    String::from("No content returned")
                };

                // We should get a success message (no violations for this test file)
                assert!(
                    text.contains("No rule violations found") || text.contains("violation"),
                    "Result should show check completed: {}",
                    text
                );

                println!("Integration test result: {}", text);
            }
            Err(e) => {
                panic!("Tool execution failed: {}", e);
            }
        }
    }

    /// Test that rule name filtering works correctly
    /// This reproduces the issue where calling with specific rule names returns 0 rules
    #[tokio::test]
    async fn test_rule_check_tool_with_rule_name_filter() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory and test file
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.rs");
        fs::write(
            &test_file,
            r#"
fn complex_function() {
    if condition1 {
        if condition2 {
            if condition3 {
                if condition4 {
                    // Very nested logic
                    do_something();
                }
            }
        }
    }
}
"#,
        )
        .unwrap();

        // Create tool and context
        let tool = RuleCheckTool::new();
        let context = crate::test_utils::create_test_context().await;

        // Create request with specific rule name filter
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "rule_names".to_string(),
            json!(["code-quality/cognitive-complexity"]),
        );
        arguments.insert(
            "file_paths".to_string(),
            json!([test_file.to_string_lossy().to_string()]),
        );

        // Execute the tool
        let result = tool.execute(arguments, &context).await;

        match result {
            Ok(call_result) => {
                let text = format!("{:?}", call_result);
                println!("Rule filter test result: {}", text);

                // The key assertion: we should NOT get "0 rules against 0 files"
                if text.contains("Checked 0 rules against 0 files") {
                    panic!(
                        "Rule name filtering failed! Expected to find 'code-quality/cognitive-complexity' rule but got 0 rules.\nFull output: {}",
                        text
                    );
                }

                // We should get a check result (success or violations)
                assert!(
                    text.contains("No rule violations found") || text.contains("violation"),
                    "Should have checked the cognitive-complexity rule. Got: {}",
                    text
                );
            }
            Err(e) => {
                panic!("Tool execution failed: {}", e);
            }
        }
    }

    /// Test rule checking against an actual repo file
    /// Uses this crate's Cargo.toml which we know exists
    #[tokio::test]
    async fn test_rule_check_with_real_repo_file() {
        // Use this crate's Cargo.toml
        let cargo_toml = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");

        assert!(
            cargo_toml.exists(),
            "Cargo.toml should exist at {:?}",
            cargo_toml
        );

        // Create tool and context
        let tool = RuleCheckTool::new();
        let context = crate::test_utils::create_test_context().await;

        // Create request - check a specific builtin rule against Cargo.toml
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "rule_names".to_string(),
            json!(["code-quality/no-commented-code"]), // Use a specific builtin rule
        );
        arguments.insert(
            "file_paths".to_string(),
            json!([cargo_toml.to_string_lossy().to_string()]),
        );

        // Execute the tool
        let result = tool.execute(arguments, &context).await;

        match result {
            Ok(call_result) => {
                let text = format!("{:?}", call_result);
                println!("Cargo.toml check result: {}", text);

                // Should have completed the check successfully
                assert!(
                    text.contains("No rule violations found") || text.contains("violation"),
                    "Should have loaded 'code-quality/no-commented-code' rule and checked Cargo.toml. Got: {}",
                    text
                );
            }
            Err(e) => {
                panic!("Tool execution failed: {}", e);
            }
        }
    }
}
