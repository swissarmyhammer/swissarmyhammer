// sah rule ignore acp/capability-enforcement
//! MCP server implementation for serving prompts and workflows.
//!
//! [`McpServer`] is defined here together with its construction, tool-registry
//! wiring, and tool dispatch. Everything else the server does lives in a
//! submodule, one per concern:
//!
//! - [`instructions`] — the server instructions, capabilities, and
//!   implementation identity advertised at handshake.
//! - [`retry`] — the exponential-backoff retry helper shared by the reload and
//!   file-watch paths.
//! - [`code_context`] — code-context workspace startup: leader election, the
//!   LSP supervisor, the indexing workers, and the diagnostics fan-out.
//! - [`profiles`] — the pre-scoped server clones (validator, agent tools) and
//!   the serve-time native-tool deny.
//! - [`prompts`] — prompt reload and the content signature that decides whether
//!   a reload changed anything.
//! - [`file_watch`] — prompt-directory watching, start and stop.
//! - [`agent_identity`] — the kanban actor a connecting MCP client is given.
//! - [`handler`] — the [`rmcp::ServerHandler`] implementation that turns MCP
//!   requests into calls on the modules above.
//!
//! Note: This is an MCP server, not an ACP agent. ACP capability checking
//! happens at the agent layer (claude-agent), not at the MCP layer.

mod agent_identity;
mod code_context;
mod file_watch;
mod handler;
mod instructions;
mod profiles;
mod prompts;
mod retry;

use crate::mcp::file_watcher::FileWatcher;
use std::path::PathBuf;
use std::sync::Arc;

use swissarmyhammer_common::{Result, SwissArmyHammerError};
use swissarmyhammer_config::model::ModelManager;
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_templating::{PromptResolver, TemplateLibrary};

use tokio::sync::{Mutex, RwLock};

use super::tool_handlers::ToolHandlers;
use super::tool_registry::{
    register_code_context_tools, register_diagnostics_tools, register_file_tools,
    register_git_tools, register_kanban_tools, register_questions_tools, register_ralph_tools,
    register_review_tools, register_shell_tools, register_web_tools, ToolContext, ToolRegistry,
};
use super::tools::agent::register_agent_tools;
use super::tools::skill::register_skill_tools;
use swissarmyhammer_agents::AgentLibrary;
use swissarmyhammer_skills::SkillLibrary;

/// MCP server for all SwissArmyHammer functionality.
#[derive(Clone)]
pub struct McpServer {
    /// Prompt template library the server renders prompts from.
    library: Arc<RwLock<TemplateLibrary>>,

