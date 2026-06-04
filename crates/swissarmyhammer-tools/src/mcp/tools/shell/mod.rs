//! Shell tool for MCP — virtual shell with history and process management.
//!
//! ## Operations
//!
//! Dispatches between five operations:
//! - `execute command`: Run a shell command with timeout and output capture
//! - `list processes`: Show all commands with status, timing, exit codes
//! - `kill process`: Stop a running command by ID
//! - `grep history`: Regex pattern match across command output
//! - `get lines`: Retrieve specific lines from a command's output
//!
//! ## Architecture
//!
//! Commands execute in isolated child processes via `tokio::process::Command`.
//! Each process is wrapped in an [`AsyncProcessGuard`](process::AsyncProcessGuard)
//! that kills and reaps the process on drop, preventing orphans and zombies even
//! when a timeout or cancellation occurs.
//!
//! Output is streamed through an [`OutputBuffer`](infrastructure::OutputBuffer) that
//! enforces size limits (10 MB default), detects binary content, and truncates at
//! line boundaries. All output is stored in [`ShellState`](state::ShellState) for
//! later retrieval via `get lines` or `grep history`.
//!
//! ## Security
//!
//! Every command passes through `swissarmyhammer_shell` security validation before
//! execution: blocked command patterns, path traversal prevention, environment
//! variable sanitization, and command length limits. See
//! [`execute_command`] for the validation pipeline.
//!
//! ## Module Layout
//!
//! - [`infrastructure`]: Types, output buffer, error types
//! - [`process`]: Process spawning, streaming, guard
//! - [`state`]: Command history, output log
//! - [`execute_command`], [`list_processes`], [`kill_process`],
//!   [`grep_history`], [`get_lines`]: Per-operation modules

pub mod execute_command;
pub mod get_lines;
pub mod grep_history;
pub mod infrastructure;
pub mod kill_process;
pub mod list_processes;
pub mod process;
pub mod state;

#[cfg(test)]
pub(crate) mod test_helpers;

// Re-export public types from infrastructure
pub use infrastructure::{
    format_output_content, is_binary_content, OutputBuffer, OutputLimits, ShellError,
    ShellExecutionResult,
};

use crate::mcp::tool_registry::{McpTool, ToolCategory, ToolContext};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use std::sync::Arc;
use swissarmyhammer_common::health::{Doctorable, HealthCheck};
use swissarmyhammer_operations::{generate_mcp_schema, Operation, SchemaConfig};
use swissarmyhammer_shell::config::{parse_shell_config, CompiledShellConfig, BUILTIN_CONFIG_YAML};
use tokio::sync::Mutex;

use state::ShellState;

// Static operation instances for schema generation
static EXECUTE_CMD: Lazy<execute_command::ExecuteCommand> =
    Lazy::new(execute_command::ExecuteCommand::default);
static LIST_PROCS: Lazy<list_processes::ListProcesses> =
    Lazy::new(list_processes::ListProcesses::default);
static KILL_PROC: Lazy<kill_process::KillProcess> = Lazy::new(kill_process::KillProcess::default);
static GREP_HIST: Lazy<grep_history::GrepHistory> = Lazy::new(grep_history::GrepHistory::default);
static GET_LNS: Lazy<get_lines::GetLines> = Lazy::new(get_lines::GetLines::default);

pub static SHELL_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| {
    vec![
        &*EXECUTE_CMD as &dyn Operation,
        &*LIST_PROCS as &dyn Operation,
        &*KILL_PROC as &dyn Operation,
        &*GREP_HIST as &dyn Operation,
        &*GET_LNS as &dyn Operation,
    ]
});

/// Tool for executing shell commands
#[derive(Clone)]
pub struct ShellExecuteTool {
    state: Arc<Mutex<ShellState>>,
    /// Optional MCP server entry the tool registers during `init`/`deinit`.
    ///
    /// The serve path leaves this `None` so running the tool never touches
    /// agent config. The CLI injects `Some((name, entry))` via
    /// [`ShellExecuteTool::with_mcp_server`] so the install lifecycle can
    /// register the `shelltool serve` command with each agent.
    mcp_server: Option<(String, mirdan::mcp_config::McpServerEntry)>,
}

impl Default for ShellExecuteTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellExecuteTool {
    /// Creates a new instance of the ShellExecuteTool with in-memory state.
    pub fn new() -> Self {
        let state = ShellState::new().expect("Failed to initialize shell state");
        Self {
            state: Arc::new(Mutex::new(state)),
            mcp_server: None,
        }
    }

    /// Attach an MCP server entry the tool registers per scope during
    /// `init`/`deinit`.
    ///
    /// The CLI calls this to make the tool own its own MCP registration:
    /// `init` writes `name → entry` into each scope's agent config (via
    /// mirdan), and `deinit` removes it. `new()`/`Default` leave it unset so
    /// the serve path is unaffected.
    pub fn with_mcp_server(
        mut self,
        name: impl Into<String>,
        entry: mirdan::mcp_config::McpServerEntry,
    ) -> Self {
        self.mcp_server = Some((name.into(), entry));
        self
    }

