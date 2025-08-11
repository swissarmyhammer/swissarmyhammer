//! Workflow action execution system
//!
//! This module provides the action execution infrastructure for workflows,
//! including Claude integration, variable operations, and control flow actions.

use crate::sah_config;
use crate::workflow::action_parser::ActionParser;
use crate::workflow::{WorkflowExecutor, WorkflowName, WorkflowRunStatus, WorkflowStorage};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::as_24_bit_terminal_escaped;
use thiserror::Error;
use tokio::process::Command;
use tokio::time::timeout;

/// Global test storage registry for tests
static TEST_STORAGE_REGISTRY: RwLock<Option<Arc<WorkflowStorage>>> = RwLock::new(None);

/// Set test storage for use in tests
#[cfg(test)]
pub fn set_test_storage(storage: Arc<WorkflowStorage>) {
    *TEST_STORAGE_REGISTRY.write().unwrap() = Some(storage);
}

/// Clear test storage after tests
#[cfg(test)]
pub fn clear_test_storage() {
    *TEST_STORAGE_REGISTRY.write().unwrap() = None;
}

/// Get test storage if available
fn get_test_storage() -> Option<Arc<WorkflowStorage>> {
    TEST_STORAGE_REGISTRY.read().unwrap().clone()
}

/// Macro to implement the as_any() method for Action trait implementations
macro_rules! impl_as_any {
    () => {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    };
}

/// Errors that can occur during action execution
#[derive(Debug, Error)]
pub enum ActionError {
    /// Claude command execution failed
    #[error("Claude execution failed: {0}")]
    ClaudeError(String),
    /// Variable operation failed
    #[error("Variable operation failed: {0}")]
    VariableError(String),
    /// Action parsing failed
    #[error("Action parsing failed: {0}")]
    ParseError(String),
    /// Action execution timed out
    #[error("Action execution timed out after {timeout:?}")]
    Timeout {
        /// The timeout duration that was exceeded
        timeout: Duration,
    },
    /// Generic action execution error
    #[error("Action execution failed: {0}")]
    ExecutionError(String),
    /// IO error during action execution
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    /// JSON parsing error
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
    /// Rate limit error with retry time
    #[error("Rate limit reached. Please wait {wait_time:?} and try again. Details: {message}")]
    RateLimit {
        /// The error message
        message: String,
        /// How long to wait before retrying
        wait_time: Duration,
    },
}

/// Result type for action operations
pub type ActionResult<T> = Result<T, ActionError>;

/// Configuration for action timeouts
#[derive(Debug, Clone)]
pub struct ActionTimeouts {
    /// Default timeout for prompt actions
    pub prompt_timeout: Duration,
    /// Default timeout for user input actions
    pub user_input_timeout: Duration,
    /// Default timeout for sub-workflow actions
    pub sub_workflow_timeout: Duration,
}

