//! MCP server implementation for serving prompts and workflows

use crate::mcp::file_watcher::{FileWatcher, McpFileWatcherCallback};
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use swissarmyhammer_common::{Result, SwissArmyHammerError};
use swissarmyhammer_config::model::{parse_model_config, ModelManager};
use swissarmyhammer_config::{ModelUseCase, TemplateContext};
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};

use tokio::sync::{Mutex, RwLock};

use super::tool_handlers::ToolHandlers;
use super::tool_registry::{
    register_abort_tools, register_file_tools, register_flow_tools, register_git_tools,
    register_questions_tools, register_rules_tools, register_shell_tools, register_todo_tools,
    register_web_fetch_tools, register_web_search_tools, ToolContext, ToolRegistry,
};

/// Server instructions displayed to MCP clients
const SERVER_INSTRUCTIONS: &str =
    "The only coding assistant you'll ever need. Write specs, not code.";

/// Maximum retry attempts for operations with transient errors
const MAX_RETRIES: u32 = 3;

/// Initial backoff delay in milliseconds for retry operations
const INITIAL_BACKOFF_MS: u64 = 100;

/// MCP server for all SwissArmyHammer functionality.
#[derive(Clone)]
pub struct McpServer {
    library: Arc<RwLock<PromptLibrary>>,

    file_watcher: Arc<Mutex<FileWatcher>>,
    tool_registry: Arc<RwLock<ToolRegistry>>,
    pub tool_context: Arc<ToolContext>,
}

/// Determine if a retry should be attempted based on the error and attempt count.
///
/// # Arguments
///
/// * `attempt` - Current attempt number (1-indexed)
/// * `error` - The error that occurred
/// * `is_retryable` - Function to determine if an error is retryable
///
/// # Returns
///
/// * `bool` - True if should retry, false otherwise
fn should_retry(
    attempt: u32,
    error: &SwissArmyHammerError,
    is_retryable: fn(&SwissArmyHammerError) -> bool,
) -> bool {
    attempt < MAX_RETRIES && is_retryable(error)
}

/// Log a retry attempt with backoff information.
///
/// # Arguments
///
/// * `operation_name` - Name of the operation being retried
/// * `attempt` - Current attempt number (1-indexed)
/// * `backoff_ms` - Backoff delay in milliseconds
/// * `error` - The error that occurred
fn log_retry_attempt(
    operation_name: &str,
    attempt: u32,
    backoff_ms: u64,
    error: &SwissArmyHammerError,
) {
    tracing::warn!(
        "⚠️ {} attempt {} failed, retrying in {}ms: {}",
        operation_name,
        attempt,
        backoff_ms,
        error
    );
}

/// Retry an async operation with exponential backoff.
///
/// # Arguments
///
/// * `operation` - The async operation to retry
/// * `is_retryable` - Function to determine if an error is retryable
/// * `operation_name` - Name of the operation for logging
///
/// # Returns
///
/// * `Result<T>` - The result of the operation
async fn retry_with_backoff<F, T, Fut>(
    mut operation: F,
    is_retryable: fn(&SwissArmyHammerError) -> bool,
    operation_name: &str,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut last_error = None;
    let mut backoff_ms = INITIAL_BACKOFF_MS;

    for attempt in 1..=MAX_RETRIES {
        match operation().await {
            Ok(result) => {
                if attempt > 1 {
                    tracing::info!("✓ {} succeeded on attempt {}", operation_name, attempt);
                }
                return Ok(result);
            }
            Err(e) => {
                if should_retry(attempt, &e, is_retryable) {
                    log_retry_attempt(operation_name, attempt, backoff_ms, &e);
                    tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                    backoff_ms *= 2;
                }
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| SwissArmyHammerError::Other {
        message: format!("{} failed", operation_name),
    }))
}

/// Create ServerCapabilities for MCP protocol
fn create_server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        prompts: Some(PromptsCapability {
            list_changed: Some(true),
        }),
        tools: Some(ToolsCapability {
            list_changed: Some(true),
        }),
        resources: None,
        logging: None,
        completions: None,
        experimental: None,
    }
}

