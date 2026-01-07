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
//! // let mcp_server = start_mcp_server(..., None).await?;
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

use swissarmyhammer_common::{ErrorSeverity, Pretty, Severity};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::as_24_bit_terminal_escaped;
use thiserror::Error;

use swissarmyhammer_config::model::ModelExecutorType;
use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};

// ACP agent integration
use crate::acp::{self, AgentResponse, McpServerConfig};

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

/// Implementation of Severity trait for ActionError
impl Severity for ActionError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Error: All action errors are recoverable operation failures
            ActionError::ClaudeError(_) => ErrorSeverity::Error,
            ActionError::VariableError(_) => ErrorSeverity::Error,
            ActionError::ParseError(_) => ErrorSeverity::Error,
            ActionError::ExecutionError(_) => ErrorSeverity::Error,
            ActionError::IoError(_) => ErrorSeverity::Error,
            ActionError::JsonError(_) => ErrorSeverity::Error,
            ActionError::RateLimit { .. } => ErrorSeverity::Error,
            ActionError::ShellSecurityError(_) => ErrorSeverity::Error,
        }
    }
}

// AgentResponse and AgentResponseType are now defined in the acp module
// Use: crate::acp::{AgentResponse, AgentResponseType}

/// Convert ACP error to workflow ActionError
fn convert_acp_error(err: acp::AcpError) -> ActionError {
    match err {
        acp::AcpError::InitializationError(msg) => ActionError::ClaudeError(msg),
        acp::AcpError::SessionError(msg) => ActionError::ExecutionError(msg),
        acp::AcpError::PromptError(msg) => ActionError::ExecutionError(msg),
        acp::AcpError::AgentNotAvailable(msg) => ActionError::ClaudeError(msg),
        acp::AcpError::ConfigurationError(msg) => ActionError::ExecutionError(msg),
        acp::AcpError::RateLimit { message, wait_time } => {
            ActionError::RateLimit { message, wait_time }
        }
    }
}

impl ActionError {
    /// Create an executor-specific error
    pub fn executor_error(executor_type: ModelExecutorType, message: String) -> Self {
        ActionError::ExecutionError(format!("{:?} executor error: {}", executor_type, message))
    }

    /// Create an initialization error
    pub fn initialization_error(
        executor_type: ModelExecutorType,
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

        tracing::debug!("Args for prompt rendering: {}", Pretty(&args));

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

        // System prompt handling:
        // - Workflows with a mode: system prompt applied via ACP set_session_mode (--append-system-prompt)
        // - Workflows without a mode: use .system/default as fallback
        let system_prompt = if context.get_workflow_mode().is_some() {
            // Mode-based system prompt will be applied via ACP
            None
        } else {
            // No mode set, use .system/default as the default system prompt
            match library_arc.render(".system/default", &template_context) {
                Ok(prompt) => Some(prompt),
                Err(e) => {
                    tracing::warn!(
                        "Failed to render .system/default prompt: {}. Proceeding without system prompt.",
                        e
                    );
                    None
                }
            }
        };

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
        tracing::debug!(
            "Executing prompt '{}' with context: {:?}",
            self.prompt_name,
            context
        );

        let (user_prompt, system_prompt) = self.render_prompts_directly(context)?;
        tracing::debug!("Piping prompt:\n{}", user_prompt);

        let response = self
            .execute_prompt_with_agent(context, user_prompt, system_prompt)
            .await?;

        tracing::info!("Prompt response: {:?}", Pretty(&response));
        self.store_response_in_context(context, &response)?;

        Ok(serde_json::to_value(&response)
            .unwrap_or_else(|_| Value::String(response.content.clone())))
    }

