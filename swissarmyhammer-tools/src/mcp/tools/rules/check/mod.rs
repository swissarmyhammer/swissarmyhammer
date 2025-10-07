//! Rule checking MCP tool that validates code against SwissArmyHammer rules.
//!
//! This tool provides an MCP interface to the SwissArmyHammer rule checking functionality.
//! It shells out to the CLI command `sah rule check` to perform the actual checking,
//! maintaining proper separation between the CLI and MCP layers.
//!
//! Note: This implementation uses the CLI rather than direct library integration due to
//! architectural constraints. The `swissarmyhammer-rules` crate requires an agent executor,
//! which creates a circular dependency if used directly from the tools crate (tools → rules →
//! agent-executor → tools). The CLI-based approach avoids this cycle while maintaining
//! full functionality.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::process::Command;

/// Request structure for rule checking operations via MCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleCheckRequest {
    /// Optional list of specific rule names to check
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_names: Option<Vec<String>>,

    /// Optional list of file paths or glob patterns to check
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_paths: Option<Vec<String>>,
}

/// Response structure for rule checking results from CLI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleCheckResponse {
    /// List of rule violations found
    pub violations: Vec<RuleViolation>,

    /// Number of rules checked
    pub rules_checked: usize,

    /// Number of files checked
    pub files_checked: usize,

    /// Execution time in milliseconds
    pub execution_time_ms: u64,
}

/// Individual rule violation details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleViolation {
    /// Name of the rule that was violated
    pub rule_name: String,

    /// Path to the file with the violation
    pub file_path: String,

    /// Severity level
    pub severity: String,

    /// Violation message
    pub message: String,

    /// Optional line number where the violation was found
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
}

/// Tool for checking code against rules via CLI invocation
///
/// This tool maintains separation between the CLI and MCP layers by invoking
/// the `sah rule check` command as a subprocess. This approach avoids circular
/// dependencies while providing full rule checking functionality through MCP.
#[derive(Default, Clone)]
pub struct RuleCheckTool;

impl RuleCheckTool {
    /// Creates a new instance of the RuleCheckTool
    pub fn new() -> Self {
        Self
    }

