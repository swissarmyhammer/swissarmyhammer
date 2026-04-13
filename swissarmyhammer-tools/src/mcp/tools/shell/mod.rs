//! Shell tool for MCP — virtual shell with history, process management, and semantic search.
//!
//! ## Operations
//!
//! Dispatches between six operations:
//! - `execute command`: Run a shell command with timeout and output capture
//! - `list processes`: Show all commands with status, timing, exit codes
//! - `kill process`: Stop a running command by ID
//! - `search history`: Semantic search across command output
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
//! later retrieval via `get lines`, `grep history`, or `search history`.
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
//! - [`state`]: Command history, output log, embedding search
//! - [`execute_command`], [`list_processes`], [`kill_process`],
//!   [`search_history`], [`grep_history`], [`get_lines`]: Per-operation modules

pub mod execute_command;
pub mod get_lines;
pub mod grep_history;
pub mod infrastructure;
pub mod kill_process;
pub mod list_processes;
pub mod process;
pub mod search_history;
pub mod state;

#[cfg(test)]
pub(crate) mod test_helpers;

// Re-export public types from infrastructure
pub use infrastructure::{
    format_output_content, is_binary_content, OutputBuffer, OutputLimits, ShellError,
    ShellExecutionResult,
};

use crate::mcp::tool_registry::{McpTool, ToolContext};
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
static SEARCH_HIST: Lazy<search_history::SearchHistory> =
    Lazy::new(search_history::SearchHistory::default);
static GREP_HIST: Lazy<grep_history::GrepHistory> = Lazy::new(grep_history::GrepHistory::default);
static GET_LNS: Lazy<get_lines::GetLines> = Lazy::new(get_lines::GetLines::default);

pub static SHELL_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| {
    vec![
        &*EXECUTE_CMD as &dyn Operation,
        &*LIST_PROCS as &dyn Operation,
        &*KILL_PROC as &dyn Operation,
        &*SEARCH_HIST as &dyn Operation,
        &*GREP_HIST as &dyn Operation,
        &*GET_LNS as &dyn Operation,
    ]
});

/// Tool for executing shell commands
#[derive(Clone)]
pub struct ShellExecuteTool {
    state: Arc<Mutex<ShellState>>,
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
        }
    }

    /// Creates an instance rooted in an isolated temp directory.
    ///
    /// Use this in tests to avoid depending on the process CWD, which can
    /// become invalid when concurrent tests delete their temp directories.
    #[cfg(test)]
    pub(crate) fn new_isolated() -> Self {
        let dir = std::env::temp_dir().join(format!(".shell-test-{}", ulid::Ulid::new()));
        let state = ShellState::with_dir(dir).expect("Failed to initialize isolated shell state");
        Self {
            state: Arc::new(Mutex::new(state)),
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
                Some(format!("Check file permissions on {}", path.display())),
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

/// Check that `.claude/settings.json` denies the built-in `Bash` tool.
///
/// Denying Bash forces agents through the shell tool's security pipeline.
fn check_bash_denied(cat: &str) -> HealthCheck {
    let path = std::path::PathBuf::from(".claude").join("settings.json");
    if !path.exists() {
        return HealthCheck::warning(
            "Bash denied",
            "No .claude/settings.json found — Bash may not be denied for agents",
            Some("Create .claude/settings.json with {\"permissions\":{\"deny\":[\"Bash\"]}} to enforce shell tool security policies".to_string()),
            cat,
        );
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            return HealthCheck::warning(
                "Bash denied",
                format!(".claude/settings.json could not be read: {}", e),
                Some("Check file permissions on .claude/settings.json".to_string()),
                cat,
            );
        }
    };
    let settings: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            return HealthCheck::warning(
                "Bash denied",
                format!(".claude/settings.json could not be parsed as JSON: {}", e),
                Some(
                    "Ensure .claude/settings.json is valid JSON with a permissions.deny array"
                        .to_string(),
                ),
                cat,
            );
        }
    };
    if settings_denies_bash(&settings) {
        HealthCheck::ok(
            "Bash denied",
            "Bash is correctly denied in .claude/settings.json — shell tool is the intended execution path",
            cat,
        )
    } else {
        HealthCheck::warning(
            "Bash denied",
            "Bash is not denied in .claude/settings.json — agents may bypass the shell tool's security controls",
            Some("Add \"Bash\" to permissions.deny in .claude/settings.json to enforce shell tool security policies".to_string()),
            cat,
        )
    }
}

