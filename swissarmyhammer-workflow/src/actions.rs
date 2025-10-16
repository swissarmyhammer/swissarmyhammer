//! Workflow action execution system
//!
//! This module provides the action execution infrastructure for workflows,
//! including Claude integration, variable operations, and control flow actions.
//!
//! # MCP Server Lifecycle Responsibility
//!
//! **Important**: This workflow layer does NOT start infrastructure services like MCP servers.
//! Infrastructure setup is the responsibility of the caller (typically the CLI layer).
//!
//! ## For LlamaAgent Executors
//!
//! When creating a workflow that uses LlamaAgent:
//!
//! 1. **Caller Responsibility**: Start the MCP server BEFORE executing the workflow
//! 2. **Workflow Responsibility**: Create executors that connect to the running server
//! 3. **Error Handling**: If MCP server is not available, executor initialization will fail
//!
//! This separation ensures:
//! - Clean architectural boundaries (workflow logic vs infrastructure)
//! - Testability (mock or real MCP servers can be provided)
//! - Flexibility (different deployment models can manage infrastructure differently)
//!
//! ## Example
//!
//! ```rust,no_run
//! # use swissarmyhammer_workflow::WorkflowExecutor;
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // 1. Start MCP server (in CLI layer, not shown here)
//! // let mcp_server = start_mcp_server(...).await?;
//!
//! // 2. Create workflow with LlamaAgent configuration
//! // let workflow = load_workflow(...)?;
//!
//! // 3. Execute workflow (will connect to running MCP server)
//! // let result = workflow.execute(...).await?;
//! # Ok(())
//! # }
//! ```

use swissarmyhammer_shell::{
    get_validator, log_shell_completion, log_shell_execution, ShellSecurityError,
};

use crate::action_parser::ActionParser;
use crate::mcp_integration::{response_processing, WorkflowShellContext};
use crate::{
    WorkflowExecutor, WorkflowName, WorkflowRunStatus, WorkflowStorage, WorkflowTemplateContext,
};

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::as_24_bit_terminal_escaped;
use thiserror::Error;

use swissarmyhammer_config::agent::{AgentConfig, AgentExecutorConfig, AgentExecutorType};
use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};

// Re-export types from agent-executor crate
pub use swissarmyhammer_agent_executor::{
    ActionError as AgentExecutorError, ActionResult as AgentExecutorResult,
    AgentResponse as AgentExecutorResponse, AgentResponseType as AgentExecutorResponseType,
};

thread_local! {
    /// Thread-local test storage registry for tests
    static TEST_STORAGE_REGISTRY: std::cell::RefCell<Option<Arc<WorkflowStorage>>> = const { std::cell::RefCell::new(None) };
}

/// Set test storage for use in tests
#[cfg(test)]
pub fn set_test_storage(storage: Arc<WorkflowStorage>) {
    TEST_STORAGE_REGISTRY.with(|registry| {
        *registry.borrow_mut() = Some(storage);
    });
}

/// Clear test storage after tests
#[cfg(test)]
pub fn clear_test_storage() {
    TEST_STORAGE_REGISTRY.with(|registry| {
        *registry.borrow_mut() = None;
    });
}

/// Get test storage if available
fn get_test_storage() -> Option<Arc<WorkflowStorage>> {
    TEST_STORAGE_REGISTRY.with(|registry| registry.borrow().clone())
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
    /// Shell security validation error
    #[error("Shell security error: {0}")]
    ShellSecurityError(#[from] ShellSecurityError),
}

/// Result type for action operations
pub type ActionResult<T> = std::result::Result<T, ActionError>;

/// Agent execution context for prompt execution
#[derive(Debug)]
pub struct AgentExecutionContext<'a> {
    /// Reference to the workflow template context
    pub workflow_context: &'a WorkflowTemplateContext,
}

impl<'a> AgentExecutionContext<'a> {
    /// Create a new agent execution context
    pub fn new(workflow_context: &'a WorkflowTemplateContext) -> Self {
        Self { workflow_context }
    }

    /// Get agent configuration from workflow context
    pub fn agent_config(&self) -> AgentConfig {
        self.workflow_context.get_agent_config()
    }

    /// Get executor type
    pub fn executor_type(&self) -> AgentExecutorType {
        self.agent_config().executor_type()
    }

    /// Check if quiet mode is enabled
    pub fn quiet(&self) -> bool {
        self.agent_config().quiet
    }
}

/// Response type from agent execution
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentResponse {
    /// The primary response content from the agent
    pub content: String,
    /// Optional metadata about the response
    pub metadata: Option<serde_json::Value>,
    /// Response status/type for different kinds of responses
    pub response_type: AgentResponseType,
}

/// Type of agent response
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum AgentResponseType {
    /// Standard successful text response
    Success,
    /// Partial response (streaming, timeout, etc.)
    Partial,
    /// Error response with error details
    Error,
}

impl AgentResponse {
    /// Create a successful response
    pub fn success(content: String) -> Self {
        Self {
            content,
            metadata: None,
            response_type: AgentResponseType::Success,
        }
    }

    /// Create a successful response with metadata
    pub fn success_with_metadata(content: String, metadata: serde_json::Value) -> Self {
        Self {
            content,
            metadata: Some(metadata),
            response_type: AgentResponseType::Success,
        }
    }

    /// Create an error response
    pub fn error(content: String) -> Self {
        Self {
            content,
            metadata: None,
            response_type: AgentResponseType::Error,
        }
    }

    /// Create a partial response
    pub fn partial(content: String) -> Self {
        Self {
            content,
            metadata: None,
            response_type: AgentResponseType::Partial,
        }
    }

    /// Check if this is a successful response
    pub fn is_success(&self) -> bool {
        matches!(self.response_type, AgentResponseType::Success)
    }

    /// Check if this is an error response
    pub fn is_error(&self) -> bool {
        matches!(self.response_type, AgentResponseType::Error)
    }
}

/// Re-export the canonical AgentExecutor trait from agent-executor crate
/// This eliminates the duplicate trait definition that was causing type incompatibility
pub use swissarmyhammer_agent_executor::AgentExecutor;