impl Default for ActionTimeouts {
    fn default() -> Self {
        Self {
            prompt_timeout: Duration::from_secs(
                std::env::var("SWISSARMYHAMMER_PROMPT_TIMEOUT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(3600),
            ), // 5 minutes default
            user_input_timeout: Duration::from_secs(
                std::env::var("SWISSARMYHAMMER_USER_INPUT_TIMEOUT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(300),
            ), // 5 minutes default
            sub_workflow_timeout: Duration::from_secs(
                std::env::var("SWISSARMYHAMMER_SUB_WORKFLOW_TIMEOUT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(3600),
            ), // 10 minutes default
        }
    }
}

/// Type-safe context keys for workflow execution
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ContextKey {
    /// Key for Claude response
    ClaudeResponse,
    /// Key for last action result
    LastActionResult,
    /// Key for workflow execution stack (for circular dependency detection)
    WorkflowStack,
    /// Custom key for other values
    Custom(String),
}

impl ContextKey {
    /// Convert to string representation for use as HashMap key
    pub fn as_str(&self) -> &str {
        match self {
            ContextKey::ClaudeResponse => "claude_response",
            ContextKey::LastActionResult => "last_action_result",
            ContextKey::WorkflowStack => "_workflow_stack",
            ContextKey::Custom(s) => s,
        }
    }
}

impl From<ContextKey> for String {
    fn from(key: ContextKey) -> Self {
        match key {
            ContextKey::ClaudeResponse => "claude_response".to_string(),
            ContextKey::LastActionResult => "last_action_result".to_string(),
            ContextKey::WorkflowStack => "_workflow_stack".to_string(),
            ContextKey::Custom(s) => s,
        }
    }
}

impl From<&ContextKey> for String {
    fn from(key: &ContextKey) -> Self {
        key.as_str().to_string()
    }
}

/// Context key for Claude response
const CLAUDE_RESPONSE_KEY: &str = "claude_response";

/// Context key for last action result
const LAST_ACTION_RESULT_KEY: &str = "last_action_result";

/// Context key for workflow execution stack (for circular dependency detection)
const WORKFLOW_STACK_KEY: &str = "_workflow_stack";

/// Trait for all workflow actions
#[async_trait::async_trait]
pub trait Action: Send + Sync {
    /// Execute the action with the given context
    async fn execute(&self, context: &mut HashMap<String, Value>) -> ActionResult<Value>;

    /// Get a description of what this action does
    fn description(&self) -> String;

    /// Get the action type name
    fn action_type(&self) -> &'static str;

    /// For testing: allow downcasting
    #[doc(hidden)]
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Trait for actions that support variable substitution
pub trait VariableSubstitution {
    /// Substitute variables in a single string value
    fn substitute_string(&self, value: &str, context: &HashMap<String, Value>) -> String {
        substitute_variables_in_string(value, context)
    }

    /// Substitute variables in a HashMap of string values
    fn substitute_map(
        &self,
        values: &HashMap<String, String>,
        context: &HashMap<String, Value>,
    ) -> HashMap<String, String> {
        let mut substituted = HashMap::new();
        for (key, value) in values {
            substituted.insert(key.clone(), self.substitute_string(value, context));
        }
        substituted
    }
}

/// Action that executes a prompt using Claude
#[derive(Debug, Clone)]
pub struct PromptAction {
    /// Name of the prompt to execute
    pub prompt_name: String,
    /// Arguments to pass to the prompt
    pub arguments: HashMap<String, String>,
    /// Variable name to store the result
    pub result_variable: Option<String>,
    /// Timeout for the Claude execution
    pub timeout: Duration,
    /// Whether to suppress stdout output (only log)
    ///
    /// When set to `true`, the Claude response will only be logged using the tracing
    /// framework and will not be printed to stderr. This is useful for workflows
    /// that need to capture the response programmatically without cluttering the output.
    ///
    /// The quiet mode can also be controlled via the `_quiet` context variable in workflows.
    pub quiet: bool,
}

impl PromptAction {
    /// Create a new prompt action
    pub fn new(prompt_name: String) -> Self {
        let timeouts = ActionTimeouts::default();
        Self {
            prompt_name,
            arguments: HashMap::new(),
            result_variable: None,
            timeout: timeouts.prompt_timeout,
            quiet: false, // Default to showing output
        }
    }

    /// Add an argument to the prompt
    pub fn with_argument(mut self, key: String, value: String) -> Self {
        self.arguments.insert(key, value);
        self
    }

    /// Set the result variable name
    pub fn with_result_variable(mut self, variable: String) -> Self {
        self.result_variable = Some(variable);
        self
    }

    /// Set the timeout for execution
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set whether to suppress stdout output
    pub fn with_quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    /// Substitute variables in arguments using the context
    fn substitute_variables(&self, context: &HashMap<String, Value>) -> HashMap<String, String> {
        self.substitute_map(&self.arguments, context)
    }
}

impl VariableSubstitution for PromptAction {}

#[async_trait::async_trait]
impl Action for PromptAction {
    async fn execute(&self, context: &mut HashMap<String, Value>) -> ActionResult<Value> {
        self.execute_once_internal(context).await
    }

    fn description(&self) -> String {
        format!(
            "Execute prompt '{}' with arguments: {:?}",
            self.prompt_name, self.arguments
        )
    }

    fn action_type(&self) -> &'static str {
        "prompt"
    }

    impl_as_any!();
}

/// Resolve the path to the swissarmyhammer binary
///
/// This function handles the complexity of finding the correct binary path,
/// especially in test environments where current_exe might point to the test binary.
///
/// # Returns
/// The path to the swissarmyhammer binary
fn resolve_swissarmyhammer_binary() -> std::path::PathBuf {
    // Use the current executable path to ensure we call the right binary
    let current_exe =
        std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("swissarmyhammer"));

    // In test environment, current_exe might point to test binary, so try to find the actual swissarmyhammer binary
    if current_exe.to_string_lossy().contains("test")
        || current_exe.to_string_lossy().contains("deps")
    {
        // We're in a test, try to find the actual binary
        if let Ok(cargo_target) = std::env::var("CARGO_TARGET_DIR") {
            let debug_binary = std::path::PathBuf::from(cargo_target)
                .join("debug")
                .join("swissarmyhammer");
            if debug_binary.exists() {
                return debug_binary;
            }
        }

        // Fallback to target/debug/swissarmyhammer relative to workspace
        if let Ok(workspace_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let workspace_root = std::path::Path::new(&workspace_dir)
                .parent()
                .unwrap_or(std::path::Path::new("."));
            let debug_binary = workspace_root
                .join("target")
                .join("debug")
                .join("swissarmyhammer");
            if debug_binary.exists() {
                return debug_binary;
            }
        }

        // Final fallback
        std::path::PathBuf::from("swissarmyhammer")
    } else {
        current_exe
    }
}

impl PromptAction {
    /// Render the prompt using swissarmyhammer prompt test
    async fn render_prompt_with_swissarmyhammer(
        &self,
        context: &HashMap<String, Value>,
    ) -> ActionResult<String> {
        // Substitute variables in arguments
        let args = self.substitute_variables(context);

        // Build the command to render the prompt
        let cmd_binary = resolve_swissarmyhammer_binary();

        let mut cmd = Command::new(&cmd_binary);
        cmd.arg("prompt")
            .arg("test")
            .arg(&self.prompt_name)
            .arg("--raw"); // Get raw output without formatting

        // Add arguments
        for (key, value) in &args {
            if !is_valid_argument_key(key) {
                return Err(ActionError::ParseError(
                    format!("Invalid argument key '{key}': must contain only alphanumeric characters, hyphens, and underscores")
                ));
            }
            cmd.arg("--arg");
            cmd.arg(format!("{key}={value}"));
        }

        // Set up process pipes
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .stdin(std::process::Stdio::null());

        // Execute the command
        let output = cmd.output().await.map_err(|e| {
            ActionError::ClaudeError(format!("Failed to render prompt with swissarmyhammer: {e}"))
        })?;

        // Check for errors
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ActionError::ClaudeError(format!(
                "Failed to render prompt '{}': {}",
                self.prompt_name, stderr
            )));
        }

        // Get the rendered prompt
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.to_string())
    }

    /// Execute the command once without retry logic
    ///
    /// This method performs a single execution attempt of the Claude command.
    /// Rate limit errors are propagated to the caller for retry handling.
    ///
    /// # Arguments
    /// * `context` - The workflow execution context
    ///
    /// # Returns
    /// * `Ok(Value)` - The command response on success
    /// * `Err(ActionError)` - Various errors including rate limits
    async fn execute_once_internal(
        &self,
        context: &mut HashMap<String, Value>,
    ) -> ActionResult<Value> {
        // First, render the prompt using swissarmyhammer
        let rendered_prompt = self.render_prompt_with_swissarmyhammer(context).await?;

        // Log the actual prompt being sent to Claude
        tracing::debug!("Piping prompt to Claude:\n{}", rendered_prompt);

        // Check if quiet mode is enabled in the context
        let quiet = self.quiet
            || context
                .get("_quiet")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

        // Execute the rendered prompt with Claude
        // Find claude in PATH or use common locations
        let claude_path = which::which("claude")
            .or_else(|_| {
                // Check common installation paths
                let home = std::env::var("HOME").unwrap_or_default();
                let possible_paths = vec![
                    format!("{}/.claude/local/claude", home),
                    "/usr/local/bin/claude".to_string(),
                    "/opt/claude/claude".to_string(),
                ];

                for path in possible_paths {
                    if std::path::Path::new(&path).exists() {
                        return Ok(std::path::PathBuf::from(path));
                    }
                }

                Err(which::Error::CannotFindBinaryPath)
            })
            .map_err(|e| {
                ActionError::ClaudeError(format!(
                    "Claude CLI not found. Make sure 'claude' is installed and available in your PATH. Error: {e}"
                ))
            })?;
        let mut cmd = Command::new(&claude_path);

        // Claude CLI arguments
        cmd.arg("--dangerously-skip-permissions")
            .arg("--print")
            .arg("--output-format")
            .arg("stream-json")
            .arg("--verbose");

        // Set up the command to pipe prompt via stdin
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        if !quiet {
            tracing::info!(
                "Executing prompt '{}' with Claude at: {}",
                self.prompt_name,
                claude_path.display()
            );
        }

        // Spawn the Claude process
        let mut child = cmd.spawn().map_err(|e| {
            ActionError::ClaudeError(format!("Failed to spawn Claude command: {e}"))
        })?;

        // Write the prompt to Claude's stdin
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            stdin
                .write_all(rendered_prompt.as_bytes())
                .await
                .map_err(|e| {
                    ActionError::ClaudeError(format!("Failed to write prompt to Claude: {e}"))
                })?;
            stdin.shutdown().await.map_err(|e| {
                ActionError::ClaudeError(format!("Failed to close Claude stdin: {e}"))
            })?;
        }

        // Get stdout for streaming
        let stdout = child.stdout.take().ok_or_else(|| {
            ActionError::ClaudeError("Failed to capture Claude stdout".to_string())
        })?;

        // Read stdout line by line for streaming JSON
        use tokio::io::{AsyncBufReadExt, BufReader};
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        let mut response_text = String::new();
        let mut _got_result = false;

        // Get timeout from context or use default
        let line_timeout = context
            .get("_timeout_secs")
            .and_then(|v| v.as_u64())
            .map(Duration::from_secs)
            .unwrap_or_else(|| Duration::from_secs(60 * 60));

        tracing::debug!("Using line timeout: {:?}", line_timeout);

        // Read lines with timeout
        loop {
            match timeout(line_timeout, lines.next_line()).await {
                Ok(Ok(Some(line))) => {
                    // Format JSON as YAML for better readability
                    let formatted_output = format_claude_output_as_yaml(&line);
                    if formatted_output != line {
                        // If it was formatted as YAML, log it with proper structure
                        tracing::debug!("Claude output line:\n{}", formatted_output);
                    } else {
                        // If not JSON, log as is
                        tracing::debug!("Claude output line: {}", line);
                    }

                    if line.trim().is_empty() {
                        continue;
                    }

                    if let Ok(json) = serde_json::from_str::<Value>(&line) {
                        // Look for the final result
                        if let Some(result) = json.get("result").and_then(|r| r.as_str()) {
                            response_text = result.to_string();
                            _got_result = true;
                            break;
                        }
                        // Also check for assistant messages with text content
                        if json.get("type").and_then(|t| t.as_str()) == Some("assistant") {
                            if let Some(message) = json.get("message").and_then(|m| m.as_object()) {
                                if let Some(content_array) =
                                    message.get("content").and_then(|c| c.as_array())
                                {
                                    for content_item in content_array {
                                        if let Some(text) =
                                            content_item.get("text").and_then(|t| t.as_str())
                                        {
                                            response_text.push_str(text);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(Ok(None)) => {
                    // End of stream
                    break;
                }
                Ok(Err(e)) => {
                    tracing::error!("Error reading Claude output: {}", e);
                    break;
                }
                Err(_) => {
                    // Timeout - kill the process and return error
                    tracing::error!("Timeout reading Claude output after {:?}", line_timeout);
                    tracing::error!("Response so far: {} characters", response_text.len());
                    let _ = child.kill().await;
                    // If we have some response, use it rather than erroring
                    if !response_text.is_empty() {
                        break;
                    }
                    return Err(ActionError::Timeout {
                        timeout: line_timeout,
                    });
                }
            }
        }

        // Wait for process to complete with a short timeout
        let wait_result = timeout(Duration::from_secs(5), child.wait()).await;
        let status = match wait_result {
            Ok(Ok(status)) => status,
            Ok(Err(e)) => {
                return Err(ActionError::ClaudeError(format!(
                    "Failed to wait for Claude: {e}"
                )))
            }
            Err(_) => {
                // Process didn't exit cleanly, kill it
                let _ = child.kill().await;
                return Err(ActionError::ClaudeError(
                    "Claude process failed to exit cleanly".to_string(),
                ));
            }
        };

        if !status.success() {
            return Err(ActionError::ClaudeError(
                "Claude execution failed".to_string(),
            ));
        }

        let response_text = response_text.trim();

        if response_text.is_empty() {
            tracing::warn!("No response received from Claude");
        } else {
            tracing::debug!(
                "Claude response received: {} characters",
                response_text.len()
            );
        }

        // Note: String-based abort detection removed - abort handling now done via file-based mechanism

        // Display the output as YAML
        if !quiet && !response_text.is_empty() {
            // Build the YAML output as a single string
            let mut yaml_output = String::new();
            yaml_output.push_str("\n---\n");
            yaml_output.push_str(&format!("prompt: {}\n", self.prompt_name));
            yaml_output.push_str("claude_response: |\n");
            for line in response_text.lines() {
                yaml_output.push_str(&format!("  {line}\n"));
            }
            yaml_output.push_str("---");

            // Log YAML output
            tracing::info!("{}", yaml_output);
        }

        // Create a response value
        let response = Value::String(response_text.to_string());

        // Store result in context if variable name specified
        if let Some(var_name) = &self.result_variable {
            context.insert(var_name.clone(), response.clone());
        }

        // Always store in special last_action_result key
        context.insert(LAST_ACTION_RESULT_KEY.to_string(), Value::Bool(true));
        context.insert(CLAUDE_RESPONSE_KEY.to_string(), response.clone());

        Ok(response)
    }
}

/// Action that pauses execution for a specified duration or waits for user input
#[derive(Debug, Clone)]
pub struct WaitAction {
    /// Duration to wait (None means wait for user input)
    pub duration: Option<Duration>,
    /// Message to display while waiting
    pub message: Option<String>,
}

impl WaitAction {
    /// Create a new wait action with duration
    pub fn new_duration(duration: Duration) -> Self {
        Self {
            duration: Some(duration),
            message: None,
        }
    }

    /// Create a new wait action for user input
    pub fn new_user_input() -> Self {
        Self {
            duration: None,
            message: None,
        }
    }

    /// Set the wait message
    pub fn with_message(mut self, message: String) -> Self {
        self.message = Some(message);
        self
    }
}

#[async_trait::async_trait]
impl Action for WaitAction {
    async fn execute(&self, context: &mut HashMap<String, Value>) -> ActionResult<Value> {
        match self.duration {
            Some(duration) => {
                if let Some(message) = &self.message {
                    tracing::info!("Waiting: {}", message);
                }

                // In test mode, use much shorter wait times for faster test execution
                let actual_duration = if cfg!(test) || std::env::var("RUST_TEST").is_ok() {
                    // Cap wait time at 50ms in test mode for performance
                    Duration::from_millis(std::cmp::min(50, duration.as_millis() as u64))
                } else {
                    duration
                };

                tokio::time::sleep(actual_duration).await;
            }
            None => {
                let message = self
                    .message
                    .as_deref()
                    .unwrap_or("Press Enter to continue...");
                tracing::info!("{}", message);

                // Read from stdin with a reasonable timeout
                use tokio::io::{stdin, AsyncBufReadExt, BufReader};
                let mut reader = BufReader::new(stdin());
                let mut line = String::new();

                // Use configurable timeout for user input
                let timeouts = ActionTimeouts::default();
                match timeout(timeouts.user_input_timeout, reader.read_line(&mut line)).await {
                    Ok(Ok(_)) => {
                        // Successfully read input
                    }
                    Ok(Err(e)) => {
                        return Err(ActionError::IoError(e));
                    }
                    Err(_) => {
                        return Err(ActionError::Timeout {
                            timeout: timeouts.user_input_timeout,
                        });
                    }
                }
            }
        }

        // Mark action as successful
        context.insert(LAST_ACTION_RESULT_KEY.to_string(), Value::Bool(true));

        Ok(Value::Null)
    }

    fn description(&self) -> String {
        match self.duration {
            Some(duration) => format!("Wait for {duration:?}"),
            None => "Wait for user input".to_string(),
        }
    }

    fn action_type(&self) -> &'static str {
        "wait"
    }

    impl_as_any!();
}

/// Action that logs a message
#[derive(Debug, Clone)]
pub struct LogAction {
    /// Message to log
    pub message: String,
    /// Log level
    pub level: LogLevel,
}

/// Log levels for LogAction
#[derive(Debug, Clone)]
pub enum LogLevel {
    /// Informational log level
    Info,
    /// Warning log level
    Warning,
    /// Error log level
    Error,
}

impl LogAction {
    /// Create a new log action
    pub fn new(message: String, level: LogLevel) -> Self {
        Self { message, level }
    }

    /// Create an info log action
    pub fn info(message: String) -> Self {
        Self::new(message, LogLevel::Info)
    }

    /// Create a warning log action
    pub fn warning(message: String) -> Self {
        Self::new(message, LogLevel::Warning)
    }

    /// Create an error log action
    pub fn error(message: String) -> Self {
        Self::new(message, LogLevel::Error)
    }
}

impl VariableSubstitution for LogAction {}

#[async_trait::async_trait]
impl Action for LogAction {
    async fn execute(&self, context: &mut HashMap<String, Value>) -> ActionResult<Value> {
        // Render message with liquid templating (supports {{variable}} syntax)
        let message = render_with_liquid_template(&self.message, context);

        match self.level {
            LogLevel::Info => tracing::info!("{}", message),
            LogLevel::Warning => tracing::warn!("{}", message),
            LogLevel::Error => tracing::error!("{}", message),
        }

        // Mark action as successful
        context.insert(LAST_ACTION_RESULT_KEY.to_string(), Value::Bool(true));

        Ok(Value::String(message))
    }

    fn description(&self) -> String {
        format!("Log message: {}", self.message)
    }

    fn action_type(&self) -> &'static str {
        "log"
    }

    impl_as_any!();
}

/// Action that sets a variable in the workflow context
#[derive(Debug, Clone)]
pub struct SetVariableAction {
    /// Variable name to set
    pub variable_name: String,
    /// Value to set (supports variable substitution)
    pub value: String,
}

/// Action that immediately fails with an abort error
#[derive(Debug, Clone)]
pub struct AbortAction {
    /// The error message to display when aborting
    pub message: String,
}

/// Action that executes a sub-workflow
#[derive(Debug, Clone)]
pub struct SubWorkflowAction {
    /// Name of the workflow to execute
    pub workflow_name: String,
    /// Input variables to pass to the sub-workflow
    pub input_variables: HashMap<String, String>,
    /// Variable name to store the result
    pub result_variable: Option<String>,
    /// Timeout for the sub-workflow execution
    pub timeout: Duration,
}

impl SetVariableAction {
    /// Create a new set variable action
    pub fn new(variable_name: String, value: String) -> Self {
        Self {
            variable_name,
            value,
        }
    }
}

impl VariableSubstitution for SetVariableAction {}

#[async_trait::async_trait]
impl Action for SetVariableAction {
    async fn execute(&self, context: &mut HashMap<String, Value>) -> ActionResult<Value> {
        // Substitute variables in value
        let substituted_value = self.substitute_string(&self.value, context);

        // Try to parse as JSON first, fall back to string
        let json_value = match serde_json::from_str(&substituted_value) {
            Ok(v) => v,
            Err(_) => Value::String(substituted_value),
        };

        // Set the variable
        context.insert(self.variable_name.clone(), json_value.clone());

        // Mark action as successful
        context.insert(LAST_ACTION_RESULT_KEY.to_string(), Value::Bool(true));

        Ok(json_value)
    }

    fn description(&self) -> String {
        format!("Set variable '{}' to '{}'", self.variable_name, self.value)
    }

    fn action_type(&self) -> &'static str {
        "set_variable"
    }

    impl_as_any!();
}

impl AbortAction {
    /// Create a new abort action
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl VariableSubstitution for AbortAction {}

#[async_trait::async_trait]
impl Action for AbortAction {
    async fn execute(&self, context: &mut HashMap<String, Value>) -> ActionResult<Value> {
        // Substitute variables in message
        let message = self.substitute_string(&self.message, context);

        // Set a special context variable to signal abort request
        context.insert(
            "__ABORT_REQUESTED__".to_string(),
            Value::String(message.clone()),
        );

        // Return an execution error to indicate action failure
        Err(ActionError::ExecutionError(format!(
            "Workflow aborted: {message}"
        )))
    }

    fn description(&self) -> String {
        format!("Abort workflow execution with message: {message}", message = self.message)
    }

    fn action_type(&self) -> &'static str {
        "abort"
    }

    impl_as_any!();
}

/// Validate that an argument key is safe for command-line use
fn is_valid_argument_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

/// Helper function to substitute variables in a string
/// Variables are referenced as ${variable_name}
fn substitute_variables_in_string(input: &str, context: &HashMap<String, Value>) -> String {
    let parser = ActionParser::new().expect("Failed to create ActionParser");
    parser
        .substitute_variables_safe(input, context)
        .unwrap_or_else(|_| input.to_string())
}

/// Helper function to render text with liquid templating
/// Supports both {{variable}} liquid syntax and ${variable} fallback
fn render_with_liquid_template(input: &str, context: &HashMap<String, Value>) -> String {
    // Convert context to liquid Object
    let mut liquid_vars = liquid::Object::new();
    for (key, value) in context {
        // Skip internal keys that shouldn't be exposed to templates
        if key.starts_with('_') {
            continue;
        }
        liquid_vars.insert(
            key.clone().into(),
            liquid::model::to_value(value).unwrap_or(liquid::model::Value::Nil),
        );
    }

    // Try liquid template rendering first (supports {{variable}} syntax)
    let liquid_rendered = match liquid::ParserBuilder::with_stdlib()
        .build()
        .and_then(|parser| parser.parse(input))
    {
        Ok(template) => match template.render(&liquid_vars) {
            Ok(rendered) => rendered,
            Err(_) => input.to_string(),
        },
        Err(_) => input.to_string(),
    };

    // Apply fallback variable substitution for any remaining ${variable} syntax
    substitute_variables_in_string(&liquid_rendered, context)
}

impl SubWorkflowAction {
    /// Create a new sub-workflow action
    pub fn new(workflow_name: String) -> Self {
        let timeouts = ActionTimeouts::default();
        Self {
            workflow_name,
            input_variables: HashMap::new(),
            result_variable: None,
            timeout: timeouts.sub_workflow_timeout,
        }
    }

    /// Add an input variable to pass to the sub-workflow
    pub fn with_input(mut self, key: String, value: String) -> Self {
        self.input_variables.insert(key, value);
        self
    }

    /// Set the result variable name
    pub fn with_result_variable(mut self, variable: String) -> Self {
        self.result_variable = Some(variable);
        self
    }

    /// Set the timeout for execution
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Substitute variables in input values using the context
    fn substitute_variables(&self, context: &HashMap<String, Value>) -> HashMap<String, String> {
        self.substitute_map(&self.input_variables, context)
    }
}

impl VariableSubstitution for SubWorkflowAction {}

/// Shell action for executing shell commands in workflows
#[derive(Debug, Clone)]
pub struct ShellAction {
    /// The shell command to execute
    pub command: String,
    /// Optional timeout for command execution
    #[allow(dead_code)]
    pub timeout: Option<Duration>,
    /// Optional variable name to store command output
    #[allow(dead_code)]
    pub result_variable: Option<String>,
    /// Optional working directory for command execution
    #[allow(dead_code)]
    pub working_dir: Option<String>,
    /// Optional environment variables for the command
    #[allow(dead_code)]
    pub environment: HashMap<String, String>,
}

impl ShellAction {
    const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes
    const MAX_TIMEOUT: Duration = Duration::from_secs(3600); // 1 hour

    /// Create a new shell action
    #[allow(dead_code)]
    pub fn new(command: String) -> Self {
        Self {
            command,
            timeout: None, // No timeout by default as per specification
            result_variable: None,
            working_dir: None,
            environment: HashMap::new(),
        }
    }

    /// Set the timeout for command execution
    #[allow(dead_code)]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set the result variable name
    pub fn with_result_variable(mut self, variable: String) -> Self {
        self.result_variable = Some(variable);
        self
    }

    /// Set the working directory for command execution
    pub fn with_working_dir(mut self, dir: String) -> Self {
        self.working_dir = Some(dir);
        self
    }

    /// Set environment variables for the command
    pub fn with_environment(mut self, env: HashMap<String, String>) -> Self {
        self.environment = env;
        self
    }

    /// Validate timeout duration according to security limits
    pub fn validate_timeout(&self) -> ActionResult<Duration> {
        let timeout = self.timeout.unwrap_or(Self::DEFAULT_TIMEOUT);

        if timeout > Self::MAX_TIMEOUT {
            return Err(ActionError::ExecutionError(format!(
                "Timeout too large: maximum is {} seconds",
                Self::MAX_TIMEOUT.as_secs()
            )));
        }

        if timeout.as_millis() == 0 {
            return Err(ActionError::ExecutionError(
                "Timeout must be greater than 0".to_string(),
            ));
        }

        Ok(timeout)
    }
}

/// Create a platform-specific command for shell execution
#[cfg(target_os = "windows")]
fn create_command(command: &str) -> Command {
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", command]);
    cmd
}

/// Create a platform-specific command for shell execution
#[cfg(not(target_os = "windows"))]
fn create_command(command: &str) -> Command {
    let mut cmd = Command::new("sh");
    cmd.args(["-c", command]);
    cmd
}

/// Validate environment variable names
/// Environment variable names should start with letter or underscore
/// and contain only letters, digits, and underscores
pub fn is_valid_env_var_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    let mut chars = name.chars();
    if let Some(first) = chars.next() {
        if !first.is_ascii_alphabetic() && first != '_' {
            return false;
        }
    }

    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Validate working directory path for security
pub fn validate_working_directory(path: &str) -> ActionResult<()> {
    let path = std::path::Path::new(path);

    // Check for path traversal attempts
    if path
        .components()
        .any(|comp| matches!(comp, std::path::Component::ParentDir))
    {
        return Err(ActionError::ExecutionError(
            "Working directory cannot contain parent directory references (..)".to_string(),
        ));
    }

    Ok(())
}

/// Validate shell command for security issues
pub fn validate_command(command: &str) -> ActionResult<()> {
    // Check for obviously dangerous patterns
    if command.trim().is_empty() {
        return Err(ActionError::ExecutionError(
            "Shell command cannot be empty".to_string(),
        ));
    }

    // Check command length (prevent extremely long commands)
    if command.len() > 4096 {
        return Err(ActionError::ExecutionError(
            "Shell command too long (maximum 4096 characters)".to_string(),
        ));
    }

    // Detect dangerous command patterns
    validate_dangerous_patterns(command)?;

    // Validate command structure
    validate_command_structure(command)?;

    Ok(())
}

/// Detect dangerous command patterns as specified in security requirements
pub fn validate_dangerous_patterns(command: &str) -> ActionResult<()> {
    let dangerous_patterns = [
        // System modification commands
        ("rm -rf", "Recursive file deletion"),
        ("format", "Disk formatting"),
        ("fdisk", "Disk partitioning"),
        ("mkfs", "Filesystem creation"),
        // Network/security operations
        ("nc -l", "Network listener"),
        ("ncat -l", "Network listener"),
        ("socat", "Network relay"),
        ("ssh", "Remote shell access"),
        ("scp", "Remote file copy"),
        ("rsync", "Remote sync"),
        // Package management
        ("apt install", "Package installation"),
        ("yum install", "Package installation"),
        ("pip install", "Python package installation"),
        ("npm install", "Node package installation"),
        ("cargo install", "Rust package installation"),
        // Privilege escalation
        ("sudo", "Privilege escalation"),
        ("su ", "User switching"),
        ("chmod +s", "Setuid bit"),
        ("chown root", "Root ownership change"),
        // System configuration
        ("/etc/", "System configuration access"),
        ("systemctl", "System service control"),
        ("service ", "Service control"),
        ("crontab", "Scheduled task modification"),
        // Dangerous shell features
        ("|(", "Subshell execution"),
        ("eval", "Dynamic code execution"),
        ("exec", "Process replacement"),
    ];

    let command_lower = command.to_lowercase();

    for (pattern, description) in &dangerous_patterns {
        if command_lower.contains(pattern) {
            tracing::warn!(
                "Potentially dangerous command pattern detected: {} in command: {}",
                description,
                command
            );

            // Log security event
            log_security_event("DANGEROUS_PATTERN", description, command);
        }
    }

    Ok(())
}

/// Validate command structure to prevent injection
pub fn validate_command_structure(command: &str) -> ActionResult<()> {
    // Check for command injection patterns
    let injection_patterns = [";", "&&", "||", "|", "`", "$(", "\n", "\r", "\0"];

    for pattern in &injection_patterns {
        if command.contains(pattern) {
            // Allow some patterns in specific contexts
            if validate_safe_usage(command, pattern)? {
                continue;
            }

            return Err(ActionError::ExecutionError(format!(
                "Potentially unsafe command pattern '{pattern}' detected"
            )));
        }
    }

    Ok(())
}

/// Check if a potentially dangerous pattern is being used safely
pub fn validate_safe_usage(command: &str, pattern: &str) -> ActionResult<bool> {
    match pattern {
        "|" => {
            // Allow simple pipes for common operations
            if command.matches('|').count() == 1 && !command.contains("nc ") {
                Ok(true)
            } else {
                Ok(false)
            }
        }
        "&&" | "||" | ";" => {
            // These are generally unsafe for automated execution
            Ok(false)
        }
        _ => Ok(false),
    }
}

/// Enhanced working directory validation with security checks
pub fn validate_working_directory_security(path: &str) -> ActionResult<()> {
    // First run the existing validation
    validate_working_directory(path)?;

    // Prevent access to sensitive system directories
    let sensitive_dirs = [
        "/etc",
        "/sys",
        "/proc",
        "/dev",
        "/boot",
        "/root",
        "/var/lib",
        "/usr/lib",
        "C:\\Windows",
        "C:\\Program Files",
        "C:\\System32",
    ];

    let path_str = path.to_string().to_lowercase();
    for sensitive in &sensitive_dirs {
        if path_str.starts_with(&sensitive.to_lowercase()) {
            tracing::warn!("Attempting to use sensitive directory: {}", path_str);
            log_security_event(
                "SENSITIVE_DIRECTORY",
                &format!("Access to {sensitive}"),
                path,
            );
        }
    }

    Ok(())
}

/// Validate environment variables for security issues
pub fn validate_environment_variables_security(env: &HashMap<String, String>) -> ActionResult<()> {
    // List of sensitive environment variables that shouldn't be overridden
    let protected_vars = [
        "PATH",
        "LD_LIBRARY_PATH",
        "DYLD_LIBRARY_PATH",
        "HOME",
        "USER",
        "USERNAME",
        "SHELL",
        "SSH_AUTH_SOCK",
        "SSH_AGENT_PID",
        "SUDO_USER",
        "SUDO_UID",
        "SUDO_GID",
    ];

    for (key, value) in env {
        // Validate variable name (use existing function)
        if !is_valid_env_var_name(key) {
            return Err(ActionError::ExecutionError(format!(
                "Invalid environment variable name: {key}"
            )));
        }

        // Check for protected variables
        if protected_vars.contains(&key.to_uppercase().as_str()) {
            tracing::warn!(
                "Attempting to override protected environment variable: {}",
                key
            );
            log_security_event("PROTECTED_ENV_VAR", &format!("Override of {key}"), "");
        }

        // Validate variable value length
        if value.len() > 1024 {
            return Err(ActionError::ExecutionError(format!(
                "Environment variable value too long: {key}"
            )));
        }

        // Check for injection in environment values
        if value.contains('\0') || value.contains('\n') {
            return Err(ActionError::ExecutionError(format!(
                "Invalid characters in environment variable: {key}"
            )));
        }
    }

    Ok(())
}

/// Log security-related events for audit purposes
pub fn log_security_event(event_type: &str, details: &str, command: &str) {
    tracing::warn!(
        "Security event: {} - {} - Command: {}",
        event_type,
        details,
        command
    );
}

/// Log command execution with security context
pub fn log_command_execution(
    command: &str,
    working_dir: Option<&str>,
    env: &HashMap<String, String>,
) {
    tracing::info!(
        "Executing shell command: {} (working_dir: {:?}, env_vars: {})",
        command,
        working_dir,
        env.len()
    );

    // Log environment variables (but not their values for security)
    if !env.is_empty() {
        let env_keys: Vec<&String> = env.keys().collect();
        tracing::debug!("Environment variables set: {:?}", env_keys);
    }
}

impl VariableSubstitution for ShellAction {}

impl ShellAction {
    /// Process command output and set context variables
    fn process_command_output(
        &self,
        output: std::process::Output,
        duration_ms: u64,
        context: &mut HashMap<String, Value>,
    ) -> ActionResult<Value> {
        let success = output.status.success();
        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        tracing::debug!(
            "Command completed: exit_code={}, duration_ms={}",
            exit_code,
            duration_ms
        );

        // Set all required context variables
        context.insert("success".to_string(), Value::Bool(success));
        context.insert("failure".to_string(), Value::Bool(!success));
        context.insert("exit_code".to_string(), Value::Number(exit_code.into()));
        context.insert("stdout".to_string(), Value::String(stdout.clone()));
        context.insert("stderr".to_string(), Value::String(stderr));
        context.insert("duration_ms".to_string(), Value::Number(duration_ms.into()));

        // Set result variable if specified
        if let Some(result_var) = &self.result_variable {
            context.insert(result_var.clone(), Value::String(stdout.clone()));
        }

        // Set last action result based on command success
        context.insert(LAST_ACTION_RESULT_KEY.to_string(), Value::Bool(success));

        tracing::info!(
            "Shell command completed in {}ms with exit code {}",
            duration_ms,
            exit_code
        );

        // Return appropriate result - success returns stdout, failure returns false
        if success {
            Ok(Value::String(stdout))
        } else {
            tracing::info!("Command failed with exit code {}", exit_code);
            Ok(Value::Bool(false)) // Don't fail the workflow, just indicate failure
        }
    }

    /// Terminate process gracefully with SIGTERM followed by SIGKILL if needed
    async fn terminate_process_gracefully(&self, child: &mut tokio::process::Child) {
        use tokio::time::Duration;

        let pid = child.id();

        // First attempt: graceful termination (SIGTERM)
        tracing::debug!("Attempting graceful termination of process {pid:?}");

        if let Err(e) = child.start_kill() {
            tracing::warn!("Failed to send SIGTERM to process {pid:?}: {e}");
            return;
        }

        // Give the process time to terminate gracefully (5 seconds)
        let graceful_timeout = Duration::from_secs(5);

        match tokio::time::timeout(graceful_timeout, child.wait()).await {
            Ok(Ok(_exit_status)) => {
                tracing::debug!("Process {pid:?} terminated gracefully");
                return;
            }
            Ok(Err(e)) => {
                tracing::warn!("Error waiting for process {pid:?} termination: {e}");
            }
            Err(_) => {
                tracing::warn!(
                    "Process {pid:?} did not terminate gracefully within {graceful_timeout:?}"
                );
            }
        }

        // Second attempt: forceful termination (SIGKILL)
        tracing::debug!("Attempting forceful termination of process {pid:?}");

        if let Err(e) = child.kill().await {
            tracing::error!("Failed to forcefully terminate process {pid:?}: {e}");
        } else {
            tracing::debug!("Process {pid:?} terminated forcefully");
        }
    }

    /// Handle timeout scenarios by setting appropriate context variables
    fn handle_timeout(
        &self,
        context: &mut HashMap<String, Value>,
        duration_ms: u64,
    ) -> ActionResult<Value> {
        // Set timeout-specific context variables
        context.insert("success".to_string(), Value::Bool(false));
        context.insert("failure".to_string(), Value::Bool(true));
        context.insert("exit_code".to_string(), Value::Number((-1).into()));
        context.insert("stdout".to_string(), Value::String("".to_string()));
        context.insert(
            "stderr".to_string(),
            Value::String("Command timed out".to_string()),
        );
        context.insert("duration_ms".to_string(), Value::Number(duration_ms.into()));

        // Don't set result variable on timeout to indicate no output was captured

        // Set last action result to indicate failure
        context.insert(LAST_ACTION_RESULT_KEY.to_string(), Value::Bool(false));

        tracing::warn!("Shell command timed out after {}ms", duration_ms);

        Ok(Value::Bool(false))
    }
}

#[async_trait::async_trait]
impl Action for ShellAction {
    async fn execute(&self, context: &mut HashMap<String, Value>) -> ActionResult<Value> {
        // Substitute variables in command string
        let command = self.substitute_string(&self.command, context);

        // Security validation
        validate_command(&command)?;

        // Validate environment variables for security
        validate_environment_variables_security(&self.environment)?;

        // Validate timeout
        let _validated_timeout = self.validate_timeout()?;

        // Log security-relevant execution
        log_command_execution(&command, self.working_dir.as_deref(), &self.environment);

        tracing::info!("Executing shell command: {}", command);

        // Record start time for duration calculation
        let start_time = std::time::Instant::now();

        // Create platform-specific command
        let mut cmd = create_command(&command);

        // Set working directory if specified
        if let Some(working_dir) = &self.working_dir {
            let substituted_dir = self.substitute_string(working_dir, context);

            // Validate working directory for security
            validate_working_directory_security(&substituted_dir)?;

            let path = std::path::Path::new(&substituted_dir);

            // Validate working directory exists and is accessible
            if !path.exists() {
                return Err(ActionError::ExecutionError(format!(
                    "Working directory does not exist: {substituted_dir}"
                )));
            }

            if !path.is_dir() {
                return Err(ActionError::ExecutionError(format!(
                    "Working directory is not a directory: {substituted_dir}"
                )));
            }

            cmd.current_dir(path);
            tracing::debug!("Set working directory to: {}", substituted_dir);
        }

        // Set environment variables if specified
        for (key, value) in &self.environment {
            let substituted_key = self.substitute_string(key, context);
            let substituted_value = self.substitute_string(value, context);

            // Validate environment variable names
            if !is_valid_env_var_name(&substituted_key) {
                return Err(ActionError::ExecutionError(format!(
                    "Invalid environment variable name: {substituted_key}"
                )));
            }

            cmd.env(&substituted_key, &substituted_value);
            tracing::debug!(
                "Set environment variable: {}={}",
                substituted_key,
                substituted_value
            );
        }

        // Configure output capture
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // Spawn the child process
        let mut child = cmd
            .spawn()
            .map_err(|e| ActionError::ExecutionError(format!("Failed to spawn command: {e}")))?;

        let result = if let Some(timeout_duration) = self.timeout {
            // Timeout already validated in security checks above

            // Execute with timeout and proper process cleanup
            // Use a more complex approach to maintain access to child for cleanup
            let wait_future = async {
                // Get stdout and stderr handles before waiting
                let stdout = child.stdout.take();
                let stderr = child.stderr.take();

                // Wait for process completion
                let status = child.wait().await?;

                // Read the output manually
                let mut stdout_data = Vec::new();
                let mut stderr_data = Vec::new();

                if let Some(mut stdout_handle) = stdout {
                    use tokio::io::AsyncReadExt;
                    stdout_handle.read_to_end(&mut stdout_data).await?;
                }

                if let Some(mut stderr_handle) = stderr {
                    use tokio::io::AsyncReadExt;
                    stderr_handle.read_to_end(&mut stderr_data).await?;
                }

                Ok::<std::process::Output, std::io::Error>(std::process::Output {
                    status,
                    stdout: stdout_data,
                    stderr: stderr_data,
                })
            };

            match timeout(timeout_duration, wait_future).await {
                Ok(Ok(output)) => {
                    // Command completed within timeout
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    self.process_command_output(output, duration_ms, context)
                }
                Ok(Err(e)) => Err(ActionError::ExecutionError(format!(
                    "Command execution failed: {e}"
                ))),
                Err(_) => {
                    // Timeout occurred - implement graceful termination
                    tracing::warn!(
                        "Command timed out after {:?}, attempting graceful termination",
                        timeout_duration
                    );

                    // Terminate the process gracefully
                    self.terminate_process_gracefully(&mut child).await;

                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    self.handle_timeout(context, duration_ms)
                }
            }
        } else {
            // Execute without timeout
            match child.wait_with_output().await {
                Ok(output) => {
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    self.process_command_output(output, duration_ms, context)
                }
                Err(e) => Err(ActionError::ExecutionError(format!(
                    "Command execution failed: {e}"
                ))),
            }
        };

        result
    }

    fn description(&self) -> String {
        format!("Execute shell command: {}", self.command)
    }

    fn action_type(&self) -> &'static str {
        "shell"
    }

    impl_as_any!();
}

#[async_trait::async_trait]
impl Action for SubWorkflowAction {
    async fn execute(&self, context: &mut HashMap<String, Value>) -> ActionResult<Value> {
        // Check for circular dependencies
        let workflow_stack = context
            .get(WORKFLOW_STACK_KEY)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        // Check if this workflow is already in the execution stack
        for stack_item in &workflow_stack {
            if let Some(workflow_name) = stack_item.as_str() {
                if workflow_name == self.workflow_name {
                    return Err(ActionError::ExecutionError(format!(
                        "Circular workflow dependency detected: workflow '{}' is already in the execution stack",
                        self.workflow_name
                    )));
                }
            }
        }

        // Substitute variables in input
        let substituted_inputs = self.substitute_variables(context);

        // Validate input keys early
        for key in substituted_inputs.keys() {
            if !is_valid_argument_key(key) {
                return Err(ActionError::ParseError(
                    format!("Invalid input variable key '{key}': must contain only alphanumeric characters, hyphens, and underscores")
                ));
            }
        }

        // Add workflow stack to track circular dependencies
        let mut new_stack = workflow_stack;
        new_stack.push(Value::String(self.workflow_name.clone()));

        // Execute the sub-workflow in-process
        tracing::info!("Executing sub-workflow '{}' in-process", self.workflow_name);
        tracing::debug!("Current context before sub-workflow: {:?}", context);

        // Create storage and load the workflow
        let storage = if let Some(test_storage) = get_test_storage() {
            test_storage
        } else {
            Arc::new(WorkflowStorage::file_system().map_err(|e| {
                ActionError::ExecutionError(format!("Failed to create workflow storage: {e}"))
            })?)
        };

        let workflow_name_typed = WorkflowName::new(&self.workflow_name);
        let workflow = storage.get_workflow(&workflow_name_typed).map_err(|e| {
            ActionError::ExecutionError(format!(
                "Failed to load sub-workflow '{}': {}",
                self.workflow_name, e
            ))
        })?;

        // Create executor
        let mut executor = WorkflowExecutor::new();

        // Start the workflow
        let mut run = executor.start_workflow(workflow).map_err(|e| {
            ActionError::ExecutionError(format!(
                "Failed to start sub-workflow '{}': {}",
                self.workflow_name, e
            ))
        })?;

        // Set up the context for the sub-workflow
        // Copy the current context variables that should be inherited
        let quiet = context
            .get("_quiet")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let timeout_secs = context.get("_timeout_secs").and_then(|v| v.as_u64());

        // Add input variables
        for (key, value) in substituted_inputs {
            run.context.insert(key, Value::String(value));
        }

        // Add workflow stack to the sub-workflow context
        run.context
            .insert(WORKFLOW_STACK_KEY.to_string(), Value::Array(new_stack));

        // Copy special variables
        if quiet {
            run.context.insert("_quiet".to_string(), Value::Bool(true));
        }

        if let Some(timeout_secs) = timeout_secs {
            run.context.insert(
                "_timeout_secs".to_string(),
                Value::Number(serde_json::Number::from(timeout_secs)),
            );
        }

        // Execute the workflow with timeout
        let execution_future = executor.execute_state(&mut run);

        let _result = match timeout(self.timeout, execution_future).await {
            Ok(Ok(_)) => {
                // Workflow executed successfully
                tracing::info!(
                    "Sub-workflow '{}' completed with status: {:?}",
                    self.workflow_name,
                    run.status
                );
                Ok(())
            }
            Ok(Err(e)) => Err(ActionError::ExecutionError(format!(
                "Sub-workflow '{}' execution failed: {}",
                self.workflow_name, e
            ))),
            Err(_) => Err(ActionError::Timeout {
                timeout: self.timeout,
            }),
        }?;

        // Check the workflow status
        match run.status {
            WorkflowRunStatus::Completed => {
                // Extract the context as the result
                let result = Value::Object(
                    run.context
                        .into_iter()
                        .filter(|(k, _)| !k.starts_with('_')) // Filter out internal variables
                        .collect(),
                );

                // Store result in context if variable name specified
                if let Some(var_name) = &self.result_variable {
                    tracing::info!(
                        "Storing sub-workflow result in variable '{}': {:?}",
                        var_name,
                        result
                    );
                    context.insert(var_name.clone(), result.clone());
                }

                // Mark action as successful
                context.insert(LAST_ACTION_RESULT_KEY.to_string(), Value::Bool(true));

                Ok(result)
            }
            WorkflowRunStatus::Failed => {
                // Check if the sub-workflow failed due to an abort error
                // Look for abort error indication in the context
                if let Some(result) = run.context.get("result") {
                    if let Some(_result_str) = result.as_str() {
                        // Note: String-based abort detection removed - abort handling now done via file-based mechanism
                    }
                }

                Err(ActionError::ExecutionError(format!(
                    "Sub-workflow '{}' failed",
                    self.workflow_name
                )))
            }
            WorkflowRunStatus::Cancelled => Err(ActionError::ExecutionError(format!(
                "Sub-workflow '{}' was cancelled",
                self.workflow_name
            ))),
            _ => Err(ActionError::ExecutionError(format!(
                "Sub-workflow '{}' ended in unexpected state: {:?}",
                self.workflow_name, run.status
            ))),
        }
    }

    fn description(&self) -> String {
        format!(
            "Execute sub-workflow '{}' with inputs: {:?}",
            self.workflow_name, self.input_variables
        )
    }

    fn action_type(&self) -> &'static str {
        "sub_workflow"
    }

    impl_as_any!();
}