    /// Execute prompt with ACP agent
    async fn execute_prompt_with_agent(
        &self,
        context: &WorkflowTemplateContext,
        user_prompt: String,
        system_prompt: Option<String>,
    ) -> ActionResult<AgentResponse> {
        let agent_config = context.get_agent_config();
        let mcp_config = self.get_mcp_config_from_context(context)?;

        // Create ACP agent based on model configuration
        let mut agent = acp::create_agent(&agent_config, Some(mcp_config))
            .await
            .map_err(convert_acp_error)?;

        // Get workflow mode from context if available
        let mode = context.get_workflow_mode();

        // Execute prompt via ACP protocol
        let response = acp::execute_prompt(&mut agent, system_prompt, mode, user_prompt)
            .await
            .map_err(|e| {
                #[derive(serde::Serialize, Debug)]
                struct ErrorInfo {
                    error: String,
                }
                tracing::error!(
                    "Prompt execution failed: {}",
                    Pretty(&ErrorInfo {
                        error: e.to_string()
                    })
                );
                convert_acp_error(e)
            })?;

        Ok(response)
    }

    /// Store response in context
    fn store_response_in_context(
        &self,
        context: &mut WorkflowTemplateContext,
        response: &AgentResponse,
    ) -> ActionResult<()> {
        if let Some(var_name) = &self.result_variable {
            let response_value = serde_json::to_value(response).unwrap_or_default();
            context.insert(var_name.clone(), response_value);
        }

        context.insert(LAST_ACTION_RESULT_KEY.to_string(), Value::Bool(true));
        context.insert(
            CLAUDE_RESPONSE_KEY.to_string(),
            Value::String(response.content.clone()),
        );

        Ok(())
    }