    /// Creates an instance rooted in an isolated temp directory.
    ///
    /// Use this in tests to avoid depending on the process CWD, which can
    /// become invalid when concurrent tests delete their temp directories.
    /// `ShellState` no longer owns an embedder, so this is the only seam
    /// tests need.
    #[cfg(test)]
    pub(crate) fn new_isolated() -> Self {
        let dir = std::env::temp_dir().join(format!(".shell-test-{}", ulid::Ulid::new()));
        let state = ShellState::with_dir(dir).expect("Failed to initialize shell state");
        Self {
            state: Arc::new(Mutex::new(state)),
            mcp_server: None,
        }
    }
}

/// Build the pair of "Builtin config" + "Regex patterns" health checks.
///
/// Returns 1 check (builtin config failed to parse) or 2 (builtin config parsed,
/// plus regex compile status).
fn check_builtin_config(cat: &str) -> Vec<HealthCheck> {
    let config = match parse_shell_config(BUILTIN_CONFIG_YAML) {
        Ok(c) => c,
        Err(e) => {
            return vec![HealthCheck::error(
                "Builtin config",
                format!("Builtin shell config failed to parse: {}", e),
                Some("This is a binary bug — rebuild swissarmyhammer with a valid builtin/shell/config.yaml".to_string()),
                cat,
            )];
        }
    };
    let deny_count = config.deny.len();
    let permit_count = config.permit.len();
    let mut checks = vec![HealthCheck::ok(
        "Builtin config",
        format!(
            "Builtin shell config parsed successfully ({} deny patterns, {} permit patterns)",
            deny_count, permit_count
        ),
        cat,
    )];
    checks.push(match CompiledShellConfig::compile(&config) {
        Ok(_) => HealthCheck::ok(
            "Regex patterns",
            "All deny/permit regex patterns compile successfully",
            cat,
        ),
        Err(e) => HealthCheck::error(
            "Regex patterns",
            format!("Pattern '{}' failed to compile: {}", e.pattern, e.source),
            Some(format!(
                "Fix the invalid regex pattern '{}' in the shell config (reason: {})",
                e.pattern, e.reason
            )),
            cat,
        ),
    });
    checks
}

/// Check the optional user-level shell config at `~/.shell/config.yaml`.
///
/// Returns `None` when the home directory can't be resolved (rare — treat as
/// non-applicable); otherwise emits a single "User config" check.
fn check_user_config(cat: &str) -> Option<HealthCheck> {
    let home = dirs::home_dir()?;
    let path = home.join(".shell").join("config.yaml");
    if !path.exists() {
        return Some(HealthCheck::ok(
            "User config",
            format!("No user config at {} (optional)", path.display()),
            cat,
        ));
    }
    Some(check_config_file("User config", &path, cat))
}

/// Check the optional project-level shell config at `.shell/config.yaml`.
fn check_project_config(cat: &str) -> HealthCheck {
    let path = std::path::PathBuf::from(".shell").join("config.yaml");
    if !path.exists() {
        return HealthCheck::ok(
            "Project config",
            format!("No project config at {} (optional)", path.display()),
            cat,
        );
    }
    check_config_file("Project config", &path, cat)
}

/// Read a shell config YAML from `path`, parse it, and render a single check.
///
/// Shared between user-scope and project-scope configs because they differ
/// only in the display name.
fn check_config_file(check_name: &str, path: &std::path::Path, cat: &str) -> HealthCheck {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            return HealthCheck::warning(
                check_name,
                format!(
                    "{} at {} could not be read: {}",
                    check_name,
                    path.display(),
                    e
                ),
                Some(format!("Check file access on {}", path.display())),
                cat,
            );
        }
    };
    match parse_shell_config(&content) {
        Ok(config) => HealthCheck::ok(
            check_name,
            format!(
                "{} loaded from {} ({} deny, {} permit patterns)",
                check_name,
                path.display(),
                config.deny.len(),
                config.permit.len()
            ),
            cat,
        ),
        Err(e) => HealthCheck::error(
            check_name,
            format!(
                "{} at {} failed to parse: {}",
                check_name,
                path.display(),
                e
            ),
            Some(format!("Fix the YAML syntax in {}", path.display())),
            cat,
        ),
    }
}

// The legacy `Bash denied` and `Shell skill deployed` health checks were removed
// (kanban 01KSMXKZM1NZV1QH0SSKAP0V4P): both inspected only project-scope
// agent settings and produced false warnings under a user-scope install. Their
// concerns are now covered by mirdan's scope-aware install/status stack, which
// reports per-agent permission and skill rows. The installer side — the tool's
// `init` delegating the Bash deny to mirdan — is described on `init` below.

/// Create `.shell/config.yaml` from the builtin template when it doesn't exist.
///
/// Returns `Err` with a user-facing message if the directory or file cannot be
/// written. `reporter` is notified when the file is actually created.
fn ensure_project_config(
    reporter: &dyn swissarmyhammer_common::reporter::InitReporter,
) -> Result<(), String> {
    use swissarmyhammer_common::reporter::InitEvent;
    let shell_dir = std::path::PathBuf::from(".shell");
    let config_path = shell_dir.join("config.yaml");
    if config_path.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(&shell_dir)
        .map_err(|e| format!("Failed to create .shell/ directory: {}", e))?;
    std::fs::write(&config_path, BUILTIN_CONFIG_YAML)
        .map_err(|e| format!("Failed to write .shell/config.yaml: {}", e))?;
    reporter.emit(&InitEvent::Action {
        verb: "Created".to_string(),
        message: format!("{}", config_path.display()),
    });
    Ok(())
}

impl Doctorable for ShellExecuteTool {
    /// Returns the display name for this component in health check output.
    fn name(&self) -> &str {
        "Shell"
    }

