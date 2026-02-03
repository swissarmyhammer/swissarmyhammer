//! In-process CLI execution for testing
//!
//! This module provides a CLI executor that can run CLI commands without spawning
//! external processes, enabling fast, isolated integration tests.

use crate::dynamic_cli::CliBuilder;
use crate::mcp_integration::response_formatting;
use crate::mcp_integration::CliToolContext;
use serde_json::{Map, Value};
use std::error::Error;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Result of CLI command execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Standard output from the command
    pub stdout: String,
    /// Standard error from the command
    pub stderr: String,
    /// Exit code (0 = success)
    pub exit_code: i32,
}

impl ExecutionResult {
    /// Create a successful result
    pub fn success(stdout: String) -> Self {
        Self {
            stdout,
            stderr: String::new(),
            exit_code: 0,
        }
    }

    /// Create an error result
    pub fn error(stderr: String) -> Self {
        Self {
            stdout: String::new(),
            stderr,
            exit_code: 1,
        }
    }

    /// Check if the execution was successful
    pub fn is_success(&self) -> bool {
        self.exit_code == 0
    }
}

/// In-process CLI executor for testing
///
/// This executor allows running CLI commands without spawning external processes,
/// which makes tests faster and more isolated.
pub struct CliExecutor {
    cli_tool_context: Arc<CliToolContext>,
    tool_registry: Arc<RwLock<swissarmyhammer_tools::ToolRegistry>>,
}

impl CliExecutor {
    /// Create a new CLI executor with the given working directory
    pub async fn new(working_dir: &Path) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let cli_tool_context = CliToolContext::new_with_dir(working_dir)
            .await
            .map_err(|e| Box::<dyn Error + Send + Sync>::from(e.to_string()))?;
        let tool_registry = cli_tool_context.get_tool_registry_arc();

