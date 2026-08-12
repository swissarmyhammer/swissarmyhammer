//! Shell tool for MCP — virtual shell with history and process management.
//!
//! ## Operations
//!
//! Dispatches between five operations:
//! - `execute command`: Run a shell command with timeout and output capture.
//!   The response includes the last 32 output lines (or the full output when
//!   it is 32 lines or fewer); use `get lines` to retrieve the rest.
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
//! line boundaries. The output of a command that exits is stored in
//! [`ShellState`](state::ShellState) for later retrieval via `get lines` or
//! `grep history`. A command that the timeout kills stores no output, because
//! only the completion path writes to the log.
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

use crate::mcp::lifecycle_utils::applier_error;
use crate::mcp::tool_registry::{McpTool, ToolCategory, ToolContext};
use anyhow::Context;
use async_trait::async_trait;
use once_cell::sync::Lazy;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use std::sync::Arc;
use swissarmyhammer_common::health::{Doctorable, HealthCheck};
use swissarmyhammer_directory::{DirectoryConfig, ShellConfig};
use swissarmyhammer_operations::{
    generate_mcp_schema_full, generate_mcp_schema_wire, Operation, SchemaConfig,
};
use swissarmyhammer_shell::config::{parse_shell_config, CompiledShellConfig, BUILTIN_CONFIG_YAML};
use tokio::sync::Mutex;

use state::ShellState;

/// Name of the shell tool's config file inside the shell directory
/// ([`ShellConfig::DIR_NAME`]) — `~/.shell/config.yaml` at user scope and
/// `.shell/config.yaml` at project scope.
const SHELL_CONFIG_FILE: &str = "config.yaml";

/// Argument key that names the operation to run.
const OP_KEY: &str = "op";

/// Message reported when the shell state cannot open its log directory.
const SHELL_STATE_INIT_FAILED: &str = "failed to initialize shell state";

/// Name of the host's native tool that this tool supersedes. `init` denies it
/// per agent, `deinit` allows it again, and the tool category names it.
const BASH_TOOL_NAME: &str = "Bash";

/// Health check name for the config compiled into the binary.
const BUILTIN_CONFIG_CHECK: &str = "Builtin config";

/// Health check name for the deny/permit regex compile status.
const REGEX_PATTERNS_CHECK: &str = "Regex patterns";

/// Health check name for the user-scope config.
const USER_CONFIG_CHECK: &str = "User config";

/// Health check name for the project-scope config.
const PROJECT_CONFIG_CHECK: &str = "Project config";

/// Category the shell tool reports for both health checks and lifecycle
/// results.
const SHELL_TOOL_CATEGORY: &str = "tools";

/// Operation string that runs a shell command. The tool also runs it when the
/// caller sends no `op`.
const EXECUTE_COMMAND_OP: &str = "execute command";

/// Operation string that lists every command this session has run.
const LIST_PROCESSES_OP: &str = "list processes";

/// Operation string that stops a running command by id.
const KILL_PROCESS_OP: &str = "kill process";

/// Operation string that searches stored command output by regex.
const GREP_HISTORY_OP: &str = "grep history";

/// Operation string that reads stored output lines back by command id.
const GET_LINES_OP: &str = "get lines";

// Static operation instances for schema generation
static EXECUTE_CMD: Lazy<execute_command::ExecuteCommand> =
    Lazy::new(execute_command::ExecuteCommand::default);
static LIST_PROCS: Lazy<list_processes::ListProcesses> =
    Lazy::new(list_processes::ListProcesses::default);
static KILL_PROC: Lazy<kill_process::KillProcess> = Lazy::new(kill_process::KillProcess::default);
static GREP_HIST: Lazy<grep_history::GrepHistory> = Lazy::new(grep_history::GrepHistory::default);
static GET_LNS: Lazy<get_lines::GetLines> = Lazy::new(get_lines::GetLines::default);

/// Static registry of every operation the `shell` tool supports — `execute
/// command`, `list processes`, `kill process`, `grep history`, and `get
/// lines`.
///
/// It is the single source of truth for the tool's operation set: schema
/// generation, [`McpTool::operations`], and the unknown-operation error
/// message all read it, so adding an operation here is enough for all three.
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
#[derive(Clone, Debug)]
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