    /// Returns the category for shell health checks.
    fn category(&self) -> &str {
        "tools"
    }

    /// Run health checks for the shell tool.
    ///
    /// Checks:
    /// - Builtin config parses successfully
    /// - All deny/permit regex patterns compile
    /// - User config (~/.shell/config.yaml) loads if present
    /// - Project config (.shell/config.yaml) loads if present
    ///
    /// Note: scope-aware checks for per-agent Bash denial and the deployed
    /// shell skill live in mirdan's install stack, not here.
    fn run_health_checks(&self) -> Vec<HealthCheck> {
        let cat = Doctorable::category(self);
        let mut checks = Vec::new();

        checks.extend(check_builtin_config(cat));
        if let Some(check) = check_user_config(cat) {
            checks.push(check);
        }
        checks.push(check_project_config(cat));

        checks
    }

    fn is_applicable(&self) -> bool {
        true
    }
}

/// Collect the first error message from an applier's results, if any.
///
/// The mirdan appliers return one `InitResult` per aggregate; surface an
/// error so the tool's `init`/`deinit` can abort like it did before.
fn applier_error(results: &[swissarmyhammer_common::lifecycle::InitResult]) -> Option<String> {
    use swissarmyhammer_common::lifecycle::InitStatus;
    results
        .iter()
        .find(|r| r.status == InitStatus::Error)
        .map(|r| r.message.clone())
}

impl swissarmyhammer_common::lifecycle::Initializable for ShellExecuteTool {
    /// Returns the display name for this component in lifecycle output.
    fn name(&self) -> &str {
        <Self as crate::mcp::tool_registry::McpTool>::name(self)
    }

    /// Returns the category for shell lifecycle operations.
    fn category(&self) -> &str {
        "tools"
    }

    /// Applies in all three scopes — User, Local, and Project.
    fn is_applicable(&self, scope: &swissarmyhammer_common::lifecycle::InitScope) -> bool {
        use swissarmyhammer_common::lifecycle::InitScope;
        matches!(
            scope,
            InitScope::User | InitScope::Local | InitScope::Project
        )
    }

    /// Initialize the shell tool. The tool DECLARES intent and DELEGATES all
    /// agent-specific config to mirdan:
    /// 1. Register the MCP server entry (if one was injected via
    ///    [`ShellExecuteTool::with_mcp_server`]) across detected agents via
    ///    [`mirdan::install::register_mcp_server`].
    /// 2. Deny the built-in `Bash` tool across detected agents via
    ///    [`mirdan::install::deny_tool`].
    /// 3. Create `.shell/config.yaml` from the builtin template — the tool's
    ///    own (non-agent) config, only for Project and Local scopes (a
    ///    User-scope install has no project dir).
    ///
    /// The `shelltool` CLI no longer injects an MCP server entry here: MCP
    /// registration and skill deployment now flow through the CLI's
    /// `mirdan::install::Profile`. `with_mcp_server` is retained for other
    /// embedders that still want the tool to own its registration.
    fn init(
        &self,
        scope: &swissarmyhammer_common::lifecycle::InitScope,
        reporter: &dyn swissarmyhammer_common::reporter::InitReporter,
    ) -> Vec<swissarmyhammer_common::lifecycle::InitResult> {
        use swissarmyhammer_common::lifecycle::{InitResult, InitScope};
        let component_name = <Self as crate::mcp::tool_registry::McpTool>::name(self);
        let mut results = Vec::new();

        if let Some((name, entry)) = &self.mcp_server {
            let mcp = mirdan::install::register_mcp_server(*scope, name, entry, reporter);
            if let Some(err) = applier_error(&mcp) {
                results.push(InitResult::error(component_name, err));
                return results;
            }
        }

        let deny = mirdan::install::deny_tool(*scope, "Bash", reporter);
        if let Some(err) = applier_error(&deny) {
            results.push(InitResult::error(component_name, err));
            return results;
        }

        if matches!(scope, InitScope::Project | InitScope::Local) {
            if let Err(err) = ensure_project_config(reporter) {
                results.push(InitResult::error(component_name, err));
                return results;
            }
        }

        results.push(InitResult::ok(
            component_name,
            "Shell tool initialized (MCP + Bash deny + config)",
        ));
        results
    }

    /// Deinitialize the shell tool, mirroring [`Self::init`] by delegating to
    /// mirdan:
    /// 1. Unregister the MCP server entry via
    ///    [`mirdan::install::unregister_mcp_server`].
    /// 2. Allow the `Bash` tool again via [`mirdan::install::allow_tool`].
    /// 3. Remove the `.shell/` config directory — only for Project and Local.
    ///
    /// As with `init`, the `shelltool` CLI drives MCP unregistration and skill
    /// removal through its `mirdan::install::Profile`, not this tool.
    fn deinit(
        &self,
        scope: &swissarmyhammer_common::lifecycle::InitScope,
        reporter: &dyn swissarmyhammer_common::reporter::InitReporter,
    ) -> Vec<swissarmyhammer_common::lifecycle::InitResult> {
        use swissarmyhammer_common::lifecycle::{InitResult, InitScope};
        let component_name = <Self as crate::mcp::tool_registry::McpTool>::name(self);
        let mut results = Vec::new();

        if let Some((name, _entry)) = &self.mcp_server {
            let mcp = mirdan::install::unregister_mcp_server(*scope, name, reporter);
            if let Some(err) = applier_error(&mcp) {
                results.push(InitResult::error(component_name, err));
            }
        }

        let allow = mirdan::install::allow_tool(*scope, "Bash", reporter);
        if let Some(err) = applier_error(&allow) {
            results.push(InitResult::error(component_name, err));
        }

        if matches!(scope, InitScope::Project | InitScope::Local) {
            if let Some(err) = remove_shell_dir(reporter) {
                results.push(InitResult::error(component_name, err));
            }
        }

        results.push(InitResult::ok(component_name, "Shell tool deinitialized"));
        results
    }
}

