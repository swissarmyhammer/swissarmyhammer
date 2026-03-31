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
        let mut checks = Vec::new();
        let cat = self.category();

        // Check 1: Builtin config parses successfully
        match parse_shell_config(BUILTIN_CONFIG_YAML) {
            Ok(config) => {
                checks.push(HealthCheck::ok(
                    "Builtin config",
                    format!(
                        "Builtin shell config parsed successfully ({} deny patterns, {} permit patterns)",
                        config.deny.len(),
                        config.permit.len()
                    ),
                    cat,
                ));

                // Check 2: All deny/permit regex patterns compile
                match CompiledShellConfig::compile(&config) {
                    Ok(_) => {
                        checks.push(HealthCheck::ok(
                            "Regex patterns",
                            "All deny/permit regex patterns compile successfully",
                            cat,
                        ));
                    }
                    Err(e) => {
                        checks.push(HealthCheck::error(
                            "Regex patterns",
                            format!("Pattern '{}' failed to compile: {}", e.pattern, e.source),
                            Some(format!(
                                "Fix the invalid regex pattern '{}' in the shell config (reason: {})",
                                e.pattern, e.reason
                            )),
                            cat,
                        ));
                    }
                }
            }
            Err(e) => {
                checks.push(HealthCheck::error(
                    "Builtin config",
                    format!("Builtin shell config failed to parse: {}", e),
                    Some("This is a binary bug — rebuild swissarmyhammer with a valid builtin/shell/config.yaml".to_string()),
                    cat,
                ));
            }
        }

        // Check 3: User config (~/.shell/config.yaml) loads if present
        if let Some(home) = dirs::home_dir() {
            let user_config = home.join(".shell").join("config.yaml");
            if user_config.exists() {
                match std::fs::read_to_string(&user_config) {
                    Ok(content) => match parse_shell_config(&content) {
                        Ok(config) => {
                            checks.push(HealthCheck::ok(
                                "User config",
                                format!(
                                    "User config loaded from {} ({} deny, {} permit patterns)",
                                    user_config.display(),
                                    config.deny.len(),
                                    config.permit.len()
                                ),
                                cat,
                            ));
                        }
                        Err(e) => {
                            checks.push(HealthCheck::error(
                                "User config",
                                format!(
                                    "User config at {} failed to parse: {}",
                                    user_config.display(),
                                    e
                                ),
                                Some(format!("Fix the YAML syntax in {}", user_config.display())),
                                cat,
                            ));
                        }
                    },
                    Err(e) => {
                        checks.push(HealthCheck::warning(
                            "User config",
                            format!(
                                "User config at {} could not be read: {}",
                                user_config.display(),
                                e
                            ),
                            Some(format!(
                                "Check file permissions on {}",
                                user_config.display()
                            )),
                            cat,
                        ));
                    }
                }
            } else {
                checks.push(HealthCheck::ok(
                    "User config",
                    format!("No user config at {} (optional)", user_config.display()),
                    cat,
                ));
            }
        }

        // Check 4: Project config (.shell/config.yaml) loads if present
        let project_config = std::path::PathBuf::from(".shell").join("config.yaml");
        if project_config.exists() {
            match std::fs::read_to_string(&project_config) {
                Ok(content) => match parse_shell_config(&content) {
                    Ok(config) => {
                        checks.push(HealthCheck::ok(
                            "Project config",
                            format!(
                                "Project config loaded from {} ({} deny, {} permit patterns)",
                                project_config.display(),
                                config.deny.len(),
                                config.permit.len()
                            ),
                            cat,
                        ));
                    }
                    Err(e) => {
                        checks.push(HealthCheck::error(
                            "Project config",
                            format!(
                                "Project config at {} failed to parse: {}",
                                project_config.display(),
                                e
                            ),
                            Some(format!(
                                "Fix the YAML syntax in {}",
                                project_config.display()
                            )),
                            cat,
                        ));
                    }
                },
                Err(e) => {
                    checks.push(HealthCheck::warning(
                        "Project config",
                        format!(
                            "Project config at {} could not be read: {}",
                            project_config.display(),
                            e
                        ),
                        Some(format!(
                            "Check file permissions on {}",
                            project_config.display()
                        )),
                        cat,
                    ));
                }
            }
        } else {
            checks.push(HealthCheck::ok(
                "Project config",
                format!(
                    "No project config at {} (optional)",
                    project_config.display()
                ),
                cat,
            ));
        }

        // Check 5: Bash is denied in .claude/settings.json permissions.deny
        let claude_settings = std::path::PathBuf::from(".claude").join("settings.json");
        if claude_settings.exists() {
            match std::fs::read_to_string(&claude_settings) {
                Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                    Ok(settings) => {
                        let bash_denied = settings
                            .get("permissions")
                            .and_then(|p| p.get("deny"))
                            .and_then(|d| d.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .any(|v| v.as_str().map(|s| s == "Bash").unwrap_or(false))
                            })
                            .unwrap_or(false);

                        if bash_denied {
                            checks.push(HealthCheck::ok(
                                "Bash denied",
                                "Bash is correctly denied in .claude/settings.json — shell tool is the intended execution path",
                                cat,
                            ));
                        } else {
                            checks.push(HealthCheck::warning(
                                "Bash denied",
                                "Bash is not denied in .claude/settings.json — agents may bypass the shell tool's security controls",
                                Some("Add \"Bash\" to permissions.deny in .claude/settings.json to enforce shell tool security policies".to_string()),
                                cat,
                            ));
                        }
                    }
                    Err(e) => {
                        checks.push(HealthCheck::warning(
                            "Bash denied",
                            format!(".claude/settings.json could not be parsed as JSON: {}", e),
                            Some("Ensure .claude/settings.json is valid JSON with a permissions.deny array".to_string()),
                            cat,
                        ));
                    }
                },
                Err(e) => {
                    checks.push(HealthCheck::warning(
                        "Bash denied",
                        format!(".claude/settings.json could not be read: {}", e),
                        Some("Check file permissions on .claude/settings.json".to_string()),
                        cat,
                    ));
                }
            }
        } else {
            checks.push(HealthCheck::warning(
                "Bash denied",
                "No .claude/settings.json found — Bash may not be denied for agents",
                Some("Create .claude/settings.json with {\"permissions\":{\"deny\":[\"Bash\"]}} to enforce shell tool security policies".to_string()),
                cat,
            ));
        }

        // Check 6: Shell skill is deployed (symlink exists in .claude/skills/shell)
        let skill_path = std::path::PathBuf::from(".claude")
            .join("skills")
            .join("shell");
        if skill_path.exists() {
            let is_symlink = skill_path
                .symlink_metadata()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false);
            if is_symlink {
                checks.push(HealthCheck::ok(
                    "Shell skill deployed",
                    "Shell skill is deployed as a symlink in .claude/skills/shell",
                    cat,
                ));
            } else {
                checks.push(HealthCheck::ok(
                    "Shell skill deployed",
                    "Shell skill directory exists at .claude/skills/shell",
                    cat,
                ));
            }
        } else {
            checks.push(HealthCheck::warning(
                "Shell skill deployed",
                "Shell skill not found at .claude/skills/shell — agents may not have shell instructions",
                Some("Run `sah init` or create a symlink from .claude/skills/shell to the shell skill directory".to_string()),
                cat,
            ));
        }

        checks
    }

    fn is_applicable(&self) -> bool {
        true
    }
}

