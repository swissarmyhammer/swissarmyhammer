//! MCP server implementation for serving prompts and workflows
// sah rule ignore acp/capability-enforcement

use crate::mcp::file_watcher::{FileWatcher, McpFileWatcherCallback};
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use swissarmyhammer_common::utils::find_git_repository_root_from;
use swissarmyhammer_common::{is_prompt_visible, Result, SwissArmyHammerError};
use swissarmyhammer_config::model::{parse_model_config, ModelManager};
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};

use tokio::sync::{Mutex, RwLock};

use super::tool_handlers::ToolHandlers;
use super::tool_registry::{
    register_code_context_tools, register_file_tools, register_git_tools, register_kanban_tools,
    register_questions_tools, register_ralph_tools, register_shell_tools, register_web_tools,
    ToolContext, ToolRegistry,
};
use super::tools::agent::register_agent_tools;
use super::tools::skill::register_skill_tools;
use swissarmyhammer_agents::AgentLibrary;
use swissarmyhammer_skills::SkillLibrary;

/// Server instructions displayed to MCP clients
const SERVER_INSTRUCTIONS: &str =
    "The only coding assistant you'll ever need. Agent-driven engineering.";

/// Build server instructions, optionally appending LSP health status.
///
/// When a work directory is provided, runs the doctor check to detect project
/// types and their LSP servers. If any LSP servers are missing, a
/// `setupStatus:` block is appended listing the missing servers with install
/// hints. If all servers are installed (or no projects are detected), returns
/// just the base instructions to avoid noise.
pub(crate) fn build_instructions_with_health(work_dir: Option<&Path>) -> String {
    let Some(path) = work_dir else {
        return SERVER_INSTRUCTIONS.to_string();
    };

    let report = crate::mcp::tools::code_context::doctor::run_doctor(path);

    let missing: Vec<_> = report.lsp_servers.iter().filter(|s| !s.installed).collect();

    if missing.is_empty() {
        return SERVER_INSTRUCTIONS.to_string();
    }

    let mut instructions = SERVER_INSTRUCTIONS.to_string();
    instructions.push_str("\n\nsetupStatus: This workspace could benefit from additional tooling.");
    for server in &missing {
        let hint = server.install_hint.as_deref().unwrap_or("see project docs");
        instructions.push_str(&format!("\n  {}: NOT INSTALLED — {}", server.name, hint));
    }

    instructions
}

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
    /// Skill library - kept alive to back the SkillTool's shared reference
    #[allow(dead_code)]
    skill_library: Arc<RwLock<SkillLibrary>>,
    /// Agent library - kept alive to back the AgentTool's shared reference
    #[allow(dead_code)]
    agent_library: Arc<RwLock<AgentLibrary>>,
    /// Working directory — stored for deferred initialization (e.g. code-context)
    work_dir: Option<PathBuf>,
    /// Watches tools.yaml for changes, reloads on list_tools() calls
    tool_config_watcher: Arc<Mutex<super::tool_config::ToolConfigWatcher>>,
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
        "{} attempt {} failed, retrying in {}ms: {}",
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
                    tracing::info!("{} succeeded on attempt {}", operation_name, attempt);
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
    let mut caps = ServerCapabilities::default();
    caps.prompts = Some(PromptsCapability {
        list_changed: Some(true),
    });
    caps.tools = Some(ToolsCapability {
        list_changed: Some(true),
    });
    caps
}