/// Create Implementation information for the MCP server
fn create_server_implementation() -> Implementation {
    Implementation {
        name: "SwissArmyHammer".into(),
        version: crate::VERSION.into(),
        icons: None,
        title: Some("SwissArmyHammer MCP Server".into()),
        website_url: Some("https://github.com/swissarmyhammer/swissarmyhammer".into()),
    }
}

impl McpServer {
    /// Create a new MCP server with the provided prompt library.
    ///
    /// # Arguments
    ///
    /// * `library` - The prompt library to serve via MCP
    ///
    /// # Returns
    ///
    /// * `Result<Self>` - The MCP server instance or an error if initialization fails
    ///
    /// # Errors
    ///
    pub async fn new(library: PromptLibrary) -> Result<Self> {
        let work_dir = std::env::current_dir().unwrap_or_else(|_| {
            // Fallback to a temporary directory if current directory is not accessible
            std::env::temp_dir()
        });
        Self::new_with_work_dir(library, work_dir, None).await
    }

    /// Create a new MCP server with the provided prompt library and working directory.
    ///
    /// # Arguments
    ///
    /// * `library` - The prompt library to serve via MCP
    /// * `work_dir` - The working directory to use for issue storage and git operations
    /// * `model_override` - Optional model name to override all use case model assignments
    ///
    /// # Returns
    ///
    /// * `Result<Self>` - The MCP server instance or an error if initialization fails
    ///
    /// # Errors
    ///
    pub async fn new_with_work_dir(
        library: PromptLibrary,
        work_dir: PathBuf,
        model_override: Option<String>,
    ) -> Result<Self> {
        let git_ops_arc = Self::initialize_git_operations(work_dir);
        let tool_handlers = ToolHandlers::new();
        let agent_config = Self::load_template_context()?;
        let use_case_agents = Self::initialize_use_case_agents(model_override)?;
        let (tool_registry_arc, tool_context) = Self::create_tool_context_and_registry(
            tool_handlers,
            git_ops_arc,
            agent_config,
            use_case_agents,
        );

        let server = Self {
            library: Arc::new(RwLock::new(library)),
            file_watcher: Arc::new(Mutex::new(FileWatcher::new())),
            tool_registry: tool_registry_arc,
            tool_context,
        };

        Ok(server)
    }

    /// Initialize git operations for the given working directory.
    ///
    /// # Arguments
    ///
    /// * `work_dir` - The working directory for git operations
    ///
    /// # Returns
    ///
    /// * `Arc<Mutex<Option<GitOperations>>>` - Wrapped git operations instance
    fn initialize_git_operations(work_dir: PathBuf) -> Arc<Mutex<Option<GitOperations>>> {
        let git_ops = match GitOperations::with_work_dir(work_dir) {
            Ok(ops) => Some(ops),
            Err(e) => {
                tracing::warn!("Git operations not available: {}", e);
                None
            }
        };
        Arc::new(Mutex::new(git_ops))
    }

    /// Load template context and agent configuration.
    ///
    /// # Returns
    ///
    /// * `Result<Arc<swissarmyhammer_config::model::ModelConfig>>` - Agent configuration
    fn load_template_context() -> Result<Arc<swissarmyhammer_config::model::ModelConfig>> {
        let template_context = TemplateContext::load_for_cli().map_err(|e| {
            tracing::warn!("Failed to load configuration, using default: {}", e);
            SwissArmyHammerError::Other {
                message: format!("Failed to load configuration: {}", e),
            }
        })?;
        Ok(Arc::new(template_context.get_agent_config(None)))
    }

    /// Initialize agent configurations for all use cases.
    ///
    /// # Arguments
    ///
    /// * `model_override` - Optional model name to override all use case assignments
    ///
    /// # Returns
    ///
    /// * `Result<HashMap<ModelUseCase, Arc<swissarmyhammer_config::model::ModelConfig>>>` - Use case agent map
    fn initialize_use_case_agents(
        model_override: Option<String>,
    ) -> Result<HashMap<ModelUseCase, Arc<swissarmyhammer_config::model::ModelConfig>>> {
        let mut use_case_agents = HashMap::new();

        if let Some(override_model_name) = model_override {
            Self::apply_model_override(&mut use_case_agents, override_model_name)?;
        } else {
            Self::resolve_use_case_agents(&mut use_case_agents);
        }

        Ok(use_case_agents)
    }

