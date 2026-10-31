//! Shared execution context handed to every tool.

use super::registry::ToolRegistry;
use crate::mcp::plan_notifications::PlanSender;
use crate::mcp::tool_handlers::ToolHandlers;
use rmcp::model::{CallToolResult, ProgressNotificationParam, ProgressToken};
use rmcp::{ErrorData as McpError, Peer, RoleServer};
use std::path::PathBuf;
use std::sync::Arc;

use swissarmyhammer_config::model::ChatModelConfig;
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_templating::TemplateLibrary;
use tokio::sync::{Mutex, RwLock};

/// Context shared by all tools during execution
///
/// The `ToolContext` provides tools with access to all necessary storage backends
/// and services required for their operation. It serves as the dependency injection
/// mechanism for the tool registry pattern.
///
/// # Architecture Notes
///
/// The context maintains both legacy `tool_handlers` for backward compatibility
/// and direct storage access for new tool implementations. This dual approach
/// allows for gradual migration from the old delegation pattern to the new
/// direct access pattern.
///
/// # Thread Safety
///
/// All storage backends are wrapped in appropriate synchronization primitives:
/// - `Mutex` for exclusive access operations
/// - `Arc` for shared ownership across async tasks
#[derive(Clone)]
pub struct ToolContext {
    /// The tool handlers instance containing the business logic (for backward compatibility)
    ///
    /// This field exists to support legacy tools that haven't been migrated to the
    /// new registry pattern. New tools should prefer direct storage access.
    pub tool_handlers: Arc<ToolHandlers>,

    /// Direct access to git operations for new tool implementations
    ///
    /// Git operations are wrapped in `Option` to handle cases where git is not
    /// available or not initialized. Always check for `None` before use.
    pub git_ops: Arc<Mutex<Option<GitOperations>>>,

    /// Chat configuration for tool operations (root/default scope)
    ///
    /// Claude Code is the only chat executor, so this carries no executor
    /// choice — only the Claude CLI `--model` switch resolved from the project
    /// config. Tools that execute agent operations build their agent from it.
    pub agent_config: Arc<ChatModelConfig>,

    /// Optional plan sender for task management operations
    ///
    /// When present, task management tools (like kanban) can send plan notifications
    /// containing the complete task list after each mutation. These notifications
    /// can be converted to ACP Plan format by agents for client communication.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// if let Some(sender) = &context.plan_sender {
    ///     let entries = vec![PlanEntry::new("task-1", "Do something", ...)];
    ///     sender.send_plan(entries, "add task", Some("task-1"))?;
    /// }
    /// ```
    pub plan_sender: Option<PlanSender>,

    /// MCP server port (for tools that need to connect to the server)
    ///
    /// Populated by the HTTP server on startup. Uses interior mutability
    /// to allow updates after context creation.
    pub mcp_server_port: Arc<RwLock<Option<u16>>>,

    /// Optional MCP peer for tools that need to communicate with the client
    ///
    /// When present, tools can send requests to the MCP client (e.g., elicitation requests).
    /// This field is populated during tool execution in the call_tool handler.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// if let Some(peer) = &context.peer {
    ///     let result = peer.create_elicitation(request).await?;
    /// }
    /// ```
    pub peer: Option<Arc<Peer<RoleServer>>>,

    /// Optional tool registry for tools that need to call other tools
    ///
    /// When present, tools can call other tools through their MCP interface.
    /// This enables tool composition while maintaining proper layering.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// if let Some(registry) = &context.tool_registry {
    ///     let result = context.call_tool("kanban", json!({
    ///         "op": "add task",
    ///         "title": "Fix issue",
    ///         "description": "Details..."
    ///     })).await?;
    /// }
    /// ```
    pub tool_registry: Option<Arc<RwLock<ToolRegistry>>>,

    /// Working directory for tool operations
    ///
    /// This is the base directory where tools should operate. For example:
    /// - Kanban storage will be in `{working_dir}/.sah/kanban/`
    ///
    /// If None, tools should use `std::env::current_dir()` or git root detection
    /// as a fallback. In tests, this should always be set to an isolated directory.
    ///
    /// This explicit approach avoids reliance on environment variables (global state)
    /// and makes test isolation more reliable and explicit.
    pub working_dir: Option<PathBuf>,