impl ShellExecuteTool {
    /// Creates a new instance of the `ShellExecuteTool`.
    ///
    /// The state lives in a shell directory ([`ShellConfig::DIR_NAME`]) under
    /// the current directory, falling back to a temp directory when that
    /// location is not writable. Command output is appended to a log file
    /// there, so `get lines` and `grep history` can read it back later.
    ///
    /// The tool has no [`Default`], because a constructor that reads the
    /// filesystem cannot answer `Self` on its own.
    ///
    /// # Errors
    ///
    /// Reports a shell state it could not initialize, when neither the shell
    /// directory under the current directory nor the temp fallback can be
    /// created — a read-only or missing location, or a permission the process
    /// does not hold. That is an expected failure of the environment rather
    /// than a broken invariant, so the caller decides what to do about it.
    pub fn new() -> anyhow::Result<Self> {
        let state = ShellState::new().context(SHELL_STATE_INIT_FAILED)?;
        Ok(Self {
            state: Arc::new(Mutex::new(state)),
            mcp_server: None,
        })
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
        let dir = std::env::temp_dir().join(format!(
            "{}-test-{}",
            ShellConfig::DIR_NAME,
            ulid::Ulid::new()
        ));
        let state = ShellState::with_dir(dir).expect(SHELL_STATE_INIT_FAILED);
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
                BUILTIN_CONFIG_CHECK,
                format!("builtin shell config failed to parse: {}", e),
                Some(format!(
                    "This is a binary bug — rebuild swissarmyhammer with a valid builtin/shell/{}",
                    SHELL_CONFIG_FILE
                )),
                cat,
            )];
        }
    };
    let deny_count = config.deny.len();
    let permit_count = config.permit.len();
    let mut checks = vec![HealthCheck::ok(
        BUILTIN_CONFIG_CHECK,
        format!(
            "Builtin shell config parsed successfully ({} deny patterns, {} permit patterns)",
            deny_count, permit_count
        ),
        cat,
    )];
    checks.push(match CompiledShellConfig::compile(&config) {
        Ok(_) => HealthCheck::ok(
            REGEX_PATTERNS_CHECK,
            "All deny/permit regex patterns compile successfully",
            cat,
        ),
        Err(e) => HealthCheck::error(
            REGEX_PATTERNS_CHECK,
            format!("pattern '{}' failed to compile: {}", e.pattern, e.source),
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
    let path = home.join(ShellConfig::DIR_NAME).join(SHELL_CONFIG_FILE);
    if !path.exists() {
        return Some(HealthCheck::ok(
            USER_CONFIG_CHECK,
            format!("No user config at {} (optional)", path.display()),
            cat,
        ));
    }
    Some(check_config_file(USER_CONFIG_CHECK, &path, cat))
}

/// Check the optional project-level shell config at `.shell/config.yaml`.
fn check_project_config(cat: &str) -> HealthCheck {
    let path = std::path::PathBuf::from(ShellConfig::DIR_NAME).join(SHELL_CONFIG_FILE);
    if !path.exists() {
        return HealthCheck::ok(
            PROJECT_CONFIG_CHECK,
            format!("No project config at {} (optional)", path.display()),
            cat,
        );
    }
    check_config_file(PROJECT_CONFIG_CHECK, &path, cat)
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
    let shell_dir = std::path::PathBuf::from(ShellConfig::DIR_NAME);
    let config_path = shell_dir.join(SHELL_CONFIG_FILE);
    if config_path.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(&shell_dir).map_err(|e| {
        format!(
            "failed to create {}/ directory: {}",
            ShellConfig::DIR_NAME,
            e
        )
    })?;
    std::fs::write(&config_path, BUILTIN_CONFIG_YAML).map_err(|e| {
        format!(
            "failed to write {}/{}: {}",
            ShellConfig::DIR_NAME,
            SHELL_CONFIG_FILE,
            e
        )
    })?;
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
        SHELL_TOOL_CATEGORY
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

    /// Returns whether the shell health checks apply. They always do — the
    /// builtin config is compiled in, so there is nothing to detect first.
    fn is_applicable(&self) -> bool {
        true
    }
}

/// The direction one lifecycle run takes.
///
/// `init` and `deinit` walk the same three steps over the same scope, in the
/// same order, and each step differs only in which mirdan call it makes. This
/// names that difference so one function runs both directions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LifecycleDirection {
    /// Registers the MCP server entry, denies `Bash`, and writes the project
    /// config.
    Install,
    /// Unregisters the MCP server entry, allows `Bash` again, and removes the
    /// project config.
    Remove,
}

impl LifecycleDirection {
    /// The message a run reports when no step failed.
    fn success_message(self) -> &'static str {
        match self {
            Self::Install => "Shell tool initialized (MCP + Bash deny + config)",
            Self::Remove => "Shell tool deinitialized",
        }
    }

    /// Whether a failed step ends the run.
    ///
    /// An install stops, because each step stands on the one before it. A
    /// removal carries on, so it takes away everything it still can.
    fn stops_at_first_error(self) -> bool {
        matches!(self, Self::Install)
    }
}

