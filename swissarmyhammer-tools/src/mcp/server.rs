//! MCP server implementation for serving prompts and workflows

use crate::mcp::file_watcher::{FileWatcher, McpFileWatcherCallback};
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
// TODO: Move workflow storage to swissarmyhammer-common to fix circular dependency
// use swissarmyhammer_workflow::{
//     FileSystemWorkflowRunStorage, FileSystemWorkflowStorage, WorkflowRunStorageBackend,
//     WorkflowStorage, WorkflowStorageBackend,
// };

use swissarmyhammer_common::{Result, SwissArmyHammerError};
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_issues::{FileSystemIssueStorage, IssueStorage};
use swissarmyhammer_memoranda::{MarkdownMemoStorage, MemoStorage};
use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};

use tokio::sync::{Mutex, RwLock};

use super::tool_handlers::ToolHandlers;
use super::tool_registry::{
    register_abort_tools, register_file_tools, register_git_tools, register_issue_tools,
    register_memo_tools, register_notify_tools, register_outline_tools, register_rules_tools,
    register_search_tools, register_shell_tools, register_todo_tools, register_web_fetch_tools,
    register_web_search_tools, ToolContext, ToolRegistry,
};

/// MCP server for all SwissArmyHammer functionality.
#[derive(Clone)]
pub struct McpServer {
    library: Arc<RwLock<PromptLibrary>>,