    /// MCP session identifier
    ///
    /// Auto-generated when the context is created. Tools can use this to
    /// identify the current session without requiring the caller to pass it.
    pub session_id: String,

    /// Session actor ID for auto-assigning tasks.
    ///
    /// Set when an MCP client connects via `initialize`, storing the slugified
    /// client name as the actor ID. Kanban tools inject this as the `actor` arg
    /// when the caller does not supply one explicitly.
    ///
    /// Uses interior mutability so it can be set after the context is wrapped in
    /// `Arc` (the same pattern as `mcp_server_port`).
    pub session_actor: Arc<RwLock<Option<String>>>,

    /// Optional MCP server instance (for creating filtering proxies)
    ///
    /// Uses interior mutability to allow setting after context creation.
    /// This is necessary because the server contains the context, creating a
    /// circular reference that must be resolved after construction.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// if let Some(server) = context.mcp_server.read().await.clone() {
    ///     let proxy = FilteringMcpProxy::new(server.clone(), filter);
    ///     // Use proxy for restricted tool access
    /// }
    /// ```
    pub mcp_server: Arc<RwLock<Option<Arc<crate::mcp::McpServer>>>>,

    /// Optional prompt library for template rendering
    ///
    /// When present, tools can render Liquid templates with partial resolution
    /// (e.g., `{% include "_partials/project-types/rust" %}`). This enables
    /// tools like `detect projects` to render project-type guidelines through
    /// the same pipeline used by skills and agents.
    pub prompt_library: Option<Arc<RwLock<TemplateLibrary>>>,

    /// Optional MCP progress token for tool calls that report progress.
    ///
    /// Set from the incoming request's `_meta.progressToken` field by the
    /// MCP server before dispatching the tool. When present, long-running
    /// tools (e.g. `code_context` with `op: "rebuild index"`) build an
    /// `McpProgressReporter` that forwards `IndexProgress` events as
    /// `notifications/progress` messages through `peer`. When absent, those
    /// tools use a `NoopReporter` and emit no progress notifications.
    pub progress_token: Option<ProgressToken>,

    /// Optional in-process sink for `ProgressNotificationParam` events.
    ///
    /// Used by in-process callers (e.g. the `code-context` CLI) that invoke
    /// `tool.execute(...)` directly without a real MCP stdio server in the
    /// loop. When set, tools that report progress (`code_context` with
    /// `op: "rebuild index"`) drive the same `McpProgressReporter` they
    /// would use for a real peer, but the drain task forwards each
    /// `ProgressNotificationParam` to this sender instead of calling
    /// `Peer::send_notification`. The receiver half lives on the caller and
    /// feeds a renderer (TUI bar, JSON tap, etc.).
    ///
    /// Requires `progress_token` to also be set — without a token the tool
    /// has no identifier to attach to outgoing notifications and will fall
    /// back to a no-op reporter.
    ///
    /// `progress_sink` and `peer` are independent: a context may carry one,
    /// the other, or both. When both are present the sink takes priority
    /// because it is the explicit in-process opt-in.
    pub progress_sink: Option<tokio::sync::mpsc::UnboundedSender<ProgressNotificationParam>>,
}

// `ToolContext` is shared across async tasks. A field that is not `Send + Sync`
// must fail the build here, not at a distant `tokio::spawn` call site.
const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ToolContext>();
};

impl ToolContext {
    /// Create a new tool context
    pub fn new(
        tool_handlers: Arc<ToolHandlers>,
        git_ops: Arc<Mutex<Option<GitOperations>>>,
        agent_config: Arc<ChatModelConfig>,
    ) -> Self {
        Self {
            tool_handlers,
            git_ops,
            agent_config,
            plan_sender: None,
            mcp_server_port: Arc::new(RwLock::new(None)),
            peer: None,
            tool_registry: None,
            working_dir: None,
            session_id: ulid::Ulid::new().to_string(),
            session_actor: Arc::new(RwLock::new(None)),
            mcp_server: Arc::new(RwLock::new(None)),
            prompt_library: None,
            progress_token: None,
            progress_sink: None,
        }
    }