        Ok(Self {
            cli_tool_context: Arc::new(cli_tool_context),
            tool_registry,
        })
    }

    /// Execute CLI command and capture output
    ///
    /// # Arguments
    ///
    /// * `args` - Command line arguments (without the program name)
    ///
    /// # Returns
    ///
    /// ExecutionResult containing stdout, stderr, and exit code
    pub async fn execute(&self, args: &[&str]) -> ExecutionResult {
        // Build CLI with Clap
        let cli_builder = CliBuilder::new(self.tool_registry.clone());
        let cmd = cli_builder.build_cli_with_warnings(None);

        // Parse args with program name prepended
        let args_with_program: Vec<String> = std::iter::once("sah".to_string())
            .chain(args.iter().map(|s| s.to_string()))
            .collect();

        match cmd.try_get_matches_from(args_with_program) {
            Ok(matches) => self.dispatch_command(&matches).await,
            Err(e) => {
                // Handle clap errors (help, version, parse errors)
                match e.kind() {
                    clap::error::ErrorKind::DisplayHelp
                    | clap::error::ErrorKind::DisplayVersion => {
                        ExecutionResult::success(e.to_string())
                    }
                    _ => ExecutionResult::error(e.to_string()),
                }
            }
        }
    }

    /// Dispatch command based on parsed matches
    async fn dispatch_command(&self, matches: &clap::ArgMatches) -> ExecutionResult {
        // Route based on subcommand
        match matches.subcommand() {
            Some(("tool", tool_matches)) => {
                self.handle_tool_command(tool_matches).await
            }
            Some((cmd, _)) => {
                ExecutionResult::error(format!("Command '{}' not supported in test executor", cmd))
            }
            None => ExecutionResult::error("No command specified".to_string()),
        }
    }

    /// Handle tool subcommand
    async fn handle_tool_command(&self, matches: &clap::ArgMatches) -> ExecutionResult {
        match matches.subcommand() {
            Some((tool_name, tool_matches)) => {
                self.execute_tool(tool_name, tool_matches).await
            }
            None => ExecutionResult::error("No tool specified".to_string()),
        }
    }

    /// Execute a specific tool
    async fn execute_tool(&self, tool_name: &str, matches: &clap::ArgMatches) -> ExecutionResult {
        // Look up the tool
        let registry = self.tool_registry.read().await;
        let tool = match registry.get_tool(tool_name) {
            Some(t) => t,
            None => return ExecutionResult::error(format!("Tool not found: {}", tool_name)),
        };

        let operations = tool.operations();
        let schema = tool.schema();
        drop(registry);

        // Build arguments from matches
        let arguments = if !operations.is_empty() {
            // Operation-based tool with noun-grouped structure
            match self.extract_noun_verb_arguments(matches, &schema) {
                Ok(args) => args,
                Err(e) => return ExecutionResult::error(e),
            }
        } else {
            // Schema-based tool
            match self.extract_schema_arguments(matches, &schema) {
                Ok(args) => args,
                Err(e) => return ExecutionResult::error(e),
            }
        };

        // Execute the tool
        match self
            .cli_tool_context
            .execute_tool(tool_name, arguments)
            .await
        {
            Ok(result) => {
                if result.is_error.unwrap_or(false) {
                    ExecutionResult::error(response_formatting::format_error_response(&result))
                } else {
                    ExecutionResult::success(response_formatting::format_success_response(&result))
                }
            }
            Err(e) => ExecutionResult::error(format!("Tool execution error: {}", e)),
        }
    }

    /// Extract arguments for noun-verb structured operations
    fn extract_noun_verb_arguments(
        &self,
        matches: &clap::ArgMatches,
        schema: &Value,
    ) -> Result<Map<String, Value>, String> {
        match matches.subcommand() {
            Some((noun, noun_matches)) => match noun_matches.subcommand() {
                Some((verb, verb_matches)) => {
                    let op_string = format!("{} {}", verb, noun);
                    self.build_arguments_from_matches(verb_matches, &op_string, schema)
                }
                None => Err(format!("No verb specified for '{}'", noun)),
            },
            None => Err("No noun specified".to_string()),
        }
    }

    /// Extract arguments for schema-based tools
    fn extract_schema_arguments(
        &self,
        matches: &clap::ArgMatches,
        schema: &Value,
    ) -> Result<Map<String, Value>, String> {
        self.build_arguments_from_matches(matches, "", schema)
    }

    /// Build JSON arguments from clap matches
    fn build_arguments_from_matches(
        &self,
        matches: &clap::ArgMatches,
        op_string: &str,
        schema: &Value,
    ) -> Result<Map<String, Value>, String> {
        let mut arguments = Map::new();

        // Add op string if provided
        if !op_string.is_empty() {
            arguments.insert("op".to_string(), Value::String(op_string.to_string()));
        }

        // Extract properties from schema
        if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
            for (prop_name, prop_schema) in properties {
                if prop_name == "op" {
                    continue; // Already handled
                }

                if let Some(value) = self.extract_value_from_matches(matches, prop_name, prop_schema) {
                    arguments.insert(prop_name.clone(), value);
                }
            }
        }

        Ok(arguments)
    }

    /// Extract a value from clap matches based on schema type
    fn extract_value_from_matches(
        &self,
        matches: &clap::ArgMatches,
        name: &str,
        schema: &Value,
    ) -> Option<Value> {
        let type_str = schema.get("type").and_then(|t| t.as_str());

        match type_str {
            Some("boolean") => matches
                .get_flag(name)
                .then_some(Value::Bool(true)),
            Some("integer") => matches
                .get_one::<i64>(name)
                .map(|v| Value::Number((*v).into())),
            Some("number") => matches
                .get_one::<f64>(name)
                .and_then(|v| serde_json::Number::from_f64(*v))
                .map(Value::Number),
            Some("array") => {
                let values: Vec<String> = matches
                    .get_many::<String>(name)
                    .map(|v| v.cloned().collect())
                    .unwrap_or_default();
                if values.is_empty() {
                    None
                } else {
                    Some(Value::Array(values.into_iter().map(Value::String).collect()))
                }
            }
            _ => matches
                .get_one::<String>(name)
                .map(|v| Value::String(v.clone())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;

    #[tokio::test]
    async fn test_executor_creation() {
        let env = IsolatedTestEnvironment::new().unwrap();
        let executor = CliExecutor::new(&env.temp_dir()).await;
        assert!(executor.is_ok(), "Failed to create CliExecutor");
    }

    #[tokio::test]
    async fn test_help_command() {
        let env = IsolatedTestEnvironment::new().unwrap();
        let executor = CliExecutor::new(&env.temp_dir()).await.unwrap();

        let result = executor.execute(&["--help"]).await;
        assert!(result.is_success());
        // Help text contains the program name or description
        assert!(
            result.stdout.contains("coding assistant") || result.stdout.contains("sah"),
            "Help output should contain relevant content"
        );
    }
}