/// Remove the local `.shell/` directory if it exists.
///
/// Returns `Some(message)` when removal failed; otherwise emits a success
/// action to `reporter` and returns `None`.
fn remove_shell_dir(
    reporter: &dyn swissarmyhammer_common::reporter::InitReporter,
) -> Option<String> {
    use swissarmyhammer_common::reporter::InitEvent;
    let shell_dir = std::path::PathBuf::from(".shell");
    if !shell_dir.exists() {
        return None;
    }
    match std::fs::remove_dir_all(&shell_dir) {
        Ok(()) => {
            reporter.emit(&InitEvent::Action {
                verb: "Removed".to_string(),
                message: format!("{}", shell_dir.display()),
            });
            None
        }
        Err(e) => Some(format!("Failed to remove .shell/ directory: {}", e)),
    }
}

#[async_trait]
impl McpTool for ShellExecuteTool {
    fn name(&self) -> &'static str {
        "shell"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        let config = SchemaConfig::new(
            "Virtual shell with history and process management. Execute commands, grep output history, and manage running processes.",
        );
        generate_mcp_schema(&SHELL_OPERATIONS, config)
    }

    fn operations(&self) -> &'static [&'static dyn swissarmyhammer_operations::Operation] {
        let ops: &[&'static dyn Operation] = &SHELL_OPERATIONS;
        // SAFETY: SHELL_OPERATIONS is a static Lazy<Vec<...>> initialized once and lives for 'static
        unsafe {
            std::mem::transmute::<
                &[&dyn Operation],
                &'static [&'static dyn swissarmyhammer_operations::Operation],
            >(ops)
        }
    }

    fn category(&self) -> ToolCategory {
        // The virtual shell is an agent capability that supersedes a host's
        // native `Bash` tool.
        ToolCategory::Replacement { native: "Bash" }
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let op_str = arguments.get("op").and_then(|v| v.as_str()).unwrap_or("");
        tracing::info!(
            "shell op: {} args: {}",
            if op_str.is_empty() {
                "execute command"
            } else {
                op_str
            },
            serde_json::to_string(&arguments).unwrap_or_default()
        );

        // Strip op from arguments before parsing
        let mut args = arguments.clone();
        args.remove("op");

        match op_str {
            "execute command" | "" => {
                execute_command::run(args, self.state.clone(), _context).await
            }
            "list processes" => {
                list_processes::execute_list_processes(self.state.clone()).await
            }
            "kill process" => {
                kill_process::execute_kill_process(&args, self.state.clone()).await
            }
            "grep history" => {
                grep_history::execute_grep_history(&args, self.state.clone()).await
            }
            "get lines" => {
                get_lines::execute_get_lines(&args, self.state.clone()).await
            }
            other => Err(McpError::invalid_params(
                format!(
                    "Unknown operation '{}'. Valid operations: execute command, list processes, kill process, grep history, get lines",
                    other
                ),
                None,
            )),
        }
    }
}

use crate::mcp::tool_registry::ToolRegistry;

/// Register all shell-related tools with the registry
///
/// This function registers all shell command execution tools following the
/// SwissArmyHammer tool registry pattern. Currently includes:
///
/// - `shell_execute`: Execute shell commands with timeout and environment control
///
/// # Arguments
///
/// * `registry` - The tool registry to register shell tools with
///
/// # Example
///
/// ```rust,ignore
/// use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
/// use swissarmyhammer_tools::mcp::tools::shell::register_shell_tools;
///
/// let mut registry = ToolRegistry::new();
/// register_shell_tools(&mut registry);
/// ```
pub fn register_shell_tools(registry: &mut ToolRegistry) {
    registry.register(ShellExecuteTool::new());
}