    /// Set the prompt library for template rendering
    ///
    /// # Arguments
    ///
    /// * `library` - The prompt library to use for Liquid template rendering
    ///
    /// # Returns
    ///
    /// A new `ToolContext` with the prompt library set
    pub fn with_prompt_library(mut self, library: Arc<RwLock<TemplateLibrary>>) -> Self {
        self.prompt_library = Some(library);
        self
    }

    /// Set the plan sender for plan notifications
    ///
    /// # Arguments
    ///
    /// * `sender` - The plan sender to use
    ///
    /// # Returns
    ///
    /// A new `ToolContext` with the plan sender set
    pub fn with_plan_sender(mut self, sender: PlanSender) -> Self {
        self.plan_sender = Some(sender);
        self
    }

    /// Set the MCP peer for this context
    ///
    /// Creates a new context with the peer added. This allows tools to
    /// communicate with the MCP client (e.g., for elicitation requests).
    ///
    /// # Arguments
    ///
    /// * `peer` - The MCP peer to use
    ///
    /// # Returns
    ///
    /// A new `ToolContext` with the peer set
    pub fn with_peer(mut self, peer: Arc<Peer<RoleServer>>) -> Self {
        self.peer = Some(peer);
        self
    }

    /// Set the MCP progress token for this context.
    ///
    /// Creates a new context carrying the progress token from the incoming
    /// request's `_meta.progressToken` field. Tools that report progress
    /// (currently `code_context` with `op: "rebuild index"`) read this to
    /// decide whether to send `notifications/progress` messages back to
    /// the MCP client.
    ///
    /// # Arguments
    ///
    /// * `token` - The MCP progress token supplied by the client
    ///
    /// # Returns
    ///
    /// A new `ToolContext` with the progress token set
    pub fn with_progress_token(mut self, token: ProgressToken) -> Self {
        self.progress_token = Some(token);
        self
    }

    /// Set the in-process progress sink for this context.
    ///
    /// Creates a new context that routes `ProgressNotificationParam` events to
    /// the supplied channel sender instead of (or in addition to) the MCP peer.
    /// This is the entry point used by in-process callers — most notably the
    /// `code-context` CLI — that invoke `tool.execute(...)` directly and need
    /// to consume progress without a stdio MCP server in the loop.
    ///
    /// Callers that use this **must** also set a `progress_token` via
    /// [`Self::with_progress_token`] so the emitted notifications carry a
    /// stable identifier the renderer can key off.
    ///
    /// # Arguments
    ///
    /// * `sink` - The sender half of an unbounded mpsc channel; the matching
    ///   receiver should be drained by the caller's renderer task.
    ///
    /// # Returns
    ///
    /// A new `ToolContext` with the progress sink set.
    pub fn with_progress_sink(
        mut self,
        sink: tokio::sync::mpsc::UnboundedSender<ProgressNotificationParam>,
    ) -> Self {
        self.progress_sink = Some(sink);
        self
    }

    /// Set the tool registry for this context
    ///
    /// Creates a new context with the tool registry added. This allows tools to
    /// call other tools through their MCP interface.
    ///
    /// # Arguments
    ///
    /// * `registry` - The tool registry to use
    ///
    /// # Returns
    ///
    /// A new `ToolContext` with the tool registry set
    pub fn with_tool_registry(mut self, registry: Arc<RwLock<ToolRegistry>>) -> Self {
        self.tool_registry = Some(registry);
        self
    }

    /// Set the working directory for this context
    ///
    /// Creates a new context with the working directory set. This allows tools
    /// to operate in an isolated directory, avoiding global state from environment
    /// variables and making test isolation more reliable.
    ///
    /// # Arguments
    ///
    /// * `working_dir` - The base directory where tools should operate
    ///
    /// # Returns
    ///
    /// A new `ToolContext` with the working directory set
    pub fn with_working_dir(mut self, working_dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(working_dir.into());
        self
    }