    /// Apply model override to all use cases.
    ///
    /// # Arguments
    ///
    /// * `use_case_agents` - Map to populate with agent configurations
    /// * `override_model_name` - Model name to use for all use cases
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Ok if override is successfully applied
    fn apply_model_override(
        use_case_agents: &mut HashMap<
            ModelUseCase,
            Arc<swissarmyhammer_config::model::ModelConfig>,
        >,
        override_model_name: String,
    ) -> Result<()> {
        tracing::info!(
            "Using global model override '{}' for all use cases",
            override_model_name
        );

        let override_agent = match ModelManager::find_agent_by_name(&override_model_name) {
            Ok(info) => match parse_model_config(&info.content) {
                Ok(config) => config,
                Err(e) => {
                    return Err(SwissArmyHammerError::Other {
                        message: format!("Invalid model override '{}': {}", override_model_name, e),
                    });
                }
            },
            Err(e) => {
                return Err(SwissArmyHammerError::Other {
                    message: format!("Invalid model override '{}': {}", override_model_name, e),
                });
            }
        };

        for use_case in [
            ModelUseCase::Root,
            ModelUseCase::Rules,
            ModelUseCase::Workflows,
        ] {
            use_case_agents.insert(use_case, Arc::new(override_agent.clone()));
        }

        Ok(())
    }