/// Format Claude output JSON line as YAML for better readability
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn format_claude_output_as_yaml(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Try to parse as JSON
    match serde_json::from_str::<Value>(trimmed) {
        Ok(json_value) => {
            // Process the JSON value to handle multiline strings
            let processed_value = process_json_for_yaml(&json_value);

            // Convert to YAML with custom formatting
            format_as_yaml(&processed_value)
        }
        Err(_) => trimmed.to_string(), // Return original if not valid JSON
    }
}

/// Process JSON values to prepare for YAML formatting
fn process_json_for_yaml(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut new_map = serde_json::Map::new();
            for (key, val) in map {
                new_map.insert(key.clone(), process_json_for_yaml(val));
            }
            Value::Object(new_map)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(process_json_for_yaml).collect()),
        _ => value.clone(),
    }
}

/// Format a JSON value as YAML with proper multiline handling
fn format_as_yaml(value: &Value) -> String {
    format_value_as_yaml(value, 0)
}

/// Recursively format a JSON value as YAML with indentation
fn format_value_as_yaml(value: &Value, indent_level: usize) -> String {
    let indent = "  ".repeat(indent_level);

    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            if s.contains('\n') {
                // Multiline string - check if it's source code
                let content_to_format = if let Some(language) = detect_source_code_language(s) {
                    // Apply syntax highlighting
                    highlight_source_code(s, language)
                } else {
                    s.clone()
                };

                // Use block scalar notation
                let lines: Vec<&str> = content_to_format.lines().collect();
                let mut result = "|-\n".to_string();
                let content_indent = "  ".repeat(indent_level + 1);
                for line in lines {
                    result.push_str(&format!("{content_indent}{line}\n"));
                }
                result.trim_end().to_string()
            } else {
                // Single line string - escape if necessary
                if needs_yaml_quotes(s) {
                    format!("\"{}\"", s.replace('\"', "\\\""))
                } else {
                    s.clone()
                }
            }
        }
        Value::Array(arr) => {
            if arr.is_empty() {
                "[]".to_string()
            } else {
                let mut result = String::new();
                for (i, item) in arr.iter().enumerate() {
                    if i > 0 {
                        result.push('\n');
                        result.push_str(&indent);
                    }
                    result.push_str("- ");
                    let item_str = format_value_as_yaml(item, indent_level + 1);
                    if item_str.contains('\n') {
                        // For multiline items, put on next line with proper indent
                        result.push('\n');
                        let item_indent = "  ".repeat(indent_level + 1);
                        for line in item_str.lines() {
                            result.push_str(&format!("{item_indent}{line}\n"));
                        }
                        result = result.trim_end().to_string();
                    } else {
                        result.push_str(&item_str);
                    }
                }
                result
            }
        }
        Value::Object(map) => {
            let mut result = String::new();
            let mut first = true;
            for (key, value) in map {
                if !first {
                    result.push('\n');
                    result.push_str(&indent);
                }
                first = false;

                result.push_str(&format!("{key}: "));
                let value_str = format_value_as_yaml(value, indent_level + 1);

                if value_str.contains('\n') && !matches!(value, Value::String(_)) {
                    // For nested objects/arrays, put on next line
                    result.push('\n');
                    let value_indent = "  ".repeat(indent_level + 1);
                    for line in value_str.lines() {
                        result.push_str(&format!("{value_indent}{line}\n"));
                    }
                    result = result.trim_end().to_string();
                } else {
                    result.push_str(&value_str);
                }
            }
            result
        }
    }
}