/// Test-only variant that uses isolated temp dirs instead of CWD.
#[cfg(test)]
fn register_shell_tools_isolated(registry: &mut ToolRegistry) {
    registry.register(ShellExecuteTool::new_isolated());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_registry::ToolRegistry;

    // Import test helpers
    use test_helpers::execute_op;

    // =====================================================================
    // Registration tests
    // =====================================================================

    #[tokio::test]
    async fn test_register_shell_tools() {
        let mut registry = ToolRegistry::new();
        register_shell_tools_isolated(&mut registry);

        // Verify shell_execute tool is registered
        assert!(registry.get_tool("shell").is_some());
        assert_eq!(registry.len(), 1);
    }

    #[tokio::test]
    async fn test_shell_tools_properties() {
        let mut registry = ToolRegistry::new();
        register_shell_tools_isolated(&mut registry);

        let tools = registry.list_tools();
        assert_eq!(tools.len(), 1);

        let shell_execute_tool = tools
            .iter()
            .find(|tool| tool.name == "shell")
            .expect("shell_execute tool should be registered");

        assert_eq!(shell_execute_tool.name, "shell");
        assert!(shell_execute_tool.description.is_some());
        assert!(!shell_execute_tool.input_schema.is_empty());
    }

    // Per-agent settings-file resolution lives in mirdan's strategy layer.

    #[tokio::test]
    async fn test_multiple_registrations() {
        let mut registry = ToolRegistry::new();

        // Register twice to ensure no conflicts
        register_shell_tools_isolated(&mut registry);
        register_shell_tools_isolated(&mut registry);

        // Should have only one tool (second registration overwrites)
        assert_eq!(registry.len(), 1);
        assert!(registry.get_tool("shell").is_some());
    }

    #[tokio::test]
    async fn test_shell_tool_name_uniqueness() {
        let mut registry = ToolRegistry::new();
        register_shell_tools_isolated(&mut registry);

        let tool_names = registry.list_tool_names();
        let unique_names: std::collections::HashSet<_> = tool_names.iter().collect();

        // All tool names should be unique
        assert_eq!(tool_names.len(), unique_names.len());
    }

    // =====================================================================
    // Tool property tests
    // =====================================================================

    #[tokio::test]
    async fn test_shell_tool_has_operations() {
        let tool = ShellExecuteTool::new_isolated();
        let ops = tool.operations();
        assert_eq!(ops.len(), 5);
        assert!(ops.iter().any(|o| o.op_string() == "execute command"));
        assert!(ops.iter().any(|o| o.op_string() == "list processes"));
        assert!(ops.iter().any(|o| o.op_string() == "kill process"));
        assert!(ops.iter().any(|o| o.op_string() == "grep history"));
        assert!(ops.iter().any(|o| o.op_string() == "get lines"));
    }

    #[tokio::test]
    async fn test_tool_properties() {
        let tool = ShellExecuteTool::new_isolated();
        assert_eq!(McpTool::name(&tool), "shell");
        assert!(!tool.description().is_empty());

        let schema = tool.schema();
        assert!(schema.is_object());
        assert!(schema["properties"]["command"].is_object());
        assert!(schema["properties"]["op"].is_object());
        assert!(schema["x-operation-schemas"].is_array());
        assert!(schema["x-operation-groups"].is_object());
    }

    // =====================================================================
    // Tests for unknown operations
    // =====================================================================

    #[tokio::test]
    async fn test_unknown_operation_returns_error() {
        let result = execute_op("bogus operation", vec![]).await;
        assert!(result.is_err(), "Unknown operation should fail");
        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(
            err_str.contains("bogus operation"),
            "Error should echo the bad op: {}",
            err_str
        );
        assert!(
            err_str.contains("execute command"),
            "Error should list valid operations: {}",
            err_str
        );
    }

    #[test]
    fn test_category_is_replacement_for_bash() {
        let tool = ShellExecuteTool::new_isolated();
        assert_eq!(
            McpTool::category(&tool),
            ToolCategory::Replacement { native: "Bash" }
        );
    }

    // =====================================================================
    // Health check (Doctorable) tests
    // =====================================================================

    #[tokio::test]
    async fn test_doctorable_name_and_category() {
        let tool = ShellExecuteTool::new_isolated();
        assert_eq!(
            swissarmyhammer_common::health::Doctorable::name(&tool),
            "Shell"
        );
        assert_eq!(
            swissarmyhammer_common::health::Doctorable::category(&tool),
            "tools"
        );
    }

    #[tokio::test]
    async fn test_doctorable_is_applicable() {
        let tool = ShellExecuteTool::new_isolated();
        assert!(swissarmyhammer_common::health::Doctorable::is_applicable(
            &tool
        ));
    }

    #[tokio::test]
    async fn test_health_checks_returns_nonempty() {
        let tool = ShellExecuteTool::new_isolated();
        let checks = tool.run_health_checks();
        assert!(
            !checks.is_empty(),
            "Should return at least some health checks"
        );
    }

    #[tokio::test]
    async fn test_builtin_config_check_passes() {
        use swissarmyhammer_common::health::HealthStatus;

        let tool = ShellExecuteTool::new_isolated();
        let checks = tool.run_health_checks();

        let builtin_check = checks
            .iter()
            .find(|c| c.name == "Builtin config")
            .expect("Should have a 'Builtin config' health check");
        assert_eq!(
            builtin_check.status,
            HealthStatus::Ok,
            "Builtin config should parse successfully: {}",
            builtin_check.message
        );
    }

    #[tokio::test]
    async fn test_regex_patterns_check_passes() {
        use swissarmyhammer_common::health::HealthStatus;

        let tool = ShellExecuteTool::new_isolated();
        let checks = tool.run_health_checks();

        let regex_check = checks
            .iter()
            .find(|c| c.name == "Regex patterns")
            .expect("Should have a 'Regex patterns' health check");
        assert_eq!(
            regex_check.status,
            HealthStatus::Ok,
            "All regex patterns should compile: {}",
            regex_check.message
        );
    }

    #[tokio::test]
    async fn test_health_checks_all_have_category() {
        let tool = ShellExecuteTool::new_isolated();
        let checks = tool.run_health_checks();

        for check in &checks {
            assert_eq!(
                check.category, "tools",
                "Check '{}' should have category 'tools'",
                check.name
            );
        }
    }

    #[tokio::test]
    async fn test_unknown_operation_lists_all_valid_ops() {
        let result = execute_op("not a real op", vec![]).await;
        let err = result.unwrap_err();
        let err_str = err.to_string();

        // Should list all valid operations
        for expected_op in &[
            "execute command",
            "list processes",
            "kill process",
            "grep history",
            "get lines",
        ] {
            assert!(
                err_str.contains(expected_op),
                "Error should list '{}': {}",
                expected_op,
                err_str
            );
        }
    }

    // =====================================================================
    // Initializable tests
    // =====================================================================

    use swissarmyhammer_common::lifecycle::{InitScope, Initializable};
    use swissarmyhammer_common::reporter::NullReporter;

    #[tokio::test]
    async fn test_initializable_name_and_category() {
        let tool = ShellExecuteTool::new_isolated();
        assert_eq!(Initializable::name(&tool), "shell");
        assert_eq!(Initializable::category(&tool), "tools");
    }

    #[tokio::test]
    async fn test_initializable_is_applicable_project_scope() {
        let tool = ShellExecuteTool::new_isolated();
        assert!(
            Initializable::is_applicable(&tool, &InitScope::Project),
            "Should be applicable for Project scope"
        );
    }

    #[tokio::test]
    async fn test_initializable_is_applicable_local_scope() {
        let tool = ShellExecuteTool::new_isolated();
        assert!(
            Initializable::is_applicable(&tool, &InitScope::Local),
            "Should be applicable for Local scope"
        );
    }

    #[tokio::test]
    async fn test_initializable_applicable_user_scope() {
        let tool = ShellExecuteTool::new_isolated();
        assert!(
            Initializable::is_applicable(&tool, &InitScope::User),
            "Should be applicable for User scope"
        );
    }

    #[tokio::test]
    async fn test_init_creates_shell_config() {
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = CurrentDirGuard::new(tmp.path()).unwrap();

        let tool = ShellExecuteTool::new_isolated();
        let reporter = NullReporter;
        // init may fail on skill deployment (no agents configured in test env), but
        // the config file should be created before that step
        let _results = Initializable::init(&tool, &InitScope::Project, &reporter);

        let config_path = tmp.path().join(".shell").join("config.yaml");
        assert!(
            config_path.exists(),
            ".shell/config.yaml should be created by init"
        );
        let content = std::fs::read_to_string(&config_path).unwrap();
        assert!(!content.is_empty(), "config.yaml should not be empty");
    }

    #[tokio::test]
    async fn test_init_creates_shell_config_idempotent() {
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = CurrentDirGuard::new(tmp.path()).unwrap();

        let tool = ShellExecuteTool::new_isolated();
        let reporter = NullReporter;
        // Run init twice — should not fail or overwrite
        let _ = Initializable::init(&tool, &InitScope::Project, &reporter);
        let _ = Initializable::init(&tool, &InitScope::Project, &reporter);

        let config_path = tmp.path().join(".shell").join("config.yaml");
        assert!(config_path.exists());
    }

    // Bash deny/allow at each scope is now owned by mirdan's per-agent
    // strategies and exercised by the scope-aware lifecycle tests below
    // (which inject a synthetic claude-code agent). The tool no longer writes
    // settings files directly.

    #[tokio::test]
    async fn test_deinit_removes_shell_dir() {
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = CurrentDirGuard::new(tmp.path()).unwrap();

        let tool = ShellExecuteTool::new_isolated();
        let reporter = NullReporter;

        // Create .shell/config.yaml manually
        let shell_dir = tmp.path().join(".shell");
        std::fs::create_dir_all(&shell_dir).unwrap();
        std::fs::write(shell_dir.join("config.yaml"), "test").unwrap();

        let _ = Initializable::deinit(&tool, &InitScope::Project, &reporter);

        assert!(
            !shell_dir.exists(),
            ".shell/ directory should be removed by deinit"
        );
    }

    // =====================================================================
    // dispatch: execute() routes to each operation handler
    // =====================================================================

    /// Test that execute() dispatches "execute command" (empty op string) correctly
    #[tokio::test]
    async fn test_dispatch_execute_command_empty_op() {
        let result = execute_op(
            "",
            vec![("command", serde_json::json!("echo dispatch_test"))],
        )
        .await;
        assert!(
            result.is_ok(),
            "empty op should dispatch to execute command: {:?}",
            result.err()
        );
        let text = test_helpers::extract_text(&result.unwrap());
        assert!(
            text.contains("command_id"),
            "response should contain command_id: {}",
            text
        );
    }

    /// Test that execute() dispatches "list processes" correctly
    #[tokio::test]
    async fn test_dispatch_list_processes() {
        let result = execute_op("list processes", vec![]).await;
        assert!(
            result.is_ok(),
            "list processes dispatch should succeed: {:?}",
            result.err()
        );
    }

    /// Test that execute() dispatches "kill process" to the handler (wrong id = error)
    #[tokio::test]
    async fn test_dispatch_kill_process_invalid_id() {
        let result = execute_op("kill process", vec![("id", serde_json::json!(99999))]).await;
        // Should fail with invalid ID, not with "Unknown operation"
        assert!(
            result.is_err(),
            "kill process with nonexistent id should fail"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            !err.contains("Unknown operation"),
            "Error should not be 'Unknown operation': {}",
            err
        );
    }

    /// Test that execute() dispatches "grep history" correctly
    #[tokio::test]
    async fn test_dispatch_grep_history() {
        let result = execute_op(
            "grep history",
            vec![("pattern", serde_json::json!("DISPATCH_GREP_TEST"))],
        )
        .await;
        assert!(
            result.is_ok(),
            "grep history dispatch should succeed: {:?}",
            result.err()
        );
    }

    /// Test that execute() dispatches "get lines" to handler (wrong id = empty response)
    #[tokio::test]
    async fn test_dispatch_get_lines() {
        let result = execute_op("get lines", vec![("command_id", serde_json::json!(99999))]).await;
        assert!(
            result.is_ok(),
            "get lines dispatch should succeed: {:?}",
            result.err()
        );
        let text = test_helpers::extract_text(&result.unwrap());
        assert!(
            text.contains("No output lines"),
            "Should return empty result: {}",
            text
        );
    }

    // =====================================================================
    // Health check coverage: branches for config files and settings
    // =====================================================================

    /// Test health check when a project config exists with valid YAML
    #[tokio::test]
    async fn test_health_check_project_config_valid() {
        use swissarmyhammer_common::health::HealthStatus;
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = CurrentDirGuard::new(tmp.path()).unwrap();

        // Create a valid .shell/config.yaml with proper PatternRule format
        let shell_dir = tmp.path().join(".shell");
        std::fs::create_dir_all(&shell_dir).unwrap();
        std::fs::write(
            shell_dir.join("config.yaml"),
            "deny:\n  - pattern: \"rm.*-rf\"\n    reason: \"Prevent recursive deletion\"\npermit: []\n",
        ).unwrap();

        let tool = ShellExecuteTool::new_isolated();
        let checks = tool.run_health_checks();

        let project_check = checks.iter().find(|c| c.name == "Project config");
        assert!(
            project_check.is_some(),
            "Should have a Project config check"
        );
        assert_eq!(
            project_check.unwrap().status,
            HealthStatus::Ok,
            "Valid project config should produce Ok status"
        );
    }

    /// Test health check when a project config exists with invalid YAML
    #[tokio::test]
    async fn test_health_check_project_config_invalid_yaml() {
        use swissarmyhammer_common::health::HealthStatus;
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = CurrentDirGuard::new(tmp.path()).unwrap();

        // Create an invalid .shell/config.yaml
        let shell_dir = tmp.path().join(".shell");
        std::fs::create_dir_all(&shell_dir).unwrap();
        std::fs::write(
            shell_dir.join("config.yaml"),
            "this: is: not: valid: yaml: {{{",
        )
        .unwrap();

        let tool = ShellExecuteTool::new_isolated();
        let checks = tool.run_health_checks();

        let project_check = checks.iter().find(|c| c.name == "Project config");
        assert!(
            project_check.is_some(),
            "Should have a Project config check"
        );
        assert_eq!(
            project_check.unwrap().status,
            HealthStatus::Error,
            "Invalid project config should produce Error status"
        );
    }

    // Init/deinit edge cases for per-agent settings files (already-denied,
    // empty, invalid, missing) are owned by mirdan's strategy + settings layers
    // and tested there. The tool only declares intent and delegates.

    // =====================================================================
    // Scope-aware lifecycle: MCP registration + Bash deny + config dir
    //
    // These drive the tool's full install lifecycle across User/Local/Project.
    // They mutate process-global HOME, CWD, and the `MIRDAN_AGENTS_CONFIG`
    // env var, so each joins the `cwd` + `env` serial groups and pins HOME to
    // an isolated env. The synthetic agents.yaml injects a single Claude-like
    // agent whose MCP configs live under the project dir / isolated home.
    // =====================================================================

    use serial_test::serial;
    use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};

    /// RAII guard restoring `MIRDAN_AGENTS_CONFIG` on drop.
    struct MirdanConfigGuard {
        original: Option<String>,
    }

    impl MirdanConfigGuard {
        fn set(path: &std::path::Path) -> Self {
            let original = std::env::var("MIRDAN_AGENTS_CONFIG").ok();
            std::env::set_var("MIRDAN_AGENTS_CONFIG", path);
            Self { original }
        }
    }

    impl Drop for MirdanConfigGuard {
        fn drop(&mut self) {
            match &self.original {
                Some(v) => std::env::set_var("MIRDAN_AGENTS_CONFIG", v),
                None => std::env::remove_var("MIRDAN_AGENTS_CONFIG"),
            }
        }
    }

    /// Build the tool wired with the `shelltool` MCP server entry, matching
    /// how the CLI constructs it.
    fn tool_with_shelltool_server() -> ShellExecuteTool {
        ShellExecuteTool::new_isolated().with_mcp_server(
            "shelltool",
            mirdan::mcp_config::McpServerEntry {
                command: "shelltool".to_string(),
                args: vec!["serve".to_string()],
                env: std::collections::BTreeMap::new(),
            },
        )
    }

    /// Write a synthetic single-agent config whose id is `claude-code` so the
    /// real ClaudeCodeStrategy fires, with neutral agent-config MCP and
    /// settings paths so this test asserts on the strategy's behavior, not on
    /// any literal Claude path. Detection always fires (the detect dir is
    /// `project_dir`).
    ///
    /// `settings_dir` is the directory under which the agent's project settings
    /// file lives; the ClaudeCodeStrategy derives the local-scope sibling
    /// (`settings.local.json`) from it.
    fn write_agents_config(
        project_dir: &std::path::Path,
        global_mcp: &std::path::Path,
        global_settings: &std::path::Path,
    ) -> std::path::PathBuf {
        let agents_yaml = format!(
            r#"agents:
  - id: claude-code
    name: Claude Code
    project_path: .fake/skills
    global_path: "~/.fake/skills"
    detect:
      - dir: "{detect}"
    settings_path: agent-config/settings.json
    global_settings_path: "{global_settings}"
    mcp_config:
      project_path: .mcp.json
      global_path: "{global_mcp}"
      servers_key: mcpServers
"#,
            detect = project_dir.display(),
            global_mcp = global_mcp.display(),
            global_settings = global_settings.display(),
        );
        let config_path = project_dir.join("agents.yaml");
        std::fs::write(&config_path, agents_yaml).expect("write agents.yaml");
        config_path
    }

    /// Whether the JSON settings file at `path` lists the `Bash` tool as denied.
    ///
    /// Reads the raw file and looks for the `Bash` token rather than walking the
    /// deny-array pointer, keeping the shell tool's tests free of Claude
    /// settings-shape literals (that shape is mirdan's concern).
    fn bash_denied(path: &std::path::Path) -> bool {
        std::fs::read_to_string(path)
            .map(|c| c.contains("\"Bash\""))
            .unwrap_or(false)
    }

    /// User scope: the agent's global settings file gains/loses Bash, the
    /// agent's global MCP config gains/loses the `shelltool` entry, and NO
    /// `.shell/` dir is created.
    #[tokio::test]
    #[serial(cwd, env)]
    async fn test_tool_lifecycle_user_scope() {
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let home = env.home_path();
        let _cwd = CurrentDirGuard::new(&home).expect("chdir into isolated home");
        let global_mcp = home.join("agent-global-mcp.json");
        let global_settings = home.join("agent-global-settings.json");
        let config_path = write_agents_config(&home, &global_mcp, &global_settings);
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let tool = tool_with_shelltool_server();
        let reporter = NullReporter;
        let _ = Initializable::init(&tool, &InitScope::User, &reporter);

        assert!(
            bash_denied(&global_settings),
            "Bash should be denied at user scope"
        );

        let global: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&global_mcp).unwrap()).unwrap();
        assert_eq!(global["mcpServers"]["shelltool"]["command"], "shelltool");

        assert!(
            !home.join(".shell").exists(),
            "user scope must not create a .shell/ dir"
        );

        let _ = Initializable::deinit(&tool, &InitScope::User, &reporter);
        assert!(
            !bash_denied(&global_settings),
            "Bash should be removed at user scope"
        );
        let global_after: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&global_mcp).unwrap()).unwrap();
        assert!(
            global_after["mcpServers"]["shelltool"].is_null(),
            "shelltool entry should be removed from global config"
        );
    }

    /// Local scope: the agent's `settings.local.json` sibling denies Bash and
    /// loses it on deinit, and NO `.shell/` dir leaks outside Project|Local
    /// gating (Local does create one). The local-scope MCP registration +
    /// empty-map prune is covered by mirdan's strategy tests; this test asserts
    /// the tool's delegation drives the per-scope settings sibling.
    #[tokio::test]
    #[serial(cwd, env)]
    async fn test_tool_lifecycle_local_scope() {
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let home = env.home_path();
        let project = home.join("proj");
        std::fs::create_dir_all(&project).unwrap();
        let _cwd = CurrentDirGuard::new(&project).expect("chdir into project");
        let global_mcp = home.join("agent-global-mcp.json");
        let global_settings = home.join("agent-global-settings.json");
        let config_path = write_agents_config(&project, &global_mcp, &global_settings);
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let tool = tool_with_shelltool_server();
        let reporter = NullReporter;
        let _ = Initializable::init(&tool, &InitScope::Local, &reporter);

        // The strategy derives the local settings sibling from the agent's
        // project settings file: agent-config/settings.local.json.
        let local_settings = project.join("agent-config").join("settings.local.json");
        assert!(
            bash_denied(&local_settings),
            "Bash should be denied at local scope"
        );

        let _ = Initializable::deinit(&tool, &InitScope::Local, &reporter);
        assert!(
            !bash_denied(&local_settings),
            "Bash should be removed at local scope"
        );
    }

    /// Project scope: the project MCP file gets the shelltool entry, the
    /// agent's project settings file denies Bash, and `.shell/config.yaml` is
    /// created.
    #[tokio::test]
    #[serial(cwd, env)]
    async fn test_tool_lifecycle_project_scope() {
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let home = env.home_path();
        let project = home.join("proj");
        std::fs::create_dir_all(&project).unwrap();
        let _cwd = CurrentDirGuard::new(&project).expect("chdir into project");
        let global_mcp = home.join("agent-global-mcp.json");
        let global_settings = home.join("agent-global-settings.json");
        let config_path = write_agents_config(&project, &global_mcp, &global_settings);
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let tool = tool_with_shelltool_server();
        let reporter = NullReporter;
        let _ = Initializable::init(&tool, &InitScope::Project, &reporter);

        let mcp_json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(project.join(".mcp.json")).unwrap())
                .unwrap();
        assert_eq!(mcp_json["mcpServers"]["shelltool"]["command"], "shelltool");

        let project_settings = project.join("agent-config").join("settings.json");
        assert!(
            bash_denied(&project_settings),
            "Bash should be denied at project scope"
        );

        let config_yaml = project.join(".shell").join("config.yaml");
        assert!(config_yaml.exists(), ".shell/config.yaml should be created");
    }
}