    /// Watches the prompt directories and reloads `library` when they change.
    file_watcher: Arc<Mutex<FileWatcher>>,
    /// Handle to the in-flight background file-watch startup task, if any.
    ///
    /// `stop_file_watching()` aborts this so a slow FSEvents registration that
    /// is still in flight cannot resurrect an active watcher after shutdown.
    file_watcher_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Set once shutdown has run; suppresses a late, off-lock store from an
    /// in-flight startup task so the watcher is never resurrected post-shutdown.
    file_watch_stopped: Arc<std::sync::atomic::AtomicBool>,
    /// Every MCP tool this server can dispatch, keyed by tool name.
    tool_registry: Arc<RwLock<ToolRegistry>>,
    /// Shared state every tool handler reads — storage backends, session
    /// working directory, and the rate limiter.
    pub tool_context: Arc<ToolContext>,
    /// Skill library - kept alive to back the SkillTool's shared reference
    #[allow(dead_code)]
    skill_library: Arc<RwLock<SkillLibrary>>,
    /// Agent library - kept alive to back the agent tool's shared reference
    #[allow(dead_code)]
    agent_library: Arc<RwLock<AgentLibrary>>,
    /// Working directory — stored for deferred initialization (e.g. code-context)
    work_dir: Option<PathBuf>,
    /// Watches tools.yaml for changes, reloads on list_tools() calls
    tool_config_watcher: Arc<Mutex<super::tool_config::ToolConfigWatcher>>,
    /// Whether `list_tools` composes its advertised set per connecting client.
    ///
    /// `true` for the full server: each connection's `tools/list` is filtered by
    /// the client's [`Host`](super::host::Host) identity and each tool's
    /// `category()` (Claude → `Shared` + `Replacement`; unknown → `Shared`
    /// only). `false` for the registries that are pre-scoped and must be served
    /// verbatim:
    /// - the validator server, whose minimal registry is already exactly the
    ///   validator profile (`tools::register_validator_tools`); and
    /// - the agent-tools server (`create_agent_tools_server`), whose registry is
    ///   already scoped to the intrinsic Agent tools and must surface in full
    ///   even though no [`Host`] reports `serves(Agent)`, so per-client filtering
    ///   would serve zero tools.
    compose_per_client: bool,
    /// Latches once the serve-time native-tool deny has run for a Claude client.
    ///
    /// When a Claude client connects, the serve path denies the native host
    /// tool(s) superseded by the served `Replacement` tools (e.g. `Bash`, since
    /// `shell` replaces it) so the served tool truly supersedes Claude's native
    /// rather than competing with it. The deny is idempotent, but this flag keeps
    /// it from re-writing settings on every `initialize`; the first Claude
    /// connection latches it. Shared across clones so all share the same latch.
    bash_denied: Arc<std::sync::atomic::AtomicBool>,
}