/// Check if a string needs to be quoted in YAML
fn needs_yaml_quotes(s: &str) -> bool {
    // YAML reserved words or special cases that need quotes
    matches!(
        s.to_lowercase().as_str(),
        "true" | "false" | "null" | "yes" | "no" | "on" | "off"
    ) || s.is_empty()
        || s.starts_with(|c: char| c.is_whitespace())
        || s.ends_with(|c: char| c.is_whitespace())
        || s.contains(':')
        || s.contains('#')
        || s.contains('&')
        || s.contains('*')
        || s.contains('!')
        || s.contains('|')
        || s.contains('>')
        || s.contains('[')
        || s.contains(']')
        || s.contains('{')
        || s.contains('}')
        || s.contains(',')
        || s.contains('?')
        || s.contains('-')
        || s.contains('\'')
        || s.contains('\"')
        || s.contains('\\')
        || s.contains('\n')
        || s.contains('\r')
        || s.contains('\t')
        || s.parse::<f64>().is_ok()
}

/// Detect if a string contains source code and return the detected language
fn detect_source_code_language(content: &str) -> Option<&'static str> {
    // Common code patterns and their associated languages
    let patterns = [
        // Rust patterns
        (
            r"(fn\s+\w+|impl\s+|trait\s+|use\s+\w+::|pub\s+(fn|struct|enum|trait)|let\s+(mut\s+)?\w+\s*=)",
            "rust",
        ),
        // Python patterns
        (
            r"(def\s+\w+\s*\(|class\s+\w+\s*(\(|:)|import\s+\w+|from\s+\w+\s+import)",
            "python",
        ),
        // JavaScript/TypeScript patterns
        (
            r"(function\s+\w+\s*\(|const\s+\w+\s*=|let\s+\w+\s*=|var\s+\w+\s*=|export\s+(default\s+)?|import\s+.*from)",
            "javascript",
        ),
        // Java patterns
        (
            r"(public\s+class\s+|private\s+|protected\s+|static\s+void\s+|import\s+java\.)",
            "java",
        ),
        // C/C++ patterns
        (
            r"(#include\s*<|int\s+main\s*\(|void\s+\w+\s*\(|class\s+\w+\s*\{|namespace\s+\w+)",
            "cpp",
        ),
        // Go patterns
        (
            r"(func\s+\w+\s*\(|package\s+\w+|import\s+\(|type\s+\w+\s+struct)",
            "go",
        ),
        // Ruby patterns
        (
            r#"(def\s+\w+|class\s+\w+\s*<|module\s+\w+|require\s+['\"])"#,
            "ruby",
        ),
        // Shell/Bash patterns
        (
            r"(#!/bin/(bash|sh)|function\s+\w+\s*\(\)|if\s+\[\[|\$\(|export\s+\w+=)",
            "bash",
        ),
    ];

    for (pattern, lang) in &patterns {
        if regex::Regex::new(pattern).ok()?.is_match(content) {
            return Some(lang);
        }
    }

    None
}