/// Return `true` when the Claude settings object lists `"Bash"` under
/// `permissions.deny`. Missing keys or wrong types all evaluate to `false`.
fn settings_denies_bash(settings: &serde_json::Value) -> bool {
    let Some(deny) = settings
        .get("permissions")
        .and_then(|p| p.get("deny"))
        .and_then(|d| d.as_array())
    else {
        return false;
    };
    deny.iter().any(|v| v.as_str() == Some("Bash"))
}

/// Check whether the shell skill is deployed under `.claude/skills/shell`.
fn check_shell_skill_deployed(cat: &str) -> HealthCheck {
    let path = std::path::PathBuf::from(".claude")
        .join("skills")
        .join("shell");
    if !path.exists() {
        return HealthCheck::warning(
            "Shell skill deployed",
            "Shell skill not found at .claude/skills/shell — agents may not have shell instructions",
            Some("Run `sah init` or create a symlink from .claude/skills/shell to the shell skill directory".to_string()),
            cat,
        );
    }
    let is_symlink = path
        .symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false);
    let message = if is_symlink {
        "Shell skill is deployed as a symlink in .claude/skills/shell"
    } else {
        "Shell skill directory exists at .claude/skills/shell"
    };
    HealthCheck::ok("Shell skill deployed", message, cat)
}

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

/// Load an existing Claude settings file or start from an empty object.
///
/// Empty/whitespace files are treated as "no settings yet" rather than an error.
fn load_claude_settings(path: &std::path::Path) -> Result<serde_json::Value, String> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    if content.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse {}: {}", path.display(), e))
}

/// Insert `"Bash"` into `permissions.deny`, returning `true` when the settings
/// were actually mutated.
///
/// Creates `permissions` and `permissions.deny` if they are missing.
fn insert_bash_deny(settings: &mut serde_json::Value) -> bool {
    let root = settings
        .as_object_mut()
        .expect("settings must be a JSON object");
    let permissions = root
        .entry("permissions")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .expect("permissions must be a JSON object");
    let deny = permissions
        .entry("deny")
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .expect("permissions.deny must be a JSON array");
    if deny.iter().any(|v| v.as_str() == Some("Bash")) {
        return false;
    }
    deny.push(serde_json::json!("Bash"));
    true
}

/// Serialize `settings` and write it to `path`, creating parent directories as needed.
fn write_claude_settings(
    path: &std::path::Path,
    settings: &serde_json::Value,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "Failed to create {} parent directory: {}",
                    path.display(),
                    e
                )
            })?;
        }
    }
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize {}: {}", path.display(), e))?;
    std::fs::write(path, content).map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}