    /// Resolve the session working directory — the root every tool operates against.
    ///
    /// The working directory of a board *is* the working directory of its agent
    /// session, so all file tools (grep, glob, read, write, edit) resolve
    /// relative paths and default search roots from this value, never from the
    /// process current directory. For the bundled GUI app the process CWD is `/`
    /// (a read-only filesystem root); a single app process also hosts multiple
    /// boards, so process CWD cannot be a per-session root anyway.
    ///
    /// Resolution order:
    /// 1. [`Self::working_dir`] when set (the normal case — the server threads the
    ///    board directory in at construction).
    /// 2. The process current directory as a last resort, for stand-alone callers
    ///    (CLI / tests) that constructed a context without a working dir. This is
    ///    the *only* sanctioned `current_dir()` fallback; tool handlers must call
    ///    this method instead of reaching for `std::env::current_dir()` directly,
    ///    so that the in-process multi-board host never roots a search at `/`.
    ///
    /// Falls back to `.` when even the current directory cannot be determined
    /// (e.g. it was deleted). This relative fallback is intentionally *not* a
    /// usable search root: search tools (grep, glob) call
    /// [`reject_filesystem_root`](crate::mcp::tools::files::shared_utils::reject_filesystem_root)
    /// on the resolved root, which rejects a relative `.` (and the filesystem
    /// root `/`) rather than walking the process CWD this method exists to avoid.
    pub fn session_root(&self) -> PathBuf {
        self.working_dir
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Set the MCP server instance for this context
    ///
    /// Uses interior mutability to set the server reference after context creation.
    /// This is necessary to avoid circular reference issues during server construction.
    ///
    /// # Arguments
    ///
    /// * `server` - The MCP server instance
    pub async fn set_mcp_server(&self, server: Arc<crate::mcp::McpServer>) {
        *self.mcp_server.write().await = Some(server);
    }

    /// Call another MCP tool from within a tool
    ///
    /// This method allows tools to compose by calling other tools through their
    /// MCP interface, maintaining proper architectural layering. Tools should use
    /// this instead of directly accessing storage or implementation details.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tool to call
    /// * `params` - The parameters to pass to the tool (must be a JSON object)
    ///
    /// # Returns
    ///
    /// * `Result<CallToolResult, McpError>` - The result from the called tool
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The tool registry is not available in this context
    /// - The specified tool is not found
    /// - The parameters are not a JSON object
    /// - The tool execution fails
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// async fn execute(&self, args: Args, context: &ToolContext) -> Result<CallToolResult, McpError> {
    ///     // Call kanban tool instead of direct storage access
    ///     let result = context.call_tool("kanban", json!({
    ///         "op": "add task",
    ///         "title": "Fix violation",
    ///         "description": "Details about the violation"
    ///     })).await?;
    ///
    ///     // Extract data from result if needed
    ///     Ok(result)
    /// }
    /// ```
    pub async fn call_tool(
        &self,
        name: &str,
        params: serde_json::Value,
    ) -> Result<CallToolResult, McpError> {
        use tracing::Instrument;

        let tool_name = name.to_string();
        let arg_count = match &params {
            serde_json::Value::Object(map) => map.len(),
            _ => 0,
        };
        // The dispatched op rides on the span so per-op log aggregation works
        // on any line of the call — including `tool_call complete` — without
        // joining back to the args line (which breaks under concurrent calls).
        let op_field = params
            .get("op")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let span = tracing::info_span!(
            "tool_call",
            tool = %tool_name,
            op = op_field.as_str(),
            args = arg_count,
            caller = "internal",
            status = tracing::field::Empty,
        );

        async {
            // Get the tool registry
            let registry = self.tool_registry.as_ref().ok_or_else(|| {
                McpError::internal_error("tool registry not available in this context", None)
            })?;

            // Look up the tool
            let registry_guard = registry.read().await;
            let tool = registry_guard.get_tool(name).ok_or_else(|| {
                tracing::error!(tool = %tool_name, "tool not found in registry");
                McpError::internal_error(format!("tool '{}' not found in registry", name), None)
            })?;

            // Convert params to a map
            let params_map = match params {
                serde_json::Value::Object(map) => map,
                _ => {
                    return Err(McpError::invalid_params(
                        format!("tool parameters must be a JSON object, got: {:?}", params),
                        None,
                    ));
                }
            };

            let inner_context = self.clone();
            let start = std::time::Instant::now();
            let result = tool.execute(params_map, &inner_context).await;
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
}