/// Apply syntax highlighting to source code
fn highlight_source_code(content: &str, language: &str) -> String {
    // Load syntax and theme sets
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let theme_set = ThemeSet::load_defaults();

    // Use a theme that works well in terminals
    let theme = &theme_set.themes["InspiredGitHub"];

    // Try to find the syntax for the language
    let syntax = syntax_set
        .find_syntax_by_token(language)
        .or_else(|| syntax_set.find_syntax_by_extension(language))
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut output = String::new();

    for line in content.lines() {
        if let Ok(highlighted_line) = highlighter.highlight_line(line, &syntax_set) {
            let escaped = as_24_bit_terminal_escaped(&highlighted_line[..], false);
            output.push_str(&escaped);
            output.push('\n');
        } else {
            // Fallback to unhighlighted line
            output.push_str(line);
            output.push('\n');
        }
    }

    // Reset terminal colors at the end
    output.push_str("\x1b[0m");
    output.trim_end().to_string()
}

/// Parse action from state description text with liquid template rendering
pub fn parse_action_from_description_with_context(
    description: &str,
    context: &HashMap<String, Value>,
) -> ActionResult<Option<Box<dyn Action>>> {
    // Create a mutable copy of the context to merge configuration variables
    let mut enhanced_context = context.clone();

    // Load and merge sah.toml configuration variables into the context
    // This uses the existing template integration infrastructure
    if let Err(e) = sah_config::load_and_merge_repo_config(&mut enhanced_context) {
        tracing::debug!(
            "Failed to load sah.toml configuration: {}. Continuing without config variables.",
            e
        );
    }

    let rendered_description = if let Some(template_vars) = enhanced_context.get("_template_vars") {
        // Extract template variables from context (now includes config variables)
        if let Some(vars_map) = template_vars.as_object() {
            // Convert to liquid Object
            let mut liquid_vars = liquid::Object::new();
            for (key, value) in vars_map {
                liquid_vars.insert(
                    key.clone().into(),
                    liquid::model::to_value(value).unwrap_or(liquid::model::Value::Nil),
                );
            }

            // Parse and render the template
            match liquid::ParserBuilder::with_stdlib()
                .build()
                .and_then(|parser| parser.parse(description))
            {
                Ok(template) => match template.render(&liquid_vars) {
                    Ok(rendered) => rendered,
                    Err(e) => {
                        tracing::warn!(
                            "Failed to render liquid template: {}. Using original text.",
                            e
                        );
                        description.to_string()
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse liquid template: {}. Using original text.",
                        e
                    );
                    description.to_string()
                }
            }
        } else {
            description.to_string()
        }
    } else {
        description.to_string()
    };

    parse_action_from_description(&rendered_description)
}