    /// Resolve agent configurations for all use cases from configuration.
    ///
    /// # Arguments
    ///
    /// * `use_case_agents` - Map to populate with agent configurations
    fn resolve_use_case_agents(
        use_case_agents: &mut HashMap<
            ModelUseCase,
            Arc<swissarmyhammer_config::model::ModelConfig>,
        >,
    ) {
        for use_case in [
            ModelUseCase::Root,
            ModelUseCase::Rules,
            ModelUseCase::Workflows,
        ] {
            match ModelManager::resolve_agent_config_for_use_case(use_case) {
                Ok(config) => {
                    tracing::debug!(
                        "Resolved {} use case to agent: {:?}",
                        use_case,
                        config.executor_type()
                    );
                    use_case_agents.insert(use_case, Arc::new(config));
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to resolve agent for {} use case: {}, using root",
                        use_case,
                        e
                    );
                }
            }
        }
    }

    /// Create tool context and registry with all tools registered.
    ///
    /// # Arguments
    ///
    /// * `tool_handlers` - Tool handlers instance
    /// * `git_ops_arc` - Git operations wrapped in Arc<Mutex>
    /// * `agent_config` - Agent configuration
    /// * `use_case_agents` - Use case to agent configuration map
    ///
    /// # Returns
    ///
    /// * `(Arc<RwLock<ToolRegistry>>, Arc<ToolContext>)` - Registry and context
    fn create_tool_context_and_registry(
        tool_handlers: ToolHandlers,
        git_ops_arc: Arc<Mutex<Option<GitOperations>>>,
        agent_config: Arc<swissarmyhammer_config::model::ModelConfig>,
        use_case_agents: HashMap<ModelUseCase, Arc<swissarmyhammer_config::model::ModelConfig>>,
    ) -> (Arc<RwLock<ToolRegistry>>, Arc<ToolContext>) {
        let mut tool_registry = ToolRegistry::new();
        Self::register_all_tools(&mut tool_registry);

        let mut tool_context = ToolContext::new(Arc::new(tool_handlers), git_ops_arc, agent_config);
        tool_context.use_case_agents = Arc::new(use_case_agents);

        let tool_registry_arc = Arc::new(RwLock::new(tool_registry));
        let tool_context = Arc::new(tool_context.with_tool_registry(tool_registry_arc.clone()));

        (tool_registry_arc, tool_context)
    }

    /// Register all available tools in the tool registry.
    ///
    /// # Arguments
    ///
    /// * `tool_registry` - Registry to register tools into
    fn register_all_tools(tool_registry: &mut ToolRegistry) {
        register_abort_tools(tool_registry);
        register_file_tools(tool_registry);
        register_flow_tools(tool_registry);
        register_git_tools(tool_registry);
        register_questions_tools(tool_registry);
        register_rules_tools(tool_registry);
        register_shell_tools(tool_registry);
        register_todo_tools(tool_registry);
        register_web_fetch_tools(tool_registry);
        register_web_search_tools(tool_registry);
        tracing::debug!("Registered all tool handlers");
    }

    /// Get a reference to the underlying prompt library.
    ///
    /// # Returns
    ///
    /// * `&Arc<RwLock<PromptLibrary>>` - Reference to the wrapped prompt library
    pub fn library(&self) -> &Arc<RwLock<PromptLibrary>> {
        &self.library
    }

    /// Set the MCP server port in the tool context
    ///
    /// This should be called after the server is bound to a port, so that
    /// workflows executed via MCP tools can access the server.
    ///
    /// # Arguments
    ///
    /// * `port` - The port the MCP server is listening on
    pub async fn set_server_port(&self, port: u16) {
        tracing::debug!("Setting MCP server port to {} in tool context", port);
        let mut port_lock = self.tool_context.mcp_server_port.write().await;
        *port_lock = Some(port);
    }

    /// Initialize the server.
    ///
    /// This method loads all prompts using the PromptResolver.
    /// It should be called before starting the MCP server.
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Ok if initialization succeeds, error otherwise
    ///
    /// # Errors
    ///
    /// Returns an error if prompt loading fails.
    pub async fn initialize(&self) -> Result<()> {
        let mut library = self.library.write().await;
        let mut resolver = PromptResolver::new();

        // Use the same loading logic as CLI
        resolver
            .load_all_prompts(&mut library)
            .map_err(|e| SwissArmyHammerError::Other {
                message: e.to_string(),
            })?;

        let total = library
            .list()
            .map_err(|e| SwissArmyHammerError::Other {
                message: e.to_string(),
            })?
            .len();
        tracing::debug!("Loaded {} prompts total", total);

        Ok(())
    }

    /// List all available prompts, excluding partial templates.
    ///
    /// Partial templates are filtered out as they are meant for internal use
    /// and should not be exposed via the MCP interface.
    ///
    /// # Returns
    ///
    /// * `Result<Vec<String>>` - List of prompt names or an error
    pub async fn list_prompts(&self) -> Result<Vec<String>> {
        let library = self.library.read().await;
        let prompts = library.list().map_err(|e| SwissArmyHammerError::Other {
            message: e.to_string(),
        })?;
        Ok(prompts
            .iter()
            .filter(|p| !p.is_partial_template())
            .map(|p| p.name.clone())
            .collect())
    }

    /// List all available tools from the tool registry.
    ///
    /// # Returns
    ///
    /// * `Vec<rmcp::model::Tool>` - List of all registered tools
    pub async fn list_tools(&self) -> Vec<rmcp::model::Tool> {
        self.tool_registry.read().await.list_tools()
    }

    /// Get a tool by name for execution.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tool to retrieve
    ///
    /// # Returns
    ///
    /// * `bool` - True if the tool exists, false otherwise
    pub async fn has_tool(&self, name: &str) -> bool {
        self.tool_registry.read().await.get_tool(name).is_some()
    }

    /// Execute a tool by name with the given arguments.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tool to execute
    /// * `arguments` - The arguments to pass to the tool
    ///
    /// # Returns
    ///
    /// * `Result<rmcp::model::CallToolResult, rmcp::ErrorData>` - The tool execution result
    pub async fn execute_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> std::result::Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
        let registry = self.tool_registry.read().await;
        if let Some(tool) = registry.get_tool(name) {
            // Convert Value to Map<String, Value> for tool execution
            let arguments_map = match arguments {
                serde_json::Value::Object(map) => map,
                _ => serde_json::Map::new(), // Use empty map if not an object
            };
            tool.execute(arguments_map, &self.tool_context).await
        } else {
            Err(rmcp::ErrorData::invalid_request(
                format!("Unknown tool: {}", name),
                None,
            ))
        }
    }

    /// Get a specific prompt by name, with optional template argument rendering.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the prompt to retrieve
    /// * `arguments` - Optional template arguments for rendering
    ///
    /// # Returns
    ///
    /// * `Result<String>` - The rendered prompt content or an error
    ///
    /// # Errors
    ///
    /// Returns an error if the prompt is not found, is a partial template,
    /// or if template rendering fails.
    pub async fn get_prompt(
        &self,
        name: &str,
        arguments: Option<&HashMap<String, String>>,
    ) -> Result<String> {
        let library = self.library.read().await;
        let prompt = library.get(name).map_err(|e| SwissArmyHammerError::Other {
            message: e.to_string(),
        })?;

        // Check if this is a partial template
        if prompt.is_partial_template() {
            return Err(SwissArmyHammerError::Other { message: format!(
                "Cannot access partial template '{name}' via MCP. Partial templates are for internal use only."
            ) });
        }

        // Handle arguments if provided
        let content = if let Some(args) = arguments {
            {
                let template_context = TemplateContext::with_template_vars(
                    args.iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect(),
                )
                .map_err(|e| SwissArmyHammerError::Other {
                    message: format!("Failed to create template context: {e}"),
                })?;
                library.render(name, &template_context).map_err(|e| {
                    SwissArmyHammerError::Other {
                        message: e.to_string(),
                    }
                })?
            }
        } else {
            prompt.template.clone()
        };

        Ok(content)
    }

    /// Convert serde_json::Map to HashMap<String, String> for template rendering.
    ///
    /// This helper method converts MCP tool arguments from JSON format to
    /// the string format expected by the template engine.
    ///
    /// # Arguments
    ///
    /// * `args` - The JSON map of arguments from MCP
    ///
    /// # Returns
    ///
    /// * `HashMap<String, String>` - The converted arguments
    fn json_map_to_string_map(args: &serde_json::Map<String, Value>) -> HashMap<String, String> {
        let mut template_args = HashMap::new();
        for (key, value) in args {
            let value_str = match value {
                Value::String(s) => s.clone(),
                v => v.to_string(),
            };
            template_args.insert(key.clone(), value_str);
        }
        template_args
    }

    /// Reload prompts from disk with retry logic.
    ///
    /// This method reloads all prompts from the file system and updates
    /// the internal library. It includes retry logic for transient errors.
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Ok if reload succeeds, error otherwise
    pub async fn reload_prompts(&self) -> Result<()> {
        self.reload_prompts_with_retry().await
    }

    /// Reload prompts with retry logic for transient file system errors
    async fn reload_prompts_with_retry(&self) -> Result<()> {
        retry_with_backoff(
            || self.reload_prompts_internal(),
            Self::is_retryable_fs_error,
            "Reload",
        )
        .await
    }

    /// Check if an error is a retryable file system error
    fn is_retryable_fs_error(error: &SwissArmyHammerError) -> bool {
        // Check for common transient file system errors
        if let SwissArmyHammerError::Io(io_err) = error {
            matches!(
                io_err.kind(),
                std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::Interrupted
                    | std::io::ErrorKind::WouldBlock
                    | std::io::ErrorKind::UnexpectedEof
            )
        } else {
            // Also retry if the error message contains certain patterns
            let error_str = error.to_string().to_lowercase();
            error_str.contains("temporarily unavailable")
                || error_str.contains("resource busy")
                || error_str.contains("locked")
        }
    }

    /// Internal reload method that performs the actual reload
    async fn reload_prompts_internal(&self) -> Result<()> {
        let mut library = self.library.write().await;
        let mut resolver = PromptResolver::new();

        // Get count before reload (default to 0 if library.list() fails)
        let before_count = library.list().map(|p| p.len()).unwrap_or(0);

        // Clear existing prompts and reload
        *library = PromptLibrary::new();
        resolver
            .load_all_prompts(&mut library)
            .map_err(|e| SwissArmyHammerError::Other {
                message: e.to_string(),
            })?;

        let after_count = library
            .list()
            .map_err(|e| SwissArmyHammerError::Other {
                message: e.to_string(),
            })?
            .len();
        tracing::info!(
            "🔄 Reloaded prompts: {} → {} prompts",
            before_count,
            after_count
        );

        Ok(())
    }

    /// Start watching prompt directories for file changes.
    ///
    /// When files change, the server will automatically reload prompts and
    /// send notifications to the MCP client.
    ///
    /// # Arguments
    ///
    /// * `peer` - The MCP peer connection for sending notifications
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Ok if watching starts successfully, error otherwise
    ///
    /// # Errors
    ///
    /// Returns an error if file watching cannot be initialized.
    pub async fn start_file_watching(&self, peer: rmcp::Peer<RoleServer>) -> Result<()> {
        // Create callback that handles file changes and notifications
        let callback = McpFileWatcherCallback::new(self.clone(), peer);

        retry_with_backoff(
            || async {
                let mut watcher = self.file_watcher.lock().await;
                watcher.start_watching(callback.clone()).await
            },
            Self::is_retryable_fs_error,
            "File watcher initialization",
        )
        .await
    }

    /// Stop watching prompt directories for file changes.
    ///
    /// This should be called when the MCP server is shutting down.
    pub async fn stop_file_watching(&self) {
        let mut watcher = self.file_watcher.lock().await;
        watcher.stop_watching();
    }

    /// Validate that a prompt can be accessed via MCP.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The prompt to validate
    /// * `name` - The name of the prompt for error messages
    ///
    /// # Returns
    ///
    /// * `Result<(), McpError>` - Ok if prompt is accessible, error if it's a partial template
    fn validate_prompt_access(
        prompt: &swissarmyhammer_prompts::Prompt,
        name: &str,
    ) -> std::result::Result<(), McpError> {
        if prompt.is_partial_template() {
            return Err(McpError::invalid_request(
                format!(
                    "Cannot access partial template '{}' via MCP. Partial templates are for internal use only.",
                    name
                ),
                None,
            ));
        }
        Ok(())
    }

    /// Render a prompt with the provided arguments.
    ///
    /// # Arguments
    ///
    /// * `library` - The prompt library containing the template
    /// * `name` - The name of the prompt to render
    /// * `prompt` - The prompt object
    /// * `arguments` - Optional arguments for template rendering
    ///
    /// # Returns
    ///
    /// * `Result<String, McpError>` - The rendered content or an error
    fn render_prompt_with_args(
        library: &PromptLibrary,
        name: &str,
        prompt: &swissarmyhammer_prompts::Prompt,
        arguments: &Option<serde_json::Map<String, Value>>,
    ) -> std::result::Result<String, McpError> {
        if let Some(args) = arguments {
            let template_args = Self::json_map_to_string_map(args);

            let template_context = TemplateContext::with_template_vars(
                template_args
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect(),
            )
            .map_err(|e| {
                McpError::internal_error(format!("Failed to create template context: {}", e), None)
            })?;

            library.render(name, &template_context).map_err(|e| {
                McpError::internal_error(format!("Template rendering error: {e}"), None)
            })
        } else {
            Ok(prompt.template.clone())
        }
    }

    /// Prepare tool context with peer for elicitation support.
    ///
    /// # Arguments
    ///
    /// * `peer` - The MCP peer connection
    ///
    /// # Returns
    ///
    /// * `ToolContext` - Tool context with peer configured
    fn prepare_tool_context(&self, peer: rmcp::Peer<RoleServer>) -> ToolContext {
        (*self.tool_context)
            .clone()
            .with_peer(Arc::new(peer.clone()))
    }

    /// Execute a tool with logging.
    ///
    /// # Arguments
    ///
    /// * `tool` - The tool to execute
    /// * `name` - The name of the tool
    /// * `arguments` - Tool arguments
    /// * `context` - Tool execution context
    ///
    /// # Returns
    ///
    /// * `Result<CallToolResult, McpError>` - The execution result
    async fn execute_tool_with_logging(
        tool: &dyn super::tool_registry::McpTool,
        name: &str,
        arguments: serde_json::Map<String, Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        tracing::info!("🔧 Executing tool: {}", name);
        let result = tool.execute(arguments, context).await;
        tracing::debug!("🔧 Tool execution result for {}: {:?}", name, result);
        result
    }
}

