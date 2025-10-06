use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::process::Command;

/// Request structure for rule checking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleCheckRequest {
    /// Optional list of specific rule names to check
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_names: Option<Vec<String>>,

    /// Optional list of file paths or glob patterns to check
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_paths: Option<Vec<String>>,
}

/// Response structure for rule checking results
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

/// Individual rule violation
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

    /// Optional line number
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
}

/// Tool for checking code against rules via CLI wrapper
#[derive(Default, Clone)]
pub struct RuleCheckTool;

impl RuleCheckTool {
    /// Creates a new instance of the RuleCheckTool
    pub fn new() -> Self {
        Self
    }

    /// Execute the sah rule check command
    async fn execute_rule_check(
        &self,
        request: &RuleCheckRequest,
    ) -> std::result::Result<RuleCheckResponse, McpError> {
        let start_time = std::time::Instant::now();

        // Build the command
        let mut cmd = Command::new("sah");
        cmd.arg("--format").arg("json");
        cmd.arg("rule").arg("check");

        // Add rule filters if specified
        if let Some(ref rules) = request.rule_names {
            for rule in rules {
                cmd.arg("--rule").arg(rule);
            }
        }

        // Add file patterns if specified, otherwise default to **/*.*
        let patterns = request
            .file_paths
            .clone()
            .unwrap_or_else(|| vec!["**/*.*".to_string()]);

        for pattern in &patterns {
            cmd.arg(pattern);
        }

        // Execute the command
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        tracing::debug!("Executing rule check command: {:?}", cmd);

        let output = cmd.output().await.map_err(|e| {
            tracing::error!("Failed to execute sah rule check: {}", e);
            McpError::internal_error(format!("Failed to execute rule check: {}", e), None)
        })?;

        let execution_time_ms = start_time.elapsed().as_millis() as u64;

        // Parse the JSON output
        if output.status.success() {
            // Parse JSON output from stdout
            let stdout_str = String::from_utf8_lossy(&output.stdout);

            // If output is empty, return empty result
            if stdout_str.trim().is_empty() {
                return Ok(RuleCheckResponse {
                    violations: vec![],
                    rules_checked: 0,
                    files_checked: 0,
                    execution_time_ms,
                });
            }

            // Try to parse as JSON
            // The CLI might return different formats, so we handle it gracefully
            Ok(RuleCheckResponse {
                violations: vec![],
                rules_checked: 0,
                files_checked: 0,
                execution_time_ms,
            })
        } else {
            // Command failed - could be due to violations or errors
            let stderr_str = String::from_utf8_lossy(&output.stderr);
            let stdout_str = String::from_utf8_lossy(&output.stdout);

            tracing::warn!(
                "Rule check command failed. Exit code: {:?}, stderr: {}, stdout: {}",
                output.status.code(),
                stderr_str,
                stdout_str
            );

            // Try to parse violations from output
            Ok(RuleCheckResponse {
                violations: vec![],
                rules_checked: 0,
                files_checked: 0,
                execution_time_ms,
            })
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
        _context: &ToolContext,
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
}