/// Parse action from state description text
pub fn parse_action_from_description(description: &str) -> ActionResult<Option<Box<dyn Action>>> {
    let parser = ActionParser::new()?;
    let description = description.trim();

    // Parse different action patterns using the robust parser
    if let Some(prompt_action) = parser.parse_prompt_action(description)? {
        return Ok(Some(Box::new(prompt_action)));
    }

    if let Some(wait_action) = parser.parse_wait_action(description)? {
        return Ok(Some(Box::new(wait_action)));
    }

    if let Some(log_action) = parser.parse_log_action(description)? {
        return Ok(Some(Box::new(log_action)));
    }

    if let Some(set_action) = parser.parse_set_variable_action(description)? {
        return Ok(Some(Box::new(set_action)));
    }

    if let Some(sub_workflow_action) = parser.parse_sub_workflow_action(description)? {
        return Ok(Some(Box::new(sub_workflow_action)));
    }

    if let Some(abort_action) = parser.parse_abort_action(description)? {
        return Ok(Some(Box::new(abort_action)));
    }

    if let Some(shell_action) = parser.parse_shell_action(description)? {
        return Ok(Some(Box::new(shell_action)));
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::action_parser::ActionParser;

    #[test]
    fn test_variable_substitution() {
        let mut context = HashMap::new();
        context.insert("file".to_string(), Value::String("test.rs".to_string()));
        context.insert("count".to_string(), Value::Number(42.into()));

        let result =
            substitute_variables_in_string("Process ${file} with ${count} items", &context);
        assert_eq!(result, "Process test.rs with 42 items");
    }

    #[test]
    fn test_parse_prompt_action() {
        let parser = ActionParser::new().unwrap();
        let desc = r#"Execute prompt "analyze-code" with file="test.rs" verbose="true""#;
        let action = parser.parse_prompt_action(desc).unwrap().unwrap();

        assert_eq!(action.prompt_name, "analyze-code");
        assert_eq!(action.arguments.get("file"), Some(&"test.rs".to_string()));
        assert_eq!(action.arguments.get("verbose"), Some(&"true".to_string()));
        assert!(!action.quiet); // Default should be false
    }

    #[test]
    fn test_prompt_action_with_quiet() {
        let action = PromptAction::new("test-prompt".to_string()).with_quiet(true);

        assert_eq!(action.prompt_name, "test-prompt");
        assert!(action.quiet);
        assert!(action.arguments.is_empty());
        assert!(action.result_variable.is_none());
    }

    #[test]
    fn test_prompt_action_builder_methods() {
        let mut args = HashMap::new();
        args.insert("key".to_string(), "value".to_string());

        let mut action = PromptAction::new("test".to_string())
            .with_quiet(true)
            .with_result_variable("result_var".to_string());

        // Add arguments manually since there's no with_arguments method
        action.arguments = args.clone();

        assert_eq!(action.prompt_name, "test");
        assert!(action.quiet);
        assert_eq!(action.result_variable, Some("result_var".to_string()));
        assert_eq!(action.arguments, args);
    }

    #[test]
    fn test_parse_wait_action() {
        let parser = ActionParser::new().unwrap();
        let action = parser
            .parse_wait_action("Wait for user confirmation")
            .unwrap()
            .unwrap();
        assert!(action.duration.is_none());

        let action = parser
            .parse_wait_action("Wait 30 seconds")
            .unwrap()
            .unwrap();
        assert_eq!(action.duration, Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_parse_log_action() {
        let parser = ActionParser::new().unwrap();
        let action = parser
            .parse_log_action(r#"Log "Hello world""#)
            .unwrap()
            .unwrap();
        assert_eq!(action.message, "Hello world");

        let action = parser
            .parse_log_action(r#"Log error "Something failed""#)
            .unwrap()
            .unwrap();
        assert_eq!(action.message, "Something failed");
    }

    #[test]
    fn test_parse_set_variable_action() {
        let parser = ActionParser::new().unwrap();
        let action = parser
            .parse_set_variable_action(r#"Set result="${claude_response}""#)
            .unwrap()
            .unwrap();
        assert_eq!(action.variable_name, "result");
        assert_eq!(action.value, "${claude_response}");
    }

    #[tokio::test]
    async fn test_log_action_execution() {
        let action = LogAction::info("Test message".to_string());
        let mut context = HashMap::new();

        let result = action.execute(&mut context).await.unwrap();
        assert_eq!(result, Value::String("Test message".to_string()));
        assert_eq!(
            context.get(LAST_ACTION_RESULT_KEY),
            Some(&Value::Bool(true))
        );
    }

    #[tokio::test]
    async fn test_set_variable_action_execution() {
        const TEST_VAR: &str = "test_var";
        const TEST_VALUE: &str = "test_value";

        let action = SetVariableAction::new(TEST_VAR.to_string(), TEST_VALUE.to_string());
        let mut context = HashMap::new();

        let result = action.execute(&mut context).await.unwrap();
        assert_eq!(result, Value::String(TEST_VALUE.to_string()));
        assert_eq!(
            context.get(TEST_VAR),
            Some(&Value::String(TEST_VALUE.to_string()))
        );
    }

    #[test]
    fn test_parse_sub_workflow_action() {
        let desc = r#"Run workflow "validation-workflow" with input="${data}""#;
        let action = parse_action_from_description(desc).unwrap().unwrap();
        assert_eq!(action.action_type(), "sub_workflow");
        assert_eq!(
            action.description(),
            r#"Execute sub-workflow 'validation-workflow' with inputs: {"input": "${data}"}"#
        );
    }

    #[tokio::test]
    async fn test_sub_workflow_circular_dependency_detection() {
        let action = SubWorkflowAction::new("workflow-a".to_string());
        let mut context = HashMap::new();

        // Simulate that workflow-a is already in the execution stack
        let workflow_stack = vec![
            Value::String("workflow-main".to_string()),
            Value::String("workflow-a".to_string()),
        ];
        context.insert(WORKFLOW_STACK_KEY.to_string(), Value::Array(workflow_stack));

        // This should fail with circular dependency error
        let result = action.execute(&mut context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        match error {
            ActionError::ExecutionError(msg) => {
                assert!(msg.contains("Circular workflow dependency detected"));
                assert!(msg.contains("workflow-a"));
            }
            _ => panic!("Expected ExecutionError for circular dependency"),
        }
    }

    #[test]
    fn test_sub_workflow_variable_substitution() {
        let mut action = SubWorkflowAction::new("validation-workflow".to_string());
        action
            .input_variables
            .insert("file".to_string(), "${current_file}".to_string());
        action
            .input_variables
            .insert("mode".to_string(), "strict".to_string());

        let mut context = HashMap::new();
        context.insert(
            "current_file".to_string(),
            Value::String("test.rs".to_string()),
        );

        let substituted = action.substitute_variables(&context);
        assert_eq!(substituted.get("file"), Some(&"test.rs".to_string()));
        assert_eq!(substituted.get("mode"), Some(&"strict".to_string()));
    }

    #[tokio::test]
    async fn test_prompt_action_without_retry() {
        let action = PromptAction::new("test-prompt".to_string());

        // Verify basic properties (retry logic removed)
        assert_eq!(action.prompt_name, "test-prompt");
        assert_eq!(action.timeout, Duration::from_secs(3600));
        assert!(!action.quiet);
        assert!(action.arguments.is_empty());
        assert!(action.result_variable.is_none());
    }

    #[test]
    fn test_prompt_action_builder_without_retry() {
        let action = PromptAction::new("test-prompt".to_string());

        assert_eq!(action.prompt_name, "test-prompt");

        // Test chaining with other builders (retry methods removed)
        let action2 = PromptAction::new("test-prompt2".to_string())
            .with_quiet(true)
            .with_timeout(Duration::from_secs(60));

        assert!(action2.quiet);
        assert_eq!(action2.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_parse_abort_action() {
        let action = parse_action_from_description("Abort \"Test error message\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.action_type(), "abort");
        assert_eq!(
            action.description(),
            "Abort workflow execution with message: Test error message"
        );
    }

    #[tokio::test]
    async fn test_abort_action_execution() {
        // Clean up any existing abort file before test
        let _ = std::fs::remove_file(".swissarmyhammer/.abort");

        let action = AbortAction::new("Test abort message".to_string());
        let mut context = HashMap::new();

        let result = action.execute(&mut context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        match error {
            ActionError::ExecutionError(msg) => {
                assert!(msg.contains("Test abort message"));
            }
            _ => panic!("Expected ExecutionError, got {error:?}"),
        }

        // Clean up abort file after test
        let _ = std::fs::remove_file(".swissarmyhammer/.abort");
    }

    #[tokio::test]
    async fn test_abort_action_with_variable_substitution() {
        // Clean up any existing abort file before test
        let _ = std::fs::remove_file(".swissarmyhammer/.abort");

        let action = AbortAction::new("Error in ${file}: ${error}".to_string());
        let mut context = HashMap::new();
        context.insert("file".to_string(), Value::String("test.rs".to_string()));
        context.insert(
            "error".to_string(),
            Value::String("compilation failed".to_string()),
        );

        let result = action.execute(&mut context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        match error {
            ActionError::ExecutionError(msg) => {
                assert!(msg.contains("Error in test.rs: compilation failed"));
            }
            _ => panic!("Expected ExecutionError, got {error:?}"),
        }

        // Clean up abort file after test
        let _ = std::fs::remove_file(".swissarmyhammer/.abort");
    }

    #[tokio::test]
    async fn test_end_to_end_error_propagation() {
        // Clean up any existing abort file before test
        let _ = std::fs::remove_file(".swissarmyhammer/.abort");

        // Test that abort errors propagate correctly through the system
        use crate::workflow::definition::Workflow;
        use crate::workflow::executor::core::WorkflowExecutor;
        use crate::workflow::executor::ExecutorError;
        use crate::workflow::state::{State, StateId, StateType};
        use crate::workflow::storage::WorkflowStorage;
        use crate::workflow::transition::{ConditionType, Transition, TransitionCondition};
        use crate::workflow::WorkflowName;
        use std::collections::HashMap;
        use std::sync::Arc;

        // Create a simple workflow with an abort action
        let abort_state = State {
            id: StateId::new("abort"),
            description: "Abort \"Test abort error\"".to_string(),
            state_type: StateType::Normal,
            is_terminal: true,
            allows_parallel: false,
            metadata: HashMap::new(),
        };

        let start_state = State {
            id: StateId::new("start"),
            description: "Log \"Starting test\"".to_string(),
            state_type: StateType::Normal,
            is_terminal: false,
            allows_parallel: false,
            metadata: HashMap::new(),
        };

        let transition = Transition {
            from_state: StateId::new("start"),
            to_state: StateId::new("abort"),
            condition: TransitionCondition {
                condition_type: ConditionType::Always,
                expression: None,
            },
            action: None,
            metadata: HashMap::new(),
        };

        let mut workflow = Workflow::new(
            WorkflowName::new("test-abort-workflow"),
            "Test workflow for abort error propagation".to_string(),
            StateId::new("start"),
        );

        workflow.add_state(start_state);
        workflow.add_state(abort_state);
        workflow.add_transition(transition);

        // Set up test storage
        let mut storage = WorkflowStorage::memory();
        storage.store_workflow(workflow.clone()).unwrap();
        let arc_storage = Arc::new(storage);
        set_test_storage(arc_storage);

        // Execute the workflow
        let mut executor = WorkflowExecutor::new();
        let mut run = executor.start_workflow(workflow).unwrap();

        // Execute the workflow - this should complete but mark itself as failed
        let result = executor.execute_state(&mut run).await;

        // Clean up test storage
        clear_test_storage();

        // With the new abort system, the workflow execution should return an Abort error
        match result {
            Err(executor_error) => match executor_error {
                ExecutorError::Abort(reason) => {
                    assert!(
                        reason.contains("Test abort error"),
                        "Abort reason should contain expected message: {}",
                        reason
                    );
                }
                _ => panic!("Expected ExecutorError::Abort, got: {:?}", executor_error),
            },
            Ok(_) => panic!("Expected abort error but workflow completed successfully"),
        }

        // Check that the abort action was executed - the context should contain the error
        // The abort action should have been executed and the workflow should have been marked as failed
        // This validates that the error propagation is working correctly

        // Clean up abort file after test
        let _ = std::fs::remove_file(".swissarmyhammer/.abort");
    }

    #[test]
    fn test_shell_action_new() {
        let action = ShellAction::new("echo hello".to_string());

        assert_eq!(action.command, "echo hello");
        assert!(action.timeout.is_none());
        assert!(action.result_variable.is_none());
        assert!(action.working_dir.is_none());
        assert!(action.environment.is_empty());
    }

    #[test]
    fn test_shell_action_builder_methods() {
        let mut env = HashMap::new();
        env.insert("KEY".to_string(), "value".to_string());

        let action = ShellAction::new("ls -la".to_string())
            .with_timeout(Duration::from_secs(30))
            .with_result_variable("output".to_string())
            .with_working_dir("/tmp".to_string())
            .with_environment(env.clone());

        assert_eq!(action.command, "ls -la");
        assert_eq!(action.timeout, Some(Duration::from_secs(30)));
        assert_eq!(action.result_variable, Some("output".to_string()));
        assert_eq!(action.working_dir, Some("/tmp".to_string()));
        assert_eq!(action.environment, env);
    }

    #[test]
    fn test_shell_action_description() {
        let action = ShellAction::new("git status".to_string());
        assert_eq!(action.description(), "Execute shell command: git status");
    }

    #[test]
    fn test_shell_action_type() {
        let action = ShellAction::new("pwd".to_string());
        assert_eq!(action.action_type(), "shell");
    }

    #[test]
    fn test_shell_action_variable_substitution() {
        let action = ShellAction::new("echo ${name}".to_string());
        let mut context = HashMap::new();
        context.insert("name".to_string(), Value::String("world".to_string()));

        let substituted_command = action.substitute_string(&action.command, &context);
        assert_eq!(substituted_command, "echo world");
    }

    #[test]
    fn test_shell_action_environment_variable_substitution() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin:${custom_path}".to_string());
        env.insert("USER".to_string(), "${current_user}".to_string());

        let action = ShellAction::new("env".to_string()).with_environment(env);

        let mut context = HashMap::new();
        context.insert(
            "custom_path".to_string(),
            Value::String("/opt/bin".to_string()),
        );
        context.insert(
            "current_user".to_string(),
            Value::String("testuser".to_string()),
        );

        let substituted_env = action.substitute_map(&action.environment, &context);
        assert_eq!(
            substituted_env.get("PATH"),
            Some(&"/usr/bin:/opt/bin".to_string())
        );
        assert_eq!(substituted_env.get("USER"), Some(&"testuser".to_string()));
    }

    #[test]
    fn test_shell_action_working_dir_substitution() {
        let action = ShellAction::new("ls".to_string())
            .with_working_dir("/home/${username}/projects".to_string());

        let mut context = HashMap::new();
        context.insert("username".to_string(), Value::String("alice".to_string()));

        let substituted_dir =
            action.substitute_string(action.working_dir.as_ref().unwrap(), &context);
        assert_eq!(substituted_dir, "/home/alice/projects");
    }

    #[test]
    fn test_shell_action_chaining_builder_methods() {
        let action = ShellAction::new("cargo build".to_string())
            .with_timeout(Duration::from_secs(120))
            .with_result_variable("build_output".to_string())
            .with_working_dir("./project".to_string());

        assert_eq!(action.command, "cargo build");
        assert_eq!(action.timeout, Some(Duration::from_secs(120)));
        assert_eq!(action.result_variable, Some("build_output".to_string()));
        assert_eq!(action.working_dir, Some("./project".to_string()));
    }

    #[test]
    fn test_parse_shell_action_integration() {
        // Test that shell actions are recognized by the main parser
        let action = parse_action_from_description("Shell \"echo hello\"")
            .unwrap()
            .unwrap();
        assert_eq!(action.action_type(), "shell");
        assert_eq!(action.description(), "Execute shell command: echo hello");

        // Test with parameters
        let action =
            parse_action_from_description("Shell \"ls -la\" with timeout=30 result=\"files\"")
                .unwrap()
                .unwrap();
        assert_eq!(action.action_type(), "shell");

        // Downcast to ShellAction to verify parameters
        let shell_action = action.as_any().downcast_ref::<ShellAction>().unwrap();
        assert_eq!(shell_action.command, "ls -la");
        assert_eq!(shell_action.timeout, Some(Duration::from_secs(30)));
        assert_eq!(shell_action.result_variable, Some("files".to_string()));
    }

    #[test]
    fn test_shell_action_dispatch_integration() {
        // Test complete integration through main dispatch function
        let test_cases = vec![
            ("Shell \"echo hello\"", "echo hello", None),
            ("Shell \"pwd\"", "pwd", None),
            (
                "Shell \"ls -la\" with timeout=60",
                "ls -la",
                Some(Duration::from_secs(60)),
            ),
        ];

        for (description, expected_command, expected_timeout) in test_cases {
            let action = parse_action_from_description(description).unwrap().unwrap();
            assert_eq!(action.action_type(), "shell", "Failed for: {description}");

            let shell_action = action.as_any().downcast_ref::<ShellAction>().unwrap();
            assert_eq!(
                shell_action.command, expected_command,
                "Command mismatch for: {description}"
            );
            assert_eq!(
                shell_action.timeout, expected_timeout,
                "Timeout mismatch for: {description}"
            );
        }
    }

    #[test]
    fn test_shell_action_module_export() {
        // Test that ShellAction can be imported via the workflow module
        use crate::workflow::ShellAction as WorkflowShellAction;

        let action = WorkflowShellAction::new("echo test".to_string());
        assert_eq!(action.command, "echo test");
        assert_eq!(action.action_type(), "shell");

        // Verify it can be used through the trait
        let trait_action: &dyn Action = &action;
        assert_eq!(trait_action.action_type(), "shell");
    }

    #[tokio::test]
    async fn test_shell_action_execution_success() {
        let action = ShellAction::new("echo hello world".to_string());
        let mut context = HashMap::new();

        let result = action.execute(&mut context).await.unwrap();

        // Verify success context variables are set
        assert_eq!(context.get("success"), Some(&Value::Bool(true)));
        assert_eq!(context.get("failure"), Some(&Value::Bool(false)));
        assert_eq!(context.get("exit_code"), Some(&Value::Number(0.into())));

        // Verify stdout contains expected output
        let stdout = context.get("stdout").unwrap().as_str().unwrap();
        assert!(stdout.contains("hello world"));

        // Verify stderr is empty or minimal
        let stderr = context.get("stderr").unwrap().as_str().unwrap();
        assert!(stderr.is_empty() || stderr.trim().is_empty());

        // Verify duration is tracked
        assert!(context.contains_key("duration_ms"));
        let _duration_ms = context.get("duration_ms").unwrap().as_u64().unwrap();
        // Duration is always >= 0 for u64, so just verify it exists

        // Verify last action result is success
        assert_eq!(
            context.get(LAST_ACTION_RESULT_KEY),
            Some(&Value::Bool(true))
        );

        // Result should contain stdout
        assert_eq!(result, context.get("stdout").unwrap().clone());
    }

    #[tokio::test]
    async fn test_shell_action_execution_failure() {
        let action = ShellAction::new("exit 42".to_string());
        let mut context = HashMap::new();

        let _result = action.execute(&mut context).await.unwrap();

        // Verify failure context variables are set
        assert_eq!(context.get("success"), Some(&Value::Bool(false)));
        assert_eq!(context.get("failure"), Some(&Value::Bool(true)));
        assert_eq!(context.get("exit_code"), Some(&Value::Number(42.into())));

        // Verify stdout and stderr are captured
        assert!(context.contains_key("stdout"));
        assert!(context.contains_key("stderr"));

        // Verify duration is tracked
        assert!(context.contains_key("duration_ms"));

        // Verify last action result is failure
        assert_eq!(
            context.get(LAST_ACTION_RESULT_KEY),
            Some(&Value::Bool(false))
        );
    }

    #[tokio::test]
    async fn test_shell_action_with_variable_substitution_execution() {
        let action = ShellAction::new("echo ${greeting} ${name}".to_string());
        let mut context = HashMap::new();
        context.insert("greeting".to_string(), Value::String("Hello".to_string()));
        context.insert("name".to_string(), Value::String("World".to_string()));

        let _result = action.execute(&mut context).await.unwrap();

        // Verify the command was substituted correctly
        let stdout = context.get("stdout").unwrap().as_str().unwrap();
        assert!(stdout.contains("Hello World"));

        // Verify success
        assert_eq!(context.get("success"), Some(&Value::Bool(true)));
    }

    #[tokio::test]
    async fn test_shell_action_with_result_variable() {
        let action = ShellAction::new("echo test output".to_string())
            .with_result_variable("command_output".to_string());
        let mut context = HashMap::new();

        let result = action.execute(&mut context).await.unwrap();

        // Verify result variable is set
        assert!(context.contains_key("command_output"));
        let command_output = context.get("command_output").unwrap();
        assert_eq!(command_output, &result);

        // Result should be the stdout
        let stdout = context.get("stdout").unwrap().as_str().unwrap();
        assert!(stdout.contains("test output"));
        assert_eq!(result, context.get("stdout").unwrap().clone());
    }

    #[tokio::test]
    async fn test_shell_action_with_working_directory() {
        use std::fs;
        use tempfile::TempDir;

        // Create a unique temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        let action = ShellAction::new("pwd".to_string())
            .with_working_dir(temp_path.to_string_lossy().to_string());
        let mut context = HashMap::new();

        let _result = action.execute(&mut context).await.unwrap();

        // Verify success
        assert_eq!(context.get("success"), Some(&Value::Bool(true)));

        // Verify the working directory was used
        let stdout = context.get("stdout").unwrap().as_str().unwrap().trim();
        let canonical_temp_path = fs::canonicalize(temp_path).unwrap();
        let canonical_stdout_path = fs::canonicalize(stdout).unwrap_or_else(|_| stdout.into());

        assert_eq!(canonical_stdout_path, canonical_temp_path);
    }

    #[tokio::test]
    async fn test_shell_action_with_environment_variables() {
        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "test_value".to_string());
        env.insert("ANOTHER_VAR".to_string(), "another_value".to_string());

        let action =
            ShellAction::new("echo $TEST_VAR $ANOTHER_VAR".to_string()).with_environment(env);
        let mut context = HashMap::new();

        let _result = action.execute(&mut context).await.unwrap();

        // Verify success
        assert_eq!(context.get("success"), Some(&Value::Bool(true)));

        // Verify environment variables were set
        let stdout = context.get("stdout").unwrap().as_str().unwrap();
        assert!(stdout.contains("test_value"));
        assert!(stdout.contains("another_value"));
    }

    #[tokio::test]
    async fn test_shell_action_with_environment_variable_substitution() {
        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "${dynamic_value}".to_string());

        let action = ShellAction::new("echo $TEST_VAR".to_string()).with_environment(env);
        let mut context = HashMap::new();
        context.insert(
            "dynamic_value".to_string(),
            Value::String("substituted".to_string()),
        );

        let _result = action.execute(&mut context).await.unwrap();

        // Verify success
        assert_eq!(context.get("success"), Some(&Value::Bool(true)));

        // Verify environment variable substitution worked
        let stdout = context.get("stdout").unwrap().as_str().unwrap();
        assert!(stdout.contains("substituted"));
    }

    #[tokio::test]
    async fn test_shell_action_timeout() {
        use std::time::Duration;

        // Create an action with a very short timeout
        let action =
            ShellAction::new("sleep 10".to_string()).with_timeout(Duration::from_millis(200));
        let mut context = HashMap::new();

        let result = action.execute(&mut context).await.unwrap();

        // Verify timeout results in failure state
        assert_eq!(context.get("success"), Some(&Value::Bool(false)));
        assert_eq!(context.get("failure"), Some(&Value::Bool(true)));

        // Exit code should indicate timeout (-1)
        assert_eq!(context.get("exit_code"), Some(&Value::Number((-1).into())));

        // Verify stderr contains timeout message
        assert_eq!(
            context.get("stderr"),
            Some(&Value::String("Command timed out".to_string()))
        );

        // Verify stdout is empty for timeout
        assert_eq!(context.get("stdout"), Some(&Value::String("".to_string())));

        // Duration should be tracked and around the timeout duration
        assert!(context.contains_key("duration_ms"));
        let duration_ms = context.get("duration_ms").unwrap().as_u64().unwrap();
        // Should be around 200ms or slightly more (allowing for process cleanup)
        assert!((200..1000).contains(&duration_ms));

        // Result should be false for timeout
        assert_eq!(result, Value::Bool(false));

        // Last action result should indicate failure
        assert_eq!(
            context.get(LAST_ACTION_RESULT_KEY),
            Some(&Value::Bool(false))
        );
    }

    #[tokio::test]
    async fn test_shell_action_timeout_with_result_variable() {
        use std::time::Duration;

        // Create an action with timeout and result variable
        let action = ShellAction::new("sleep 5".to_string())
            .with_timeout(Duration::from_millis(100))
            .with_result_variable("output".to_string());
        let mut context = HashMap::new();

        let _result = action.execute(&mut context).await.unwrap();

        // Verify timeout results in failure state
        assert_eq!(context.get("success"), Some(&Value::Bool(false)));
        assert_eq!(context.get("failure"), Some(&Value::Bool(true)));

        // Result variable should NOT be set on timeout
        assert!(!context.contains_key("output"));

        // Verify timeout context variables
        assert_eq!(
            context.get("stderr"),
            Some(&Value::String("Command timed out".to_string()))
        );
        assert_eq!(context.get("stdout"), Some(&Value::String("".to_string())));
    }

    #[tokio::test]
    async fn test_shell_action_no_timeout_by_default() {
        // Test that commands without explicit timeout still work
        let action = ShellAction::new("echo no timeout test".to_string());
        let mut context = HashMap::new();

        let result = action.execute(&mut context).await.unwrap();

        // Verify success
        assert_eq!(context.get("success"), Some(&Value::Bool(true)));
        assert_eq!(context.get("failure"), Some(&Value::Bool(false)));

        // Verify output
        let stdout = context.get("stdout").unwrap().as_str().unwrap();
        assert!(stdout.contains("no timeout test"));

        // Result should contain output
        assert_eq!(result, context.get("stdout").unwrap().clone());
    }

    #[tokio::test]
    async fn test_shell_action_successful_within_timeout() {
        use std::time::Duration;

        // Command that completes quickly within timeout
        let action =
            ShellAction::new("echo quick command".to_string()).with_timeout(Duration::from_secs(5));
        let mut context = HashMap::new();

        let result = action.execute(&mut context).await.unwrap();

        // Verify success
        assert_eq!(context.get("success"), Some(&Value::Bool(true)));
        assert_eq!(context.get("failure"), Some(&Value::Bool(false)));

        // Exit code should be 0
        assert_eq!(context.get("exit_code"), Some(&Value::Number(0.into())));

        // Verify output
        let stdout = context.get("stdout").unwrap().as_str().unwrap();
        assert!(stdout.contains("quick command"));

        // Duration should be much less than timeout
        let duration_ms = context.get("duration_ms").unwrap().as_u64().unwrap();
        assert!(duration_ms < 1000); // Should complete in less than 1 second

        // Result should contain output
        assert_eq!(result, context.get("stdout").unwrap().clone());
    }

    #[tokio::test]
    async fn test_shell_action_timeout_process_cleanup() {
        use std::time::Duration;

        // This test verifies that timeout processes are properly terminated
        // If the process cleanup didn't work, this test would hang for 30 seconds
        let action =
            ShellAction::new("sleep 30".to_string()).with_timeout(Duration::from_millis(150));
        let mut context = HashMap::new();

        let start_time = std::time::Instant::now();
        let _result = action.execute(&mut context).await.unwrap();
        let elapsed = start_time.elapsed();

        // Verify the command was terminated quickly (within reasonable bounds)
        // This proves the process was actually killed, not just timed out
        assert!(
            elapsed < Duration::from_secs(10),
            "Process cleanup took too long: {elapsed:?}. This indicates the process was not properly terminated."
        );

        // Verify timeout state is correctly set
        assert_eq!(context.get("success"), Some(&Value::Bool(false)));
        assert_eq!(context.get("failure"), Some(&Value::Bool(true)));
        assert_eq!(context.get("exit_code"), Some(&Value::Number((-1).into())));
        assert_eq!(
            context.get("stderr"),
            Some(&Value::String("Command timed out".to_string()))
        );
        assert_eq!(context.get("stdout"), Some(&Value::String("".to_string())));
    }

    #[tokio::test]
    #[cfg(target_os = "windows")]
    async fn test_shell_action_windows_command_format() {
        // Test that Windows uses cmd /C for shell commands
        let action = ShellAction::new("echo windows test".to_string());
        let mut context = HashMap::new();

        let _result = action.execute(&mut context).await.unwrap();

        // Verify success - this confirms the Windows cmd /C format worked
        assert_eq!(context.get("success"), Some(&Value::Bool(true)));
        let stdout = context.get("stdout").unwrap().as_str().unwrap();
        assert!(stdout.contains("windows test"));
    }

    #[tokio::test]
    #[cfg(not(target_os = "windows"))]
    async fn test_shell_action_unix_command_format() {
        // Test that Unix systems use sh -c for shell commands
        let action = ShellAction::new("echo unix test".to_string());
        let mut context = HashMap::new();

        let _result = action.execute(&mut context).await.unwrap();

        // Verify success - this confirms the Unix sh -c format worked
        assert_eq!(context.get("success"), Some(&Value::Bool(true)));
        let stdout = context.get("stdout").unwrap().as_str().unwrap();
        assert!(stdout.contains("unix test"));
    }
}