/// Runs one direction of the shell tool's lifecycle over `scope`.
///
/// The three steps — the MCP server entry, the `Bash` permission, and the
/// tool's own `.shell/` config — are the same for both directions, so `init`
/// and `deinit` differ only in the `direction` they hand in.
///
/// A step answers `None` when it succeeded and `Some(message)` when it failed.
/// Each failure becomes an error result, and a run with no failed step ends
/// with the direction's own success message.
fn run_lifecycle(
    tool: &ShellExecuteTool,
    direction: LifecycleDirection,
    scope: &swissarmyhammer_common::lifecycle::InitScope,
    reporter: &dyn swissarmyhammer_common::reporter::InitReporter,
) -> Vec<swissarmyhammer_common::lifecycle::InitResult> {
    use swissarmyhammer_common::lifecycle::{InitResult, InitScope};

    let mcp_server = || -> Option<String> {
        let (name, entry) = tool.mcp_server.as_ref()?;
        applier_error(&match direction {
            LifecycleDirection::Install => {
                mirdan::install::register_mcp_server(*scope, name, entry, reporter)
            }
            LifecycleDirection::Remove => {
                mirdan::install::unregister_mcp_server(*scope, name, reporter)
            }
        })
    };

    let bash_permission = || -> Option<String> {
        applier_error(&match direction {
            LifecycleDirection::Install => {
                mirdan::install::deny_tool(*scope, BASH_TOOL_NAME, reporter)
            }
            LifecycleDirection::Remove => {
                mirdan::install::allow_tool(*scope, BASH_TOOL_NAME, reporter)
            }
        })
    };

    let project_config = || -> Option<String> {
        if !matches!(scope, InitScope::Project | InitScope::Local) {
            return None;
        }
        match direction {
            LifecycleDirection::Install => ensure_project_config(reporter).err(),
            LifecycleDirection::Remove => remove_shell_dir(reporter),
        }
    };

    let component_name = <ShellExecuteTool as crate::mcp::tool_registry::McpTool>::name(tool);
    let mut results = Vec::new();
    for step in [
        &mcp_server as &dyn Fn() -> Option<String>,
        &bash_permission,
        &project_config,
    ] {
        if let Some(err) = step() {
            results.push(InitResult::error(component_name, err));
            if direction.stops_at_first_error() {
                return results;
            }
        }
    }
    results.push(InitResult::ok(component_name, direction.success_message()));
    results
}

impl swissarmyhammer_common::lifecycle::Initializable for ShellExecuteTool {
    /// Returns the display name for this component in lifecycle output.
    fn name(&self) -> &str {
        <Self as crate::mcp::tool_registry::McpTool>::name(self)
    }

    /// Returns the category for shell lifecycle operations.
    fn category(&self) -> &str {
        SHELL_TOOL_CATEGORY
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
        run_lifecycle(self, LifecycleDirection::Install, scope, reporter)
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
        run_lifecycle(self, LifecycleDirection::Remove, scope, reporter)
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
    let shell_dir = std::path::PathBuf::from(ShellConfig::DIR_NAME);
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
        Err(e) => Some(format!(
            "failed to remove {}/ directory: {}",
            ShellConfig::DIR_NAME,
            e
        )),
    }
}

/// Shared schema config for the shell tool, so the wire and full generators
/// stay in lockstep on the description.
fn shell_schema_config() -> SchemaConfig {
    SchemaConfig::new(
        "Virtual shell with history and process management. Execute commands, grep output history, and manage running processes.",
    )
}