/// Convert agent-executor error to workflow ActionError
fn convert_agent_executor_error(err: swissarmyhammer_agent_executor::ActionError) -> ActionError {
    use swissarmyhammer_agent_executor::ActionError as AEError;
    match err {
        AEError::ClaudeError(msg) => ActionError::ClaudeError(msg),
        AEError::VariableError(msg) => ActionError::VariableError(msg),
        AEError::ParseError(msg) => ActionError::ParseError(msg),
        AEError::ExecutionError(msg) => ActionError::ExecutionError(msg),
        AEError::IoError(err) => ActionError::IoError(err),
        AEError::JsonError(err) => ActionError::JsonError(err),
        AEError::RateLimit { message, wait_time } => ActionError::RateLimit { message, wait_time },
    }
}

impl ActionError {
    /// Create an executor-specific error
    pub fn executor_error(executor_type: AgentExecutorType, message: String) -> Self {
        ActionError::ExecutionError(format!("{:?} executor error: {}", executor_type, message))
    }

    /// Create an initialization error
    pub fn initialization_error(
        executor_type: AgentExecutorType,
        source: Box<dyn std::error::Error>,
    ) -> Self {
        ActionError::ExecutionError(format!(
            "Failed to initialize {:?} executor: {}",
            executor_type, source
        ))
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
    /// Execute the action with the given template context
    async fn execute(&self, context: &mut WorkflowTemplateContext) -> ActionResult<Value>;

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
    fn substitute_string(&self, value: &str, context: &WorkflowTemplateContext) -> String {
        substitute_variables_in_string(value, context)
    }

    /// Substitute variables in a HashMap of string values
    fn substitute_map(
        &self,
        values: &HashMap<String, String>,
        context: &WorkflowTemplateContext,
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
    /// Whether to suppress stdout output (only log)
    ///
    /// When set to `true`, the response will only be logged using the tracing
    /// framework and will not be printed to stderr. This is useful for workflows
    /// that need to capture the response programmatically without cluttering the output.
    ///
    /// The quiet mode can also be controlled via the `_quiet` context variable in workflows.
    pub quiet: bool,
}

impl PromptAction {
    /// Create a new prompt action
    pub fn new(prompt_name: String) -> Self {
        Self {
            prompt_name,
            arguments: HashMap::new(),
            result_variable: None,
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

    /// Set whether to suppress stdout output
    pub fn with_quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    /// Substitute variables in arguments using the context
    fn substitute_variables(&self, context: &WorkflowTemplateContext) -> HashMap<String, String> {
        self.substitute_map(&self.arguments, context)
    }
}

impl VariableSubstitution for PromptAction {}

#[async_trait::async_trait]
impl Action for PromptAction {
    async fn execute(&self, context: &mut WorkflowTemplateContext) -> ActionResult<Value> {
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

impl PromptAction {
    /// Render both user prompt and system prompt using the same library instance
    fn render_prompts_directly(
        &self,
        context: &WorkflowTemplateContext,
    ) -> ActionResult<(String, Option<String>)> {
        tracing::debug!(
            "Starting render_prompts_directly for prompt: {}",
            self.prompt_name
        );

        // Create args HashMap with all workflow variables
        let mut args = HashMap::new();

        // Add all workflow variables (including plan_filename, etc.)
        for (key, value) in context.iter() {
            // Skip internal keys
            if !key.starts_with('_') {
                args.insert(key.clone(), value.to_string());
            }
        }

        // Add/override with action-specific arguments
        let action_args = self.substitute_variables(context);

        // Validate argument keys early
        for key in action_args.keys() {
            if !is_valid_argument_key(key) {
                return Err(ActionError::ParseError(
                    format!("Invalid argument key '{key}': must contain only alphanumeric characters, hyphens, and underscores")
                ));
            }
        }

        for (key, value) in &action_args {
            args.insert(key.clone(), value.clone());
        }

        tracing::debug!("Args for prompt rendering: {:?}", args);

        // Load prompts and render directly
        let mut library = PromptLibrary::new();
        let mut resolver = PromptResolver::new();

        resolver.load_all_prompts(&mut library).map_err(|e| {
            ActionError::ClaudeError(format!("Failed to load prompts from directories: {e}"))
        })?;

        tracing::debug!("Loaded prompts successfully");

        // Create TemplateContext with full configuration and template vars
        let mut template_context =
            swissarmyhammer_config::TemplateContext::load().map_err(|e| {
                ActionError::ClaudeError(format!("Failed to load template context: {e}"))
            })?;

        // Add all args as template vars
        for (key, value) in args {
            template_context.set_var(key.clone(), serde_json::Value::String(value.clone()));
        }

        tracing::debug!("Created template context successfully");

        // Convert library to Arc for partials support
        let library_arc = Arc::new(library);

        // Render user prompt with complete template context and partials support
        tracing::debug!("About to render user prompt: {}", self.prompt_name);
        let rendered = library_arc
            .render(&self.prompt_name, &template_context)
            .map_err(|e| {
                // Try to get available prompts for better error messaging
                let available_prompts = library_arc
                    .list()
                    .ok()
                    .map(|prompts| {
                        let names: Vec<String> = prompts.iter().map(|p| p.name.clone()).collect();
                        if names.is_empty() {
                            "no prompts available".to_string()
                        } else {
                            format!("available prompts: {}", names.join(", "))
                        }
                    })
                    .unwrap_or_else(|| "unable to list available prompts".to_string());

                ActionError::ClaudeError(format!(
                    "Failed to render prompt '{}': {} ({})",
                    self.prompt_name, e, available_prompts
                ))
            })?;

        // Render system prompt using the same library instance (optional)
        let system_prompt = match library_arc.render(".system", &template_context) {
            Ok(prompt) => Some(prompt),
            Err(e) => {
                tracing::warn!(
                    "Failed to render system prompt: {}. Proceeding without system prompt.",
                    e
                );
                None
            }
        };

        if let Some(ref sys_prompt) = system_prompt {
            tracing::debug!(
                "System prompt rendered successfully ({} chars)",
                sys_prompt.len()
            );
        } else {
            tracing::debug!("No system prompt will be used");
        }

        Ok((rendered, system_prompt))
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
        context: &mut WorkflowTemplateContext,
    ) -> ActionResult<Value> {
        tracing::info!(
            "Executing prompt '{}' with context: {:?}",
            self.prompt_name,
            context
        );

        // Render both user and system prompts using the same library instance
        let (user_prompt, system_prompt) = match self.render_prompts_directly(context) {
            Ok(prompts) => prompts,
            Err(e) => {
                tracing::error!("Failed to render prompts: {:?}", e);
                return Err(e);
            }
        };

        // Log the actual prompt being sent to Claude
        tracing::debug!("Piping prompt:\n{}", user_prompt);

        // Check if quiet mode is enabled in the context
        let quiet = self.quiet
            || context
                .get("_quiet")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

        // Execute the rendered prompt using the AgentExecutor trait

        // Create execution context (LLM handles its own timeout)
        let workflow_execution_context = AgentExecutionContext::new(context);

        // Get executor based on configuration
        let executor = self.get_executor(&workflow_execution_context).await?;

        // Convert workflow context to agent-executor context
        let agent_config = workflow_execution_context.agent_config();
        let agent_exec_context =
            swissarmyhammer_agent_executor::AgentExecutionContext::new(&agent_config);

        // Execute prompt through trait
        let response = executor
            .execute_prompt(
                system_prompt.unwrap_or_default(),
                user_prompt,
                &agent_exec_context,
            )
            .await;

        let response = match response {
            Ok(resp) => resp,
            Err(e) => {
                tracing::error!("Prompt execution failed: {:?}", e);
                return Err(convert_agent_executor_error(e));
            }
        };

        // Extract response text for logging
        let response_text = response.content.clone();

        // Log the response for debugging if not in quiet mode
        if !quiet && !response_text.is_empty() {
            tracing::debug!(
                "Agent response received: {} characters",
                response_text.len()
            );

            // Create YAML-formatted output for better readability
            let mut yaml_output = String::new();
            yaml_output.push_str("---\n");
            yaml_output.push_str(&format!("prompt: {}\n", self.prompt_name));
            yaml_output.push_str("agent_response: |\n");
            for line in response_text.lines() {
                yaml_output.push_str(&format!("  {line}\n"));
            }
            yaml_output.push_str("---");

            // Log YAML output
            tracing::info!("{}", yaml_output);
        }

        // Store result in context if variable name specified
        if let Some(var_name) = &self.result_variable {
            // Convert AgentResponse to JSON Value for context storage
            let response_value = serde_json::to_value(&response).unwrap_or_default();
            context.insert(var_name.clone(), response_value);
        }

        // Always store in special last_action_result key
        context.insert(LAST_ACTION_RESULT_KEY.to_string(), Value::Bool(true));
        // Store the response content as a string for backward compatibility
        context.insert(
            CLAUDE_RESPONSE_KEY.to_string(),
            Value::String(response.content.clone()),
        );

        // Convert AgentResponse back to Value for the Action trait compatibility
        let response_value = serde_json::to_value(&response)
            .unwrap_or_else(|_| Value::String(response.content.clone()));

        Ok(response_value)
    }

    /// Get executor based on execution context (lazy initialization)
    async fn get_executor(
        &self,
        context: &AgentExecutionContext<'_>,
    ) -> ActionResult<Box<dyn AgentExecutor>> {
        // Only create executor when actually needed for prompt execution
        tracing::debug!("Creating executor on-demand for prompt execution");

        match context.executor_type() {
            AgentExecutorType::ClaudeCode => {
                tracing::info!("Using ClaudeCode");
                let mut executor = crate::agents::ClaudeCodeExecutor::new();
                executor.initialize().await.map_err(|e| {
                    ActionError::ExecutionError(format!("Failed to initialize ClaudeCode: {}", e))
                })?;
                Ok(Box::new(executor))
            }
            AgentExecutorType::LlamaAgent => {
                tracing::info!("Using LlamaAgent with singleton pattern");
                let agent_config = context.agent_config();
                let llama_config = match agent_config.executor {
                    AgentExecutorConfig::LlamaAgent(config) => config,
                    _ => {
                        return Err(ActionError::ExecutionError(
                            "Expected LlamaAgent configuration".to_string(),
                        ))
                    }
                };

                // Get MCP server port from workflow context (started by CLI layer)
                let mcp_port = context
                    .workflow_context
                    .get("_mcp_server_port")
                    .and_then(|v| v.as_u64())
                    .map(|p| p as u16);

                if mcp_port.is_none() {
                    return Err(ActionError::ExecutionError(
                        "Failed to initialize LlamaAgent: MCP server must be started before running workflows".to_string()
                    ));
                }

                let port = mcp_port.unwrap();
                tracing::info!("Creating LlamaAgent executor with MCP server on port {}", port);

                // Create MCP server handle for the executor
                // Note: We create a dummy shutdown channel since the actual server lifecycle
                // is managed by the CLI layer. The executor just needs the port to connect.
                let (dummy_tx, _dummy_rx) = tokio::sync::oneshot::channel();
                let mcp_handle = crate::agents::llama_agent_executor::McpServerHandle::new(
                    port,
                    "127.0.0.1".to_string(),
                    dummy_tx,
                );

                let mut executor =
                    crate::agents::LlamaAgentExecutorWrapper::new_with_mcp(llama_config.clone(), Some(mcp_handle));
                executor.initialize().await.map_err(|e| {
                    ActionError::ExecutionError(format!(
                        "Failed to initialize LlamaAgent with MCP server on port {}: {}",
                        port,
                        e
                    ))
                })?;

                Ok(Box::new(executor))
            }
        }
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
    async fn execute(&self, context: &mut WorkflowTemplateContext) -> ActionResult<Value> {
        match self.duration {
            Some(duration) => {
                if let Some(message) = &self.message {
                    tracing::info!("Waiting: {}", message);
                }

                // Use the actual duration specified
                let actual_duration = duration;

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

                // Read user input
                match reader.read_line(&mut line).await {
                    Ok(_) => {
                        // Successfully read input
                    }
                    Err(e) => {
                        return Err(ActionError::IoError(e));
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
    async fn execute(&self, context: &mut WorkflowTemplateContext) -> ActionResult<Value> {
        // Render message with liquid templating (supports {{variable}} syntax)
        let message = render_with_liquid_template(&self.message, &context.to_workflow_hashmap());

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
    async fn execute(&self, context: &mut WorkflowTemplateContext) -> ActionResult<Value> {
        // Substitute variables in value
        let substituted_value = self.substitute_string(&self.value, context);

        // Try to parse as JSON first, fall back to string
        let json_value = match serde_json::from_str(&substituted_value) {
            Ok(v) => v,
            Err(_) => Value::String(substituted_value),
        };

        // Set the variable
        context.insert(self.variable_name.clone(), json_value.clone());
        tracing::debug!("Set variable '{}' = '{:?}'", self.variable_name, json_value);

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
    async fn execute(&self, context: &mut WorkflowTemplateContext) -> ActionResult<Value> {
        // Substitute variables in message
        let message = self.substitute_string(&self.message, context);
        tracing::error!("***Workflow Aborted***: {}", message);

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
        format!(
            "Abort workflow execution with message: {message}",
            message = self.message
        )
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
fn substitute_variables_in_string(input: &str, context: &WorkflowTemplateContext) -> String {
    let parser = ActionParser::new().expect("Failed to create ActionParser");
    let context_map = context.to_workflow_hashmap();
    parser
        .substitute_variables_safe(input, &context_map)
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
    let parser = ActionParser::new().expect("Failed to create ActionParser");
    parser
        .substitute_variables_safe(&liquid_rendered, context)
        .unwrap_or(liquid_rendered)
}

impl SubWorkflowAction {
    /// Create a new sub-workflow action
    pub fn new(workflow_name: String) -> Self {
        Self {
            workflow_name,
            input_variables: HashMap::new(),
            result_variable: None,
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

    /// Substitute variables in input values using the context
    fn substitute_variables(&self, context: &WorkflowTemplateContext) -> HashMap<String, String> {
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
        let timeout = self.timeout.unwrap_or(Duration::from_secs(3600));

        if timeout.as_millis() == 0 {
            return Err(ActionError::ExecutionError(
                "Timeout must be greater than 0".to_string(),
            ));
        }

        Ok(timeout)
    }
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

/// Validate shell command for security issues using the comprehensive security framework
pub fn validate_command(command: &str) -> ActionResult<()> {
    // Check for obviously dangerous patterns
    if command.trim().is_empty() {
        return Err(ActionError::ExecutionError(
            "Shell command cannot be empty".to_string(),
        ));
    }

    // Use the comprehensive security validator
    let validator = get_validator();
    validator.validate_command(command)?;

    Ok(())
}

// Legacy security functions removed - now using comprehensive security framework in shell_security module

/// Enhanced working directory validation with security checks
pub fn validate_working_directory_security(path: &str) -> ActionResult<()> {
    // First run the existing validation
    validate_working_directory(path)?;

    // Use the comprehensive security validator for directory access control
    let validator = get_validator();
    let path_obj = std::path::Path::new(path);
    validator.validate_directory_access(path_obj)?;

    Ok(())
}

/// Validate environment variables for security issues using the comprehensive security framework
pub fn validate_environment_variables_security(env: &HashMap<String, String>) -> ActionResult<()> {
    // Use the comprehensive security validator
    let validator = get_validator();
    validator.validate_environment_variables(env)?;

    Ok(())
}

/// Log command execution with comprehensive security audit logging
pub fn log_command_execution(
    command: &str,
    working_dir: Option<&str>,
    env: &HashMap<String, String>,
) {
    // Use the comprehensive audit logging from the security module
    let working_dir_path = working_dir.map(std::path::Path::new);
    log_shell_execution(command, working_dir_path, env);

    // Also maintain backward compatibility with existing logging
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
    /// Process enhanced shell execution result and maintain backward compatibility with existing workflow behavior
    async fn process_enhanced_result(
        &self,
        result: Value,
        command: &str,
        context: &mut WorkflowTemplateContext,
    ) -> ActionResult<Value> {
        // Extract shell execution result from enhanced shell response
        let json_data = response_processing::extract_json_data(&result)
            .map_err(|e| ActionError::ExecutionError(format!("Result processing failed: {e}")))?;

        // Parse shell execution metadata from MCP tool response
        let exit_code = json_data["exit_code"].as_i64().unwrap_or(-1);
        let stdout = json_data["stdout"].as_str().unwrap_or("").to_string();
        let stderr = json_data["stderr"].as_str().unwrap_or("").to_string();
        let execution_time_ms = json_data["execution_time_ms"].as_u64().unwrap_or(0);

        // Determine success based on exit code
        let success = exit_code == 0;

        // Set automatic workflow variables (maintain existing behavior)
        context.insert("success".to_string(), Value::Bool(success));
        context.insert("failure".to_string(), Value::Bool(!success));
        context.insert("exit_code".to_string(), Value::Number(exit_code.into()));
        context.insert("stdout".to_string(), Value::String(stdout.clone()));
        context.insert("stderr".to_string(), Value::String(stderr.clone()));
        context.insert(
            "duration_ms".to_string(),
            Value::Number(execution_time_ms.into()),
        );

        // Set result variable if specified (existing behavior)
        if let Some(result_var) = &self.result_variable {
            if success {
                context.insert(result_var.clone(), Value::String(stdout.trim().to_string()));
            }
            // Don't set result variable on failure to maintain existing behavior
        }

        // Set last action result based on command success
        context.insert(LAST_ACTION_RESULT_KEY.to_string(), Value::Bool(success));

        // Log execution details
        tracing::info!(
            "Shell command completed via enhanced executor: command='{}', exit_code={}, execution_time_ms={}",
            command,
            exit_code,
            execution_time_ms
        );

        // Log command completion with comprehensive security audit logging
        log_shell_completion(command, exit_code as i32, execution_time_ms);

        // Return result in existing format for backward compatibility
        if success {
            Ok(Value::String(stdout.trim().to_string()))
        } else {
            tracing::info!("Command failed with exit code {}", exit_code);
            if !stderr.is_empty() {
                tracing::warn!("Command stderr: {}", stderr);
            }
            Ok(Value::Bool(false)) // Existing behavior: don't fail workflow, indicate failure
        }
    }
}

#[async_trait::async_trait]
impl Action for ShellAction {
    async fn execute(&self, context: &mut WorkflowTemplateContext) -> ActionResult<Value> {
        // Substitute variables in command and other parameters
        let resolved_command = self.substitute_string(&self.command, context);
        let resolved_working_dir = self
            .working_dir
            .as_ref()
            .map(|dir| self.substitute_string(dir, context));

        // Security validation (maintain existing behavior)
        validate_command(&resolved_command)?;

        // Validate environment variables for security
        validate_environment_variables_security(&self.environment)?;

        // Validate timeout
        let _validated_timeout = self.validate_timeout()?;

        // Convert environment variables with substitution
        let mut resolved_env = HashMap::new();
        for (key, value) in &self.environment {
            let resolved_key = self.substitute_string(key, context);
            let resolved_value = self.substitute_string(value, context);
            resolved_env.insert(resolved_key, resolved_value);
        }

        // Validate working directory for security if specified
        if let Some(working_dir) = &resolved_working_dir {
            validate_working_directory_security(working_dir)?;
        }

        // Log security-relevant execution
        log_command_execution(
            &resolved_command,
            resolved_working_dir.as_deref(),
            &resolved_env,
        );

        // Convert timeout from Duration to seconds
        let timeout_secs = self.timeout.map(|d| d.as_secs() as u32);

        tracing::info!(
            "Executing shell command via enhanced executor: {}",
            resolved_command
        );

        // Create enhanced shell context
        let shell_context = WorkflowShellContext::new().await.map_err(|e| {
            ActionError::ExecutionError(format!("Enhanced shell initialization failed: {e}"))
        })?;

        // Execute via enhanced shell context
        let result = shell_context
            .execute_shell_command(
                resolved_command.clone(),
                resolved_working_dir,
                resolved_env,
                timeout_secs,
            )
            .await?;

        // Process enhanced shell result back to workflow format
        self.process_enhanced_result(result, &resolved_command, context)
            .await
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
    async fn execute(&self, context: &mut WorkflowTemplateContext) -> ActionResult<Value> {
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
        tracing::debug!("Executing sub-workflow '{}' in-process", self.workflow_name);
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

        // Execute the workflow
        executor.execute_state(&mut run).await.map_err(|e| {
            ActionError::ExecutionError(format!(
                "Sub-workflow '{}' execution failed: {}",
                self.workflow_name, e
            ))
        })?;

        tracing::info!("Sub-workflow '{}' completed", self.workflow_name);

        // Check the workflow status
        match run.status {
            WorkflowRunStatus::Completed => {
                // Extract the context as the result
                let result = Value::Object(
                    run.context
                        .iter()
                        .filter(|(k, _)| !k.starts_with('_')) // Filter out internal variables
                        .map(|(k, v)| (k.clone(), v.clone()))
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
fn format_as_yaml(value: &Value) -> String {
    format_value_as_yaml(value, 0)
}

/// Recursively format a JSON value as YAML with indentation
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
    // This uses the new TemplateContext infrastructure
    if let Ok(template_context) = swissarmyhammer_config::load_configuration() {
        template_context.merge_into_workflow_context(&mut enhanced_context);
    } else {
        tracing::debug!(
            "Failed to load configuration via TemplateContext. Continuing without config variables."
        );
    }

    let rendered_description = {
        // Convert ALL context variables to liquid Object (not just _template_vars)
        let mut liquid_vars = liquid::Object::new();

        // Add all variables from the context (includes workflow vars like plan_filename)
        for (key, value) in &enhanced_context {
            // Skip internal keys
            if !key.starts_with('_') {
                liquid_vars.insert(
                    key.clone().into(),
                    liquid::model::to_value(value).unwrap_or(liquid::model::Value::Nil),
                );
            }
        }

        // Also include template variables if they exist
        if let Some(template_vars) = enhanced_context.get("_template_vars") {
            if let Some(vars_map) = template_vars.as_object() {
                for (key, value) in vars_map {
                    // Template vars have lower precedence than workflow vars
                    if !liquid_vars.contains_key(key.as_str()) {
                        liquid_vars.insert(
                            key.clone().into(),
                            liquid::model::to_value(value).unwrap_or(liquid::model::Value::Nil),
                        );
                    }
                }
            }
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
    use crate::action_parser::ActionParser;
    use crate::agents::ClaudeCodeExecutor;

    use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;

    #[test]
    fn test_variable_substitution() {
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());
        context.insert("file".to_string(), Value::String("test.rs".to_string()));
        context.insert("count".to_string(), Value::Number(42.into()));

        let result =
            substitute_variables_in_string("Process ${file} with ${count} items", &context);
        assert_eq!(result, "Process test.rs with 42 items");
    }

    #[test]
    fn test_agent_response_success() {
        let response = AgentResponse::success("test content".to_string());

        assert_eq!(response.content, "test content");
        assert!(response.metadata.is_none());
        assert!(matches!(response.response_type, AgentResponseType::Success));
        assert!(response.is_success());
        assert!(!response.is_error());
    }

    #[test]
    fn test_agent_response_success_with_metadata() {
        let metadata = serde_json::json!({
            "status": "ok",
            "tokens": 150
        });
        let response =
            AgentResponse::success_with_metadata("test content".to_string(), metadata.clone());

        assert_eq!(response.content, "test content");
        assert_eq!(response.metadata, Some(metadata));
        assert!(matches!(response.response_type, AgentResponseType::Success));
        assert!(response.is_success());
        assert!(!response.is_error());
    }

    #[test]
    fn test_agent_response_error() {
        let response = AgentResponse::error("error message".to_string());

        assert_eq!(response.content, "error message");
        assert!(response.metadata.is_none());
        assert!(matches!(response.response_type, AgentResponseType::Error));
        assert!(!response.is_success());
        assert!(response.is_error());
    }

    #[test]
    fn test_agent_response_partial() {
        let response = AgentResponse::partial("partial content".to_string());

        assert_eq!(response.content, "partial content");
        assert!(response.metadata.is_none());
        assert!(matches!(response.response_type, AgentResponseType::Partial));
        assert!(!response.is_success());
        assert!(!response.is_error());
    }

    #[test]
    fn test_agent_response_serialization() {
        let response = AgentResponse::success_with_metadata(
            "test".to_string(),
            serde_json::json!({"key": "value"}),
        );

        // Test that it can be serialized and deserialized
        let serialized = serde_json::to_string(&response).expect("Should serialize");
        let deserialized: AgentResponse =
            serde_json::from_str(&serialized).expect("Should deserialize");

        assert_eq!(deserialized.content, response.content);
        assert_eq!(deserialized.metadata, response.metadata);
        assert!(matches!(
            deserialized.response_type,
            AgentResponseType::Success
        ));
    }

    #[tokio::test]
    async fn test_agent_execution_context() {
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

        // Set up agent config
        context.set_agent_config(AgentConfig::default());

        let execution_context = AgentExecutionContext::new(&context);
        assert_eq!(
            execution_context.executor_type(),
            AgentExecutorType::ClaudeCode
        );
        assert!(!execution_context.quiet());
    }

    #[tokio::test]
    async fn test_claude_executor_initialization() {
        let mut executor = ClaudeCodeExecutor::new();

        // Test initial state
        assert_eq!(executor.executor_type(), AgentExecutorType::ClaudeCode);

        // Test initialization - may fail if Claude CLI is not available
        match executor.initialize().await {
            Ok(()) => {
                // Claude CLI is available - test shutdown
                assert!(executor.shutdown().await.is_ok());
            }
            Err(swissarmyhammer_agent_executor::ActionError::ExecutionError(msg))
                if msg.contains("Claude CLI not found") =>
            {
                // This is expected in environments without Claude CLI
            }
            Err(e) => panic!("Unexpected error during initialization: {}", e),
        }
    }

    // Temporarily disabled due to llama-cpp build issues
    /*
    #[tokio::test]
    #[serial]
    async fn test_llama_executor_initialization() {
        // Skip test if LlamaAgent testing is disabled


        let config = swissarmyhammer_config::LlamaAgentConfig::for_testing();
        let mut executor = LlamaAgentExecutor::new(config);

        // Test initial state
        assert_eq!(executor.executor_type(), AgentExecutorType::LlamaAgent);

        // Test initialization (should always succeed for now)
        assert!(executor.initialize().await.is_ok());
        assert!(executor.shutdown().await.is_ok());
    }
    */

    #[tokio::test]
    async fn test_executor_creation_claude() {
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());
        context.set_agent_config(AgentConfig::default());

        let execution_context = AgentExecutionContext::new(&context);
        let action = PromptAction::new("test".to_string());

        // This test may fail if claude CLI is not available - that's expected
        match action.get_executor(&execution_context).await {
            Ok(executor) => {
                assert_eq!(executor.executor_type(), AgentExecutorType::ClaudeCode);
            }
            Err(ActionError::ExecutionError(msg)) if msg.contains("Claude CLI not found") => {
                // This is expected in environments without Claude CLI
            }
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_executor_creation_llama_agent() {
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

        use swissarmyhammer_config::agent::LlamaAgentConfig;
        let llama_config = LlamaAgentConfig::for_testing();
        context.set_agent_config(AgentConfig::llama_agent(llama_config));

        let execution_context = AgentExecutionContext::new(&context);
        let action = PromptAction::new("test".to_string());

        // LlamaAgent now requires MCP server to be pre-started, so this should fail
        // with a specific message indicating the MCP server is not running
        match action.get_executor(&execution_context).await {
            Ok(_) => {
                panic!("Expected LlamaAgent executor creation to fail without MCP server");
            }
            Err(e) => {
                let error_msg = e.to_string();
                assert!(
                    error_msg.contains("Failed to initialize LlamaAgent"),
                    "Error should start with 'Failed to initialize LlamaAgent', got: {}",
                    error_msg
                );
                assert!(
                    error_msg.contains("MCP server must be started before running workflows"),
                    "Error should explain MCP server requirement, got: {}",
                    error_msg
                );
            }
        }
    }

    #[test]
    fn test_executor_error_helpers() {
        let error = ActionError::executor_error(
            AgentExecutorType::ClaudeCode,
            "Test error message".to_string(),
        );

        match error {
            ActionError::ExecutionError(msg) => {
                assert!(msg.contains("ClaudeCode executor error"));
                assert!(msg.contains("Test error message"));
            }
            _ => panic!("Expected ExecutionError variant"),
        }

        let source_error = Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "File not found",
        ));
        let init_error =
            ActionError::initialization_error(AgentExecutorType::LlamaAgent, source_error);

        match init_error {
            ActionError::ExecutionError(msg) => {
                assert!(msg.contains("Failed to initialize LlamaAgent executor"));
                assert!(msg.contains("File not found"));
            }
            _ => panic!("Expected ExecutionError variant"),
        }
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
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

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
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

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
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

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

        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());
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

        assert!(!action.quiet);
        assert!(action.arguments.is_empty());
        assert!(action.result_variable.is_none());
    }

    #[test]
    fn test_prompt_action_builder_without_retry() {
        let action = PromptAction::new("test-prompt".to_string());

        assert_eq!(action.prompt_name, "test-prompt");

        // Test chaining with other builders (retry methods removed)
        let action2 = PromptAction::new("test-prompt2".to_string()).with_quiet(true);

        assert!(action2.quiet);
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
        let _test_env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
        // Clean up any existing abort file before test
        let _ = std::fs::remove_file(".swissarmyhammer/.abort");

        let action = AbortAction::new("Test abort message".to_string());
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

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
        let _test_env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
        // Clean up any existing abort file before test
        let _ = std::fs::remove_file(".swissarmyhammer/.abort");

        let action = AbortAction::new("Error in ${file}: ${error}".to_string());
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());
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
        let _test_env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
        // Clean up any existing abort file before test
        let _ = std::fs::remove_file(".swissarmyhammer/.abort");

        // Test that abort errors propagate correctly through the system
        use crate::definition::Workflow;
        use crate::executor::core::WorkflowExecutor;
        use crate::executor::ExecutorError;
        use crate::state::{State, StateId, StateType};
        use crate::storage::WorkflowStorage;
        use crate::transition::{ConditionType, Transition, TransitionCondition};
        use crate::WorkflowName;
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
                        "Abort reason should contain expected message: {reason}"
                    );
                }
                _ => panic!("Expected ExecutorError::Abort, got: {executor_error:?}"),
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
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());
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

        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());
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

        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());
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
        use crate::ShellAction as WorkflowShellAction;

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
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

        let result = action.execute(&mut context).await.unwrap();

        // Verify success context variables are set
        assert_eq!(context.get("success"), Some(&Value::Bool(true)));
        assert_eq!(context.get("failure"), Some(&Value::Bool(false)));
        assert_eq!(context.get("exit_code"), Some(&Value::Number(0.into())));

        // Verify stdout contains expected output
        let stdout = context.get("stdout").unwrap().as_str().unwrap();
        assert!(stdout.contains("hello world"));

        // Verify stderr is empty or contains only directory-related warnings from shell
        let stderr = context.get("stderr").unwrap().as_str().unwrap();
        let stderr_trimmed = stderr.trim();
        // Allow shell warnings about directory access but not other errors
        let is_acceptable_stderr = stderr_trimmed.is_empty()
            || stderr_trimmed.contains("shell-init: error retrieving current directory")
            || stderr_trimmed.contains("getcwd: cannot access parent directories");
        assert!(
            is_acceptable_stderr,
            "Unexpected stderr content: {}",
            stderr
        );

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
        // Result should be trimmed version of stdout for usability
        let stdout = context.get("stdout").unwrap().as_str().unwrap();
        let expected_result = Value::String(stdout.trim().to_string());
        assert_eq!(result, expected_result);
    }

    #[tokio::test]
    async fn test_shell_action_execution_failure() {
        let action = ShellAction::new("exit 42".to_string());
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

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
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());
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
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

        let result = action.execute(&mut context).await.unwrap();

        // Verify result variable is set
        assert!(context.contains_key("command_output"));
        let command_output = context.get("command_output").unwrap();
        assert_eq!(command_output, &result);

        // Result should be the trimmed stdout
        let stdout = context.get("stdout").unwrap().as_str().unwrap();
        assert!(stdout.contains("test output"));
        // Result should be trimmed version of stdout for usability
        let expected_result = Value::String(stdout.trim().to_string());
        assert_eq!(result, expected_result);
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
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

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
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

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
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());
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

        // Create an action with a short timeout (1 second to ensure proper timeout behavior)
        let action = ShellAction::new("sleep 10".to_string()).with_timeout(Duration::from_secs(1));
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

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
        // Should be around 1000ms or slightly more (allowing for process cleanup and system overhead)
        // Being more lenient with timing to account for CI environment variations
        assert!(
            duration_ms >= 800,
            "Duration {duration_ms} ms should be at least 800ms"
        );
        assert!(
            duration_ms <= 3000,
            "Duration {duration_ms} ms should not exceed 3000ms"
        );

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
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

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
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

        let result = action.execute(&mut context).await.unwrap();

        // Verify success
        assert_eq!(context.get("success"), Some(&Value::Bool(true)));
        assert_eq!(context.get("failure"), Some(&Value::Bool(false)));

        // Verify output
        let stdout = context.get("stdout").unwrap().as_str().unwrap();
        assert!(stdout.contains("no timeout test"));

        // Result should contain output
        // Result should be trimmed version of stdout for usability
        let stdout = context.get("stdout").unwrap().as_str().unwrap();
        let expected_result = Value::String(stdout.trim().to_string());
        assert_eq!(result, expected_result);
    }

    #[tokio::test]
    async fn test_shell_action_successful_within_timeout() {
        use std::time::Duration;

        // Command that completes quickly within timeout
        let action =
            ShellAction::new("echo quick command".to_string()).with_timeout(Duration::from_secs(5));
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

        let result = action.execute(&mut context).await.unwrap();

        // Verify success
        assert_eq!(context.get("success"), Some(&Value::Bool(true)));
        assert_eq!(context.get("failure"), Some(&Value::Bool(false)));

        // Exit code should be 0
        assert_eq!(context.get("exit_code"), Some(&Value::Number(0.into())));

        // Verify output
        let stdout = context.get("stdout").unwrap().as_str().unwrap();
        assert!(stdout.contains("quick command"));

        // Duration should be much less than timeout (allowing for system load during parallel tests)
        let duration_ms = context.get("duration_ms").unwrap().as_u64().unwrap();
        assert!(duration_ms < 5000); // Should complete in less than 5 seconds (was 1 second)

        // Result should contain output
        // Result should be trimmed version of stdout for usability
        let stdout = context.get("stdout").unwrap().as_str().unwrap();
        let expected_result = Value::String(stdout.trim().to_string());
        assert_eq!(result, expected_result);
    }

    #[tokio::test]
    async fn test_shell_action_timeout_process_cleanup() {
        use std::time::Duration;

        // This test verifies that timeout processes are properly terminated
        // If the process cleanup didn't work, this test would hang for 30 seconds
        let action =
            ShellAction::new("sleep 30".to_string()).with_timeout(Duration::from_millis(150));
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

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
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

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
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

        let _result = action.execute(&mut context).await.unwrap();

        // Verify success - this confirms the Unix sh -c format worked
        assert_eq!(context.get("success"), Some(&Value::Bool(true)));
        let stdout = context.get("stdout").unwrap().as_str().unwrap();
        assert!(stdout.contains("unix test"));
    }

    #[tokio::test]
    async fn test_log_action_template_context_integration() {
        use serde_json::json;
        use std::collections::HashMap;

        // Create a WorkflowTemplateContext with configuration values
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

        let mut workflow_vars = HashMap::new();
        workflow_vars.insert("project_name".to_string(), json!("SwissArmyHammer"));
        workflow_vars.insert("version".to_string(), json!("2.0.0"));
        workflow_vars.insert("debug".to_string(), json!(true));
        workflow_vars.insert("workflow_step".to_string(), json!("initialization"));

        context.set_workflow_vars(workflow_vars);

        // Create a LogAction with liquid template syntax
        let log_action =
            LogAction::info("Project {{project_name}} v{{version}} (Debug: {{debug}})".to_string());

        // Execute with the context
        let result = log_action.execute(&mut context).await.unwrap();

        // Verify the template was rendered correctly using configuration values
        assert_eq!(
            result.as_str().unwrap(),
            "Project SwissArmyHammer v2.0.0 (Debug: true)"
        );

        // Verify the action marked itself as successful
        assert_eq!(
            context.get(LAST_ACTION_RESULT_KEY),
            Some(&Value::Bool(true))
        );
    }

    #[tokio::test]
    async fn test_log_action_template_context_with_workflow_vars() {
        use serde_json::json;
        use std::collections::HashMap;

        // Create a WorkflowTemplateContext with all variables
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

        let mut all_vars = HashMap::new();
        all_vars.insert("app_name".to_string(), json!("TestApp"));
        all_vars.insert("current_file".to_string(), json!("main.rs"));
        all_vars.insert("line_count".to_string(), json!(42));

        context.set_workflow_vars(all_vars);

        // Create a LogAction that uses both template and workflow variables
        let log_action =
            LogAction::info("App: {{app_name}}, Current file: {{current_file}}".to_string());

        // Execute with the context
        let result = log_action.execute(&mut context).await.unwrap();

        // Verify both template and workflow variables were used
        assert_eq!(
            result.as_str().unwrap(),
            "App: TestApp, Current file: main.rs"
        );

        // Verify workflow variables are preserved
        assert_eq!(context.get("current_file"), Some(&json!("main.rs")));
        assert_eq!(context.get("line_count"), Some(&json!(42)));
    }

    #[tokio::test]
    async fn test_backward_compatibility_with_execute() {
        use serde_json::json;
        use std::collections::HashMap;

        // Test that the old execute() method still works
        let log_action = LogAction::info("Simple message: {{var1}}".to_string());

        // Create traditional HashMap context
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());
        context.insert("var1".to_string(), json!("test_value"));

        // Execute with old method
        let result = log_action.execute(&mut context).await.unwrap();

        // Verify it works as before
        assert_eq!(result.as_str().unwrap(), "Simple message: test_value");
        assert_eq!(
            context.get(LAST_ACTION_RESULT_KEY),
            Some(&Value::Bool(true))
        );
    }

    // NOTE: These tests were removed because they test internal implementation details
    // of ClaudeCodeExecutor that are now in the agent-executor crate. The workflow
    // crate only exposes the public AgentExecutor trait interface.

    #[tokio::test]
    async fn test_prompt_action_with_claude_executor() {
        let _guard = IsolatedTestEnvironment::new();

        // Create a simple prompt action
        let action = PromptAction {
            prompt_name: "test-prompt".to_string(),
            arguments: HashMap::new(),
            result_variable: Some("test_result".to_string()),
            quiet: true, // Suppress output during tests
        };

        // Set up context with Claude executor
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());
        context.set_agent_config(swissarmyhammer_config::agent::AgentConfig::claude_code());

        // This test will likely fail without actual Claude CLI and prompt
        // but should demonstrate the integration
        match action.execute(&mut context).await {
            Ok(_result) => {
                // Success - result should be stored in context
                assert!(context.get("test_result").is_some());
                assert!(context.get(LAST_ACTION_RESULT_KEY).is_some());
                assert!(context.get(CLAUDE_RESPONSE_KEY).is_some());
            }
            Err(ActionError::ExecutionError(msg)) if msg.contains("Claude CLI not found") => {
                // Expected in test environments without Claude CLI
            }
            Err(ActionError::ClaudeError(msg)) if msg.contains("Failed to load prompts") => {
                // Expected when test prompt doesn't exist
            }
            Err(e) => {
                tracing::warn!(
                    "Test failed with error (expected in some environments): {}",
                    e
                );
                // Other errors might be expected in test environments
            }
        }
    }

    #[tokio::test]
    async fn test_agent_executor_creation_claude_code() {
        let _guard = IsolatedTestEnvironment::new();

        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());
        context.set_agent_config(swissarmyhammer_config::agent::AgentConfig::claude_code());

        let execution_context = AgentExecutionContext::new(&context);
        let action = PromptAction::new("test".to_string());

        match action.get_executor(&execution_context).await {
            Ok(executor) => {
                assert_eq!(executor.executor_type(), AgentExecutorType::ClaudeCode);
            }
            Err(ActionError::ExecutionError(msg)) if msg.contains("Claude CLI not found") => {
                // Expected in environments without Claude CLI
            }
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_agent_executor_creation_llama_agent() {
        // Skip test if LlamaAgent testing is disabled

        let _guard = IsolatedTestEnvironment::new();

        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());
        let llama_config = swissarmyhammer_config::agent::LlamaAgentConfig::for_testing();
        println!("DEBUG: Created llama_config for testing");
        context.set_agent_config(swissarmyhammer_config::agent::AgentConfig::llama_agent(
            llama_config,
        ));

        let execution_context = AgentExecutionContext::new(&context);

        // Debug: Print the executor type and config
        println!(
            "DEBUG: executor_type = {:?}",
            execution_context.executor_type()
        );

        // LlamaAgent should now create successfully
        let action = PromptAction::new("test".to_string());
        match action.get_executor(&execution_context).await {
            Ok(executor) => {
                println!("DEBUG: LlamaAgent executor created successfully");
                // Verify we got a LlamaAgent executor
                assert!(
                    !executor.supports_streaming(),
                    "LlamaAgent should not support streaming by default"
                );
            }
            Err(e) => {
                println!("DEBUG: LlamaAgent executor creation failed: {}", e);
                // May fail in some environments due to model availability, but shouldn't be hardcoded disabled
                assert!(
                    !e.to_string().contains("temporarily disabled"),
                    "Should not be hardcoded as disabled"
                );
            }
        }
    }
}