    file_watcher: Arc<Mutex<FileWatcher>>,
    tool_registry: Arc<ToolRegistry>,
    pub tool_context: Arc<ToolContext>,
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
        Self::new_with_work_dir(library, work_dir).await
    }

    /// Create a new MCP server with the provided prompt library and working directory.
    ///
    /// # Arguments
    ///
    /// * `library` - The prompt library to serve via MCP
    /// * `work_dir` - The working directory to use for issue storage and git operations
    ///
    /// # Returns
    ///
    /// * `Result<Self>` - The MCP server instance or an error if initialization fails
    ///
    /// # Errors
    ///
    pub async fn new_with_work_dir(library: PromptLibrary, work_dir: PathBuf) -> Result<Self> {
        // Initialize issue storage using new storage defaults with working directory context
        let issue_storage = {
            let original_dir = std::env::current_dir().ok();
            let needs_dir_change = original_dir.as_ref().map_or(true, |dir| work_dir != *dir);

            // Set working directory context for storage creation if different from current
            if needs_dir_change {
                std::env::set_current_dir(&work_dir).map_err(|e| SwissArmyHammerError::Other {
                    message: format!("Failed to set working directory: {e}"),
                })?;
            }

            // Create storage
            let storage = FileSystemIssueStorage::new_default().map_err(|e| {
                tracing::error!("Failed to create issue storage: {}", e);
                SwissArmyHammerError::Other {
                    message: format!("Failed to create issue storage: {e}"),
                }
            })?;

            // Always restore original working directory if we changed it and it still exists
            if needs_dir_change {
                if let Some(ref original_dir) = original_dir {
                    if let Err(e) = std::env::set_current_dir(original_dir) {
                        tracing::warn!("Failed to restore original working directory: {}", e);
                    }
                }
            }

            Box::new(storage) as Box<dyn IssueStorage>
        };

        // Initialize memo storage with environment variable support, then default location, fallback to temp dir for tests
        let memo_storage = {
            // First check if SWISSARMYHAMMER_MEMOS_DIR environment variable is set
            if let Ok(custom_path) = std::env::var("SWISSARMYHAMMER_MEMOS_DIR") {
                let custom_dir = std::path::PathBuf::from(custom_path);
                // Try to create directory, but don't fail if it already exists or can't be created
                if let Err(e) = std::fs::create_dir_all(&custom_dir) {
                    tracing::warn!(
                        "Failed to create custom memos directory {}: {}",
                        custom_dir.display(),
                        e
                    );
                }
                Box::new(MarkdownMemoStorage::new(custom_dir)) as Box<dyn MemoStorage>
            } else {
                match MarkdownMemoStorage::new_default().await {
                    Ok(storage) => Box::new(storage) as Box<dyn MemoStorage>,
                    Err(e) => {
                        tracing::warn!("Cannot create memo storage in Git repository ({}), using temporary directory for testing", e);
                        // Fallback to temporary directory for tests
                        let temp_dir = std::env::temp_dir().join("swissarmyhammer-mcp-test");
                        std::fs::create_dir_all(&temp_dir).map_err(|err| {
                            SwissArmyHammerError::Other {
                                message: format!(
                                    "Failed to create temporary memo directory: {err}"
                                ),
                            }
                        })?;
                        Box::new(MarkdownMemoStorage::new(temp_dir)) as Box<dyn MemoStorage>
                    }
                }
            }
        };

        // Initialize git operations with work_dir - make it optional for tests
        let git_ops = match GitOperations::with_work_dir(work_dir.clone()) {
            Ok(ops) => Some(ops),
            Err(e) => {
                tracing::warn!("Git operations not available: {}", e);
                None
            }
        };

        // Create Arc wrappers for shared storage
        let issue_storage = Arc::new(RwLock::new(issue_storage));
        let memo_storage_arc = Arc::new(RwLock::new(memo_storage));
        let git_ops_arc = Arc::new(Mutex::new(git_ops));

        // Initialize tool handlers with memo storage
        let tool_handlers = ToolHandlers::new(memo_storage_arc.clone());

        // Load agent configuration from sah.yaml
        let template_context = TemplateContext::load_for_cli().map_err(|e| {
            tracing::warn!("Failed to load configuration, using default: {}", e);
            SwissArmyHammerError::Other {
                message: format!("Failed to load configuration: {}", e),
            }
        })?;
        let agent_config = Arc::new(template_context.get_agent_config(None));

        // Initialize tool registry and context
        let mut tool_registry = ToolRegistry::new();
        let tool_context = Arc::new(ToolContext::new(
            Arc::new(tool_handlers.clone()),
            issue_storage.clone(),
            git_ops_arc.clone(),
            memo_storage_arc.clone(),
            agent_config,
        ));

        // Register all available tools
        register_abort_tools(&mut tool_registry);
        register_file_tools(&mut tool_registry);
        register_git_tools(&mut tool_registry);
        register_issue_tools(&mut tool_registry);
        register_memo_tools(&mut tool_registry);
        register_notify_tools(&mut tool_registry);
        register_outline_tools(&mut tool_registry);
        register_rules_tools(&mut tool_registry);
        register_search_tools(&mut tool_registry);
        register_shell_tools(&mut tool_registry);
        register_todo_tools(&mut tool_registry);
        register_web_fetch_tools(&mut tool_registry);
        register_web_search_tools(&mut tool_registry);
        tracing::debug!("Registered all tool handlers");

        Ok(Self {
            library: Arc::new(RwLock::new(library)),
            file_watcher: Arc::new(Mutex::new(FileWatcher::new())),
            tool_registry: Arc::new(tool_registry),
            tool_context,
        })
    }

    /// Get a reference to the underlying prompt library.
    ///
    /// # Returns
    ///
    /// * `&Arc<RwLock<PromptLibrary>>` - Reference to the wrapped prompt library
    pub fn library(&self) -> &Arc<RwLock<PromptLibrary>> {
        &self.library
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
    pub fn list_tools(&self) -> Vec<rmcp::model::Tool> {
        self.tool_registry.list_tools()
    }

    /// Get a tool by name for execution.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tool to retrieve
    ///
    /// # Returns
    ///
    /// * `Option<&dyn McpTool>` - The tool if found, None otherwise
    pub fn get_tool(&self, name: &str) -> Option<&dyn crate::mcp::tool_registry::McpTool> {
        self.tool_registry.get_tool(name)
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
        if let Some(tool) = self.tool_registry.get_tool(name) {
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
        const MAX_RETRIES: u32 = 3;
        const INITIAL_BACKOFF_MS: u64 = 100;

        let mut last_error = None;
        let mut backoff_ms = INITIAL_BACKOFF_MS;

        for attempt in 1..=MAX_RETRIES {
            match self.reload_prompts_internal().await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last_error = Some(e);

                    // Check if this is a retryable error
                    if attempt < MAX_RETRIES
                        && last_error.as_ref().is_some_and(Self::is_retryable_fs_error)
                    {
                        tracing::warn!(
                            "⚠️ Reload attempt {} failed, retrying in {}ms: {}",
                            attempt,
                            backoff_ms,
                            last_error
                                .as_ref()
                                .map_or("Unknown error".to_string(), |e| e.to_string())
                        );

                        tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                        backoff_ms *= 2; // Exponential backoff
                    } else {
                        break;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| SwissArmyHammerError::Other {
            message: "Prompt reload failed".to_string(),
        }))
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
        const MAX_RETRIES: u32 = 3;
        const INITIAL_BACKOFF_MS: u64 = 100;

        // Create callback that handles file changes and notifications
        let callback = McpFileWatcherCallback::new(self.clone(), peer);

        let mut last_error = None;
        let mut backoff_ms = INITIAL_BACKOFF_MS;

        for attempt in 1..=MAX_RETRIES {
            // Start watching using the file watcher module
            let result = {
                let mut watcher = self.file_watcher.lock().await;
                watcher.start_watching(callback.clone()).await
            };

            match result {
                Ok(()) => {
                    if attempt > 1 {
                        tracing::info!(
                            "✅ File watcher started successfully on attempt {}",
                            attempt
                        );
                    }
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);

                    if attempt < MAX_RETRIES
                        && last_error.as_ref().is_some_and(Self::is_retryable_fs_error)
                    {
                        tracing::warn!(
                            "⚠️ File watcher initialization attempt {} failed, retrying in {}ms: {}",
                            attempt,
                            backoff_ms,
                            last_error.as_ref().map_or("Unknown error".to_string(), |e| e.to_string())
                        );

                        tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                        backoff_ms *= 2; // Exponential backoff
                    } else {
                        break;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| SwissArmyHammerError::Other {
            message: "File watcher initialization failed".to_string(),
        }))
    }

    /// Stop watching prompt directories for file changes.
    ///
    /// This should be called when the MCP server is shutting down.
    pub async fn stop_file_watching(&self) {
        let mut watcher = self.file_watcher.lock().await;
        watcher.stop_watching();
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
                tracing::error!("❌ Failed to start file watching for MCP client: {}", e);
                // Continue initialization even if file watching fails
            }
        }

        Ok(InitializeResult {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities {
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
            },
            instructions: Some(
                "The only coding assistant you'll ever need. Write specs, not code.".into(),
            ),
            server_info: Implementation {
                name: "SwissArmyHammer".into(),
                version: crate::VERSION.into(),
                icons: None,
                title: Some("SwissArmyHammer MCP Server".into()),
                website_url: Some("https://github.com/swissarmyhammer/swissarmyhammer".into()),
            },
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
                        // Domain prompts don't have parameters yet - using empty list for now
                        let arguments = None;

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
        match library.get(&request.name) {
            Ok(prompt) => {
                // Check if this is a partial template
                if prompt.is_partial_template() {
                    return Err(McpError::invalid_request(
                        format!(
                            "Cannot access partial template '{}' via MCP. Partial templates are for internal use only.",
                            request.name
                        ),
                        None,
                    ));
                }

                // Handle arguments if provided
                let content = if let Some(args) = &request.arguments {
                    let template_args = Self::json_map_to_string_map(args);

                    let template_context = match TemplateContext::with_template_vars(
                        template_args
                            .iter()
                            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                            .collect(),
                    ) {
                        Ok(ctx) => ctx,
                        Err(e) => {
                            return Ok(GetPromptResult {
                                description: Some(format!(
                                    "Error: Failed to create template context: {}",
                                    e
                                )),
                                messages: vec![],
                            });
                        }
                    };
                    match library.render(&request.name, &template_context) {
                        Ok(rendered) => rendered,
                        Err(e) => {
                            return Err(McpError::internal_error(
                                format!("Template rendering error: {e}"),
                                None,
                            ))
                        }
                    }
                } else {
                    prompt.template.clone()
                };

                Ok(GetPromptResult {
                    description: prompt.description.clone(),
                    messages: vec![PromptMessage {
                        role: PromptMessageRole::User,
                        content: PromptMessageContent::Text { text: content },
                    }],
                })
            }
            Err(e) => {
                tracing::warn!("Prompt '{}' not found: {}", request.name, e);
                Err(McpError::invalid_request(
                    format!(
                        "Prompt '{}' is not available. It may have been deleted or renamed.",
                        request.name
                    ),
                    None,
                ))
            }
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: self.tool_registry.list_tools(),
            next_cursor: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        tracing::info!("🔧 call_tool() invoked for tool: {}", request.name);
        tracing::debug!("🔧 Tool arguments: {:?}", request.arguments);

        if let Some(tool) = self.tool_registry.get_tool(&request.name) {
            tracing::info!("🔧 Executing tool: {}", request.name);
            let result = tool
                .execute(request.arguments.unwrap_or_default(), &self.tool_context)
                .await;
            tracing::info!(
                "🔧 Tool execution result for {}: {:?}",
                request.name,
                result.is_ok()
            );
            result
        } else {
            tracing::error!("🔧 Unknown tool requested: {}", request.name);
            Err(McpError::invalid_request(
                format!("Unknown tool: {}", request.name),
                None,
            ))
        }
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities {
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
            },
            server_info: Implementation {
                name: "SwissArmyHammer".into(),
                version: crate::VERSION.into(),
                icons: None,
                title: Some("SwissArmyHammer MCP Server".into()),
                website_url: Some("https://github.com/swissarmyhammer/swissarmyhammer".into()),
            },
            instructions: Some(
                "The only coding assistant you'll ever need. Write specs, not code.".into(),
            ),
        }
    }
}