/// Prints the fields a reader can learn without taking a lock.
///
/// The library, the watcher, the registry, the tool context and the two
/// builtin libraries all sit behind a `tokio` lock. `Debug` runs wherever a log
/// line asks for it, including inside the async runtime, so it must never wait
/// on one of those locks. It therefore names the plain fields and closes with
/// `..` for the rest.
impl std::fmt::Debug for McpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpServer")
            .field("work_dir", &self.work_dir)
            .field("compose_per_client", &self.compose_per_client)
            .field("file_watch_stopped", &self.file_watch_stopped)
            .field("bash_denied", &self.bash_denied)
            .finish_non_exhaustive()
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
    /// Returns the errors of
    /// [`new_with_work_dir`](Self::new_with_work_dir), which does the
    /// construction. The current directory adds no error of its own: if it does
    /// not read, this call uses the temporary directory as the working
    /// directory.
    pub async fn new(library: TemplateLibrary) -> Result<Self> {
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
    /// Returns [`SwissArmyHammerError`] if a construction step reports a
    /// failure. No step in the current path does. A `work_dir` that is not a
    /// git repository gives no git operations, and a chat model configuration
    /// that does not read falls back to the default configuration. Each of the
    /// two writes a warning in the log instead of an error. The one fallible
    /// call is the private `resolve_agent_config`, and this `Result` carries
    /// its error to the caller.
    pub async fn new_with_work_dir(library: TemplateLibrary, work_dir: PathBuf) -> Result<Self> {
        let git_ops_arc = Self::initialize_git_operations(work_dir.clone());
        let tool_handlers = ToolHandlers::new();
        let agent_config = Self::resolve_agent_config()?;

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
        )
        .await;

        Ok(Self {
            library: prompt_library,
            file_watcher: Arc::new(Mutex::new(FileWatcher::new())),
            file_watcher_task: Arc::new(Mutex::new(None)),
            file_watch_stopped: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            tool_registry: tool_registry_arc,
            tool_context,
            skill_library,
            agent_library,
            work_dir: Some(work_dir),
            tool_config_watcher: Arc::new(Mutex::new(super::tool_config::ToolConfigWatcher::new())),
            compose_per_client: true,
            bash_denied: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    /// Wrap `library` for shared access, populate it with its builtins, and log
    /// how many arrived.
    ///
    /// Every builtin library shares this shape: build empty, load the defaults,
    /// report the count. `load` populates the library and answers how many
    /// definitions it holds — each library type spells that count differently —
    /// and `kind` names them in the debug log.
    async fn init_builtin_library<L>(
        library: L,
        kind: &str,
        load: impl FnOnce(&mut L) -> usize,
    ) -> Arc<RwLock<L>> {
        let shared = Arc::new(RwLock::new(library));
        let mut lib = shared.write().await;
        let loaded = load(&mut lib);
        tracing::debug!("Loaded {} {}", loaded, kind);
        drop(lib);
        shared
    }

    /// Construct a `SkillLibrary` pre-populated with the builtin skills.
    async fn init_skill_library() -> Arc<RwLock<SkillLibrary>> {
        Self::init_builtin_library(SkillLibrary::new(), "skills", |lib| {
            lib.load_defaults();
            lib.len()
        })
        .await
    }

    /// Construct an `AgentLibrary` pre-populated with the builtin agents.
    async fn init_agent_library() -> Arc<RwLock<AgentLibrary>> {
        Self::init_builtin_library(AgentLibrary::new(), "agents", |lib| {
            lib.load_defaults();
            lib.names().len()
        })
        .await
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

    /// Resolve the chat configuration for the default scope.
    ///
    /// Reads the top-level `model:` from the project config, which sets the
    /// Claude CLI `--model` switch. An unreadable config falls back to plain
    /// `claude` rather than failing the server start.
    ///
    /// # Returns
    ///
    /// * `Result<Arc<swissarmyhammer_config::model::ChatModelConfig>>` - Chat configuration
    fn resolve_agent_config() -> Result<Arc<swissarmyhammer_config::model::ChatModelConfig>> {
        match ModelManager::resolve_chat_config(&swissarmyhammer_config::model::ModelPaths::sah()) {
            Ok(config) => {
                tracing::debug!("Resolved chat model switch: {:?}", config.model);
                Ok(Arc::new(config))
            }
            Err(e) => {
                tracing::warn!("Failed to resolve chat model config: {}, using default", e);
                Ok(Arc::new(
                    swissarmyhammer_config::model::ChatModelConfig::default(),
                ))
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
    ///
    /// # Returns
    ///
    /// * `(Arc<RwLock<ToolRegistry>>, Arc<ToolContext>)` - Registry and context
    #[allow(clippy::too_many_arguments)]
    async fn create_tool_context_and_registry(
        tool_handlers: ToolHandlers,
        git_ops_arc: Arc<Mutex<Option<GitOperations>>>,
        agent_config: Arc<swissarmyhammer_config::model::ChatModelConfig>,
        working_dir: Option<PathBuf>,
        skill_library: Arc<RwLock<SkillLibrary>>,
        agent_library: Arc<RwLock<AgentLibrary>>,
        prompt_library: Arc<RwLock<TemplateLibrary>>,
    ) -> (Arc<RwLock<ToolRegistry>>, Arc<ToolContext>) {
        let mut tool_registry = ToolRegistry::new();
        Self::register_all_tools(
            &mut tool_registry,
            skill_library,
            agent_library,
            prompt_library.clone(),
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
    /// All tools are registered unconditionally. Each tool carries a structural
    /// [`category()`](crate::mcp::tool_registry::McpTool::category) describing its
    /// relationship to a host agent; composing a per-client tool surface from
    /// those categories happens at the serve boundary, not here.
    async fn register_all_tools(
        tool_registry: &mut ToolRegistry,
        skill_library: Arc<RwLock<SkillLibrary>>,
        agent_library: Arc<RwLock<AgentLibrary>>,
        prompt_library: Arc<RwLock<TemplateLibrary>>,
    ) {
        register_git_tools(tool_registry);
        register_kanban_tools(tool_registry);
        register_questions_tools(tool_registry);
        register_web_tools(tool_registry);
        register_code_context_tools(tool_registry);
        register_shell_tools(tool_registry);
        register_ralph_tools(tool_registry);
        register_agent_tools(tool_registry, agent_library, prompt_library.clone());
        register_file_tools(tool_registry);
        register_review_tools(tool_registry);
        register_diagnostics_tools(tool_registry);
        register_skill_tools(tool_registry, skill_library, prompt_library);

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
    /// * `&Arc<RwLock<TemplateLibrary>>` - Reference to the wrapped prompt library
    pub fn library(&self) -> &Arc<RwLock<TemplateLibrary>> {
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
    ///
    /// # Errors
    ///
    /// Returns an invalid-request [`rmcp::ErrorData`] if the registry holds no
    /// tool with this name. If the tool runs, its own error comes back
    /// unchanged. An `arguments` value that is not a JSON object is not an
    /// error here: it becomes an empty argument map, and the tool then reports
    /// what it needs.
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
            let tool_context = (*self.tool_context).clone();
            tool.execute(arguments_map, &tool_context).await
        } else {
            Err(rmcp::ErrorData::invalid_request(
                format!("unknown tool: {}", name),
                None,
            ))
        }
    }

    /// Wire the live `review` factories into this server's tool registry.
    ///
    /// The server registers the `review` tool with no agent factory at
    /// construction (see [`register_review_tools`](crate::mcp::tools::review::register_review_tools)),
    /// so its three pipeline ops (`review file`/`working`/`sha`) return an
    /// actionable error until a factory is wired. This is the injection seam the
    /// wiring layer — a crate that may depend on `swissarmyhammer-agent`, which
    /// `swissarmyhammer-tools` cannot — calls after building the server to swap
    /// the bare tool for one that drives the configured backend.
    ///
    /// `agent_factory` mints a fresh ACP agent per review run; `embedder_factory`
    /// is `None` to keep the loaded platform-embedder default; `concurrency` pins
    /// the pool worker count (`review.concurrency`) when set. Registration is by
    /// tool name, so this overwrites the bare `review` tool. The registry is
    /// shared across server clones and read per `call_tool`, so the swap takes
    /// effect for every subsequent `review` dispatch on this server.
    pub async fn set_review_factories(
        &self,
        agent_factory: crate::mcp::tools::review::review_op::AgentFactory,
        embedder_factory: Option<crate::mcp::tools::review::review_op::EmbedderFactory>,
        concurrency: Option<usize>,
    ) {
        let mut registry = self.tool_registry.write().await;
        crate::mcp::tools::review::register_review_tool_with_factories(
            &mut registry,
            agent_factory,
            embedder_factory,
            concurrency,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------
    // Debug tests
    // ---------------------------------------------------------------

    /// `Debug` names the working directory and the per-client composition flag —
    /// the two fields that decide what a connection is served — and it answers
    /// while another task holds a lock, because a `Debug` that waits on a lock
    /// deadlocks the log line that asks for it.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_debug_names_work_dir_and_composition_under_a_held_lock() {
        let tmp = tempfile::tempdir().unwrap();
        let server =
            McpServer::new_with_work_dir(TemplateLibrary::default(), tmp.path().to_path_buf())
                .await
                .unwrap();

        let _registry = server.tool_registry.write().await;
        let rendered = format!("{server:?}");

        assert!(rendered.contains("McpServer"), "{rendered}");
        assert!(
            rendered.contains(&format!("{:?}", tmp.path())),
            "{rendered}"
        );
        assert!(rendered.contains("compose_per_client: true"), "{rendered}");
    }

    // ---------------------------------------------------------------
    // new_with_work_dir() tests
    // ---------------------------------------------------------------

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_new_with_work_dir_creates_server() {
        let tmp = tempfile::tempdir().unwrap();
        let server =
            McpServer::new_with_work_dir(TemplateLibrary::default(), tmp.path().to_path_buf())
                .await
                .unwrap();

        // The server should store the working directory
        assert_eq!(server.work_dir, Some(tmp.path().to_path_buf()));
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_new_with_work_dir_registers_agent_tools() {
        let tmp = tempfile::tempdir().unwrap();
        let server =
            McpServer::new_with_work_dir(TemplateLibrary::default(), tmp.path().to_path_buf())
                .await
                .unwrap();

        // The full tool union is always registered — agent capabilities
        // (files, web, skill, agent) are present regardless of host. Per-client
        // composition happens at the serve boundary, not here.
        let tools = server.list_tools().await;
        for expected in ["files", "web", "skill", "agent"] {
            assert!(
                tools.iter().any(|t| t.name == expected),
                "expected agent tool '{}' to be registered; got: {:?}",
                expected,
                tools.iter().map(|t| &t.name).collect::<Vec<_>>()
            );
        }
    }

    // ---------------------------------------------------------------
    // set_server_port() tests
    // ---------------------------------------------------------------

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_set_server_port() {
        let server = McpServer::new(TemplateLibrary::default()).await.unwrap();

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
        let server = McpServer::new(TemplateLibrary::default()).await.unwrap();

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
    async fn test_initialize_loads_prompts_into_library() {
        let server = McpServer::new(TemplateLibrary::default()).await.unwrap();

        // Initialize should succeed without errors and populate the library
        // (including partials) so skill/agent rendering has its templates.
        server.initialize().await.unwrap();

        // Assert directly on the effect that is unique to initialize(): the
        // library holds builtin prompts and partials immediately afterwards,
        // before any reload. This exercises the initialize() load path itself
        // rather than re-proving what the reload_prompts() tests already cover.
        let library = server.library.read().await;
        assert!(
            !library.list().unwrap().is_empty(),
            "After initialize(), the library should hold builtin prompts and partials"
        );
    }

    // ---------------------------------------------------------------
    // list_tools() tests
    // ---------------------------------------------------------------

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_list_tools_returns_registered_tools() {
        let server = McpServer::new(TemplateLibrary::default()).await.unwrap();
        let tools = server.list_tools().await;

        // Should have multiple tools registered
        assert!(
            tools.len() > 3,
            "Should have many tools registered, got {}",
            tools.len()
        );

        // Verify some core shared tools are present. The full tool union is
        // always registered; per-client composition happens at the serve
        // boundary, not in registration.
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
        let server = McpServer::new(TemplateLibrary::default()).await.unwrap();

        let result = server
            .execute_tool("nonexistent_tool", serde_json::json!({}))
            .await;

        assert!(result.is_err(), "an unknown tool should return an error");
        let err = result.unwrap_err();
        let msg = format!("{:?}", err);
        assert!(
            msg.contains("unknown tool"),
            "Error should mention unknown tool: {}",
            msg
        );
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_execute_tool_with_non_object_args() {
        let server = McpServer::new(TemplateLibrary::default()).await.unwrap();

        // A non-object `arguments` is replaced by an empty map, so the named
        // tool still runs and answers for itself.
        let result = server
            .execute_tool("files", serde_json::json!("not an object"))
            .await;

        // `files` dispatches on `op`, and an empty map carries none, so it
        // reports the missing operation. That message is the proof the
        // substitution happened: an unsubstituted non-object would have failed
        // before reaching the tool, and the message would name the arguments
        // rather than the operation.
        let err = result.expect_err("`files` with no `op` must return an error");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("cannot determine operation"),
            "the empty map must reach `files`, which then reports the missing \
             operation; got: {msg}"
        );
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_execute_tool_has_tool_check() {
        let server = McpServer::new(TemplateLibrary::default()).await.unwrap();

        // "shell" is a non-agent tool, always available
        assert!(server.has_tool("shell").await, "shell tool should exist");
        assert!(
            !server.has_tool("definitely_not_a_tool").await,
            "nonexistent tool should not exist"
        );
    }

    // ---------------------------------------------------------------
    // get_tool_registry() tests
    // ---------------------------------------------------------------

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_get_tool_registry_shares_reference() {
        let server = McpServer::new(TemplateLibrary::default()).await.unwrap();
        let registry = server.get_tool_registry();
        let tools = registry.read().await;
        assert!(!tools.is_empty(), "Registry should have tools");
    }
}