    /// Execute rule check by invoking the CLI command and parsing JSON output
    async fn execute_rule_check(
        &self,
        request: &RuleCheckRequest,
    ) -> std::result::Result<RuleCheckResponse, McpError> {
        // Build the CLI command
        let mut cmd = Command::new("sah");
        cmd.arg("--format").arg("json").arg("rule").arg("check");

        // Add rule name filters if provided
        if let Some(ref rule_names) = request.rule_names {
            for rule_name in rule_names {
                cmd.arg("--rule").arg(rule_name);
            }
        }

        // Add file path patterns if provided
        if let Some(ref file_paths) = request.file_paths {
            for file_path in file_paths {
                cmd.arg(file_path);
            }
        }

        // Configure command execution
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        tracing::debug!("Executing CLI command: {:?}", cmd);

        // Execute the command
        let output = cmd.output().await.map_err(|e| {
            McpError::internal_error(
                format!(
                    "Failed to execute sah command: {}. Ensure sah is in PATH.",
                    e
                ),
                None,
            )
        })?;

        // Parse stdout as JSON
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        tracing::debug!("CLI stdout: {}", stdout);
        tracing::debug!("CLI stderr: {}", stderr);

        // Try to parse the JSON output
        if !stdout.is_empty() {
            match serde_json::from_str::<RuleCheckResponse>(&stdout) {
                Ok(response) => Ok(response),
                Err(parse_err) => {
                    tracing::error!("Failed to parse CLI JSON output: {}", parse_err);
                    tracing::error!("Raw output: {}", stdout);

                    // Return a helpful error
                    Err(McpError::internal_error(
                        format!(
                            "Failed to parse rule check output: {}. Raw output: {}",
                            parse_err, stdout
                        ),
                        None,
                    ))
                }
            }
        } else {
            // No JSON output, likely an error
            Err(McpError::internal_error(
                format!(
                    "Rule check command failed with exit code {}. Error: {}",
                    output.status.code().unwrap_or(-1),
                    stderr
                ),
                None,
            ))
        }
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
        _context: &ToolContext, // Not used - tool operates independently
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: RuleCheckRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::debug!("Executing rule check with request: {:?}", request);

        // Execute the rule check via CLI
        let response = self.execute_rule_check(&request).await?;

        // Format the response
        let result_text = if response.violations.is_empty() {
            format!(
                "✅ No rule violations found\n\nChecked {} rules against {} files in {}ms",
                response.rules_checked, response.files_checked, response.execution_time_ms
            )
        } else {
            let violations_text = response
                .violations
                .iter()
                .map(|v| {
                    format!(
                        "❌ {} [{}] in {}{}\n   {}",
                        v.rule_name,
                        v.severity,
                        v.file_path,
                        v.line.map(|l| format!(":{}", l)).unwrap_or_default(),
                        v.message
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");

            format!(
                "Found {} violation(s) in {} files ({}ms)\n\n{}",
                response.violations.len(),
                response.files_checked,
                response.execution_time_ms,
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

    #[tokio::test]
    async fn test_rule_check_tool_name() {
        let tool = RuleCheckTool::new();
        assert_eq!(tool.name(), "rules_check");
    }

    #[tokio::test]
    async fn test_rule_check_tool_schema() {
        let tool = RuleCheckTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["rule_names"].is_object());
        assert!(schema["properties"]["file_paths"].is_object());
    }

    #[tokio::test]
    async fn test_rule_check_request_parsing() {
        let args = json!({
            "rule_names": ["no-unwrap", "no-panic"],
            "file_paths": ["src/**/*.rs"]
        });

        let request: RuleCheckRequest = serde_json::from_value(args).unwrap();
        assert_eq!(request.rule_names.unwrap(), vec!["no-unwrap", "no-panic"]);
        assert_eq!(request.file_paths.unwrap(), vec!["src/**/*.rs"]);
    }

    #[tokio::test]
    async fn test_rule_check_request_optional_fields() {
        let args = json!({});

        let request: RuleCheckRequest = serde_json::from_value(args).unwrap();
        assert!(request.rule_names.is_none());
        assert!(request.file_paths.is_none());
    }

    #[tokio::test]
    async fn test_rule_check_response_parsing() {
        // Test parsing a response with violations
        let json_response = json!({
            "violations": [
                {
                    "rule_name": "no-unwrap",
                    "file_path": "src/main.rs",
                    "severity": "error",
                    "message": "Found unwrap() call",
                    "line": 42
                }
            ],
            "rules_checked": 5,
            "files_checked": 10,
            "execution_time_ms": 150
        });

        let response: RuleCheckResponse = serde_json::from_value(json_response).unwrap();
        assert_eq!(response.violations.len(), 1);
        assert_eq!(response.violations[0].rule_name, "no-unwrap");
        assert_eq!(response.violations[0].file_path, "src/main.rs");
        assert_eq!(response.violations[0].severity, "error");
        assert_eq!(response.violations[0].message, "Found unwrap() call");
        assert_eq!(response.violations[0].line, Some(42));
        assert_eq!(response.rules_checked, 5);
        assert_eq!(response.files_checked, 10);
        assert_eq!(response.execution_time_ms, 150);
    }

    #[tokio::test]
    async fn test_rule_check_response_no_violations() {
        // Test parsing a response with no violations
        let json_response = json!({
            "violations": [],
            "rules_checked": 5,
            "files_checked": 10,
            "execution_time_ms": 120
        });

        let response: RuleCheckResponse = serde_json::from_value(json_response).unwrap();
        assert!(response.violations.is_empty());
        assert_eq!(response.rules_checked, 5);
        assert_eq!(response.files_checked, 10);
        assert_eq!(response.execution_time_ms, 120);
    }

    #[tokio::test]
    async fn test_violation_without_line_number() {
        // Test parsing violations without line numbers
        let json_response = json!({
            "violations": [
                {
                    "rule_name": "missing-docs",
                    "file_path": "src/lib.rs",
                    "severity": "warning",
                    "message": "Missing documentation"
                }
            ],
            "rules_checked": 1,
            "files_checked": 1,
            "execution_time_ms": 50
        });

        let response: RuleCheckResponse = serde_json::from_value(json_response).unwrap();
        assert_eq!(response.violations.len(), 1);
        assert_eq!(response.violations[0].line, None);
    }
}