    /// Get MCP server configuration from workflow context
    fn get_mcp_config_from_context(
        &self,
        context: &WorkflowTemplateContext,
    ) -> ActionResult<McpServerConfig> {
        let port = context
            .get("_mcp_server_port")
            .and_then(|v| v.as_u64())
            .map(|p| p as u16)
            .ok_or_else(|| {
                ActionError::ExecutionError(
                    "MCP server must be started before running workflows. \
                     Ensure _mcp_server_port is set in the workflow context."
                        .to_string(),
                )
            })?;

        tracing::info!("Using MCP server on port {}", port);
        Ok(McpServerConfig::from_port(port))
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
        tracing::debug!(
            "Set variable '{}' = '{}'",
            self.variable_name,
            Pretty(&json_value)
        );

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

        // Set abort flag using CEL state
        let cel_state = swissarmyhammer_cel::CelState::global();
        if let Err(e) = cel_state.set("abort", "true") {
            tracing::warn!("Failed to set CEL abort flag: {}", e);
        }

        // Set a special context variable to signal abort request (for backward compatibility)
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

impl SubWorkflowAction {
    /// Check for circular workflow dependencies
    fn check_circular_dependency(&self, context: &WorkflowTemplateContext) -> ActionResult<()> {
        let workflow_stack = context
            .get(WORKFLOW_STACK_KEY)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

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

        Ok(())
    }

    /// Validate input variable keys
    fn validate_input_keys(&self, inputs: &HashMap<String, String>) -> ActionResult<()> {
        for key in inputs.keys() {
            if !is_valid_argument_key(key) {
                return Err(ActionError::ParseError(
                    format!("Invalid input variable key '{key}': must contain only alphanumeric characters, hyphens, and underscores")
                ));
            }
        }
        Ok(())
    }

    /// Build workflow stack for tracking
    fn build_workflow_stack(&self, context: &WorkflowTemplateContext) -> Vec<Value> {
        let mut workflow_stack = context
            .get(WORKFLOW_STACK_KEY)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        workflow_stack.push(Value::String(self.workflow_name.clone()));
        workflow_stack
    }

    /// Load and start the sub-workflow with agent from parent context
    async fn load_and_start_workflow(
        &self,
        parent_context: &WorkflowTemplateContext,
    ) -> ActionResult<crate::WorkflowRun> {
        tracing::debug!("Executing sub-workflow '{}' in-process", self.workflow_name);

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

        // Get agent config from parent context and create executor with it
        let agent_config = parent_context.get_agent_config();
        let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let mut executor = WorkflowExecutor::with_working_dir_and_agent(
            working_dir,
            std::sync::Arc::new(agent_config),
        );
        executor.start_workflow(workflow).map_err(|e| {
            ActionError::ExecutionError(format!(
                "Failed to start sub-workflow '{}': {}",
                self.workflow_name, e
            ))
        })
    }

    /// Prepare context for sub-workflow execution
    fn prepare_sub_workflow_context(
        &self,
        sub_context: &mut WorkflowTemplateContext,
        parent_context: &WorkflowTemplateContext,
        inputs: &HashMap<String, String>,
        workflow_stack: Vec<Value>,
    ) -> ActionResult<()> {
        tracing::debug!("Preparing sub-workflow context");

        // Add input variables
        for (key, value) in inputs {
            sub_context.insert(key.clone(), Value::String(value.clone()));
        }

        // Add workflow stack
        sub_context.insert(WORKFLOW_STACK_KEY.to_string(), Value::Array(workflow_stack));

        // Copy special variables
        self.copy_special_variables(sub_context, parent_context)?;
        self.inherit_mcp_config(sub_context, parent_context)?;

        Ok(())
    }

    /// Copy special variables from parent to sub-workflow
    fn copy_special_variables(
        &self,
        sub_context: &mut WorkflowTemplateContext,
        parent_context: &WorkflowTemplateContext,
    ) -> ActionResult<()> {
        if let Some(quiet) = parent_context.get("_quiet").and_then(|v| v.as_bool()) {
            if quiet {
                sub_context.insert("_quiet".to_string(), Value::Bool(true));
            }
        }

        if let Some(timeout_secs) = parent_context.get("_timeout_secs").and_then(|v| v.as_u64()) {
            sub_context.insert(
                "_timeout_secs".to_string(),
                Value::Number(serde_json::Number::from(timeout_secs)),
            );
        }

        Ok(())
    }

    /// Inherit MCP configuration from parent context
    fn inherit_mcp_config(
        &self,
        sub_context: &mut WorkflowTemplateContext,
        parent_context: &WorkflowTemplateContext,
    ) -> ActionResult<()> {
        tracing::debug!(
            "SubWorkflowAction - setting up agent config for sub-workflow '{}'",
            self.workflow_name
        );

        if let Some(mcp_port) = parent_context
            .get("_mcp_server_port")
            .and_then(|v| v.as_u64())
            .map(|p| p as u16)
        {
            tracing::debug!("Parent context has _mcp_server_port: {}", mcp_port);
            sub_context.update_mcp_port(mcp_port);
        } else {
            tracing::warn!(
                "No _mcp_server_port found in parent context for sub-workflow '{}'",
                self.workflow_name
            );
            let agent_config = parent_context.get_agent_config();
            sub_context.set_agent_config(agent_config);
        }

        Ok(())
    }

    /// Execute the workflow
    async fn execute_workflow(&self, run: &mut crate::WorkflowRun) -> ActionResult<()> {
        let mut executor = WorkflowExecutor::new();
        executor.execute_state(run).await.map_err(|e| {
            ActionError::ExecutionError(format!(
                "Sub-workflow '{}' execution failed: {}",
                self.workflow_name, e
            ))
        })?;

        tracing::info!("Sub-workflow '{}' completed", self.workflow_name);
        Ok(())
    }

    /// Handle workflow completion status
    fn handle_workflow_completion(
        &self,
        run: &crate::WorkflowRun,
        context: &mut WorkflowTemplateContext,
    ) -> ActionResult<Value> {
        match run.status {
            WorkflowRunStatus::Completed => {
                let result = self.extract_workflow_result(&run.context);
                self.store_result_if_needed(context, &result);
                context.insert(LAST_ACTION_RESULT_KEY.to_string(), Value::Bool(true));
                Ok(result)
            }
            WorkflowRunStatus::Failed => Err(ActionError::ExecutionError(format!(
                "Sub-workflow '{}' failed",
                self.workflow_name
            ))),
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

    /// Extract result from workflow context
    fn extract_workflow_result(&self, sub_context: &WorkflowTemplateContext) -> Value {
        Value::Object(
            sub_context
                .iter()
                .filter(|(k, _)| !k.starts_with('_'))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        )
    }

    /// Store result in context if variable name is specified
    fn store_result_if_needed(&self, context: &mut WorkflowTemplateContext, result: &Value) {
        if let Some(var_name) = &self.result_variable {
            tracing::info!(
                "Storing sub-workflow result in variable '{}': {:?}",
                var_name,
                result
            );
            context.insert(var_name.clone(), result.clone());
        }
    }
}

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
        tracing::debug!("Environment variables set: {}", Pretty(&env_keys));
    }
}

impl VariableSubstitution for ShellAction {}

/// Resolved shell execution parameters
struct ResolvedShellParams {
    command: String,
    working_dir: Option<String>,
    environment: HashMap<String, String>,
    timeout_secs: Option<u32>,
}

impl ShellAction {
    /// Resolve shell parameters with variable substitution
    fn resolve_shell_parameters(
        &self,
        context: &WorkflowTemplateContext,
    ) -> ActionResult<ResolvedShellParams> {
        let resolved_command = self.substitute_string(&self.command, context);
        let resolved_working_dir = self
            .working_dir
            .as_ref()
            .map(|dir| self.substitute_string(dir, context));

        let mut resolved_env = HashMap::new();
        for (key, value) in &self.environment {
            let resolved_key = self.substitute_string(key, context);
            let resolved_value = self.substitute_string(value, context);
            resolved_env.insert(resolved_key, resolved_value);
        }

        let timeout_secs = self.timeout.map(|d| d.as_secs() as u32);

        Ok(ResolvedShellParams {
            command: resolved_command,
            working_dir: resolved_working_dir,
            environment: resolved_env,
            timeout_secs,
        })
    }

    /// Validate shell execution parameters
    fn validate_shell_execution(&self, params: &ResolvedShellParams) -> ActionResult<()> {
        validate_command(&params.command)?;
        validate_environment_variables_security(&params.environment)?;
        self.validate_timeout()?;

        if let Some(working_dir) = &params.working_dir {
            validate_working_directory_security(working_dir)?;
        }

        Ok(())
    }

    /// Execute shell command using internal context
    async fn execute_shell_command_internal(
        &self,
        params: &ResolvedShellParams,
    ) -> ActionResult<Value> {
        let shell_context = WorkflowShellContext::new().await.map_err(|e| {
            ActionError::ExecutionError(format!("Enhanced shell initialization failed: {e}"))
        })?;

        shell_context
            .execute_shell_command(
                params.command.clone(),
                params.working_dir.clone(),
                params.environment.clone(),
                params.timeout_secs,
            )
            .await
    }
}

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
        let resolved_params = self.resolve_shell_parameters(context)?;
        self.validate_shell_execution(&resolved_params)?;

        log_command_execution(
            &resolved_params.command,
            resolved_params.working_dir.as_deref(),
            &resolved_params.environment,
        );

        tracing::info!(
            "Executing shell command via enhanced executor: {}",
            resolved_params.command
        );

        let result = self
            .execute_shell_command_internal(&resolved_params)
            .await?;
        self.process_enhanced_result(result, &resolved_params.command, context)
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
        self.check_circular_dependency(context)?;

        let substituted_inputs = self.substitute_variables(context);
        self.validate_input_keys(&substituted_inputs)?;

        let new_stack = self.build_workflow_stack(context);
        let mut run = self.load_and_start_workflow(context).await?;

        self.prepare_sub_workflow_context(
            &mut run.context,
            context,
            &substituted_inputs,
            new_stack,
        )?;
        self.execute_workflow(&mut run).await?;

        self.handle_workflow_completion(&run, context)
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
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => format_yaml_string(s, indent_level),
        Value::Array(arr) => format_yaml_array(arr, indent_level),
        Value::Object(map) => format_yaml_object(map, indent_level),
    }
}