/// Create Implementation information for the MCP server
fn create_server_implementation() -> Implementation {
    Implementation::new("SwissArmyHammer", crate::VERSION)
        .with_title("SwissArmyHammer MCP Server")
        .with_website_url("https://github.com/swissarmyhammer/swissarmyhammer")
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
        Self::new_with_work_dir(library, work_dir, None, false).await
    }

    /// Create a new MCP server with the provided prompt library and working directory.
    ///
    /// # Arguments
    ///
    /// * `library` - The prompt library to serve via MCP
    /// * `work_dir` - The working directory to use for issue storage and git operations
    /// * `model_override` - Optional model name to override all use case model assignments
    /// * `agent_mode` - Whether to register agent tools (true when powering a full agent)
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
        agent_mode: bool,
    ) -> Result<Self> {
        let git_ops_arc = Self::initialize_git_operations(work_dir.clone());
        let tool_handlers = ToolHandlers::new();
        let agent_config = Self::resolve_agent_config(model_override)?;

        let skill_library = Self::init_skill_library().await;
        let agent_library = Self::init_agent_library().await;
        let prompt_library = Arc::new(RwLock::new(library));

        let (tool_registry_arc, tool_context) = Self::create_tool_context_and_registry(
            tool_handlers,
            git_ops_arc,
            agent_config,
            Some(work_dir.clone()),
            skill_library.clone(),
            agent_library.clone(),
            prompt_library.clone(),
            agent_mode,
        )
        .await;

        Ok(Self {
            library: prompt_library,
            file_watcher: Arc::new(Mutex::new(FileWatcher::new())),
            tool_registry: tool_registry_arc,
            tool_context,
            skill_library,
            agent_library,
            work_dir: Some(work_dir),
            tool_config_watcher: Arc::new(Mutex::new(super::tool_config::ToolConfigWatcher::new())),
        })
    }

    /// Construct a `SkillLibrary` pre-populated with the builtin skills.
    async fn init_skill_library() -> Arc<RwLock<SkillLibrary>> {
        let library = Arc::new(RwLock::new(SkillLibrary::new()));
        let mut lib = library.write().await;
        lib.load_defaults();
        tracing::debug!("Loaded {} skills", lib.len());
        drop(lib);
        library
    }

    /// Construct an `AgentLibrary` pre-populated with the builtin agents.
    async fn init_agent_library() -> Arc<RwLock<AgentLibrary>> {
        let library = Arc::new(RwLock::new(AgentLibrary::new()));
        let mut lib = library.write().await;
        lib.load_defaults();
        tracing::debug!("Loaded {} agents", lib.names().len());
        drop(lib);
        library
    }

    /// How often followers retry promotion to leader.
    const REELECTION_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

    /// How often the LSP supervisor is polled for daemon health.
    const LSP_HEALTH_CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);

    /// Poll interval when waiting for the LSP supervisor OnceCell to be set.
    const LSP_SUPERVISOR_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

    /// Maximum total time to wait for the LSP supervisor after a promotion.
    const LSP_SUPERVISOR_WAIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

    /// Initialize code-context workspace and start indexing at MCP startup.
    ///
    /// Finds the git repository root from the working directory, opens a
    /// CodeContextWorkspace (which triggers file discovery and background indexing),
    /// then runs full tree-sitter indexing with symbols and call edges.
    ///
    /// Uses `std::sync::Once` to ensure this runs exactly once, even when
    /// multiple MCP connections call it concurrently (Claude Code opens ~3).
    ///
    /// # Panics
    ///
    /// Panics if called outside a Tokio runtime (internally uses `tokio::spawn`).
    fn initialize_code_context(work_dir: &std::path::Path) {
        static INIT: std::sync::Once = std::sync::Once::new();
        let work_dir = work_dir.to_path_buf();
        INIT.call_once(move || Self::do_initialize_code_context(&work_dir));
    }

    fn do_initialize_code_context(work_dir: &std::path::Path) {
        let Some(workspace_root) = resolve_workspace_root(work_dir) else {
            return;
        };
        tracing::info!(
            "code-context: initializing for workspace {}",
            workspace_root.display()
        );

        let lsp_handle = Self::spawn_lsp_supervisor(workspace_root.clone());

        let Some(ws) = open_workspace(&workspace_root) else {
            return;
        };

        // If we're already leader, start TS indexing + file watcher immediately.
        // If follower, the re-election loop below will start workers on promotion.
        Self::start_workers_if_leader(&ws, &workspace_root);

        Self::spawn_reelection_loop(Arc::clone(&ws), workspace_root.clone());
        Self::spawn_lsp_health_loop(lsp_handle, ws, workspace_root);
    }

    /// If the workspace is already leader, start TS indexing + watcher workers.
    fn start_workers_if_leader(
        ws: &Arc<std::sync::Mutex<swissarmyhammer_code_context::CodeContextWorkspace>>,
        workspace_root: &std::path::Path,
    ) {
        let ws_lock = ws.lock().expect("workspace mutex poisoned");
        if let Some(shared_db) = ws_lock.shared_db() {
            Self::start_indexing_workers(workspace_root.to_path_buf(), shared_db);
        }
    }

    /// Spawn the LSP supervisor task. Starts every configured LSP daemon,
    /// installs the supervisor into `LSP_SUPERVISOR`, and returns the list of
    /// successfully-running `(server_name, shared_client)` pairs via the
    /// task's join handle.
    fn spawn_lsp_supervisor(
        workspace_root: std::path::PathBuf,
    ) -> tokio::task::JoinHandle<Vec<(String, swissarmyhammer_code_context::SharedLspClient)>> {
        tokio::spawn(async move {
            let mut supervisor = swissarmyhammer_lsp::LspSupervisorManager::new(workspace_root);
            let results = supervisor.start().await;
            let ok_count = results.iter().filter(|r| r.is_ok()).count();
            let err_count = results.iter().filter(|r| r.is_err()).count();
            tracing::info!(
                "code-context: LSP supervisor started — {} servers ok, {} failed",
                ok_count,
                err_count
            );
            for r in &results {
                if let Err(e) = r {
                    tracing::warn!("code-context: LSP start error: {}", e);
                }
            }

            let clients = collect_running_lsp_clients(&supervisor);
            tracing::info!(
                "code-context: {} LSP clients available for indexing: {:?}",
                clients.len(),
                clients.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>()
            );

            use super::tools::code_context::LSP_SUPERVISOR;
            let _ = LSP_SUPERVISOR.set(Arc::new(tokio::sync::Mutex::new(supervisor)));

            clients
        })
    }

    /// Followers poll every 5s trying to become leader. Once promoted (or if
    /// already leader), the loop exits permanently.
    ///
    /// One-shot promotion: if leadership is lost after promotion there is no
    /// automatic recovery, but the LeaderGuard is held for the process lifetime
    /// via the Arc kept by `spawn_lsp_health_loop`.
    fn spawn_reelection_loop(
        ws: Arc<std::sync::Mutex<swissarmyhammer_code_context::CodeContextWorkspace>>,
        workspace_root: std::path::PathBuf,
    ) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Self::REELECTION_POLL_INTERVAL).await;
                let promoted = try_promote_workspace(&ws);
                if handle_promotion_result(promoted, &workspace_root) {
                    break;
                }
            }
        });
    }

    /// Waits for the LSP supervisor to finish, starts LSP indexing workers if
    /// we're the leader, then runs the 60s LSP health-check loop forever.
    fn spawn_lsp_health_loop(
        lsp_handle: tokio::task::JoinHandle<
            Vec<(String, swissarmyhammer_code_context::SharedLspClient)>,
        >,
        ws: Arc<std::sync::Mutex<swissarmyhammer_code_context::CodeContextWorkspace>>,
        workspace_root: std::path::PathBuf,
    ) {
        tokio::spawn(async move {
            let clients = lsp_handle.await.unwrap_or_default();
            if clients.is_empty() {
                tracing::info!("code-context: no LSP clients available, skipping LSP indexing");
            } else {
                start_lsp_workers_if_leader(&ws, &workspace_root, &clients, "");
            }
            run_lsp_health_check_loop().await;
        });
    }

    /// Spawn the tree-sitter indexing task and the file-watcher task.
    ///
    /// `log_suffix` is appended to the "starting" log message so callers can
    /// distinguish normal startup from post-promotion startup (e.g. pass
    /// `" (after promotion)"` or `""`).
    fn spawn_ts_and_watcher_workers(
        workspace_root: std::path::PathBuf,
        shared_db: swissarmyhammer_code_context::SharedDb,
        log_suffix: &'static str,
    ) {
        // Start tree-sitter indexing
        let ts_root = workspace_root.clone();
        let ts_db = std::sync::Arc::clone(&shared_db);
        tokio::spawn(async move {
            use super::tools::code_context::index_discovered_files_async;
            tracing::info!(
                "code-context: starting tree-sitter indexing for {}{}",
                ts_root.display(),
                log_suffix,
            );
            index_discovered_files_async(&ts_root, ts_db).await;
            tracing::info!(
                "code-context: tree-sitter indexing complete for {}",
                ts_root.display()
            );
        });

        // Start file watcher
        let watcher_root = workspace_root;
        let watcher_db = std::sync::Arc::clone(&shared_db);
        tokio::spawn(async move {
            use super::tools::code_context::watcher::start_code_context_watcher;
            let _watcher_handle = start_code_context_watcher(watcher_root, watcher_db);
            std::future::pending::<()>().await;
        });
    }

    /// Start tree-sitter indexing and file watcher workers with an existing shared DB.
    /// LSP workers are started separately by the LSP task.
    fn start_indexing_workers(
        workspace_root: std::path::PathBuf,
        shared_db: swissarmyhammer_code_context::SharedDb,
    ) {
        Self::spawn_ts_and_watcher_workers(workspace_root, shared_db, "");
    }

    /// Start indexing workers after a follower-to-leader promotion.
    /// LSP workers are started separately once the LSP supervisor is ready.
    fn start_indexing_workers_after_promotion(
        workspace_root: std::path::PathBuf,
        shared_db: swissarmyhammer_code_context::SharedDb,
    ) {
        Self::spawn_ts_and_watcher_workers(
            workspace_root.clone(),
            std::sync::Arc::clone(&shared_db),
            " (after promotion)",
        );

        // LSP workers: wait for the supervisor to become available, then start them.
        let lsp_db = std::sync::Arc::clone(&shared_db);
        tokio::spawn(async move {
            let Some(sup) = wait_for_lsp_supervisor(&workspace_root).await else {
                return;
            };
            let clients = collect_running_lsp_clients(&*sup.lock().await);
            spawn_lsp_workers_for_clients(&workspace_root, &lsp_db, &clients, " (after promotion)");
        });
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

    /// Resolve agent configuration, with optional model override.
    ///
    /// If a model override is provided, that model is loaded directly.
    /// Otherwise, resolves the configured model from the project config,
    /// falling back to claude-code as default.
    ///
    /// # Arguments
    ///
    /// * `model_override` - Optional model name to override configured model
    ///
    /// # Returns
    ///
    /// * `Result<Arc<swissarmyhammer_config::model::ModelConfig>>` - Agent configuration
    fn resolve_agent_config(
        model_override: Option<String>,
    ) -> Result<Arc<swissarmyhammer_config::model::ModelConfig>> {
        if let Some(override_model_name) = model_override {
            tracing::info!("Using model override '{}'", override_model_name);

            let info = ModelManager::find_agent_by_name(&override_model_name).map_err(|e| {
                SwissArmyHammerError::Other {
                    message: format!("Invalid model override '{}': {}", override_model_name, e),
                }
            })?;

            let config =
                parse_model_config(&info.content).map_err(|e| SwissArmyHammerError::Other {
                    message: format!("Invalid model override '{}': {}", override_model_name, e),
                })?;

            Ok(Arc::new(config))
        } else {
            match ModelManager::resolve_agent_config(
                &swissarmyhammer_config::model::ModelPaths::sah(),
            ) {
                Ok(config) => {
                    tracing::debug!("Resolved model: {:?}", config.executor_type());
                    Ok(Arc::new(config))
                }
                Err(e) => {
                    tracing::warn!("Failed to resolve model config: {}, using default", e);
                    // Fall back to loading from template context
                    let template_context = TemplateContext::load_for_cli().map_err(|e| {
                        SwissArmyHammerError::Other {
                            message: format!("Failed to load configuration: {}", e),
                        }
                    })?;
                    Ok(Arc::new(template_context.get_agent_config(None)))
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
    /// * `working_dir` - Working directory for tool operations
    /// * `skill_library` - Shared skill library
    /// * `agent_mode` - Whether to register agent tools
    ///
    /// # Returns
    ///
    /// * `(Arc<RwLock<ToolRegistry>>, Arc<ToolContext>)` - Registry and context
    #[allow(clippy::too_many_arguments)]
    async fn create_tool_context_and_registry(
        tool_handlers: ToolHandlers,
        git_ops_arc: Arc<Mutex<Option<GitOperations>>>,
        agent_config: Arc<swissarmyhammer_config::model::ModelConfig>,
        working_dir: Option<PathBuf>,
        skill_library: Arc<RwLock<SkillLibrary>>,
        agent_library: Arc<RwLock<AgentLibrary>>,
        prompt_library: Arc<RwLock<PromptLibrary>>,
        agent_mode: bool,
    ) -> (Arc<RwLock<ToolRegistry>>, Arc<ToolContext>) {
        let mut tool_registry = ToolRegistry::new();
        Self::register_all_tools(
            &mut tool_registry,
            skill_library,
            agent_library,
            prompt_library.clone(),
            agent_mode,
        )
        .await;

        let mut tool_context = ToolContext::new(Arc::new(tool_handlers), git_ops_arc, agent_config);
        tool_context.working_dir = working_dir;

        let tool_registry_arc = Arc::new(RwLock::new(tool_registry));
        let tool_context = Arc::new(
            tool_context
                .with_prompt_library(prompt_library)
                .with_tool_registry(tool_registry_arc.clone()),
        );

        (tool_registry_arc, tool_context)
    }

    /// Register all available tools in the tool registry.
    ///
    /// All tools are registered unconditionally. When `agent_mode` is false,
    /// tools that implement `AgentTool` (and return `is_agent_tool() == true`)
    /// are removed via `remove_agent_tools()`. This keeps the filtering
    /// trait-driven rather than hardcoded in if statements.
    async fn register_all_tools(
        tool_registry: &mut ToolRegistry,
        skill_library: Arc<RwLock<SkillLibrary>>,
        agent_library: Arc<RwLock<AgentLibrary>>,
        prompt_library: Arc<RwLock<PromptLibrary>>,
        agent_mode: bool,
    ) {
        register_git_tools(tool_registry);
        register_kanban_tools(tool_registry);
        register_questions_tools(tool_registry);
        register_web_tools(tool_registry);
        register_code_context_tools(tool_registry);
        register_shell_tools(tool_registry);
        register_ralph_tools(tool_registry);
        register_agent_tools(tool_registry, agent_library, prompt_library.clone());
        register_file_tools(tool_registry).await;
        register_skill_tools(tool_registry, skill_library, prompt_library);

        if !agent_mode {
            tool_registry.remove_agent_tools();
            tracing::debug!("Removed agent-only tools (agent_mode=false)");
        }

        // Apply tool enable/disable config from tools.yaml (global + project layers)
        let tool_config = super::tool_config::load_merged_tool_config();
        let disabled = tool_config.disabled_tools();
        if !disabled.is_empty() {
            super::tool_config::apply_tool_config(tool_registry, &tool_config);
            tracing::info!("Applied tool config: {} tools disabled", disabled.len());
        }

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

    /// Create a validator-only McpServer clone with a filtered tool registry.
    ///
    /// The returned server shares all state (ToolContext, prompt library, etc.)
    /// but has a separate ToolRegistry containing only validator tools
    /// (code_context + files read-only).
    pub fn create_validator_server(&self) -> McpServer {
        // Build a filtered registry with only validator tools
        let mut validator_registry = ToolRegistry::new();

        // Register the two validator tools directly
        use super::tools::code_context::CodeContextTool;
        use super::tools::files::FilesTool;

        validator_registry.register(CodeContextTool::new());
        validator_registry.register(FilesTool::read_only());

        let validator_registry_arc = Arc::new(RwLock::new(validator_registry));

        tracing::debug!("Created validator tool registry with 2 validator tools");

        // Clone the tool context but replace its registry with the validator-only one.
        // This prevents validator tools from calling non-validator tools via context.call_tool().
        let mut validator_context = (*self.tool_context).clone();
        validator_context.tool_registry = Some(validator_registry_arc.clone());
        let validator_context = Arc::new(validator_context);

        McpServer {
            library: self.library.clone(),
            file_watcher: self.file_watcher.clone(),
            tool_registry: validator_registry_arc,
            tool_context: validator_context,
            skill_library: self.skill_library.clone(),
            agent_library: self.agent_library.clone(),
            work_dir: self.work_dir.clone(),
            tool_config_watcher: self.tool_config_watcher.clone(),
        }
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

    /// List all available prompts, excluding hidden prompts and partial templates.
    ///
    /// Hidden prompts and partial templates are filtered out as they are meant
    /// for internal use and should not be exposed via the MCP interface.
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
            .filter(|p| {
                let meta = serde_json::to_value(&p.metadata).ok();
                is_prompt_visible(&p.name, p.description.as_deref(), meta.as_ref())
                    && !p.is_partial_template()
            })
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

    /// Get the tool registry for direct access.
    ///
    /// This provides shared access to the tool registry for CLI and other consumers.
    ///
    /// # Returns
    ///
    /// * `Arc<RwLock<ToolRegistry>>` - Shared reference to the tool registry
    pub fn get_tool_registry(&self) -> Arc<RwLock<ToolRegistry>> {
        self.tool_registry.clone()
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

        // Check if this prompt is visible (not hidden or a partial)
        let meta = serde_json::to_value(&prompt.metadata).ok();
        if !is_prompt_visible(&prompt.name, prompt.description.as_deref(), meta.as_ref())
            || prompt.is_partial_template()
        {
            return Err(SwissArmyHammerError::Other {
                message: format!(
                    "Cannot access hidden prompt '{name}' via MCP. Hidden prompts are for internal use only."
                ),
            });
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

    /// Compute a content signature for a set of prompts.
    ///
    /// This creates a deterministic snapshot of prompt content by serializing
    /// all relevant fields (excluding metadata like source_path) to JSON.
    /// Used to detect actual content changes vs. just file modification events.
    ///
    /// # Arguments
    ///
    /// * `prompts` - The prompts to compute signature for
    ///
    /// # Returns
    ///
    /// A BTreeMap where keys are prompt names and values are JSON representations
    /// of the prompt content (excluding source_path)
    fn compute_prompt_signature(
        prompts: &[swissarmyhammer_prompts::Prompt],
    ) -> BTreeMap<String, String> {
        let mut signature = BTreeMap::new();
        for prompt in prompts {
            // Create a simplified representation without source_path
            let content = serde_json::json!({
                "name": prompt.name,
                "description": prompt.description,
                "category": prompt.category,
                "tags": prompt.tags,
                "template": prompt.template,
                "parameters": prompt.parameters,
            });
            // Use compact JSON representation for comparison
            if let Ok(json_str) = serde_json::to_string(&content) {
                signature.insert(prompt.name.clone(), json_str);
            }
        }
        signature
    }

    /// Reload prompts from disk with retry logic.
    ///
    /// This method reloads all prompts from the file system and updates
    /// the internal library. It includes retry logic for transient errors.
    ///
    /// # Returns
    ///
    /// * `Result<bool>` - Ok(true) if prompts changed, Ok(false) if no changes, error otherwise
    ///
    /// # Errors
    ///
    /// Returns error if prompt directories cannot be read or prompts cannot be loaded
    pub async fn reload_prompts(&self) -> Result<bool> {
        self.reload_prompts_with_retry().await
    }

    /// Reload prompts with retry logic for transient file system errors
    async fn reload_prompts_with_retry(&self) -> Result<bool> {
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
    ///
    /// # Returns
    ///
    /// * `Result<bool>` - Ok(true) if prompt content changed, Ok(false) if no changes
    async fn reload_prompts_internal(&self) -> Result<bool> {
        let mut library = self.library.write().await;
        let mut resolver = PromptResolver::new();

        // Capture "before" state
        let before_prompts = library.list().unwrap_or_default();
        let before_count = before_prompts.len();
        let before_signature = Self::compute_prompt_signature(&before_prompts);

        // Clear existing prompts and reload
        *library = PromptLibrary::new();
        resolver
            .load_all_prompts(&mut library)
            .map_err(|e| SwissArmyHammerError::Other {
                message: e.to_string(),
            })?;

        // Capture "after" state
        let after_prompts = library.list().map_err(|e| SwissArmyHammerError::Other {
            message: e.to_string(),
        })?;
        let after_count = after_prompts.len();
        let after_signature = Self::compute_prompt_signature(&after_prompts);

        // Compare signatures to detect actual content changes
        let has_changes = before_signature != after_signature;

        tracing::info!(
            "🔄 Reloaded prompts: {} → {} prompts{}",
            before_count,
            after_count,
            if has_changes {
                " (content changed)"
            } else {
                " (no content changes)"
            }
        );

        Ok(has_changes)
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

    /// Spawn the prompt-directory file watcher on a background task so the
    /// MCP initialize handshake can return without waiting for FSEvents
    /// debouncer construction (~400ms on macOS). Failures are logged; they
    /// never fail the handshake.
    fn spawn_background_file_watcher(&self, peer: rmcp::Peer<RoleServer>) {
        let server = self.clone();
        tokio::spawn(async move {
            match server.start_file_watching(peer).await {
                Ok(_) => tracing::info!("🔍 File watching started for MCP client"),
                Err(e) => {
                    tracing::error!("✗ Failed to start file watching for MCP client: {}", e)
                }
            }
        });
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
        let meta = serde_json::to_value(&prompt.metadata).ok();
        if !is_prompt_visible(&prompt.name, prompt.description.as_deref(), meta.as_ref())
            || prompt.is_partial_template()
        {
            return Err(McpError::invalid_request(
                format!(
                    "Cannot access hidden prompt '{}' via MCP. Hidden prompts are for internal use only.",
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

    /// Ensure an agent actor exists for the connecting MCP client.
    ///
    /// Slugifies the client name as the actor ID, derives a deterministic color,
    /// and generates a geometric SVG avatar. Idempotent via `ensure: true`.
    async fn ensure_agent_actor(&self, client_name: &str) {
        use swissarmyhammer_kanban::actor::AddActor;
        use swissarmyhammer_kanban::{Execute, KanbanContext};

        let working_dir = self
            .tool_context
            .working_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("."));
        let kanban_dir = working_dir.join(".kanban");

        if !kanban_dir.is_dir() {
            tracing::debug!("no .kanban directory, skipping agent actor creation");
            return;
        }

        let actor_id = slugify(client_name);
        let color = agent_deterministic_color(&actor_id);

        // No stored avatar — frontend renders initials as fallback
        let ctx = KanbanContext::new(kanban_dir);
        let cmd = AddActor::new(actor_id.as_str(), client_name)
            .with_ensure()
            .with_color(&color);

        match cmd.execute(&ctx).await.into_result() {
            Ok(result) => {
                let created = result["created"].as_bool().unwrap_or(false);
                if created {
                    tracing::info!(id = %actor_id, name = %client_name, "created MCP agent actor");
                } else {
                    tracing::debug!(id = %actor_id, "MCP agent actor already exists");
                }
                // Store the actor_id so tool calls can auto-inject it
                *self.tool_context.session_actor.write().await = Some(actor_id);
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to ensure MCP agent actor");
            }
        }
    }
}

/// Slugify a name into a valid actor ID (lowercase, hyphens for spaces/special chars).
fn slugify(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Curated palette for agent actors (cooler tones to distinguish from human actors).
const AGENT_COLORS: &[&str] = &[
    "5a67d8", "3182ce", "319795", "2f855a", "805ad5", "6b46c1", "2b6cb0", "2c7a7b", "4c51bf",
    "38a169",
];

/// Derive a deterministic hex color for an agent actor.
fn agent_deterministic_color(id: &str) -> String {
    let hash: u64 = id
        .bytes()
        .fold(5381u64, |h, b| h.wrapping_mul(33).wrapping_add(b as u64));
    AGENT_COLORS[(hash as usize) % AGENT_COLORS.len()].to_string()
}

/// Walk up from `work_dir` to find the enclosing git repository root. Returns
/// `None` (and logs) if we're not inside a repo — callers use that signal to
/// skip code-context initialization.
fn resolve_workspace_root(work_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    match find_git_repository_root_from(work_dir) {
        Some(root) => Some(root),
        None => {
            tracing::info!(
                "code-context: no git repository found from {}, skipping initialization",
                work_dir.display()
            );
            None
        }
    }
}

/// Open the code-context workspace and wrap it in an `Arc<Mutex>` for sharing
/// across the spawned background tasks. Logs and returns `None` on failure.
fn open_workspace(
    workspace_root: &std::path::Path,
) -> Option<Arc<std::sync::Mutex<swissarmyhammer_code_context::CodeContextWorkspace>>> {
    tracing::info!(
        "code-context: opening workspace for {}",
        workspace_root.display()
    );
    match swissarmyhammer_code_context::CodeContextWorkspace::open(workspace_root) {
        Ok(ws) => {
            tracing::info!(
                "code-context: workspace opened as {}",
                if ws.is_leader() { "leader" } else { "follower" }
            );
            Some(Arc::new(std::sync::Mutex::new(ws)))
        }
        Err(e) => {
            tracing::warn!("code-context: failed to open workspace: {}", e);
            None
        }
    }
}

/// Collect every running daemon's `(server_name, shared_client)` pair from
/// the supervisor. Daemons that are not in the `Running` state are skipped.
fn collect_running_lsp_clients(
    supervisor: &swissarmyhammer_lsp::LspSupervisorManager,
) -> Vec<(String, swissarmyhammer_code_context::SharedLspClient)> {
    supervisor
        .daemon_names()
        .into_iter()
        .filter_map(|name| lsp_client_if_running(supervisor, name))
        .collect()
}

/// Return `(name, client)` for the daemon if it's in the `Running` state.
fn lsp_client_if_running(
    supervisor: &swissarmyhammer_lsp::LspSupervisorManager,
    name: String,
) -> Option<(String, swissarmyhammer_code_context::SharedLspClient)> {
    let daemon = supervisor.get_daemon(&name)?;
    match daemon.state() {
        swissarmyhammer_lsp::LspDaemonState::Running { .. } => Some((name, daemon.shared_client())),
        _ => None,
    }
}

/// Spawn an `spawn_lsp_indexing_worker` per running LSP client. `log_suffix`
/// is appended to the startup log so callers can distinguish fresh startup
/// from post-promotion startup (e.g. `" (after promotion)"` or `""`).
fn spawn_lsp_workers_for_clients(
    workspace_root: &std::path::Path,
    shared_db: &swissarmyhammer_code_context::SharedDb,
    clients: &[(String, swissarmyhammer_code_context::SharedLspClient)],
    log_suffix: &str,
) {
    if clients.is_empty() {
        return;
    }
    use swissarmyhammer_code_context::{
        new_shutdown_flag, spawn_lsp_indexing_worker, LspWorkerConfig,
    };
    for (server_name, shared_client) in clients {
        let worker_db = std::sync::Arc::clone(shared_db);
        spawn_lsp_indexing_worker(
            workspace_root.to_path_buf(),
            worker_db,
            std::sync::Arc::clone(shared_client),
            LspWorkerConfig::default(),
            server_name.clone(),
            new_shutdown_flag(),
        );
        tracing::info!(
            "code-context: LSP indexing worker started for {}{} (server: {})",
            workspace_root.display(),
            log_suffix,
            server_name,
        );
    }
}

/// Try to promote the workspace to leader. Returns `Ok(Some(db))` on success,
/// `Ok(None)` if the lock is still held elsewhere, `Err` on a real failure.
/// Returns `Ok(None)` (and signals "stop looping") via the caller if the
/// workspace is already the leader.
enum PromotionState {
    AlreadyLeader,
    Outcome(
        std::result::Result<
            Option<swissarmyhammer_code_context::SharedDb>,
            swissarmyhammer_code_context::CodeContextError,
        >,
    ),
}

fn try_promote_workspace(
    ws: &Arc<std::sync::Mutex<swissarmyhammer_code_context::CodeContextWorkspace>>,
) -> PromotionState {
    let mut ws_lock = ws.lock().expect("workspace mutex poisoned");
    if ws_lock.is_leader() {
        return PromotionState::AlreadyLeader;
    }
    PromotionState::Outcome(ws_lock.try_promote())
}

/// Handle the outcome of `try_promote_workspace`. Returns `true` when the
/// re-election loop should stop (either because we're already leader or the
/// promotion succeeded).
fn handle_promotion_result(state: PromotionState, workspace_root: &std::path::Path) -> bool {
    match state {
        PromotionState::AlreadyLeader => true,
        PromotionState::Outcome(Ok(Some(shared_db))) => {
            tracing::info!(
                "code-context: promoted to leader for {}, starting indexing workers",
                workspace_root.display()
            );
            McpServer::start_indexing_workers_after_promotion(
                workspace_root.to_path_buf(),
                shared_db,
            );
            true
        }
        PromotionState::Outcome(Ok(None)) => false,
        PromotionState::Outcome(Err(e)) => {
            tracing::warn!("code-context: re-election error: {}", e);
            false
        }
    }
}

/// If the workspace is currently leader, spawn LSP indexing workers for the
/// supplied clients. No-op if the workspace has no shared DB.
fn start_lsp_workers_if_leader(
    ws: &Arc<std::sync::Mutex<swissarmyhammer_code_context::CodeContextWorkspace>>,
    workspace_root: &std::path::Path,
    clients: &[(String, swissarmyhammer_code_context::SharedLspClient)],
    log_suffix: &str,
) {
    let ws_lock = ws.lock().expect("workspace mutex poisoned");
    let Some(shared_db) = ws_lock.shared_db() else {
        return;
    };
    spawn_lsp_workers_for_clients(workspace_root, &shared_db, clients, log_suffix);
}

/// Run the LSP supervisor health-check loop forever, polling on the
/// `McpServer::LSP_HEALTH_CHECK_INTERVAL` cadence.
async fn run_lsp_health_check_loop() -> ! {
    loop {
        tokio::time::sleep(McpServer::LSP_HEALTH_CHECK_INTERVAL).await;
        use super::tools::code_context::LSP_SUPERVISOR;
        if let Some(sup) = LSP_SUPERVISOR.get() {
            sup.lock().await.health_check_all().await;
        }
    }
}

/// Wait for the `LSP_SUPERVISOR` OnceCell to be initialized, polling on the
/// `McpServer::LSP_SUPERVISOR_POLL_INTERVAL` cadence up to
/// `McpServer::LSP_SUPERVISOR_WAIT_TIMEOUT`. Returns `None` and logs a
/// warning if it never appears.
async fn wait_for_lsp_supervisor(
    workspace_root: &std::path::Path,
) -> Option<Arc<tokio::sync::Mutex<swissarmyhammer_lsp::LspSupervisorManager>>> {
    use super::tools::code_context::LSP_SUPERVISOR;
    let poll = McpServer::LSP_SUPERVISOR_POLL_INTERVAL;
    let max_attempts =
        (McpServer::LSP_SUPERVISOR_WAIT_TIMEOUT.as_millis() / poll.as_millis().max(1)) as u32;
    for _ in 0..max_attempts {
        if let Some(s) = LSP_SUPERVISOR.get() {
            return Some(Arc::clone(s));
        }
        tokio::time::sleep(poll).await;
    }
    tracing::warn!(
        "code-context: LSP supervisor not available after {:?} post-promotion for {}; \
         LSP indexing will not run until next restart",
        McpServer::LSP_SUPERVISOR_WAIT_TIMEOUT,
        workspace_root.display(),
    );
    None
}

impl ServerHandler for McpServer {
    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<InitializeResult, McpError> {
        tracing::info!(
            "🚀 MCP client connecting: {} v{}",
            request.client_info.name,
            request.client_info.version
        );

        self.spawn_background_file_watcher(context.peer);

        // Auto-create agent actor for the connecting MCP client
        self.ensure_agent_actor(&request.client_info.name).await;

        // Start code-context background work (LSP, indexing, file watcher)
        // only when an MCP client actually connects — not in the constructor.
        if let Some(ref work_dir) = self.work_dir {
            Self::initialize_code_context(work_dir);
        }

        // Run Initializable::start() on all registered tools
        {
            let registry = self.tool_registry.read().await;
            for tool in registry.iter_tools() {
                let results = tool.start();
                for r in &results {
                    if r.status == swissarmyhammer_common::lifecycle::InitStatus::Error {
                        tracing::warn!("Tool start error: {} — {}", r.name, r.message);
                    }
                }
            }
        }

        Ok(InitializeResult::new(create_server_capabilities())
            .with_server_info(create_server_implementation())
            .with_instructions(build_instructions_with_health(self.work_dir.as_deref())))
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListPromptsResult, McpError> {
        let library = self.library.read().await;
        match library.list() {
            Ok(prompts) => {
                let prompt_list: Vec<Prompt> = prompts
                    .iter()
                    .filter(|p| {
                        let meta = serde_json::to_value(&p.metadata).ok();
                        is_prompt_visible(&p.name, p.description.as_deref(), meta.as_ref())
                            && !p.is_partial_template()
                    })
                    .map(|p| {
                        // Convert SwissArmyHammer prompt parameters to MCP PromptArguments
                        let arguments = if p.parameters.is_empty() {
                            None
                        } else {
                            Some(
                                p.parameters
                                    .iter()
                                    .map(|param| {
                                        PromptArgument::new(param.name.clone())
                                            .with_description(param.description.clone())
                                            .with_required(param.required)
                                    })
                                    .collect(),
                            )
                        };

                        Prompt::new(p.name.clone(), p.description.clone(), arguments)
                            .with_title(p.name.clone())
                    })
                    .collect();

                Ok(ListPromptsResult {
                    prompts: prompt_list,
                    next_cursor: None,
                    meta: None,
                })
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
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

        let mut result = GetPromptResult::new(vec![PromptMessage::new(
            PromptMessageRole::User,
            PromptMessageContent::text(content),
        )]);
        if let Some(desc) = prompt.description.clone() {
            result = result.with_description(desc);
        }
        Ok(result)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        // Hot reload: check if tools.yaml changed since last call.
        // Acquire the write lock once and read from it directly — avoids a
        // second lock acquisition for the list_tools() call below.
        let mut registry = self.tool_registry.write().await;
        {
            let mut watcher = self.tool_config_watcher.lock().await;
            watcher.check_and_reload(&mut registry);
        }
        Ok(ListToolsResult {
            tools: registry.list_tools(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        use tracing::Instrument;

        let tool_name = request.name.to_string();
        let arg_count = request.arguments.as_ref().map_or(0, |a| a.len());

        let span = tracing::info_span!(
            "tool_call",
            tool = %tool_name,
            args = arg_count,
            caller = "mcp",
            status = tracing::field::Empty,
        );

        async {
            let registry = self.tool_registry.read().await;
            let tool = registry.get_tool(&request.name).ok_or_else(|| {
                tracing::error!(tool = %request.name, "unknown tool requested");
                McpError::invalid_request(format!("Unknown tool: {}", request.name), None)
            })?;

            let tool_context_with_peer = self.prepare_tool_context(context.peer.clone());
            let arguments = request.arguments.unwrap_or_default();

            let start = std::time::Instant::now();
            let result = tool.execute(arguments, &tool_context_with_peer).await;
            let elapsed = start.elapsed();

            let is_error = match &result {
                Ok(r) => r.is_error.unwrap_or(false),
                Err(_) => true,
            };
            tracing::Span::current().record("status", if is_error { "error" } else { "ok" });

            tracing::info!(
                duration_ms = elapsed.as_millis(),
                error = is_error,
                "tool_call complete"
            );

            result
        }
        .instrument(span)
        .await
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(create_server_capabilities())
            .with_server_info(create_server_implementation())
            .with_instructions(build_instructions_with_health(self.work_dir.as_deref()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Claude Code"), "claude-code");
        assert_eq!(slugify("my_agent"), "my-agent");
        assert_eq!(slugify("  spaces  "), "spaces");
        assert_eq!(slugify("UPPER"), "upper");
        assert_eq!(slugify("a--b"), "a-b");
    }

    #[test]
    fn test_agent_deterministic_color_stable() {
        let c1 = agent_deterministic_color("claude-code");
        let c2 = agent_deterministic_color("claude-code");
        assert_eq!(c1, c2);
        assert_eq!(c1.len(), 6);
    }

    #[test]
    fn test_build_instructions_no_work_dir() {
        let result = build_instructions_with_health(None);
        assert_eq!(result, SERVER_INSTRUCTIONS);
    }

    #[test]
    fn test_build_instructions_no_projects() {
        let tmp = tempfile::tempdir().unwrap();
        let result = build_instructions_with_health(Some(tmp.path()));
        assert_eq!(result, SERVER_INSTRUCTIONS);
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_validator_server_has_only_two_tools() {
        let server = McpServer::new(PromptLibrary::default()).await.unwrap();

        // Full server should have many tools
        let full_tools = server.tool_registry.read().await;
        let full_count = full_tools.len();
        assert!(
            full_count > 2,
            "Full server should have more than 2 tools, got {}",
            full_count
        );
        drop(full_tools);

        // Validator server should have exactly 2 tools
        let validator = server.create_validator_server();
        let validator_tools = validator.tool_registry.read().await;
        assert_eq!(
            validator_tools.len(),
            2,
            "Validator should have exactly 2 tools"
        );

        // Verify the right tools are present
        assert!(
            validator_tools.get_tool("files").is_some(),
            "Validator should have 'files' tool"
        );
        assert!(
            validator_tools.get_tool("code_context").is_some(),
            "Validator should have 'code_context' tool"
        );

        // Verify disallowed tools are absent
        assert!(
            validator_tools.get_tool("kanban").is_none(),
            "Validator should NOT have 'kanban' tool"
        );
        assert!(
            validator_tools.get_tool("shell").is_none(),
            "Validator should NOT have 'shell' tool"
        );
        assert!(
            validator_tools.get_tool("git").is_none(),
            "Validator should NOT have 'git' tool"
        );
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_validator_context_registry_is_isolated() {
        let server = McpServer::new(PromptLibrary::default()).await.unwrap();
        let validator = server.create_validator_server();

        // The validator's tool_context should have its own filtered registry
        let validator_ctx_registry = validator
            .tool_context
            .tool_registry
            .as_ref()
            .expect("Validator context should have a tool_registry");
        let registry = validator_ctx_registry.read().await;

        // call_tool on the validator context should NOT find non-validator tools
        assert!(
            registry.get_tool("kanban").is_none(),
            "Validator context registry should not contain 'kanban'"
        );
        assert!(
            registry.get_tool("files").is_some(),
            "Validator context registry should contain 'files'"
        );
        assert_eq!(
            registry.len(),
            2,
            "Validator context registry should have exactly 2 tools"
        );
    }

    #[test]
    fn test_build_instructions_with_missing_lsp() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\n",
        )
        .unwrap();
        let result = build_instructions_with_health(Some(tmp.path()));
        // Result always starts with the base instructions
        assert!(result.starts_with(SERVER_INSTRUCTIONS));
        // If rust-analyzer is not installed, we should see the setupStatus block
        if result.len() > SERVER_INSTRUCTIONS.len() {
            assert!(result.contains("setupStatus:"));
            assert!(result.contains("NOT INSTALLED"));
        }
    }

    // ---------------------------------------------------------------
    // new_with_work_dir() tests
    // ---------------------------------------------------------------

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_new_with_work_dir_creates_server() {
        let tmp = tempfile::tempdir().unwrap();
        let server = McpServer::new_with_work_dir(
            PromptLibrary::default(),
            tmp.path().to_path_buf(),
            None,
            false,
        )
        .await
        .unwrap();

        // The server should store the working directory
        assert_eq!(server.work_dir, Some(tmp.path().to_path_buf()));
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_new_with_work_dir_agent_mode_registers_agent_tools() {
        let tmp = tempfile::tempdir().unwrap();
        let server_no_agent = McpServer::new_with_work_dir(
            PromptLibrary::default(),
            tmp.path().to_path_buf(),
            None,
            false,
        )
        .await
        .unwrap();

        let server_agent = McpServer::new_with_work_dir(
            PromptLibrary::default(),
            tmp.path().to_path_buf(),
            None,
            true,
        )
        .await
        .unwrap();

        let tools_no_agent = server_no_agent.list_tools().await;
        let tools_agent = server_agent.list_tools().await;

        // Agent mode should have at least as many tools as non-agent mode
        // (agent-only tools are added, not subtracted)
        assert!(
            tools_agent.len() >= tools_no_agent.len(),
            "Agent mode should have >= tools: agent={}, non-agent={}",
            tools_agent.len(),
            tools_no_agent.len()
        );
    }

    // ---------------------------------------------------------------
    // set_server_port() tests
    // ---------------------------------------------------------------

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_set_server_port() {
        let server = McpServer::new(PromptLibrary::default()).await.unwrap();

        // Initially, port should be None
        let port = server.tool_context.mcp_server_port.read().await;
        assert_eq!(*port, None);
        drop(port);

        // Set port
        server.set_server_port(8080).await;

        let port = server.tool_context.mcp_server_port.read().await;
        assert_eq!(*port, Some(8080));
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_set_server_port_updates_existing() {
        let server = McpServer::new(PromptLibrary::default()).await.unwrap();

        server.set_server_port(8080).await;
        server.set_server_port(9090).await;

        let port = server.tool_context.mcp_server_port.read().await;
        assert_eq!(*port, Some(9090));
    }

    // ---------------------------------------------------------------
    // initialize() tests
    // ---------------------------------------------------------------

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_initialize_loads_prompts() {
        let server = McpServer::new(PromptLibrary::default()).await.unwrap();

        // Initialize should succeed without errors
        server.initialize().await.unwrap();

        // After initialization, prompts should be loaded from builtin sources
        let prompts = server.list_prompts().await.unwrap();
        // There should be at least some builtin prompts
        assert!(
            !prompts.is_empty(),
            "After initialize(), the server should have loaded prompts"
        );
    }

    // ---------------------------------------------------------------
    // list_prompts() tests
    // ---------------------------------------------------------------

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_list_prompts_filters_hidden() {
        use swissarmyhammer_prompts::Prompt;

        let mut library = PromptLibrary::new();
        // Add a visible prompt
        library
            .add(Prompt::new("visible-prompt", "Hello world"))
            .unwrap();
        // Add a hidden prompt (metadata hidden: true)
        let mut hidden = Prompt::new("hidden-prompt", "Secret stuff");
        hidden
            .metadata
            .insert("hidden".to_string(), serde_json::Value::Bool(true));
        library.add(hidden).unwrap();

        let server = McpServer::new(library).await.unwrap();
        let prompts = server.list_prompts().await.unwrap();

        assert!(
            prompts.contains(&"visible-prompt".to_string()),
            "Visible prompt should appear in list"
        );
        assert!(
            !prompts.contains(&"hidden-prompt".to_string()),
            "Hidden prompt should not appear in list"
        );
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_list_prompts_filters_partials() {
        use swissarmyhammer_prompts::Prompt;

        let mut library = PromptLibrary::new();
        library.add(Prompt::new("normal", "Hello world")).unwrap();
        library
            .add(Prompt::new(
                "partial",
                "{% partial %}\nSome partial content",
            ))
            .unwrap();

        let server = McpServer::new(library).await.unwrap();
        let prompts = server.list_prompts().await.unwrap();

        assert!(prompts.contains(&"normal".to_string()));
        assert!(
            !prompts.contains(&"partial".to_string()),
            "Partial templates should not appear in list"
        );
    }

    // ---------------------------------------------------------------
    // list_tools() tests
    // ---------------------------------------------------------------

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_list_tools_returns_registered_tools() {
        let server = McpServer::new(PromptLibrary::default()).await.unwrap();
        let tools = server.list_tools().await;

        // Should have multiple tools registered
        assert!(
            tools.len() > 3,
            "Should have many tools registered, got {}",
            tools.len()
        );

        // Verify some core non-agent tools are present (agent tools like
        // "files" and "web" are removed when agent_mode=false)
        let tool_names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
        assert!(
            tool_names.contains(&"shell".to_string()),
            "shell tool should be registered, got: {:?}",
            tool_names
        );
        assert!(
            tool_names.contains(&"kanban".to_string()),
            "kanban tool should be registered, got: {:?}",
            tool_names
        );
    }

    // ---------------------------------------------------------------
    // execute_tool() tests
    // ---------------------------------------------------------------

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_execute_tool_unknown_tool_returns_error() {
        let server = McpServer::new(PromptLibrary::default()).await.unwrap();

        let result = server
            .execute_tool("nonexistent_tool", serde_json::json!({}))
            .await;

        assert!(result.is_err(), "Unknown tool should return an error");
        let err = result.unwrap_err();
        let msg = format!("{:?}", err);
        assert!(
            msg.contains("Unknown tool"),
            "Error should mention unknown tool: {}",
            msg
        );
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_execute_tool_with_non_object_args() {
        let server = McpServer::new(PromptLibrary::default()).await.unwrap();

        // Passing a non-object (e.g. string) should use an empty map, not crash
        let result = server
            .execute_tool("files", serde_json::json!("not an object"))
            .await;

        // The tool should execute (possibly with an error result, but not a panic)
        // It's OK if it returns Err or Ok with is_error=true; the point is it doesn't crash.
        // Just verify we got a response.
        let _ = result;
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_execute_tool_has_tool_check() {
        let server = McpServer::new(PromptLibrary::default()).await.unwrap();

        // "shell" is a non-agent tool, always available
        assert!(server.has_tool("shell").await, "shell tool should exist");
        assert!(
            !server.has_tool("definitely_not_a_tool").await,
            "nonexistent tool should not exist"
        );
    }

    // ---------------------------------------------------------------
    // get_prompt() tests
    // ---------------------------------------------------------------

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_get_prompt_basic() {
        use swissarmyhammer_prompts::Prompt;

        let mut library = PromptLibrary::new();
        library
            .add(Prompt::new("test-prompt", "Hello from test"))
            .unwrap();

        let server = McpServer::new(library).await.unwrap();
        let content = server.get_prompt("test-prompt", None).await.unwrap();

        assert_eq!(content, "Hello from test");
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_get_prompt_with_template_args() {
        use swissarmyhammer_prompts::Prompt;

        let mut library = PromptLibrary::new();
        library
            .add(Prompt::new("greet", "Hello {{ name }}!"))
            .unwrap();

        let server = McpServer::new(library).await.unwrap();
        let mut args = HashMap::new();
        args.insert("name".to_string(), "World".to_string());
        let content = server.get_prompt("greet", Some(&args)).await.unwrap();

        assert_eq!(content, "Hello World!");
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_get_prompt_not_found() {
        let library = PromptLibrary::new();
        let server = McpServer::new(library).await.unwrap();

        let result = server.get_prompt("nonexistent", None).await;
        assert!(result.is_err(), "Should return error for missing prompt");
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_get_prompt_hidden_prompt_rejected() {
        use swissarmyhammer_prompts::Prompt;

        let mut library = PromptLibrary::new();
        let mut hidden = Prompt::new("secret-prompt", "Secret content");
        hidden
            .metadata
            .insert("hidden".to_string(), serde_json::Value::Bool(true));
        library.add(hidden).unwrap();

        let server = McpServer::new(library).await.unwrap();
        let result = server.get_prompt("secret-prompt", None).await;

        assert!(
            result.is_err(),
            "Hidden prompts should not be accessible via get_prompt"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("hidden") || err_msg.contains("Hidden"),
            "Error should mention hidden: {}",
            err_msg
        );
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_get_prompt_partial_rejected() {
        use swissarmyhammer_prompts::Prompt;

        let mut library = PromptLibrary::new();
        library
            .add(Prompt::new("my-partial", "{% partial %}\nPartial content"))
            .unwrap();

        let server = McpServer::new(library).await.unwrap();
        let result = server.get_prompt("my-partial", None).await;

        assert!(
            result.is_err(),
            "Partial templates should not be accessible via get_prompt"
        );
    }

    // ---------------------------------------------------------------
    // reload_prompts() tests
    // ---------------------------------------------------------------

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_reload_prompts_succeeds() {
        let server = McpServer::new(PromptLibrary::default()).await.unwrap();
        // Initialize first to load prompts
        server.initialize().await.unwrap();

        // Reload should succeed
        let result = server.reload_prompts().await;
        assert!(
            result.is_ok(),
            "reload_prompts should succeed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_reload_prompts_detects_no_change() {
        let server = McpServer::new(PromptLibrary::default()).await.unwrap();
        server.initialize().await.unwrap();

        // Reloading without changes should return false (no content change)
        let changed = server.reload_prompts().await.unwrap();
        assert!(
            !changed,
            "Reloading without filesystem changes should report no change"
        );
    }

    // ---------------------------------------------------------------
    // create_validator_server() tests
    // ---------------------------------------------------------------

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_create_validator_server_shares_work_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let server = McpServer::new_with_work_dir(
            PromptLibrary::default(),
            tmp.path().to_path_buf(),
            None,
            false,
        )
        .await
        .unwrap();

        let validator = server.create_validator_server();

        assert_eq!(
            validator.work_dir, server.work_dir,
            "Validator should share the same work_dir"
        );
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_create_validator_server_tool_execution() {
        let tmp = tempfile::tempdir().unwrap();
        let server = McpServer::new_with_work_dir(
            PromptLibrary::default(),
            tmp.path().to_path_buf(),
            None,
            false,
        )
        .await
        .unwrap();

        let validator = server.create_validator_server();

        // Should be able to execute a validator tool (files)
        let result = validator
            .execute_tool(
                "files",
                serde_json::json!({"path": tmp.path().to_str().unwrap()}),
            )
            .await;
        // The call should not return "Unknown tool" error
        if let Err(e) = &result {
            let msg = format!("{:?}", e);
            assert!(
                !msg.contains("Unknown tool"),
                "files tool should be available on validator"
            );
        }

        // Should NOT be able to execute non-validator tools
        let result = validator.execute_tool("shell", serde_json::json!({})).await;
        assert!(
            result.is_err(),
            "Non-validator tool 'shell' should not be executable on validator server"
        );
    }

    // ---------------------------------------------------------------
    // stop_file_watching() tests
    // ---------------------------------------------------------------

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_stop_file_watching_is_safe_without_start() {
        let server = McpServer::new(PromptLibrary::default()).await.unwrap();

        // Stopping file watching without starting should not panic
        server.stop_file_watching().await;
    }

    // ---------------------------------------------------------------
    // Retry helper tests
    // ---------------------------------------------------------------

    #[test]
    fn test_should_retry_within_limit() {
        let err = SwissArmyHammerError::Io(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "timed out",
        ));
        assert!(should_retry(1, &err, McpServer::is_retryable_fs_error));
        assert!(should_retry(2, &err, McpServer::is_retryable_fs_error));
        assert!(!should_retry(3, &err, McpServer::is_retryable_fs_error));
    }

    #[test]
    fn test_should_retry_non_retryable() {
        let err = SwissArmyHammerError::Other {
            message: "permanent failure".to_string(),
        };
        assert!(!should_retry(1, &err, McpServer::is_retryable_fs_error));
    }

    #[test]
    fn test_is_retryable_fs_error_io_kinds() {
        let retryable_kinds = [
            std::io::ErrorKind::TimedOut,
            std::io::ErrorKind::Interrupted,
            std::io::ErrorKind::WouldBlock,
            std::io::ErrorKind::UnexpectedEof,
        ];
        for kind in retryable_kinds {
            let err = SwissArmyHammerError::Io(std::io::Error::new(kind, "test"));
            assert!(
                McpServer::is_retryable_fs_error(&err),
                "{:?} should be retryable",
                kind
            );
        }

        let non_retryable = SwissArmyHammerError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "not found",
        ));
        assert!(!McpServer::is_retryable_fs_error(&non_retryable));
    }

    #[test]
    fn test_is_retryable_fs_error_message_patterns() {
        let err = SwissArmyHammerError::Other {
            message: "resource temporarily unavailable".to_string(),
        };
        assert!(McpServer::is_retryable_fs_error(&err));

        let err = SwissArmyHammerError::Other {
            message: "file is locked by another process".to_string(),
        };
        assert!(McpServer::is_retryable_fs_error(&err));

        let err = SwissArmyHammerError::Other {
            message: "resource busy, try again".to_string(),
        };
        assert!(McpServer::is_retryable_fs_error(&err));
    }

    #[tokio::test]
    async fn test_retry_with_backoff_succeeds_immediately() {
        let mut call_count = 0u32;
        let result: swissarmyhammer_common::Result<&str> = retry_with_backoff(
            || {
                call_count += 1;
                async { Ok("success") }
            },
            |_| true,
            "test_op",
        )
        .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_retry_with_backoff_non_retryable_fails_immediately() {
        let result: swissarmyhammer_common::Result<&str> = retry_with_backoff(
            || async {
                Err(SwissArmyHammerError::Other {
                    message: "permanent".to_string(),
                })
            },
            |_| false, // never retry
            "test_op",
        )
        .await;
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // ServerCapabilities and Implementation tests
    // ---------------------------------------------------------------

    #[test]
    fn test_create_server_capabilities() {
        let caps = create_server_capabilities();
        assert!(caps.prompts.is_some(), "Should have prompts capability");
        assert!(caps.tools.is_some(), "Should have tools capability");
        assert_eq!(
            caps.prompts.as_ref().unwrap().list_changed,
            Some(true),
            "Prompts should support list_changed"
        );
        assert_eq!(
            caps.tools.as_ref().unwrap().list_changed,
            Some(true),
            "Tools should support list_changed"
        );
    }

    #[test]
    fn test_create_server_implementation() {
        let info = create_server_implementation();
        assert_eq!(info.name.as_str(), "SwissArmyHammer");
    }

    // ---------------------------------------------------------------
    // json_map_to_string_map() tests
    // ---------------------------------------------------------------

    #[test]
    fn test_json_map_to_string_map_strings() {
        let mut map = serde_json::Map::new();
        map.insert(
            "key".to_string(),
            serde_json::Value::String("value".to_string()),
        );
        let result = McpServer::json_map_to_string_map(&map);
        assert_eq!(result.get("key").unwrap(), "value");
    }

    #[test]
    fn test_json_map_to_string_map_non_strings() {
        let mut map = serde_json::Map::new();
        map.insert("num".to_string(), serde_json::json!(42));
        map.insert("bool".to_string(), serde_json::json!(true));
        let result = McpServer::json_map_to_string_map(&map);
        assert_eq!(result.get("num").unwrap(), "42");
        assert_eq!(result.get("bool").unwrap(), "true");
    }

    // ---------------------------------------------------------------
    // compute_prompt_signature() tests
    // ---------------------------------------------------------------

    #[test]
    fn test_compute_prompt_signature_deterministic() {
        use swissarmyhammer_prompts::Prompt;
        let prompts = vec![Prompt::new("a", "Hello"), Prompt::new("b", "World")];
        let sig1 = McpServer::compute_prompt_signature(&prompts);
        let sig2 = McpServer::compute_prompt_signature(&prompts);
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_compute_prompt_signature_detects_changes() {
        use swissarmyhammer_prompts::Prompt;
        let prompts_v1 = vec![Prompt::new("a", "Hello")];
        let prompts_v2 = vec![Prompt::new("a", "Hello updated")];
        let sig1 = McpServer::compute_prompt_signature(&prompts_v1);
        let sig2 = McpServer::compute_prompt_signature(&prompts_v2);
        assert_ne!(
            sig1, sig2,
            "Different content should produce different signatures"
        );
    }

    // ---------------------------------------------------------------
    // get_tool_registry() tests
    // ---------------------------------------------------------------

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_get_tool_registry_shares_reference() {
        let server = McpServer::new(PromptLibrary::default()).await.unwrap();
        let registry = server.get_tool_registry();
        let tools = registry.read().await;
        assert!(!tools.is_empty(), "Registry should have tools");
    }
}
