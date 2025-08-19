//! MCP server implementation for serving prompts and workflows

use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::{Error as McpError, RoleServer, ServerHandler};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use swissarmyhammer::common::rate_limiter::get_rate_limiter;
use swissarmyhammer::file_watcher::{FileWatcher, FileWatcherCallback};
use swissarmyhammer::git::GitOperations;
use swissarmyhammer::issues::{FileSystemIssueStorage, IssueStorage};
use swissarmyhammer::memoranda::{MarkdownMemoStorage, MemoStorage};
use swissarmyhammer::workflow::{
    FileSystemWorkflowRunStorage, FileSystemWorkflowStorage, WorkflowRunStorageBackend,
    WorkflowStorage, WorkflowStorageBackend,
};
use swissarmyhammer::{PromptLibrary, PromptResolver, Result, SwissArmyHammerError};
use tokio::sync::{Mutex, RwLock};

use super::tool_handlers::ToolHandlers;
use super::tool_registry::{
    register_abort_tools, register_file_tools, register_issue_tools, register_memo_tools,
    register_notify_tools, register_outline_tools, register_search_tools, register_shell_tools,
    register_todo_tools, register_web_fetch_tools, register_web_search_tools, ToolContext,
    ToolRegistry,
};

/// MCP server for serving prompts and workflows
#[derive(Clone)]
pub struct McpServer {
    library: Arc<RwLock<PromptLibrary>>,
    workflow_storage: Arc<RwLock<WorkflowStorage>>,
    file_watcher: Arc<Mutex<FileWatcher>>,
    tool_registry: Arc<ToolRegistry>,
    /// Tool context containing shared state for tool execution
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
    /// Returns an error if workflow storage, issue storage, or git operations fail to initialize.
    pub fn new(library: PromptLibrary) -> Result<Self> {
        let work_dir = std::env::current_dir().map_err(|e| {
            SwissArmyHammerError::Other(format!("Failed to get current directory: {e}"))
        })?;
        Self::new_with_work_dir(library, work_dir)
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
    /// Returns an error if workflow storage, issue storage, or git operations fail to initialize.
    pub fn new_with_work_dir(library: PromptLibrary, work_dir: PathBuf) -> Result<Self> {
        // Initialize workflow storage with filesystem backend
        let workflow_backend = Arc::new(FileSystemWorkflowStorage::new().map_err(|e| {
            tracing::error!("Failed to create workflow storage: {}", e);
            SwissArmyHammerError::Other(format!("Failed to create workflow storage: {e}"))
        })?) as Arc<dyn WorkflowStorageBackend>;

        // Create runs directory in user's home directory
        let runs_path = Self::get_workflow_runs_path();

        let run_backend = Arc::new(FileSystemWorkflowRunStorage::new(runs_path).map_err(|e| {
            tracing::error!("Failed to create workflow run storage: {}", e);
            SwissArmyHammerError::Other(format!("Failed to create workflow run storage: {e}"))
        })?) as Arc<dyn WorkflowRunStorageBackend>;

        let workflow_storage = WorkflowStorage::new(workflow_backend, run_backend);

        // Initialize issue storage using new storage defaults with working directory context
        let issue_storage = {
            let original_dir = std::env::current_dir().map_err(|e| {
                SwissArmyHammerError::Other(format!("Failed to get current directory: {e}"))
            })?;

            let needs_dir_change = work_dir != original_dir;

            // Set working directory context for storage creation if different from current
            if needs_dir_change {
                std::env::set_current_dir(&work_dir).map_err(|e| {
                    SwissArmyHammerError::Other(format!("Failed to set working directory: {e}"))
                })?;
            }

            // Create storage with automatic migration and detailed logging
            let storage_result =
                FileSystemIssueStorage::new_default_with_migration_info().map_err(|e| {
                    tracing::error!("Failed to create issue storage: {}", e);
                    SwissArmyHammerError::Other(format!("Failed to create issue storage: {e}"))
                });

            // Log migration results for MCP server
            let (storage, migration_result) = storage_result?;
            if let Some(result) = migration_result {
                match result {
                    swissarmyhammer::issues::filesystem::MigrationResult::Success(stats) => {
                        tracing::info!(
                            "MCP server performed automatic migration: {} files moved to .swissarmyhammer/issues",
                            stats.files_moved
                        );
                    }
                    swissarmyhammer::issues::filesystem::MigrationResult::NotNeeded(_) => {
                        tracing::debug!("No migration needed for MCP server");
                    }
                }
            }

            // Always restore original working directory if we changed it
            if needs_dir_change {
                if let Err(e) = std::env::set_current_dir(&original_dir) {
                    tracing::warn!("Failed to restore original working directory: {}", e);
                }
            }

            Box::new(storage) as Box<dyn IssueStorage>
        };

        // Initialize memo storage with default location
        let memo_storage = Box::new(MarkdownMemoStorage::new_default().map_err(|e| {
            tracing::error!("Failed to create memo storage: {}", e);
            SwissArmyHammerError::Other(format!("Failed to create memo storage: {e}"))
        })?) as Box<dyn MemoStorage>;

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

        // Initialize tool registry and context
        let mut tool_registry = ToolRegistry::new();
        let tool_context = Arc::new(ToolContext::new(
            Arc::new(tool_handlers.clone()),
            issue_storage.clone(),
            git_ops_arc.clone(),
            memo_storage_arc.clone(),
            get_rate_limiter().clone(),
        ));

        // Register all available tools
        register_abort_tools(&mut tool_registry);
        register_file_tools(&mut tool_registry);
        register_issue_tools(&mut tool_registry);
        register_memo_tools(&mut tool_registry);
        register_notify_tools(&mut tool_registry);
        register_outline_tools(&mut tool_registry);
        register_search_tools(&mut tool_registry);
        register_shell_tools(&mut tool_registry);
        register_todo_tools(&mut tool_registry);
        register_web_fetch_tools(&mut tool_registry);
        register_web_search_tools(&mut tool_registry);

        Ok(Self {
            library: Arc::new(RwLock::new(library)),
            workflow_storage: Arc::new(RwLock::new(workflow_storage)),
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

    /// Initialize the server by loading prompts and workflows from disk.
    ///
    /// This method loads all prompts using the PromptResolver and initializes
    /// workflow storage. It should be called before starting the MCP server.
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Ok if initialization succeeds, error otherwise
    ///
    /// # Errors
    ///
    /// Returns an error if prompt loading or workflow initialization fails.
    pub async fn initialize(&self) -> Result<()> {
        let mut library = self.library.write().await;
        let mut resolver = PromptResolver::new();

        // Use the same loading logic as CLI
        resolver.load_all_prompts(&mut library)?;

        let total = library.list()?.len();
        tracing::debug!("Loaded {} prompts total", total);

        // Initialize workflows - workflows are loaded automatically by FileSystemWorkflowStorage
        // so we just need to check how many are available
        let workflow_storage = self.workflow_storage.read().await;
        let workflow_count = workflow_storage.list_workflows()?.len();
        tracing::debug!("Loaded {} workflows total", workflow_count);

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
        let prompts = library.list()?;
        Ok(prompts
            .iter()
            .filter(|p| !Self::is_partial_template(p))
            .map(|p| p.name.clone())
            .collect())
    }

    /// List all available workflows loaded from the workflow storage.
    ///
    /// # Returns
    ///
    /// * `Result<Vec<String>>` - List of workflow names or an error
    pub async fn list_workflows(&self) -> Result<Vec<String>> {
        let workflow_storage = self.workflow_storage.read().await;
        let workflows = workflow_storage.list_workflows()?;
        Ok(workflows.iter().map(|w| w.name.to_string()).collect())
    }

    /// Check if a prompt is a partial template that should not be exposed over MCP.
    ///
    /// Partial templates are identified by either:
    /// 1. Starting with the `{% partial %}` marker
    /// 2. Having a description containing "Partial template for reuse"
    ///
    /// # Arguments
    ///
    /// * `prompt` - The prompt to check
    ///
    /// # Returns
    ///
    /// * `bool` - True if the prompt is a partial template
    fn is_partial_template(prompt: &swissarmyhammer::prompts::Prompt) -> bool {
        // Check if the template starts with the partial marker
        if prompt.template.trim().starts_with("{% partial %}") {
            return true;
        }

        // Check if the description indicates it's a partial template
        if let Some(description) = &prompt.description {
            if description.contains("Partial template for reuse in other prompts") {
                return true;
            }
        }

        false
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
        let prompt = library.get(name)?;

        // Check if this is a partial template
        if Self::is_partial_template(&prompt) {
            return Err(SwissArmyHammerError::Other(format!(
                "Cannot access partial template '{name}' via MCP. Partial templates are for internal use only."
            )));
        }

        // Handle arguments if provided
        let content = if let Some(args) = arguments {
            library.render_prompt(name, args)?
        } else {
            prompt.template.clone()
        };

        Ok(content)
    }

    /// Get the workflow runs directory path
    fn get_workflow_runs_path() -> std::path::PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            })
            .join(".swissarmyhammer")
            .join("workflow-runs")
    }

    /// Convert internal prompt arguments to MCP PromptArgument structures.
    ///
    /// # Arguments
    ///
    /// * `args` - The internal argument specifications
    ///
    /// # Returns
    ///
    /// * `Option<Vec<PromptArgument>>` - The converted MCP arguments or None if empty
    fn convert_prompt_arguments(
        params: &[swissarmyhammer::common::Parameter],
    ) -> Option<Vec<PromptArgument>> {
        if params.is_empty() {
            None
        } else {
            Some(
                params
                    .iter()
                    .map(|param| PromptArgument {
                        name: param.name.clone(),
                        description: Some(param.description.clone()),
                        required: Some(param.required),
                    })
                    .collect(),
            )
        }
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

        Err(last_error
            .unwrap_or_else(|| SwissArmyHammerError::Other("Prompt reload failed".to_string())))
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
        resolver.load_all_prompts(&mut library)?;

        let after_count = library.list()?.len();
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

        Err(last_error.unwrap_or_else(|| {
            SwissArmyHammerError::Other("File watcher initialization failed".to_string())
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

/// Callback implementation for file watcher that handles prompt reloading
#[derive(Clone)]
struct McpFileWatcherCallback {
    server: McpServer,
    peer: rmcp::Peer<RoleServer>,
}

impl McpFileWatcherCallback {
    fn new(server: McpServer, peer: rmcp::Peer<RoleServer>) -> Self {
        Self { server, peer }
    }
}

impl FileWatcherCallback for McpFileWatcherCallback {
    async fn on_file_changed(&self, paths: Vec<std::path::PathBuf>) -> Result<()> {
        tracing::info!("📄 Prompt file changed: {:?}", paths);

        // Reload the library
        if let Err(e) = self.server.reload_prompts().await {
            tracing::error!("❌ Failed to reload prompts: {}", e);
            return Err(e);
        }
        tracing::info!("✅ Prompts reloaded successfully");

        // Send notification to client about prompt list change
        let peer_clone = self.peer.clone();
        tokio::spawn(async move {
            match peer_clone.notify_prompt_list_changed().await {
                Ok(_) => {
                    tracing::info!("📢 Sent prompts/listChanged notification to client");
                }
                Err(e) => {
                    tracing::error!("❌ Failed to send notification: {}", e);
                }
            }
        });

        Ok(())
    }

    async fn on_error(&self, error: String) {
        tracing::error!("❌ File watcher error: {}", error);
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
            },instructions: Some("A flexible prompt and workflow management server with integrated issue tracking. Use list_prompts to see available prompts and get_prompt to retrieve and render them. Use workflow tools to execute and manage workflows. Use issue_* tools to create and manage work items tracked as markdown files in your repository.".into()),
            server_info: Implementation {
                name: "SwissArmyHammer".into(),
                version: crate::VERSION.into(),
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
                    .filter(|p| !Self::is_partial_template(p)) // Filter out partial templates
                    .map(|p| {
                        let arguments = Self::convert_prompt_arguments(&p.parameters);

                        Prompt {
                            name: p.name.clone(),
                            description: p.description.clone(),
                            arguments,
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
                if Self::is_partial_template(&prompt) {
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

                    match library.render_prompt(&request.name, &template_args) {
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
                    description: prompt.description,
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
        if let Some(tool) = self.tool_registry.get_tool(&request.name) {
            tool.execute(request.arguments.unwrap_or_default(), &self.tool_context)
                .await
        } else {
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
            },server_info: Implementation {
                name: "SwissArmyHammer".into(),
                version: crate::VERSION.into(),
            },instructions: Some("A flexible prompt and workflow management server with integrated issue tracking. Use list_prompts to see available prompts and get_prompt to retrieve and render them. Use workflow tools to execute and manage workflows. Use issue_* tools to create and manage work items tracked as markdown files in your repository.".into()),
        }
    }
}