/// Maps an init scope to its Claude Code settings file path:
/// - `Project` → `.claude/settings.json`
/// - `Local` → `.claude/settings.local.json`
/// - `User` → `~/.claude/settings.json`
fn claude_settings_path(
    scope: &swissarmyhammer_common::lifecycle::InitScope,
) -> std::path::PathBuf {
    use swissarmyhammer_common::lifecycle::InitScope;
    match scope {
        InitScope::Project => std::path::PathBuf::from(".claude/settings.json"),
        InitScope::Local => std::path::PathBuf::from(".claude/settings.local.json"),
        InitScope::User => dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".claude")
            .join("settings.json"),
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
    /// 3. Deploy the shell skill to all detected agents
    fn init(
        &self,
        scope: &swissarmyhammer_common::lifecycle::InitScope,
        reporter: &dyn swissarmyhammer_common::reporter::InitReporter,
    ) -> Vec<swissarmyhammer_common::lifecycle::InitResult> {
        use swissarmyhammer_common::lifecycle::InitResult;
        use swissarmyhammer_common::reporter::InitEvent;
        let component_name = <Self as crate::mcp::tool_registry::McpTool>::name(self);
        let mut results = Vec::new();

        // Step 1: Create .shell/config.yaml from builtin template if not present
        let shell_dir = std::path::PathBuf::from(".shell");
        let config_path = shell_dir.join("config.yaml");
        if !config_path.exists() {
            if let Err(e) = std::fs::create_dir_all(&shell_dir) {
                results.push(InitResult::error(
                    component_name,
                    format!("Failed to create .shell/ directory: {}", e),
                ));
                return results;
            }
            if let Err(e) = std::fs::write(&config_path, BUILTIN_CONFIG_YAML) {
                results.push(InitResult::error(
                    component_name,
                    format!("Failed to write .shell/config.yaml: {}", e),
                ));
                return results;
            }
            reporter.emit(&InitEvent::Action {
                verb: "Created".to_string(),
                message: format!("{}", config_path.display()),
            });
        }

        // Step 2: Deny Bash in Claude settings (scope-aware path)
        let claude_settings_path = claude_settings_path(scope);
        let mut settings = if claude_settings_path.exists() {
            match std::fs::read_to_string(&claude_settings_path) {
                Ok(content) if !content.trim().is_empty() => {
                    match serde_json::from_str::<serde_json::Value>(&content) {
                        Ok(v) => v,
                        Err(e) => {
                            results.push(InitResult::error(
                                component_name,
                                format!(
                                    "Failed to parse {}: {}",
                                    claude_settings_path.display(),
                                    e
                                ),
                            ));
                            return results;
                        }
                    }
                }
                Ok(_) => serde_json::json!({}),
                Err(e) => {
                    results.push(InitResult::error(
                        component_name,
                        format!("Failed to read {}: {}", claude_settings_path.display(), e),
                    ));
                    return results;
                }
            }
        } else {
            serde_json::json!({})
        };

        // Ensure permissions.deny contains "Bash"
        {
            if settings.get("permissions").is_none() {
                settings
                    .as_object_mut()
                    .unwrap()
                    .insert("permissions".to_string(), serde_json::json!({}));
            }
            let permissions = settings.get_mut("permissions").unwrap();
            if permissions.get("deny").is_none() {
                permissions
                    .as_object_mut()
                    .unwrap()
                    .insert("deny".to_string(), serde_json::json!([]));
            }
            let deny = permissions.get_mut("deny").unwrap().as_array_mut().unwrap();
            if !deny.iter().any(|v| v.as_str() == Some("Bash")) {
                deny.push(serde_json::json!("Bash"));
                // Write settings back
                if let Some(parent) = claude_settings_path.parent() {
                    if !parent.as_os_str().is_empty() {
                        if let Err(e) = std::fs::create_dir_all(parent) {
                            results.push(InitResult::error(
                                component_name,
                                format!(
                                    "Failed to create {} parent directory: {}",
                                    claude_settings_path.display(),
                                    e
                                ),
                            ));
                            return results;
                        }
                    }
                }
                let content = match serde_json::to_string_pretty(&settings) {
                    Ok(c) => c,
                    Err(e) => {
                        results.push(InitResult::error(
                            component_name,
                            format!(
                                "Failed to serialize {}: {}",
                                claude_settings_path.display(),
                                e
                            ),
                        ));
                        return results;
                    }
                };
                if let Err(e) = std::fs::write(&claude_settings_path, content) {
                    results.push(InitResult::error(
                        component_name,
                        format!("Failed to write {}: {}", claude_settings_path.display(), e),
                    ));
                    return results;
                }
                reporter.emit(&InitEvent::Action {
                    verb: "Configured".to_string(),
                    message: format!("Bash tool denied in {}", claude_settings_path.display()),
                });
            }
        }

        // Step 3: Deploy the shell skill to all detected agents
        let resolver = swissarmyhammer_skills::SkillResolver::new();
        let builtins = resolver.resolve_builtins();
        let skill = match builtins.get("shell") {
            Some(s) => s.clone(),
            None => {
                results.push(InitResult::error(
                    component_name,
                    "Builtin 'shell' skill not found".to_string(),
                ));
                return results;
            }
        };

        // Render {{version}} in the skill instructions
        let engine = swissarmyhammer_templating::TemplateEngine::new();
        let mut vars = std::collections::HashMap::new();
        vars.insert("version".to_string(), env!("CARGO_PKG_VERSION").to_string());
        let rendered_instructions = match engine.render(&skill.instructions, &vars) {
            Ok(r) => r,
            Err(_) => skill.instructions.clone(), // fall back to raw if render fails
        };

        // Write skill to a temp dir and deploy
        let temp_dir = match tempfile::tempdir() {
            Ok(d) => d,
            Err(e) => {
                results.push(InitResult::error(
                    component_name,
                    format!("Failed to create temp dir for skill: {}", e),
                ));
                return results;
            }
        };
        let skill_dir = temp_dir.path().join("shell");
        if let Err(e) = std::fs::create_dir_all(&skill_dir) {
            results.push(InitResult::error(
                component_name,
                format!("Failed to create temp skill dir: {}", e),
            ));
            return results;
        }
        let skill_md = skill_dir.join("SKILL.md");
        // Format frontmatter + body
        let mut skill_content = String::from("---\n");
        skill_content.push_str(&format!("name: {}\n", skill.name));
        skill_content.push_str(&format!("description: {}\n", skill.description));
        if !skill.allowed_tools.is_empty() {
            skill_content.push_str(&format!(
                "allowed-tools: \"{}\"\n",
                skill.allowed_tools.join(" ")
            ));
        }
        skill_content.push_str("---\n\n");
        skill_content.push_str(&rendered_instructions);
        skill_content.push('\n');

        if let Err(e) = std::fs::write(&skill_md, &skill_content) {
            results.push(InitResult::error(
                component_name,
                format!("Failed to write shell skill SKILL.md: {}", e),
            ));
            return results;
        }

        match mirdan::install::deploy_skill_to_agents("shell", &skill_dir, None, false) {
            Ok(targets) => {
                reporter.emit(&InitEvent::Action {
                    verb: "Deployed".to_string(),
                    message: format!("shell skill to {}", targets.join(", ")),
                });
                results.push(InitResult::ok(
                    component_name,
                    format!(
                        "Shell tool initialized (skill deployed to {})",
                        targets.join(", ")
                    ),
                ));
            }
            Err(e) => {
                results.push(InitResult::error(
                    component_name,
                    format!("Failed to deploy shell skill: {}", e),
                ));
            }
        }

        results
    }

    /// Deinitialize the shell tool:
    /// 1. Remove the shell skill from all agents
    /// 2. Remove "Bash" from Claude settings permissions.deny (scope-aware)
    /// 3. Remove `.shell/` config directory if it exists
    fn deinit(
        &self,
        scope: &swissarmyhammer_common::lifecycle::InitScope,
        reporter: &dyn swissarmyhammer_common::reporter::InitReporter,
    ) -> Vec<swissarmyhammer_common::lifecycle::InitResult> {
        use swissarmyhammer_common::lifecycle::InitResult;
        use swissarmyhammer_common::reporter::InitEvent;
        let component_name = <Self as crate::mcp::tool_registry::McpTool>::name(self);
        let mut results = Vec::new();

        // Step 1: Remove shell skill via mirdan
        if let Err(e) = mirdan::install::uninstall_skill("shell", None, false) {
            reporter.emit(&InitEvent::Warning {
                message: format!("Failed to uninstall shell skill: {}", e),
            });
        } else {
            reporter.emit(&InitEvent::Action {
                verb: "Removed".to_string(),
                message: "shell skill from agents".to_string(),
            });
        }

        // Step 2: Remove "Bash" from permissions.deny (scope-aware path)
        let claude_settings_path = claude_settings_path(scope);
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

        // Step 3: Remove .shell/ config directory if it exists
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

    // =====================================================================
    // claude_settings_path tests
    // =====================================================================

    #[test]
    fn test_claude_settings_path_project() {
        use swissarmyhammer_common::lifecycle::InitScope;
        let path = claude_settings_path(&InitScope::Project);
        assert_eq!(path, std::path::PathBuf::from(".claude/settings.json"));
    }

    #[test]
    fn test_claude_settings_path_local() {
        use swissarmyhammer_common::lifecycle::InitScope;
        let path = claude_settings_path(&InitScope::Local);
        assert_eq!(
            path,
            std::path::PathBuf::from(".claude/settings.local.json")
        );
    }

    #[test]
    fn test_claude_settings_path_user() {
        use swissarmyhammer_common::lifecycle::InitScope;
        let path = claude_settings_path(&InitScope::User);
        // Should end with .claude/settings.json under the home directory
        assert!(path.ends_with(".claude/settings.json"));
        // Should be an absolute path (not relative like project/local)
        assert!(path.is_absolute() || path.starts_with("."));
    }

    #[test]
    fn test_claude_settings_path_local_differs_from_project() {
        use swissarmyhammer_common::lifecycle::InitScope;
        let project = claude_settings_path(&InitScope::Project);
        let local = claude_settings_path(&InitScope::Local);
        assert_ne!(project, local);
    }

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
}