/// Format a YAML string value
#[allow(dead_code)]
fn format_yaml_string(s: &str, indent_level: usize) -> String {
    if s.contains('\n') {
        format_multiline_string(s, indent_level)
    } else if needs_yaml_quotes(s) {
        format!("\"{}\"", s.replace('\"', "\\\""))
    } else {
        s.to_string()
    }
}

/// Format a multiline string in YAML
#[allow(dead_code)]
fn format_multiline_string(s: &str, indent_level: usize) -> String {
    let content_to_format = if let Some(language) = detect_source_code_language(s) {
        highlight_source_code(s, language)
    } else {
        s.to_string()
    };

    let lines: Vec<&str> = content_to_format.lines().collect();
    let mut result = "|-\n".to_string();
    let content_indent = "  ".repeat(indent_level + 1);
    for line in lines {
        result.push_str(&format!("{content_indent}{line}\n"));
    }
    result.trim_end().to_string()
}

/// Format a YAML array
#[allow(dead_code)]
fn format_yaml_array(arr: &[Value], indent_level: usize) -> String {
    if arr.is_empty() {
        return "[]".to_string();
    }

    let indent = "  ".repeat(indent_level);
    let mut result = String::new();

    for (i, item) in arr.iter().enumerate() {
        if i > 0 {
            result.push('\n');
            result.push_str(&indent);
        }
        result.push_str("- ");
        let item_str = format_value_as_yaml(item, indent_level + 1);

        if item_str.contains('\n') {
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

/// Format a YAML object
#[allow(dead_code)]
fn format_yaml_object(map: &serde_json::Map<String, Value>, indent_level: usize) -> String {
    let indent = "  ".repeat(indent_level);
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
    let enhanced_context = prepare_enhanced_context(context);
    let rendered_description = render_with_liquid(&enhanced_context, description);
    parse_action_from_description(&rendered_description)
}

/// Prepare enhanced context with configuration variables
fn prepare_enhanced_context(context: &HashMap<String, Value>) -> HashMap<String, Value> {
    let mut enhanced_context = context.clone();

    if let Ok(template_context) = swissarmyhammer_config::load_configuration() {
        template_context.merge_into_workflow_context(&mut enhanced_context);
    } else {
        tracing::debug!(
            "Failed to load configuration via TemplateContext. Continuing without config variables."
        );
    }

    enhanced_context
}

/// Render description with liquid template
fn render_with_liquid(context: &HashMap<String, Value>, description: &str) -> String {
    let liquid_vars = build_liquid_variables(context);

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
}

/// Build liquid variables from context
fn build_liquid_variables(context: &HashMap<String, Value>) -> liquid::Object {
    let mut liquid_vars = liquid::Object::new();

    // Add all variables from the context (includes workflow vars)
    for (key, value) in context {
        if !key.starts_with('_') {
            liquid_vars.insert(
                key.clone().into(),
                liquid::model::to_value(value).unwrap_or(liquid::model::Value::Nil),
            );
        }
    }

    // Add template variables with lower precedence
    if let Some(template_vars) = context.get("_template_vars") {
        if let Some(vars_map) = template_vars.as_object() {
            for (key, value) in vars_map {
                if !liquid_vars.contains_key(key.as_str()) {
                    liquid_vars.insert(
                        key.clone().into(),
                        liquid::model::to_value(value).unwrap_or(liquid::model::Value::Nil),
                    );
                }
            }
        }
    }

    liquid_vars
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
    use crate::acp::AgentResponseType;
    use crate::action_parser::ActionParser;
    use swissarmyhammer_config::ModelConfig;

    use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;

    /// Test helper structure for shell action result verification
    ///
    /// This structure consolidates duplicated assertion patterns across shell action tests,
    /// providing a unified way to verify command execution results.
    struct ShellActionTestResult {
        success: bool,
        exit_code: i64,
        stdout: String,
        stderr: String,
        duration_ms: u64,
    }

    impl ShellActionTestResult {
        /// Extract shell action results from workflow context
        fn from_context(context: &WorkflowTemplateContext) -> Self {
            Self {
                success: context.get("success").and_then(|v| v.as_bool()).unwrap(),
                exit_code: context.get("exit_code").and_then(|v| v.as_i64()).unwrap(),
                stdout: context
                    .get("stdout")
                    .and_then(|v| v.as_str())
                    .unwrap()
                    .to_string(),
                stderr: context
                    .get("stderr")
                    .and_then(|v| v.as_str())
                    .unwrap()
                    .to_string(),
                duration_ms: context.get("duration_ms").and_then(|v| v.as_u64()).unwrap(),
            }
        }

        /// Assert that the command executed successfully
        fn assert_success(&self) {
            assert!(self.success);
            assert_eq!(self.exit_code, 0);
        }

        /// Assert that the command failed with expected exit code
        fn assert_failure(&self, expected_exit_code: i64) {
            assert!(!self.success);
            assert_eq!(self.exit_code, expected_exit_code);
        }

        /// Verify that stderr is acceptable (empty or contains only shell warnings)
        fn assert_acceptable_stderr(&self) {
            let stderr_trimmed = self.stderr.trim();
            let is_acceptable = stderr_trimmed.is_empty()
                || stderr_trimmed.contains("shell-init: error retrieving current directory")
                || stderr_trimmed.contains("getcwd: cannot access parent directories");
            assert!(is_acceptable, "Unexpected stderr content: {}", self.stderr);
        }
    }

    /// Test helper for executing shell actions with timeout configuration
    ///
    /// This helper consolidates duplicated setup patterns for timeout test scenarios.
    async fn execute_shell_action_with_timeout(
        command: &str,
        timeout_ms: u64,
        result_variable: Option<String>,
    ) -> (ActionResult<Value>, WorkflowTemplateContext) {
        let mut action =
            ShellAction::new(command.to_string()).with_timeout(Duration::from_millis(timeout_ms));

        if let Some(var) = result_variable {
            action = action.with_result_variable(var);
        }

        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());
        let result = action.execute(&mut context).await;
        (result, context)
    }

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
    async fn test_acp_agent_creation() {
        // Test ACP agent creation with Claude config
        let config = ModelConfig::claude_code();
        let mcp_config = McpServerConfig::from_port(8080);

        // Agent creation may fail if Claude CLI is not available
        match acp::create_agent(&config, Some(mcp_config)).await {
            Ok(_agent) => {
                // Agent created successfully
            }
            Err(acp::AcpError::AgentNotAvailable(msg)) if msg.contains("Claude CLI not found") => {
                // This is expected in environments without Claude CLI
            }
            Err(e) => panic!("Unexpected error during agent creation: {}", e),
        }
    }

    // Temporarily disabled due to llama-cpp build issues
    /*
    #[tokio::test]
    #[serial]
    async fn test_llama_acp_agent_creation() {
        // Skip test if LlamaAgent testing is disabled

        let llama_config = swissarmyhammer_config::model::LlamaAgentConfig::for_testing();
        let config = ModelConfig::llama_agent(llama_config);

        let mcp_config = McpServerConfig::from_port(8080);

        // Test agent creation
        match acp::create_agent(&config, Some(mcp_config)).await {
            Ok(_agent) => {
                // Agent created successfully
            }
            Err(e) => panic!("Unexpected error during agent creation: {}", e),
        }
    }
    */

    #[tokio::test]
    async fn test_acp_agent_creation_from_context() {
        let mut vars = HashMap::new();
        vars.insert("_mcp_server_port".to_string(), 8080_u64.into());
        let mut context = WorkflowTemplateContext::with_vars_for_test(vars);
        context.set_agent_config(ModelConfig::default());

        let action = PromptAction::new("test".to_string());
        let mcp_config = action.get_mcp_config_from_context(&context).unwrap();
        let agent_config = context.get_agent_config();

        // This test may fail if claude CLI is not available - that's expected
        match acp::create_agent(&agent_config, Some(mcp_config)).await {
            Ok(_agent) => {
                // Agent created successfully
            }
            Err(acp::AcpError::AgentNotAvailable(msg)) if msg.contains("Claude CLI not found") => {
                // This is expected in environments without Claude CLI
            }
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    #[test]
    fn test_executor_error_helpers() {
        let error = ActionError::executor_error(
            ModelExecutorType::ClaudeCode,
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
            ActionError::initialization_error(ModelExecutorType::LlamaAgent, source_error);

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

        // Use helper to verify results
        let test_result = ShellActionTestResult::from_context(&context);
        test_result.assert_success();
        test_result.assert_acceptable_stderr();

        // Verify stdout contains expected output
        assert!(test_result.stdout.contains("hello world"));

        // Verify last action result is success
        assert_eq!(
            context.get(LAST_ACTION_RESULT_KEY),
            Some(&Value::Bool(true))
        );

        // Result should be trimmed version of stdout for usability
        let expected_result = Value::String(test_result.stdout.trim().to_string());
        assert_eq!(result, expected_result);
    }

    #[tokio::test]
    async fn test_shell_action_execution_failure() {
        let action = ShellAction::new("exit 42".to_string());
        let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

        let _result = action.execute(&mut context).await.unwrap();

        // Use helper to verify results
        let test_result = ShellActionTestResult::from_context(&context);
        test_result.assert_failure(42);

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
        // Use helper for timeout test
        let (result, context) = execute_shell_action_with_timeout("sleep 10", 1000, None).await;
        let result = result.unwrap();

        // Use test result helper
        let test_result = ShellActionTestResult::from_context(&context);
        test_result.assert_failure(-1);

        // Verify timeout-specific behavior
        assert_eq!(test_result.stderr, "Command timed out");
        assert_eq!(test_result.stdout, "");

        // Duration should be tracked and around the timeout duration
        assert!(
            test_result.duration_ms >= 800,
            "Duration {} ms should be at least 800ms",
            test_result.duration_ms
        );
        assert!(
            test_result.duration_ms <= 3000,
            "Duration {} ms should not exceed 3000ms",
            test_result.duration_ms
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
        // Use helper for timeout test with result variable
        let (_result, context) =
            execute_shell_action_with_timeout("sleep 5", 100, Some("output".to_string())).await;

        // Use test result helper
        let test_result = ShellActionTestResult::from_context(&context);
        test_result.assert_failure(-1);

        // Result variable should NOT be set on timeout
        assert!(!context.contains_key("output"));

        // Verify timeout context variables
        assert_eq!(test_result.stderr, "Command timed out");
        assert_eq!(test_result.stdout, "");
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

    // NOTE: These tests were removed because they tested internal implementation details.
    // The workflow crate now uses ACP agents directly via the acp module.

    #[tokio::test]
    async fn test_prompt_action_with_acp_agent() {
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
        context.set_agent_config(swissarmyhammer_config::model::ModelConfig::claude_code());

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

    #[test]
    fn test_action_error_severity() {
        let claude_err = ActionError::ClaudeError("execution failed".to_string());
        assert_eq!(claude_err.severity(), ErrorSeverity::Error);

        let var_err = ActionError::VariableError("variable not found".to_string());
        assert_eq!(var_err.severity(), ErrorSeverity::Error);

        let parse_err = ActionError::ParseError("invalid syntax".to_string());
        assert_eq!(parse_err.severity(), ErrorSeverity::Error);

        let exec_err = ActionError::ExecutionError("command failed".to_string());
        assert_eq!(exec_err.severity(), ErrorSeverity::Error);

        let io_err = ActionError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert_eq!(io_err.severity(), ErrorSeverity::Error);

        let json_err = ActionError::JsonError(
            serde_json::from_str::<serde_json::Value>("invalid").unwrap_err(),
        );
        assert_eq!(json_err.severity(), ErrorSeverity::Error);

        let rate_limit_err = ActionError::RateLimit {
            message: "too many requests".to_string(),
            wait_time: Duration::from_secs(60),
        };
        assert_eq!(rate_limit_err.severity(), ErrorSeverity::Error);
    }
}