/// Ensure the Claude settings file at `path` denies the `Bash` tool.
///
/// No-ops when `Bash` is already denied. Reports a single "Configured" event
/// when the file is updated.
fn deny_bash_in_claude_settings(
    path: &std::path::Path,
    reporter: &dyn swissarmyhammer_common::reporter::InitReporter,
) -> Result<(), String> {
    use swissarmyhammer_common::reporter::InitEvent;
    let mut settings = load_claude_settings(path)?;
    if !insert_bash_deny(&mut settings) {
        return Ok(());
    }
    write_claude_settings(path, &settings)?;
    reporter.emit(&InitEvent::Action {
        verb: "Configured".to_string(),
        message: format!("Bash tool denied in {}", path.display()),
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
    /// - Bash is denied in .claude/settings.json permissions.deny
    /// - Shell skill is deployed (check symlink exists in .claude/skills/shell)
    fn run_health_checks(&self) -> Vec<HealthCheck> {
        let cat = self.category();
        let mut checks = Vec::new();

        checks.extend(check_builtin_config(cat));
        if let Some(check) = check_user_config(cat) {
            checks.push(check);
        }
        checks.push(check_project_config(cat));
        checks.push(check_bash_denied(cat));
        checks.push(check_shell_skill_deployed(cat));

        checks
    }

    fn is_applicable(&self) -> bool {
        true
    }
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

    /// Only applies in project and local scopes — not user/global scope.
    fn is_applicable(&self, scope: &swissarmyhammer_common::lifecycle::InitScope) -> bool {
        use swissarmyhammer_common::lifecycle::InitScope;
        matches!(scope, InitScope::Project | InitScope::Local)
    }

    /// Initialize the shell tool for the project:
    /// 1. Create `.shell/config.yaml` from builtin template if missing
    /// 2. Deny Bash in Claude settings (scope-aware: project or local)
    ///
    /// Skill deployment is handled separately by `ShelltoolSkillDeployment`
    /// in `shelltool-cli`.
    fn init(
        &self,
        scope: &swissarmyhammer_common::lifecycle::InitScope,
        reporter: &dyn swissarmyhammer_common::reporter::InitReporter,
    ) -> Vec<swissarmyhammer_common::lifecycle::InitResult> {
        use swissarmyhammer_common::lifecycle::InitResult;
        let component_name = <Self as crate::mcp::tool_registry::McpTool>::name(self);
        let mut results = Vec::new();

        if let Err(err) = ensure_project_config(reporter) {
            results.push(InitResult::error(component_name, err));
            return results;
        }

        let claude_settings_path = scope.claude_settings_path();
        if let Err(err) = deny_bash_in_claude_settings(&claude_settings_path, reporter) {
            results.push(InitResult::error(component_name, err));
            return results;
        }

        results.push(InitResult::ok(
            component_name,
            "Shell tool initialized (config + Bash deny)",
        ));
        results
    }

    /// Deinitialize the shell tool:
    /// 1. Remove "Bash" from Claude settings permissions.deny (scope-aware)
    /// 2. Remove `.shell/` config directory if it exists
    ///
    /// Skill removal is handled separately by `ShelltoolSkillDeployment`
    /// in `shelltool-cli`.
    fn deinit(
        &self,
        scope: &swissarmyhammer_common::lifecycle::InitScope,
        reporter: &dyn swissarmyhammer_common::reporter::InitReporter,
    ) -> Vec<swissarmyhammer_common::lifecycle::InitResult> {
        use swissarmyhammer_common::lifecycle::InitResult;
        use swissarmyhammer_common::reporter::InitEvent;
        let component_name = <Self as crate::mcp::tool_registry::McpTool>::name(self);
        let mut results = Vec::new();

        // Step 1: Remove "Bash" from permissions.deny (scope-aware path)
        let claude_settings_path = scope.claude_settings_path();
        if claude_settings_path.exists() {
            match std::fs::read_to_string(&claude_settings_path) {
                Ok(content) if !content.trim().is_empty() => {
                    match serde_json::from_str::<serde_json::Value>(&content) {
                        Ok(mut settings) => {
                            // Remove "Bash" from deny list
                            let changed = if let Some(deny) = settings
                                .pointer_mut("/permissions/deny")
                                .and_then(|v| v.as_array_mut())
                            {
                                let before = deny.len();
                                deny.retain(|v| v.as_str() != Some("Bash"));
                                deny.len() != before
                            } else {
                                false
                            };
                            if changed {
                                match serde_json::to_string_pretty(&settings) {
                                    Ok(c) => {
                                        if let Err(e) = std::fs::write(&claude_settings_path, c) {
                                            results.push(InitResult::error(
                                                component_name,
                                                format!(
                                                    "Failed to write {}: {}",
                                                    claude_settings_path.display(),
                                                    e
                                                ),
                                            ));
                                        } else {
                                            reporter.emit(&InitEvent::Action {
                                                verb: "Removed".to_string(),
                                                message: format!(
                                                    "Bash deny rule from {}",
                                                    claude_settings_path.display()
                                                ),
                                            });
                                        }
                                    }
                                    Err(e) => {
                                        results.push(InitResult::error(
                                            component_name,
                                            format!(
                                                "Failed to serialize {}: {}",
                                                claude_settings_path.display(),
                                                e
                                            ),
                                        ));
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            reporter.emit(&InitEvent::Warning {
                                message: format!(
                                    "Could not parse {}: {}",
                                    claude_settings_path.display(),
                                    e
                                ),
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        // Step 2: Remove .shell/ config directory if it exists
        let shell_dir = std::path::PathBuf::from(".shell");
        if shell_dir.exists() {
            match std::fs::remove_dir_all(&shell_dir) {
                Ok(()) => {
                    reporter.emit(&InitEvent::Action {
                        verb: "Removed".to_string(),
                        message: format!("{}", shell_dir.display()),
                    });
                }
                Err(e) => {
                    results.push(InitResult::error(
                        component_name,
                        format!("Failed to remove .shell/ directory: {}", e),
                    ));
                }
            }
        }

        results.push(InitResult::ok(component_name, "Shell tool deinitialized"));
        results
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
            "Virtual shell with history, process management, and semantic search. Execute commands, search output history, grep patterns, and manage running processes.",
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
            "search history" => {
                search_history::execute_search_history(&args, self.state.clone()).await
            }
            "grep history" => {
                grep_history::execute_grep_history(&args, self.state.clone()).await
            }
            "get lines" => {
                get_lines::execute_get_lines(&args, self.state.clone()).await
            }
            other => Err(McpError::invalid_params(
                format!(
                    "Unknown operation '{}'. Valid operations: execute command, list processes, kill process, search history, grep history, get lines",
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

    // claude_settings_path tests are in swissarmyhammer-common::lifecycle (single source of truth)

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
        assert_eq!(ops.len(), 6);
        assert!(ops.iter().any(|o| o.op_string() == "execute command"));
        assert!(ops.iter().any(|o| o.op_string() == "list processes"));
        assert!(ops.iter().any(|o| o.op_string() == "kill process"));
        assert!(ops.iter().any(|o| o.op_string() == "search history"));
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
            "search history",
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
    async fn test_initializable_not_applicable_user_scope() {
        let tool = ShellExecuteTool::new_isolated();
        assert!(
            !Initializable::is_applicable(&tool, &InitScope::User),
            "Should NOT be applicable for User scope"
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

    #[tokio::test]
    async fn test_init_denies_bash_in_settings() {
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = CurrentDirGuard::new(tmp.path()).unwrap();

        let tool = ShellExecuteTool::new_isolated();
        let reporter = NullReporter;
        let _ = Initializable::init(&tool, &InitScope::Project, &reporter);

        let settings_path = tmp.path().join(".claude").join("settings.json");
        assert!(
            settings_path.exists(),
            ".claude/settings.json should exist after init"
        );
        let content = std::fs::read_to_string(&settings_path).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();
        let bash_denied = settings
            .get("permissions")
            .and_then(|p| p.get("deny"))
            .and_then(|d| d.as_array())
            .map(|arr| arr.iter().any(|v| v.as_str() == Some("Bash")))
            .unwrap_or(false);
        assert!(
            bash_denied,
            "Bash should be denied in settings.json after init"
        );
    }

    #[tokio::test]
    async fn test_deinit_removes_bash_deny() {
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = CurrentDirGuard::new(tmp.path()).unwrap();

        let tool = ShellExecuteTool::new_isolated();
        let reporter = NullReporter;

        // First init to set up settings
        let _ = Initializable::init(&tool, &InitScope::Project, &reporter);

        // Verify Bash is denied
        let settings_path = tmp.path().join(".claude").join("settings.json");
        let content = std::fs::read_to_string(&settings_path).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();
        let bash_denied = settings
            .pointer("/permissions/deny")
            .and_then(|d| d.as_array())
            .map(|arr| arr.iter().any(|v| v.as_str() == Some("Bash")))
            .unwrap_or(false);
        assert!(bash_denied, "Bash should be denied after init");

        // Now deinit
        let _ = Initializable::deinit(&tool, &InitScope::Project, &reporter);

        let content = std::fs::read_to_string(&settings_path).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();
        let bash_denied_after = settings
            .pointer("/permissions/deny")
            .and_then(|d| d.as_array())
            .map(|arr| arr.iter().any(|v| v.as_str() == Some("Bash")))
            .unwrap_or(false);
        assert!(!bash_denied_after, "Bash should NOT be denied after deinit");
    }

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

    /// Test that execute() dispatches "search history" to handler (missing query = error)
    #[tokio::test]
    async fn test_dispatch_search_history_missing_query() {
        let result = execute_op("search history", vec![]).await;
        assert!(result.is_err(), "search history without query should fail");
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

    /// Test health check when .claude/settings.json exists but Bash is NOT in deny
    #[tokio::test]
    async fn test_health_check_bash_not_denied() {
        use swissarmyhammer_common::health::HealthStatus;
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = CurrentDirGuard::new(tmp.path()).unwrap();

        // Create .claude/settings.json WITHOUT Bash in deny
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(
            claude_dir.join("settings.json"),
            r#"{"permissions":{"deny":["SomeOtherTool"]}}"#,
        )
        .unwrap();

        let tool = ShellExecuteTool::new_isolated();
        let checks = tool.run_health_checks();

        let bash_check = checks.iter().find(|c| c.name == "Bash denied");
        assert!(bash_check.is_some(), "Should have a Bash denied check");
        assert_eq!(
            bash_check.unwrap().status,
            HealthStatus::Warning,
            "Bash not denied should produce Warning status: {:?}",
            bash_check.unwrap().message
        );
    }

    /// Test health check when .claude/settings.json has invalid JSON
    #[tokio::test]
    async fn test_health_check_settings_invalid_json() {
        use swissarmyhammer_common::health::HealthStatus;
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = CurrentDirGuard::new(tmp.path()).unwrap();

        // Create .claude/settings.json with invalid JSON
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(claude_dir.join("settings.json"), "not json at all {{{").unwrap();

        let tool = ShellExecuteTool::new_isolated();
        let checks = tool.run_health_checks();

        let bash_check = checks.iter().find(|c| c.name == "Bash denied");
        assert!(bash_check.is_some(), "Should have a Bash denied check");
        assert_eq!(
            bash_check.unwrap().status,
            HealthStatus::Warning,
            "Invalid settings.json should produce Warning status"
        );
    }

    /// Test health check when .claude/settings.json has Bash correctly in deny
    #[tokio::test]
    async fn test_health_check_bash_is_denied() {
        use swissarmyhammer_common::health::HealthStatus;
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = CurrentDirGuard::new(tmp.path()).unwrap();

        // Create .claude/settings.json WITH Bash in deny
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(
            claude_dir.join("settings.json"),
            r#"{"permissions":{"deny":["Bash"]}}"#,
        )
        .unwrap();

        let tool = ShellExecuteTool::new_isolated();
        let checks = tool.run_health_checks();

        let bash_check = checks.iter().find(|c| c.name == "Bash denied");
        assert!(bash_check.is_some(), "Should have a Bash denied check");
        assert_eq!(
            bash_check.unwrap().status,
            HealthStatus::Ok,
            "Bash denied should produce Ok status"
        );
    }

    /// Test health check when shell skill directory exists (not a symlink)
    #[tokio::test]
    async fn test_health_check_shell_skill_directory() {
        use swissarmyhammer_common::health::HealthStatus;
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = CurrentDirGuard::new(tmp.path()).unwrap();

        // Create .claude/skills/shell as a regular directory
        let skill_dir = tmp.path().join(".claude").join("skills").join("shell");
        std::fs::create_dir_all(&skill_dir).unwrap();

        let tool = ShellExecuteTool::new_isolated();
        let checks = tool.run_health_checks();

        let skill_check = checks.iter().find(|c| c.name == "Shell skill deployed");
        assert!(
            skill_check.is_some(),
            "Should have a Shell skill deployed check"
        );
        assert_eq!(
            skill_check.unwrap().status,
            HealthStatus::Ok,
            "Shell skill directory should produce Ok status"
        );
    }

    /// Test health check when shell skill does not exist
    #[tokio::test]
    async fn test_health_check_shell_skill_missing() {
        use swissarmyhammer_common::health::HealthStatus;
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = CurrentDirGuard::new(tmp.path()).unwrap();

        // No skill directory created
        let tool = ShellExecuteTool::new_isolated();
        let checks = tool.run_health_checks();

        let skill_check = checks.iter().find(|c| c.name == "Shell skill deployed");
        assert!(
            skill_check.is_some(),
            "Should have a Shell skill deployed check"
        );
        assert_eq!(
            skill_check.unwrap().status,
            HealthStatus::Warning,
            "Missing shell skill should produce Warning status"
        );
    }

    // =====================================================================
    // Init edge cases
    // =====================================================================

    /// Test init when .claude/settings.json already exists with Bash in deny (idempotent)
    #[tokio::test]
    async fn test_init_bash_already_denied_is_idempotent() {
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = CurrentDirGuard::new(tmp.path()).unwrap();

        // Pre-create .claude/settings.json with Bash already denied
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let initial = serde_json::json!({"permissions":{"deny":["Bash"]}});
        std::fs::write(
            claude_dir.join("settings.json"),
            serde_json::to_string_pretty(&initial).unwrap(),
        )
        .unwrap();

        let tool = ShellExecuteTool::new_isolated();
        let reporter = NullReporter;
        let _ = Initializable::init(&tool, &InitScope::Project, &reporter);

        // Verify Bash is still denied (not duplicated)
        let content = std::fs::read_to_string(claude_dir.join("settings.json")).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();
        let deny_arr = settings
            .pointer("/permissions/deny")
            .and_then(|d| d.as_array())
            .unwrap();
        let bash_count = deny_arr
            .iter()
            .filter(|v| v.as_str() == Some("Bash"))
            .count();
        assert_eq!(
            bash_count, 1,
            "Bash should appear exactly once in deny list, not duplicated"
        );
    }

    /// Test init when .claude/settings.json has empty content (treated as empty object)
    #[tokio::test]
    async fn test_init_with_empty_settings_file() {
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = CurrentDirGuard::new(tmp.path()).unwrap();

        // Create .claude/settings.json with empty content
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(claude_dir.join("settings.json"), "   ").unwrap();

        let tool = ShellExecuteTool::new_isolated();
        let reporter = NullReporter;
        let _ = Initializable::init(&tool, &InitScope::Project, &reporter);

        // Should have written settings with Bash denied
        let content = std::fs::read_to_string(claude_dir.join("settings.json")).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();
        let bash_denied = settings
            .pointer("/permissions/deny")
            .and_then(|d| d.as_array())
            .map(|arr| arr.iter().any(|v| v.as_str() == Some("Bash")))
            .unwrap_or(false);
        assert!(
            bash_denied,
            "Bash should be denied after init with empty settings file"
        );
    }

    // =====================================================================
    // Deinit edge cases
    // =====================================================================

    /// Test deinit when .claude/settings.json has invalid JSON (should warn, not crash)
    #[tokio::test]
    async fn test_deinit_with_invalid_settings_json() {
        use swissarmyhammer_common::lifecycle::InitStatus;
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = CurrentDirGuard::new(tmp.path()).unwrap();

        // Create .claude/settings.json with invalid JSON
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(claude_dir.join("settings.json"), "not json {{{{").unwrap();

        let tool = ShellExecuteTool::new_isolated();
        let reporter = NullReporter;
        // deinit should not panic or return a hard error for invalid settings
        let results = Initializable::deinit(&tool, &InitScope::Project, &reporter);
        // It should still complete (possibly with warning but not hard error)
        // The final InitResult::ok should always be present
        assert!(
            results.iter().any(|r| r.status == InitStatus::Ok),
            "deinit should succeed even with invalid settings.json"
        );
    }

    /// Test deinit when no .claude/settings.json exists
    #[tokio::test]
    async fn test_deinit_without_settings_file() {
        use swissarmyhammer_common::lifecycle::InitStatus;
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = CurrentDirGuard::new(tmp.path()).unwrap();

        let tool = ShellExecuteTool::new_isolated();
        let reporter = NullReporter;
        let results = Initializable::deinit(&tool, &InitScope::Project, &reporter);

        // Should always have a final ok result
        assert!(
            results.iter().any(|r| r.status == InitStatus::Ok),
            "deinit should succeed when no settings file exists"
        );
    }
}