impl ServerHandler for McpServer {
    async fn initialize(
        &self,
        request: InitializeRequestParam,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<InitializeResult, McpError> {
        tracing::info!(
            "🚀 MCP client connecting: {} v{}",
            request.client_info.name,
            request.client_info.version
        );

        // Start file watching when MCP client connects
        match self.start_file_watching(context.peer).await {
            Ok(_) => {
                tracing::info!("🔍 File watching started for MCP client");
            }
            Err(e) => {
                tracing::error!("✗ Failed to start file watching for MCP client: {}", e);
                // Continue initialization even if file watching fails
            }
        }

        Ok(InitializeResult {
            protocol_version: ProtocolVersion::default(),
            capabilities: create_server_capabilities(),
            instructions: Some(SERVER_INSTRUCTIONS.into()),
            server_info: create_server_implementation(),
        })
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListPromptsResult, McpError> {
        let library = self.library.read().await;
        match library.list() {
            Ok(prompts) => {
                let prompt_list: Vec<Prompt> = prompts
                    .iter()
                    .filter(|p| !p.is_partial_template()) // Filter out partial templates
                    .map(|p| {
                        // Convert SwissArmyHammer prompt parameters to MCP PromptArguments
                        let arguments = if p.parameters.is_empty() {
                            None
                        } else {
                            Some(
                                p.parameters
                                    .iter()
                                    .map(|param| PromptArgument {
                                        name: param.name.clone(),
                                        title: None, // Could use param.name here if we want to display it
                                        description: Some(param.description.clone()),
                                        required: Some(param.required),
                                    })
                                    .collect(),
                            )
                        };

                        Prompt {
                            name: p.name.clone(),
                            description: p.description.clone(),
                            arguments,
                            icons: None,
                            title: Some(p.name.clone()),
                        }
                    })
                    .collect();

                Ok(ListPromptsResult {
                    prompts: prompt_list,
                    next_cursor: None,
                })
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<GetPromptResult, McpError> {
        let library = self.library.read().await;
        let prompt = match library.get(&request.name) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("Prompt '{}' not found: {}", request.name, e);
                return Err(McpError::invalid_request(
                    format!(
                        "Prompt '{}' is not available. It may have been deleted or renamed.",
                        request.name
                    ),
                    None,
                ));
            }
        };

        Self::validate_prompt_access(&prompt, &request.name)?;

        let content =
            Self::render_prompt_with_args(&library, &request.name, &prompt, &request.arguments)?;

        Ok(GetPromptResult {
            description: prompt.description.clone(),
            messages: vec![PromptMessage {
                role: PromptMessageRole::User,
                content: PromptMessageContent::Text { text: content },
            }],
        })
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: self.tool_registry.read().await.list_tools(),
            next_cursor: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        tracing::debug!(
            "🔧 call_tool() invoked for tool: {}, arguments: {:?}",
            request.name,
            request.arguments
        );

        let registry = self.tool_registry.read().await;
        let tool = registry.get_tool(&request.name).ok_or_else(|| {
            tracing::error!("🔧 Unknown tool requested: {}", request.name);
            McpError::invalid_request(format!("Unknown tool: {}", request.name), None)
        })?;

        let tool_context_with_peer = self.prepare_tool_context(context.peer.clone());
        let arguments = request.arguments.unwrap_or_default();

        Self::execute_tool_with_logging(tool, &request.name, arguments, &tool_context_with_peer)
            .await
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: create_server_capabilities(),
            server_info: create_server_implementation(),
            instructions: Some(SERVER_INSTRUCTIONS.into()),
        }
    }
}