#[async_trait]
impl McpTool for ShellExecuteTool {
    /// Returns the wire name of the tool: `"shell"`.
    fn name(&self) -> &'static str {
        "shell"
    }

    /// Returns the agent-facing tool description, read from `description.md`
    /// at compile time.
    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    /// Returns the wire schema — a single `op` property, with the heavy
    /// CLI-facing keys dropped.
    fn schema(&self) -> serde_json::Value {
        generate_mcp_schema_wire(&SHELL_OPERATIONS, shell_schema_config())
    }

    /// Returns the full schema — flat per-operation properties plus the
    /// CLI-facing operation schemas, groups, and signatures.
    fn schema_full(&self) -> serde_json::Value {
        generate_mcp_schema_full(&SHELL_OPERATIONS, shell_schema_config())
    }

    /// Returns every operation the tool supports, taken from
    /// [`SHELL_OPERATIONS`].
    ///
    /// The registry uses the list to build per-operation CLI subcommands and
    /// help text. The transmute below only re-states the `'static` lifetime
    /// that the `Lazy` static already guarantees; it changes no types.
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

    /// Returns the tool category. The virtual shell is an agent capability
    /// that supersedes a host's native `Bash` tool, so the host denies `Bash`
    /// wherever it serves this tool.
    fn category(&self) -> ToolCategory {
        ToolCategory::Replacement {
            native: BASH_TOOL_NAME,
        }
    }

    /// Dispatch one shell operation and return its MCP result.
    ///
    /// Reads the `op` key to choose the operation, strips `op` from the
    /// arguments, and hands the rest to the matching operation module. An
    /// absent or empty `op` runs [`EXECUTE_COMMAND_OP`], which keeps the
    /// common "just run this command" call short.
    ///
    /// Every operation shares the tool's [`ShellState`], so output a command
    /// writes stays readable by `get lines` and `grep history`.
    ///
    /// An `op` that names no operation returns
    /// [`McpError::invalid_params`] listing every operation in
    /// [`SHELL_OPERATIONS`]. Errors inside an operation come back from that
    /// operation, not from here.
    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let op_str = arguments.get(OP_KEY).and_then(|v| v.as_str()).unwrap_or("");
        tracing::info!(
            "shell op: {} args: {}",
            if op_str.is_empty() {
                EXECUTE_COMMAND_OP
            } else {
                op_str
            },
            serde_json::to_string(&arguments).unwrap_or_default()
        );

        // Strip op from arguments before parsing
        let mut args = arguments.clone();
        args.remove(OP_KEY);

        match op_str {
            EXECUTE_COMMAND_OP | "" => {
                execute_command::run(args, self.state.clone(), _context).await
            }
            LIST_PROCESSES_OP => list_processes::execute_list_processes(self.state.clone()).await,
            KILL_PROCESS_OP => kill_process::execute_kill_process(&args, self.state.clone()).await,
            GREP_HISTORY_OP => grep_history::execute_grep_history(&args, self.state.clone()).await,
            GET_LINES_OP => get_lines::execute_get_lines(&args, self.state.clone()).await,
            other => Err(McpError::invalid_params(
                format!(
                    "unknown operation '{}'. Valid operations: {}",
                    other,
                    SHELL_OPERATIONS
                        .iter()
                        .map(|op| op.op_string())
                        .collect::<Vec<_>>()
                        .join(", ")
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
/// A shell tool whose state cannot be created — see
/// [`ShellExecuteTool::new`] — is reported through `tracing::error!` and left
/// out of the registry, so one unwritable directory costs the server its
/// shell tool rather than the whole registration.
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
    match ShellExecuteTool::new() {
        Ok(tool) => registry.register(tool),
        Err(error) => tracing::error!(%error, "shell tool not registered"),
    }
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
    // Construction tests
    // =====================================================================

    /// `ShellExecuteTool::new` answers a `Result` its caller reads.
    ///
    /// The constructor opens a shell state, which reads the filesystem, and a
    /// directory it cannot create — a read-only working directory, a temp
    /// location the process cannot write — is a failure of the environment
    /// rather than a broken invariant. The binding names the answer's type, so
    /// a constructor that panicked on that failure instead of reporting it
    /// would not compile here. `ShellState`'s own read-only case is measured in
    /// `state.rs`, by `falls_back_to_temp_when_preferred_dir_is_read_only`.
    ///
    /// The working directory is a temp directory for the run, because the
    /// state lands under it and the crate directory keeps no `.shell`.
    #[test]
    #[serial(cwd)]
    fn new_answers_a_result_the_caller_reads() {
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let tmp = tempfile::TempDir::new().expect("temp dir");
        let _cwd = CurrentDirGuard::new(tmp.path()).expect("chdir into temp dir");

        let created: anyhow::Result<ShellExecuteTool> = ShellExecuteTool::new();

        assert!(
            created.is_ok(),
            "a writable working directory must build the tool: {:?}",
            created.err()
        );
    }

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

    /// The dispatch constants and [`SHELL_OPERATIONS`] must name the same five
    /// operations. A constant that drifts from the registry would route an
    /// operation the schema advertises into the unknown-operation arm.
    #[test]
    fn test_dispatch_constants_match_the_operation_registry() {
        let registry: Vec<String> = SHELL_OPERATIONS.iter().map(|o| o.op_string()).collect();
        let constants = [
            EXECUTE_COMMAND_OP,
            LIST_PROCESSES_OP,
            KILL_PROCESS_OP,
            GREP_HISTORY_OP,
            GET_LINES_OP,
        ];
        assert_eq!(registry.len(), constants.len());
        for op in constants {
            assert!(
                registry.iter().any(|known| known == op),
                "dispatch constant {op:?} names no operation in SHELL_OPERATIONS: {registry:?}"
            );
        }
    }

    #[tokio::test]
    async fn test_tool_properties() {
        let tool = ShellExecuteTool::new_isolated();
        assert_eq!(McpTool::name(&tool), "shell");
        assert!(!tool.description().is_empty());

        // Wire schema: only `op` in properties, every heavy key dropped
        // (including the full-only `x-op-signatures` map).
        let wire = tool.schema();
        assert!(wire.is_object());
        assert!(wire["properties"]["op"].is_object());
        let wire_obj = wire.as_object().unwrap();
        for key in swissarmyhammer_operations::WIRE_DROPPED_KEYS {
            assert!(!wire_obj.contains_key(key), "wire schema must omit {key:?}");
        }

        // Full schema: flat per-op properties plus the heavy CLI-facing keys.
        let full = tool.schema_full();
        assert!(full["properties"]["command"].is_object());
        assert!(full["properties"]["op"].is_object());
        assert!(full["x-operation-schemas"].is_array());
        assert!(full["x-operation-groups"].is_object());
        assert!(full["x-op-signatures"].is_object());
    }

    /// The agent-facing text must state that `execute command` runs to
    /// completion before it answers, and must forbid `| tail` / `| head` /
    /// `| grep` pipelines — the tool already keeps the full output, so a
    /// pipeline throws it away. Both the tool description and the operation
    /// description carry the blocking fact.
    ///
    /// The text must also name the limit of that promise: a command the
    /// timeout kills stores nothing. `finalize_timed_out` only marks the
    /// command, and `store_command_output` runs solely in
    /// `finalize_completed`, so `get lines` on a timed-out command returns
    /// nothing.
    ///
    /// It must also send file search and file edits off the shell — the
    /// same rules the `shell` skill states (see
    /// `shell_output_guidance_states_blocking_and_no_tail` in
    /// `swissarmyhammer-skills/tests/shell_output_guidance.rs`), since the
    /// guidance is duplicated in the tool description and the skill by
    /// design.
    #[test]
    fn shell_description_states_blocking_and_no_tail() {
        let tool = ShellExecuteTool::new_isolated();
        let description = McpTool::description(&tool);
        for marker in [
            "blocks until the command exits",
            "Do not pipe to `tail`",
            "get lines",
            "grep history",
            "no output is stored",
            "Do not use grep to search files",
            "use `rg`",
            "Do not use shell to edit files",
        ] {
            assert!(
                description.contains(marker),
                "shell tool description must contain {marker:?}"
            );
        }

        assert!(
            super::EXECUTE_CMD
                .description()
                .contains("blocks until the command exits"),
            "the `execute command` operation description must say it blocks \
             until the command exits"
        );
    }

    /// `ShellExecuteTool` is a public type that carries state, so it must
    /// render with `{:?}` like every other public tool type.
    #[test]
    fn shell_execute_tool_renders_with_debug() {
        let tool = ShellExecuteTool::new_isolated();
        let rendered = format!("{tool:?}");
        assert!(
            rendered.contains("ShellExecuteTool"),
            "Debug output must name the type: {rendered}"
        );
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
            !err.contains("unknown operation"),
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

    use mirdan::test_support::MirdanConfigGuard;
    use serial_test::serial;
    use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};

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
