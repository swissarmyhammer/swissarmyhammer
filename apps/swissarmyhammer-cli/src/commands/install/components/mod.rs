//! Composable `Initializable` components for sah init/deinit.
//!
//! Each component encapsulates one aspect of the install/uninstall lifecycle
//! and implements the `Initializable` trait from `swissarmyhammer_common::lifecycle`.

use std::fs;

use swissarmyhammer_common::lifecycle::{InitRegistry, InitResult, InitScope, Initializable};
use swissarmyhammer_common::reporter::{InitEvent, InitReporter};
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_prompts::PromptLibrary;

use mirdan::settings as mirdan_settings;
use serde_json::json;

/// Top-level key for Claude Code's statusline configuration.
const STATUSLINE_KEY: &str = "statusLine";

/// Construct the desired statusline configuration value.
fn desired_statusline_value() -> serde_json::Value {
    json!({
        "type": "command",
        "command": "sah statusline"
    })
}

/// Construct the sah MCP server entry used by Claude Code local scope.
fn sah_mcp_server_entry() -> mirdan::mcp_config::McpServerEntry {
    mirdan::mcp_config::McpServerEntry {
        command: "sah".to_string(),
        args: vec!["serve".to_string()],
        env: std::collections::BTreeMap::new(),
    }
}

/// Register all install/uninstall components into the given registry.
///
/// Components use the `scope` parameter they receive in `init`/`deinit` to
/// determine project-vs-global behavior.
///
/// This function registers the in-process install components plus the
/// `KanbanTool` lifecycle hook. The canonical priority table — including
/// `SkillDeployment` (priority 60), which is registered separately by
/// [`super::super::registry::register_all`] — and the rationale for why
/// [`ProjectStructure`] skips User scope live on that function's doc
/// comment.
///
/// * `remove_directory` - Whether `ProjectStructure::deinit` should delete directories.
pub fn register_all(registry: &mut InitRegistry, remove_directory: bool) {
    registry.register(McpRegistration);
    registry.register(DenyBash);
    registry.register(Statusline);
    registry.register(ProjectStructure::new(remove_directory));
    registry.register(ClaudeMd);
    registry.register(AgentDeployment);
    registry.register(LockfileCleanup);

    // Register tools that have lifecycle operations.
    // Each tool implements Initializable — tools with no-op init/deinit
    // are harmless to include (they'll be skipped automatically).
    // sah exposes kanban through `sah serve`, NOT a separate `kanban` MCP
    // server — so it constructs the tool WITHOUT an injected MCP entry. The
    // tool's init/deinit then only manage `.kanban/` merge drivers.
    registry.register(swissarmyhammer_tools::mcp::tools::kanban::KanbanTool::new());
}

// ── McpRegistration (priority 10) ────────────────────────────────────

/// Registers/unregisters sah as an MCP server across all detected agents.
///
/// Delegates entirely to [`mirdan::install::register_mcp_server`] /
/// [`mirdan::install::unregister_mcp_server`], which iterate detected agents
/// and dispatch to each agent's `mirdan::strategy` (so Claude Code's local
/// scope, generic JSON agents, and per-scope MCP targets are all handled in
/// one place). The component just declares the `sah serve` entry.
pub struct McpRegistration;

impl Initializable for McpRegistration {
    /// The component name for MCP server registration.
    fn name(&self) -> &str {
        "mcp-registration"
    }

    /// Human-readable label for this component.
    fn display_name(&self) -> &str {
        "Register MCP server"
    }

    /// Component category: configuration tasks.
    fn category(&self) -> &str {
        "configuration"
    }

    /// Component priority: 10 (runs first; primary MCP registration step).
    fn priority(&self) -> i32 {
        10
    }

    /// Register the sah MCP server across detected agents for `scope`.
    fn init(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        mirdan::install::register_mcp_server(*scope, "sah", &sah_mcp_server_entry(), reporter)
    }

    /// Unregister the sah MCP server across detected agents for `scope`.
    fn deinit(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        mirdan::install::unregister_mcp_server(*scope, "sah", reporter)
    }
}

// ── DenyBash (priority 20) ───────────────────────────────────────────

/// Denies/allows the built-in "Bash" tool across all detected agents.
///
/// Delegates to [`mirdan::install::deny_tool`] / [`mirdan::install::allow_tool`],
/// which dispatch to each agent's `mirdan::strategy`. Agents with no permission
/// mechanism (everything but Claude Code today) are silently skipped by the
/// strategy. The component just declares the tool name and scope.
pub struct DenyBash;

impl Initializable for DenyBash {
    /// The component name for Bash denial rule configuration.
    fn name(&self) -> &str {
        "deny-bash"
    }

    /// Human-readable label for this component.
    fn display_name(&self) -> &str {
        "Permissions"
    }

    /// Component category: configuration tasks.
    fn category(&self) -> &str {
        "configuration"
    }

    /// Component priority: 20 (first per-agent settings edit, after MCP registration).
    fn priority(&self) -> i32 {
        20
    }

    /// Applicable to project, local, and user scope installations.
    fn is_applicable(&self, scope: &InitScope) -> bool {
        matches!(
            scope,
            InitScope::Project | InitScope::Local | InitScope::User
        )
    }

    /// Deny the built-in Bash tool across detected agents for `scope`.
    fn init(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        mirdan::install::deny_tool(*scope, "Bash", reporter)
    }

    /// Allow the built-in Bash tool again across detected agents for `scope`.
    fn deinit(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        mirdan::install::allow_tool(*scope, "Bash", reporter)
    }
}

// ── Statusline (priority 30) ─────────────────────────────────────────

/// Manages the `statusLine` block in each detected agent's settings file.
///
/// Follows the same agent-iterating pattern as `DenyBash`: resolves each
/// detected agent's per-scope settings file via `AgentDef`, then calls
/// `mirdan::settings::set_object` / `mirdan::settings::remove_key` with the
/// `statusLine` key. Agents without a settings path for the scope are skipped.
pub struct Statusline;

impl Initializable for Statusline {
    /// The component name for statusline configuration.
    fn name(&self) -> &str {
        "statusline"
    }

    /// Human-readable label for this component.
    fn display_name(&self) -> &str {
        "Statusline"
    }

    /// Component category: configuration tasks.
    fn category(&self) -> &str {
        "configuration"
    }

    /// Component priority: 30 (runs after `Permissions`, before project workspace setup).
    fn priority(&self) -> i32 {
        30
    }

    /// Applicable to project, local, and user scope installations.
    fn is_applicable(&self, scope: &InitScope) -> bool {
        matches!(
            scope,
            InitScope::Project | InitScope::Local | InitScope::User
        )
    }

    /// Add the statusline configuration to each detected agent's settings file.
    fn init(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        for_each_detected_agent_settings_file(
            self.name(),
            scope,
            reporter,
            install_statusline_for_agent,
        )
    }

    /// Remove the statusline configuration from each detected agent's settings file.
    fn deinit(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        for_each_detected_agent_settings_file(
            self.name(),
            scope,
            reporter,
            uninstall_statusline_for_agent,
        )
    }
}

/// Add the statusline block to a single agent's settings file.
///
/// Uses mirdan's generic JSON primitives: sets the top-level `statusLine`
/// key to the Claude-conventional `{type: "command", command: "sah statusline"}`.
fn install_statusline_for_agent(
    component_name: &str,
    def: &mirdan::agents::AgentDef,
    path: &std::path::Path,
    reporter: &dyn InitReporter,
) -> InitResult {
    let mut claude_settings = match mirdan_settings::read_json(path) {
        Ok(s) => s,
        Err(e) => return InitResult::error(component_name, e.to_string()),
    };
    let changed = mirdan_settings::set_object(
        &mut claude_settings,
        STATUSLINE_KEY,
        desired_statusline_value(),
    );
    if let Err(e) = mirdan_settings::write_json(path, &claude_settings) {
        return InitResult::error(component_name, e.to_string());
    }
    if changed {
        reporter.emit(&InitEvent::Action {
            verb: "Installed".to_string(),
            message: format!("statusline for {} ({})", def.name, path.display()),
        });
    }
    InitResult::ok(component_name, "Statusline configured")
}

/// Remove the statusline block from a single agent's settings file.
///
/// Uses mirdan's generic JSON primitives: deletes the top-level
/// `statusLine` key when present.
fn uninstall_statusline_for_agent(
    component_name: &str,
    def: &mirdan::agents::AgentDef,
    path: &std::path::Path,
    reporter: &dyn InitReporter,
) -> InitResult {
    if !path.exists() {
        return InitResult::ok(component_name, "Settings file not found");
    }
    let mut claude_settings = match mirdan_settings::read_json(path) {
        Ok(s) => s,
        Err(e) => return InitResult::error(component_name, e.to_string()),
    };
    let changed = mirdan_settings::remove_key(&mut claude_settings, STATUSLINE_KEY);
    if changed {
        if let Err(e) = mirdan_settings::write_json(path, &claude_settings) {
            return InitResult::error(component_name, e.to_string());
        }
        reporter.emit(&InitEvent::Action {
            verb: "Removed".to_string(),
            message: format!("statusline for {} ({})", def.name, path.display()),
        });
    }
    InitResult::ok(component_name, "Statusline removed")
}

// ── Shared agent-settings iteration ──────────────────────────────────

/// Resolve the per-agent settings file path for `scope`.
///
/// * `User` → the agent's absolute global settings file (e.g.
///   `~/.claude/settings.json`).
/// * `Project`/`Local` → the agent's project-relative settings file joined
///   onto `git_root` (e.g. `<git-root>/.claude/settings.json`).
///
/// Returns `None` when the agent has no settings path for the scope.
fn resolve_settings_file(
    agent: &mirdan::agents::AgentDef,
    scope: &InitScope,
    git_root: Option<&std::path::Path>,
) -> Option<std::path::PathBuf> {
    if matches!(scope, InitScope::User) {
        mirdan::agents::agent_global_settings_file(agent)
    } else {
        let relative = mirdan::agents::agent_project_settings_file(agent)?;
        git_root.map(|root| root.join(relative))
    }
}

/// Iterate every detected agent that has a settings file for `scope` and
/// invoke `action` against it, collecting per-agent results.
///
/// In project/local scope, paths are resolved against the git root; if no
/// git root is found, a warning is reported and no files are touched.
/// Agents without a settings path for the scope are skipped (not applicable),
/// so today this naturally targets Claude Code only.
fn for_each_detected_agent_settings_file(
    component_name: &str,
    scope: &InitScope,
    reporter: &dyn InitReporter,
    action: fn(&str, &mirdan::agents::AgentDef, &std::path::Path, &dyn InitReporter) -> InitResult,
) -> Vec<InitResult> {
    let config = match mirdan::agents::load_agents_config() {
        Ok(c) => c,
        Err(e) => {
            return vec![InitResult::error(
                component_name,
                format!("Failed to load agents config: {}", e),
            )];
        }
    };
    let agents = mirdan::agents::get_detected_agents(&config);

    let git_root = if matches!(scope, InitScope::User) {
        None
    } else {
        match swissarmyhammer_common::utils::find_git_repository_root() {
            Some(r) => Some(r),
            None => {
                reporter.emit(&InitEvent::Warning {
                    message: format!(
                        "No git repository found; skipping {} settings update",
                        component_name
                    ),
                });
                return vec![InitResult::error(
                    component_name,
                    "No git repository found".to_string(),
                )];
            }
        }
    };

    let mut results = Vec::new();
    for agent in &agents {
        let path = match resolve_settings_file(&agent.def, scope, git_root.as_deref()) {
            Some(p) => p,
            None => continue,
        };
        results.push(action(component_name, &agent.def, &path, reporter));
    }

    if results.is_empty() {
        return vec![InitResult::skipped(
            component_name,
            "No agents with a settings file for this scope",
        )];
    }
    results
}

// ── ProjectStructure (priority 40) ───────────────────────────────────

/// Creates/removes the `.sah/` and `.prompts/` project directories.
///
/// # User-scope behavior
///
/// `is_applicable` deliberately matches only `Project | Local` and skips
/// `User` scope. There is no corresponding global `~/.sah/` or `~/.prompts/`
/// counterpart created by this component, and that is intentional:
///
/// * `sah init --user` is a **per-agent config install** — it edits each
///   detected agent's global settings (Claude `~/.claude/settings.json`,
///   the global `CLAUDE.md` preamble, statusline config, deployed agent
///   definitions). User scope has no shared runtime artifacts of its own.
/// * Runtime state — `.sah/workflows/`, prompt overrides, kanban boards,
///   code-context indexes — is **project-local** by design. It belongs
///   inside the project tree, not in `$HOME`.
/// * The few readers that *do* look under `~/.sah/` (e.g. global
///   `tools.yaml` in `swissarmyhammer-tools::mcp::tool_config`, statusline
///   overrides in `swissarmyhammer-statusline`, `~/.prompts/` in the
///   health registry) all treat those paths as **optional, lazy
///   fallbacks**: missing-is-fine, and the dirs that need to exist are
///   created on demand by the components that write into them
///   (`Statusline`, `AgentDeployment`). Pre-creating an empty `~/.sah/`
///   here would add no behavior and would mislead a future reader into
///   thinking user scope has a shared runtime state directory.
///
/// If a future feature genuinely needs a global runtime directory under
/// `$HOME`, add a separate `GlobalUserStructure` component applicable to
/// `User` rather than widening this one — the two scopes have different
/// lifecycles and ownership.
pub struct ProjectStructure {
    remove_directory: bool,
}

impl ProjectStructure {
    /// Create a new ProjectStructure component.
    pub fn new(remove_directory: bool) -> Self {
        Self { remove_directory }
    }
}

impl Initializable for ProjectStructure {
    /// The component name for project structure creation/removal.
    fn name(&self) -> &str {
        "project-structure"
    }

    /// Human-readable label for this component.
    fn display_name(&self) -> &str {
        "Project workspace"
    }

    /// Component category: structural setup tasks.
    fn category(&self) -> &str {
        "structure"
    }

    /// Component priority: 40 (runs after per-agent settings, before the preamble).
    fn priority(&self) -> i32 {
        40
    }

    /// Only applicable to project and local scope installations.
    ///
    /// User scope is intentionally excluded — see the struct-level
    /// documentation on [`ProjectStructure`] for the rationale. In short:
    /// `sah init --user` installs per-agent config (settings, preamble,
    /// statusline, agents) but has no shared runtime artifacts of its own;
    /// sah's runtime state (`.sah/workflows/`, prompts, kanban, indexes)
    /// is project-local.
    fn is_applicable(&self, scope: &InitScope) -> bool {
        matches!(scope, InitScope::Project | InitScope::Local)
    }

    /// Create the project directory structure with .prompts, .sah, and workflows.
    ///
    /// Resolves the project root (git root, else the current directory) and
    /// delegates the actual `.sah/` + `.prompts/` creation to the root-explicit
    /// [`swissarmyhammer_workspace_init::ProjectStructure`] component, so the
    /// workspace-structure logic is shared with the kanban-app rather than
    /// forked. Root resolution stays here because the CLI is rooted at the
    /// process working directory by design.
    fn init(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        let root = match swissarmyhammer_common::utils::find_git_repository_root() {
            Some(root) => root,
            None => match std::env::current_dir() {
                Ok(cwd) => cwd,
                Err(e) => {
                    return vec![InitResult::error(
                        self.name(),
                        format!("Failed to get current directory: {}", e),
                    )];
                }
            },
        };

        swissarmyhammer_workspace_init::ProjectStructure::new(root).init(scope, reporter)
    }

    /// Remove `.sah/` and `.prompts/` directories if `remove_directory` is true.
    fn deinit(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        if !self.remove_directory {
            return vec![InitResult::skipped(
                self.name(),
                "Directory removal not requested",
            )];
        }

        let cwd = match std::env::current_dir() {
            Ok(c) => c,
            Err(e) => {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to get current directory: {}", e),
                )];
            }
        };

        let sah_dir = cwd.join(".sah");
        if sah_dir.exists() {
            if let Err(e) = fs::remove_dir_all(&sah_dir) {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to remove {}: {}", sah_dir.display(), e),
                )];
            }
            reporter.emit(&InitEvent::Action {
                verb: "Removed".to_string(),
                message: format!("{}", sah_dir.display()),
            });
        }

        let prompts_dir = cwd.join(".prompts");
        if prompts_dir.exists() {
            if let Err(e) = fs::remove_dir_all(&prompts_dir) {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to remove {}: {}", prompts_dir.display(), e),
                )];
            }
            reporter.emit(&InitEvent::Action {
                verb: "Removed".to_string(),
                message: format!("{}", prompts_dir.display()),
            });
        }

        vec![InitResult::ok(self.name(), "Project directories removed")]
    }
}

// ── AgentDeployment (priority 70) ────────────────────────────────────

/// Deploys/removes builtin agents via mirdan's store + lockfile.
///
/// Derives global-vs-project behavior from the `InitScope` parameter passed
/// to `init`/`deinit`.
pub struct AgentDeployment;

impl Initializable for AgentDeployment {
    /// The component name for agent deployment.
    fn name(&self) -> &str {
        "agent-deployment"
    }

    /// Human-readable label for this component.
    fn display_name(&self) -> &str {
        "Subagents"
    }

    /// Component category: deployment tasks.
    fn category(&self) -> &str {
        "deployment"
    }

    /// Component priority: 70 (runs after skill deployment, before lockfile cleanup).
    fn priority(&self) -> i32 {
        70
    }

    /// Install builtin agents via mirdan's deploy + lockfile.
    ///
    /// Agent instructions are rendered through the prompt library's Liquid template
    /// engine before writing to disk, so `{% include %}` partials are expanded.
    fn init(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        match init_all_agents(scope, reporter) {
            Ok(msg) => vec![InitResult::ok(self.name(), msg)],
            Err(e) => vec![InitResult::error(self.name(), e)],
        }
    }

    /// Remove builtin agent symlinks from coding agent directories and clean up the .agents/ store.
    fn deinit(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        use swissarmyhammer_agents::AgentResolver;

        let global = matches!(scope, InitScope::User);
        let store_dir = mirdan::store::agent_store_dir(global);

        let config = match mirdan::agents::load_agents_config() {
            Ok(c) => c,
            Err(e) => {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to load agents config: {}", e),
                )];
            }
        };
        let agents = mirdan::agents::get_detected_agents(&config);

        let resolver = AgentResolver::new();
        let builtins = resolver.resolve_builtins();
        let builtin_names: Vec<String> = builtins.keys().cloned().collect();

        let mut link_dirs: Vec<std::path::PathBuf> = Vec::new();
        let mut symlink_policies: Vec<mirdan::agents::SymlinkPolicy> = Vec::new();

        for agent in &agents {
            let agent_dir = if global {
                mirdan::agents::agent_global_agent_dir(&agent.def)
            } else {
                mirdan::agents::agent_project_agent_dir(&agent.def)
            };
            if let Some(dir) = agent_dir {
                link_dirs.push(dir);
                symlink_policies.push(agent.def.symlink_policy.clone());
            }
        }

        let agent_names: Vec<String> = agents.iter().map(|a| a.def.id.clone()).collect();

        mirdan::store::remove_store_entries(
            &store_dir,
            &builtin_names,
            &link_dirs,
            &symlink_policies,
            "agent",
            reporter,
        );

        reporter.emit(&InitEvent::Action {
            verb: "Removed".to_string(),
            message: format!(
                "{} agents from {}",
                builtin_names.len(),
                agent_names.join(", ")
            ),
        });

        vec![InitResult::ok(self.name(), "Builtin agents removed")]
    }
}

/// Deploy a single builtin agent to a temp dir and then to coding agents.
///
/// Returns the list of agent targets on success, or an error message.
fn deploy_single_agent(
    name: &str,
    agent: &swissarmyhammer_agents::Agent,
    prompt_library: &PromptLibrary,
    template_context: &TemplateContext,
    global: bool,
    reporter: &dyn InitReporter,
) -> Result<Vec<String>, String> {
    if !mirdan::store::is_safe_name(name) {
        return Err(format!("Unsafe agent name: {:?}", name));
    }

    let temp_dir = tempfile::tempdir().map_err(|e| format!("Failed to create temp dir: {}", e))?;
    let agent_dir = temp_dir.path().join(name);
    fs::create_dir_all(&agent_dir)
        .map_err(|e| format!("Failed to create temp agent dir: {}", e))?;

    let rendered_instructions =
        match prompt_library.render_text(&agent.instructions, template_context) {
            Ok(rendered) => rendered,
            Err(e) => {
                reporter.emit(&InitEvent::Warning {
                    message: format!("failed to render partials for agent '{}': {}", name, e),
                });
                agent.instructions.clone()
            }
        };

    // Render template variables in metadata values (e.g., version: "{{version}}")
    let mut rendered_agent = agent.clone();
    for value in rendered_agent.metadata.values_mut() {
        if value.contains("{{") {
            if let Ok(rendered_value) = prompt_library.render_text(value, template_context) {
                *value = rendered_value;
            }
        }
    }

    let agent_md_path = agent_dir.join("AGENT.md");
    let content = rendered_agent.to_agent_md(&rendered_instructions);
    fs::write(&agent_md_path, &content)
        .map_err(|e| format!("Failed to write {}: {}", agent_md_path.display(), e))?;

    mirdan::install::deploy_agent_to_agents(name, &agent_dir, None, global)
        .map_err(|e| format!("Failed to deploy agent '{}': {}", name, e))
}

/// Deploy all builtin agents, update lockfile, and report results.
///
/// `scope` controls both where each agent is deployed (project vs global store)
/// and where the lockfile is written via
/// [`mirdan::lockfile::lockfile_root_for_scope`] — user scope writes
/// `~/mirdan-lock.json`; project/local scope writes `<cwd>/mirdan-lock.json`.
fn init_all_agents(scope: &InitScope, reporter: &dyn InitReporter) -> Result<String, String> {
    use swissarmyhammer_agents::AgentResolver;

    let resolver = AgentResolver::new();
    let agents = resolver.resolve_builtins();

    let prompt_library = PromptLibrary::default();
    let template_context = agent_template_context();

    let global = matches!(scope, InitScope::User);
    let (project_root, mut lockfile) = load_agent_project_lockfile(scope)?;

    let mut installed_count = 0;
    let mut agent_targets: Vec<String> = Vec::new();

    for (name, agent) in &agents {
        let targets = deploy_single_agent(
            name,
            agent,
            &prompt_library,
            &template_context,
            global,
            reporter,
        )?;
        if agent_targets.is_empty() {
            agent_targets = targets.clone();
        }
        lockfile.add_package(name.clone(), locked_builtin_agent_package(targets));
        installed_count += 1;
    }

    save_lockfile_and_report(
        &lockfile,
        &project_root,
        installed_count,
        "agents",
        &agent_targets,
        reporter,
    )?;
    Ok(format!("Deployed {} builtin agents", installed_count))
}

fn agent_template_context() -> TemplateContext {
    let mut ctx = TemplateContext::new();
    ctx.set(
        "version".to_string(),
        serde_json::json!(env!("CARGO_PKG_VERSION")),
    );
    ctx
}

/// Resolve the lockfile root for `scope` and load (or default-construct) the
/// `mirdan-lock.json` that lives there.
///
/// Returns the resolved root alongside the loaded lockfile so callers can pass
/// the same root to [`save_lockfile_and_report`].
fn load_agent_project_lockfile(
    scope: &InitScope,
) -> Result<(std::path::PathBuf, mirdan::lockfile::Lockfile), String> {
    let project_root = mirdan::lockfile::lockfile_root_for_scope(scope)?;
    let lockfile = mirdan::lockfile::Lockfile::load(&project_root)
        .map_err(|e| format!("Failed to load lockfile: {}", e))?;
    Ok((project_root, lockfile))
}

fn locked_builtin_agent_package(targets: Vec<String>) -> mirdan::lockfile::LockedPackage {
    mirdan::lockfile::LockedPackage {
        package_type: mirdan::package_type::PackageType::Agent,
        version: "0.0.0".to_string(),
        resolved: "builtin".to_string(),
        integrity: String::new(),
        installed_at: chrono::Utc::now().to_rfc3339(),
        targets,
    }
}

// ── LockfileCleanup (priority 80) ────────────────────────────────────

/// Cleans up lockfile entries for builtin skills and agents on deinit.
///
/// Init does not need to do anything here because lockfile entries are
/// written by `SkillDeployment` and `AgentDeployment` during their init.
pub struct LockfileCleanup;

impl Initializable for LockfileCleanup {
    /// The component name for lockfile cleanup.
    fn name(&self) -> &str {
        "lockfile-cleanup"
    }

    /// Human-readable label for this component.
    fn display_name(&self) -> &str {
        "Lockfile"
    }

    /// Component category: deployment tasks.
    fn category(&self) -> &str {
        "deployment"
    }

    /// Component priority: 80 (runs last; cleans up after skill and agent deployment).
    fn priority(&self) -> i32 {
        80
    }

    /// Lockfile entries are written by SkillDeployment and AgentDeployment during their init phases.
    /// This component does not need to do anything during initialization.
    fn init(&self, _scope: &InitScope, _reporter: &dyn InitReporter) -> Vec<InitResult> {
        vec![]
    }

    /// Remove lockfile entries for all builtin skills and agents.
    ///
    /// Resolves the lockfile root via
    /// [`mirdan::lockfile::lockfile_root_for_scope`] so that `sah deinit user`
    /// cleans up `~/mirdan-lock.json` and `sah deinit` cleans up
    /// `<cwd>/mirdan-lock.json`, matching the corresponding init paths.
    fn deinit(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        use swissarmyhammer_agents::AgentResolver;
        use swissarmyhammer_skills::SkillResolver;

        let project_root = match mirdan::lockfile::lockfile_root_for_scope(scope) {
            Ok(d) => d,
            Err(e) => return vec![InitResult::error(self.name(), e)],
        };

        let lockfile_path = project_root.join("mirdan-lock.json");
        if !lockfile_path.exists() {
            return vec![InitResult::ok(self.name(), "No lockfile found")];
        }

        let mut lockfile = match mirdan::lockfile::Lockfile::load(&project_root) {
            Ok(l) => l,
            Err(e) => {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to load lockfile: {}", e),
                )];
            }
        };

        let skill_resolver = SkillResolver::new();
        for name in skill_resolver.resolve_builtins().keys() {
            lockfile.remove_package(name);
        }

        let agent_resolver = AgentResolver::new();
        for name in agent_resolver.resolve_builtins().keys() {
            lockfile.remove_package(name);
        }

        if let Err(e) = lockfile.save(&project_root) {
            return vec![InitResult::error(
                self.name(),
                format!("Failed to save lockfile: {}", e),
            )];
        }

        reporter.emit(&InitEvent::Action {
            verb: "Cleaned".to_string(),
            message: "lockfile entries".to_string(),
        });
        vec![InitResult::ok(self.name(), "Lockfile entries cleaned up")]
    }
}

// ── ClaudeMd (priority 50) ──────────────────────────────────────────

/// The preamble line that must appear at the top of CLAUDE.md.
///
/// Re-exported from [`mirdan::status::PREAMBLE_MARKER`], which is the single
/// source of truth for the marker string.
pub use mirdan::status::PREAMBLE_MARKER as CLAUDE_MD_PREAMBLE;

/// Ensures a `CLAUDE.md` file exists at the git root with the required preamble.
pub struct ClaudeMd;

/// Check if the instructions file at `path` has the required preamble as its first non-empty line.
///
/// `path` is the full path to the instructions file (e.g. `<git-root>/CLAUDE.md`
/// or `~/.claude/CLAUDE.md`), not a directory.
///
/// Returns `None` if the file does not exist, `Some(true)` if the preamble is present,
/// and `Some(false)` if it is missing.
///
/// Delegates the preamble check to [`mirdan::status::preamble_present`] so the
/// test helper agrees by construction with what `mirdan status` detects. The
/// extra "file does not exist" vs. "file exists but has no preamble" distinction
/// (returning `None` vs. `Some(false)`) is kept for the install-layer tests
/// that want to assert on file presence as well as preamble presence.
///
/// Note: an existing-but-unreadable file (e.g. permission denied on
/// `read_to_string`) is reported as `Some(false)` rather than `None`, because
/// [`mirdan::status::preamble_present`] treats read failure as "no preamble".
/// In practice the install layer creates these files itself, so an unreadable
/// CLAUDE.md is not a path the tests exercise.
#[cfg(test)]
fn preamble_file_has_preamble(path: &std::path::Path) -> Option<bool> {
    if !path.exists() {
        return None;
    }
    Some(mirdan::status::preamble_present(path))
}

/// Ensure the instructions file at `path` has the required preamble.
///
/// `path` is the full path to the instructions file. Parent directories are
/// created as needed so this works for both the project `CLAUDE.md` and an
/// absolute global file like `~/.claude/CLAUDE.md`.
///
/// Returns `"created"` if the file was created, `"already present"` if
/// no change was needed, or `"prepended"` if the preamble was prepended.
fn ensure_preamble(path: &std::path::Path) -> Result<&'static str, String> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create {}: {}", parent.display(), e))?;
        }
        std::fs::write(path, format!("{}\n", CLAUDE_MD_PREAMBLE))
            .map_err(|e| format!("Failed to create {}: {}", path.display(), e))?;
        return Ok("created");
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    if mirdan::status::preamble_present_in(&content) {
        return Ok("already present");
    }
    let new_content = format!("{}\n\n{}", CLAUDE_MD_PREAMBLE, content);
    std::fs::write(path, new_content)
        .map_err(|e| format!("Failed to update {}: {}", path.display(), e))?;
    Ok("prepended")
}

/// Remove the preamble from the instructions file at `path`. Deletes the file if it becomes empty.
///
/// `path` is the full path to the instructions file.
///
/// Returns `"removed"` if the preamble was stripped, `"deleted"` if the file
/// was deleted (only contained the preamble), `"not found"` if no file exists,
/// or `"no preamble"` if the file exists but has no preamble.
fn remove_preamble(path: &std::path::Path) -> Result<&'static str, String> {
    if !path.exists() {
        return Ok("not found");
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    if !mirdan::status::preamble_present_in(&content) {
        return Ok("no preamble");
    }
    // Remove the preamble line and any immediately following blank lines
    let mut lines = content.lines().peekable();
    let mut after_preamble = Vec::new();
    let mut found = false;
    for line in &mut lines {
        if !found && line.contains(CLAUDE_MD_PREAMBLE) {
            found = true;
            continue;
        }
        if found {
            after_preamble.push(line);
        }
    }
    // Trim leading blank lines after preamble removal
    while after_preamble.first().is_some_and(|l| l.trim().is_empty()) {
        after_preamble.remove(0);
    }
    if after_preamble.is_empty() {
        std::fs::remove_file(path)
            .map_err(|e| format!("Failed to delete {}: {}", path.display(), e))?;
        return Ok("deleted");
    }
    let new_content = after_preamble.join("\n") + "\n";
    std::fs::write(path, new_content)
        .map_err(|e| format!("Failed to update {}: {}", path.display(), e))?;
    Ok("removed")
}

impl Initializable for ClaudeMd {
    /// The component name for CLAUDE.md preamble management.
    fn name(&self) -> &str {
        "claude-md"
    }

    /// Human-readable label for this component.
    fn display_name(&self) -> &str {
        "Preamble"
    }

    /// Component category: configuration tasks.
    fn category(&self) -> &str {
        "configuration"
    }

    /// Component priority: 50 (runs after project workspace is in place, before skill/agent deployment).
    fn priority(&self) -> i32 {
        50
    }

    /// Applicable to project, local, and user scope installations.
    ///
    /// User scope targets each agent's global instructions file (e.g.
    /// `~/.claude/CLAUDE.md`); project/local target the agent's project
    /// instructions file resolved against the git root.
    fn is_applicable(&self, scope: &InitScope) -> bool {
        matches!(
            scope,
            InitScope::Project | InitScope::Local | InitScope::User
        )
    }

    /// Ensure each detected agent's instructions file has the required preamble.
    fn init(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        self.for_each_agent_path(scope, reporter, ensure_preamble_for_agent)
    }

    /// Remove the preamble from each detected agent's instructions file.
    fn deinit(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        self.for_each_agent_path(scope, reporter, remove_preamble_for_agent)
    }
}

impl ClaudeMd {
    /// Resolve each detected agent's instructions file for `scope` and apply
    /// `action` to it, collecting per-agent results.
    ///
    /// Agents whose instructions path is `None` for the scope are skipped (not
    /// applicable). In project/local scope, the agent's project-relative path is
    /// resolved against the git root; if no git root is found, a Warning is
    /// reported and no files are touched.
    fn for_each_agent_path(
        &self,
        scope: &InitScope,
        reporter: &dyn InitReporter,
        action: fn(
            &ClaudeMd,
            &mirdan::agents::AgentDef,
            &std::path::Path,
            &dyn InitReporter,
        ) -> InitResult,
    ) -> Vec<InitResult> {
        let config = match mirdan::agents::load_agents_config() {
            Ok(c) => c,
            Err(e) => {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to load agents config: {}", e),
                )];
            }
        };
        let agents = mirdan::agents::get_detected_agents(&config);

        // Project/local scope resolves agent paths relative to the git root.
        let git_root = if matches!(scope, InitScope::User) {
            None
        } else {
            match swissarmyhammer_common::utils::find_git_repository_root() {
                Some(r) => Some(r),
                None => {
                    reporter.emit(&InitEvent::Warning {
                        message: "No git repository found; skipping CLAUDE.md preamble".to_string(),
                    });
                    return vec![InitResult::error(
                        self.name(),
                        "No git repository found".to_string(),
                    )];
                }
            }
        };

        let mut results = Vec::new();
        for agent in &agents {
            let path = match resolve_instructions_file(&agent.def, scope, git_root.as_deref()) {
                Some(p) => p,
                None => continue,
            };
            results.push(action(self, &agent.def, &path, reporter));
        }

        if results.is_empty() {
            return vec![InitResult::skipped(
                self.name(),
                "No agents with an instructions file for this scope",
            )];
        }
        results
    }
}

/// Resolve the instructions file path for `agent` in the given `scope`.
///
/// * `User` → the agent's absolute global instructions file (e.g. `~/.claude/CLAUDE.md`).
/// * `Project`/`Local` → the agent's project-relative instructions file joined
///   onto `git_root` (so Claude Code keeps writing `<git-root>/CLAUDE.md`).
///
/// Returns `None` when the agent has no instructions path for the scope.
fn resolve_instructions_file(
    agent: &mirdan::agents::AgentDef,
    scope: &InitScope,
    git_root: Option<&std::path::Path>,
) -> Option<std::path::PathBuf> {
    if matches!(scope, InitScope::User) {
        mirdan::agents::agent_global_instructions_file(agent)
    } else {
        let relative = mirdan::agents::agent_project_instructions_file(agent)?;
        git_root.map(|root| root.join(relative))
    }
}

/// Ensure a single agent's instructions file has the preamble and report the outcome.
fn ensure_preamble_for_agent(
    component: &ClaudeMd,
    def: &mirdan::agents::AgentDef,
    path: &std::path::Path,
    reporter: &dyn InitReporter,
) -> InitResult {
    match ensure_preamble(path) {
        Ok("created") => {
            reporter.emit(&InitEvent::Action {
                verb: "Created".to_string(),
                message: format!("instructions for {} ({})", def.name, path.display()),
            });
            InitResult::ok(component.name(), "Instructions file created with preamble")
        }
        Ok("prepended") => {
            reporter.emit(&InitEvent::Action {
                verb: "Updated".to_string(),
                message: format!("instructions for {} ({})", def.name, path.display()),
            });
            InitResult::ok(component.name(), "Preamble prepended to instructions file")
        }
        Ok(_) => {
            reporter.emit(&InitEvent::Skipped {
                component: component.name().to_string(),
                reason: format!(
                    "{} already has the required preamble ({})",
                    def.name,
                    path.display()
                ),
            });
            InitResult::ok(component.name(), "Instructions file already has preamble")
        }
        Err(e) => InitResult::error(component.name(), e),
    }
}

/// Remove the preamble from a single agent's instructions file and report the outcome.
fn remove_preamble_for_agent(
    component: &ClaudeMd,
    def: &mirdan::agents::AgentDef,
    path: &std::path::Path,
    reporter: &dyn InitReporter,
) -> InitResult {
    match remove_preamble(path) {
        Ok("deleted") => {
            reporter.emit(&InitEvent::Action {
                verb: "Removed".to_string(),
                message: format!("instructions for {} ({})", def.name, path.display()),
            });
            InitResult::ok(component.name(), "Instructions file deleted")
        }
        Ok("removed") => {
            reporter.emit(&InitEvent::Action {
                verb: "Updated".to_string(),
                message: format!("removed preamble for {} ({})", def.name, path.display()),
            });
            InitResult::ok(component.name(), "Preamble removed from instructions file")
        }
        Ok(_) => {
            reporter.emit(&InitEvent::Skipped {
                component: component.name().to_string(),
                reason: format!(
                    "{} instructions file not found or has no preamble ({})",
                    def.name,
                    path.display()
                ),
            });
            InitResult::ok(component.name(), "Nothing to remove")
        }
        Err(e) => InitResult::error(component.name(), e),
    }
}

// ── Shared helpers ───────────────────────────────────────────────────

/// Save lockfile and emit a reporter event if any packages were installed.
pub(crate) fn save_lockfile_and_report(
    lockfile: &mirdan::lockfile::Lockfile,
    project_root: &std::path::Path,
    count: usize,
    kind: &str,
    targets: &[String],
    reporter: &dyn InitReporter,
) -> Result<(), String> {
    if count > 0 {
        lockfile
            .save(project_root)
            .map_err(|e| format!("Failed to save lockfile: {}", e))?;
        reporter.emit(&InitEvent::Action {
            verb: "Installed".to_string(),
            message: format!("{} {} → {}", count, kind, targets.join(", ")),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RAII guard for a process-wide environment variable in tests.
    ///
    /// Sets the variable on construction and calls `std::env::remove_var` on
    /// drop, so the variable is cleared even if an assertion in the test
    /// panics before reaching an explicit cleanup line. Pair with
    /// `#[serial_test::serial(...)]` on any test that reads or writes the
    /// same variable to avoid cross-test races.
    struct EnvGuard(&'static str);

    impl EnvGuard {
        /// Set `name` to `value` for the lifetime of the returned guard.
        fn set(name: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
            std::env::set_var(name, value);
            Self(name)
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            std::env::remove_var(self.0);
        }
    }

    /// Build a minimal `AgentDef` for tests that only need the agent identity +
    /// a `global_settings_path` (e.g. the install/status agreement tests for
    /// deny-Bash). All other path fields are left `None` and the detect list is
    /// empty. Centralising this means adding a new field to `AgentDef` only
    /// breaks `AgentDef` itself, not every test that happens to need a
    /// synthetic agent.
    fn synthetic_agent_def(settings: &std::path::Path) -> mirdan::agents::AgentDef {
        let s = settings.to_string_lossy().to_string();
        mirdan::agents::AgentDef {
            id: "claude-code".to_string(),
            name: "Claude Code".to_string(),
            project_path: "skills".to_string(),
            global_path: "~/global-skills".to_string(),
            detect: vec![],
            symlink_policy: mirdan::agents::SymlinkPolicy::default(),
            mcp_config: None,
            plugin_path: None,
            global_plugin_path: None,
            agent_path: None,
            global_agent_path: None,
            instructions_path: None,
            global_instructions_path: None,
            // Both scopes point at the same absolute file so callers can drive
            // either Project or User deny against a known temp path.
            settings_path: Some(s.clone()),
            global_settings_path: Some(s),
            doctor: false,
        }
    }

    /// Like [`synthetic_agent_def`] but with an `mcp_config` whose project path
    /// is the given absolute file, for exercising the strategy's `register_mcp`.
    fn synthetic_agent_with_mcp(mcp_json: &std::path::Path) -> mirdan::agents::AgentDef {
        let mut def = synthetic_agent_def(mcp_json);
        def.mcp_config = Some(mirdan::agents::McpConfigDef {
            project_path: mcp_json.to_string_lossy().to_string(),
            global_path: Some(mcp_json.to_string_lossy().to_string()),
            servers_key: "mcpServers".to_string(),
            entry_extras: std::collections::BTreeMap::new(),
        });
        def
    }

    // ── Shared-detector agreement tests ─────────────────────────────────
    //
    // These tests pin the install layer to the same predicates `mirdan::status`
    // uses for `mirdan status` / `sah doctor`. They install via the production
    // ensure/merge paths and then detect with `mirdan::status` directly, so any
    // future drift between writer and detector breaks the build.

    /// `ensure_preamble` writes a file that `mirdan::status::preamble_present`
    /// recognizes as installed.
    #[test]
    fn test_install_preamble_agrees_with_status_detector() {
        let temp = tempfile::TempDir::new().unwrap();
        let claude_md = temp.path().join("CLAUDE.md");

        // Detector reports missing before any install.
        assert!(!mirdan::status::preamble_present(&claude_md));

        // Install via the production path.
        ensure_preamble(&claude_md).unwrap();

        // Detector now reports installed.
        assert!(
            mirdan::status::preamble_present(&claude_md),
            "preamble_present must agree with what ensure_preamble wrote"
        );

        // Prepending onto an existing file must also be detected as installed.
        let other = temp.path().join("other.md");
        std::fs::write(&other, "existing project notes\n").unwrap();
        ensure_preamble(&other).unwrap();
        assert!(
            mirdan::status::preamble_present(&other),
            "preamble_present must agree after ensure_preamble prepends"
        );
    }

    /// The Claude strategy's `deny_tool` (the writer the install layer calls
    /// via [`DenyBash`]) writes a settings file that
    /// `mirdan::status::permissions_present` recognizes as installed.
    #[test]
    fn test_install_deny_bash_agrees_with_status_detector() {
        use mirdan::strategy::{AgentConfigStrategy, ClaudeCodeStrategy};

        let temp = tempfile::TempDir::new().unwrap();
        let settings = temp.path().join("settings.json");

        // Detector reports missing before any install.
        assert!(!mirdan::status::permissions_present(&settings));

        // Deny via the same strategy the DenyBash component drives, pointing the
        // agent's project settings file at the temp path.
        let def = synthetic_agent_def(&settings);
        let changed = ClaudeCodeStrategy
            .deny_tool(&def, InitScope::Project, "Bash")
            .expect("deny_tool should succeed");
        assert!(changed);

        // Detector now reports installed.
        assert!(
            mirdan::status::permissions_present(&settings),
            "permissions_present must agree with what the Claude strategy wrote"
        );
    }

    /// The Claude strategy's `register_mcp` (the writer the install layer calls
    /// via [`McpRegistration`]) writes a config file that
    /// `mirdan::status::mcp_server_installed` recognizes as installed.
    #[test]
    fn test_install_mcp_agrees_with_status_detector() {
        use mirdan::strategy::{AgentConfigStrategy, ClaudeCodeStrategy};

        let temp = tempfile::TempDir::new().unwrap();
        let mcp_json = temp.path().join("mcp.json");

        // Detector reports missing before any install.
        assert!(!mirdan::status::mcp_server_installed(&mcp_json, None));

        // Register via the Claude strategy with the agent's project MCP config
        // pointed at the temp path.
        let def = synthetic_agent_with_mcp(&mcp_json);
        ClaudeCodeStrategy
            .register_mcp(&def, InitScope::Project, "sah", &sah_mcp_server_entry())
            .expect("register_mcp should succeed");

        // Detector now reports installed.
        assert!(
            mirdan::status::mcp_server_installed(&mcp_json, None),
            "mcp_server_installed must agree with what the Claude strategy wrote"
        );
    }

    // ── McpRegistration component tests ─────────────────────────────────

    /// When no detected agent declares an `mcp_config` block, the
    /// `McpRegistration` applier completes without error and writes no
    /// non-agent-bound `.mcp.json` (the deleted legacy fallback behavior).
    #[test]
    #[serial_test::serial(home_env, cwd)]
    fn test_mcp_registration_init_skips_agent_without_mcp_config() {
        use swissarmyhammer_common::lifecycle::InitStatus;
        use swissarmyhammer_common::reporter::NullReporter;
        use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};

        let env = IsolatedTestEnvironment::new().unwrap();
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        // A single detected agent with no mcp_config (so the generic strategy
        // has nothing to register).
        std::fs::create_dir_all(env.home_path().join(".no-mcp-agent")).unwrap();
        let custom_config = env.home_path().join("agents.yaml");
        std::fs::write(
            &custom_config,
            r#"
agents:
  - id: no-mcp-agent
    name: Agent Without MCP
    project_path: .no-mcp-agent/skills
    global_path: "~/.no-mcp-agent/skills"
    detect:
      - dir: "~/.no-mcp-agent"
"#,
        )
        .unwrap();

        let _env_guard = EnvGuard::set("MIRDAN_AGENTS_CONFIG", &custom_config);
        let cwd_mcp_json = std::env::current_dir().unwrap().join(".mcp.json");
        let pre_existed = cwd_mcp_json.exists();

        let reporter = NullReporter;
        let results = McpRegistration.init(&InitScope::Project, &reporter);

        // The applier always returns a single aggregate result, never an error
        // when an agent simply has no MCP config to write.
        assert!(
            results.iter().all(|r| r.status != InitStatus::Error),
            "applier must not error for an agent without mcp_config: {results:?}"
        );

        // The deleted legacy fallback would have written a project `.mcp.json`;
        // the strategy-driven path must not touch it.
        if !pre_existed {
            assert!(
                !cwd_mcp_json.exists(),
                "no agent-bound .mcp.json should be written for an agent without mcp_config"
            );
        }
    }

    #[test]
    fn test_claude_md_creates_file_when_absent() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("CLAUDE.md");
        let result = ensure_preamble(&path).unwrap();
        assert_eq!(result, "created");

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.starts_with(CLAUDE_MD_PREAMBLE));
        assert!(content.ends_with('\n'));
    }

    #[test]
    fn test_claude_md_creates_parent_dirs() {
        // ensure_preamble must create missing parent directories so it works
        // for an absolute global file like ~/.claude/CLAUDE.md.
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("nested").join("dir").join("CLAUDE.md");
        let result = ensure_preamble(&path).unwrap();
        assert_eq!(result, "created");
        assert!(path.exists());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.starts_with(CLAUDE_MD_PREAMBLE));
    }

    #[test]
    fn test_claude_md_prepends_preamble_to_existing() {
        let temp = tempfile::TempDir::new().unwrap();
        let claude_md = temp.path().join("CLAUDE.md");
        std::fs::write(&claude_md, "existing content\n").unwrap();

        let result = ensure_preamble(&claude_md).unwrap();
        assert_eq!(result, "prepended");

        let content = std::fs::read_to_string(&claude_md).unwrap();
        assert!(content.starts_with(CLAUDE_MD_PREAMBLE));
        assert!(content.contains("existing content"));
    }

    #[test]
    fn test_claude_md_idempotent() {
        let temp = tempfile::TempDir::new().unwrap();
        let claude_md = temp.path().join("CLAUDE.md");
        std::fs::write(&claude_md, format!("{}\n", CLAUDE_MD_PREAMBLE)).unwrap();

        let result = ensure_preamble(&claude_md).unwrap();
        assert_eq!(result, "already present");

        let content = std::fs::read_to_string(&claude_md).unwrap();
        // Should not have doubled the preamble
        assert_eq!(content.matches(CLAUDE_MD_PREAMBLE).count(), 1);
    }

    #[test]
    fn test_claude_md_has_preamble_absent() {
        let temp = tempfile::TempDir::new().unwrap();
        assert_eq!(
            preamble_file_has_preamble(&temp.path().join("CLAUDE.md")),
            None
        );
    }

    #[test]
    fn test_claude_md_has_preamble_present() {
        let temp = tempfile::TempDir::new().unwrap();
        let claude_md = temp.path().join("CLAUDE.md");
        std::fs::write(&claude_md, format!("{}\nother stuff\n", CLAUDE_MD_PREAMBLE)).unwrap();
        assert_eq!(preamble_file_has_preamble(&claude_md), Some(true));
    }

    #[test]
    fn test_claude_md_has_preamble_missing() {
        let temp = tempfile::TempDir::new().unwrap();
        let claude_md = temp.path().join("CLAUDE.md");
        std::fs::write(&claude_md, "some other content\n").unwrap();
        assert_eq!(preamble_file_has_preamble(&claude_md), Some(false));
    }

    #[test]
    fn test_claude_md_deinit_deletes_preamble_only_file() {
        let temp = tempfile::TempDir::new().unwrap();
        let claude_md = temp.path().join("CLAUDE.md");
        std::fs::write(&claude_md, format!("{}\n", CLAUDE_MD_PREAMBLE)).unwrap();

        let result = remove_preamble(&claude_md).unwrap();
        assert_eq!(result, "deleted");
        assert!(!claude_md.exists());
    }

    #[test]
    fn test_claude_md_deinit_strips_preamble_keeps_content() {
        let temp = tempfile::TempDir::new().unwrap();
        let claude_md = temp.path().join("CLAUDE.md");
        std::fs::write(
            &claude_md,
            format!("{}\n\nmy project notes\nmore stuff\n", CLAUDE_MD_PREAMBLE),
        )
        .unwrap();

        let result = remove_preamble(&claude_md).unwrap();
        assert_eq!(result, "removed");
        let content = std::fs::read_to_string(&claude_md).unwrap();
        assert!(!content.contains(CLAUDE_MD_PREAMBLE));
        assert!(content.contains("my project notes"));
        assert!(content.contains("more stuff"));
    }

    #[test]
    fn test_claude_md_deinit_no_file() {
        let temp = tempfile::TempDir::new().unwrap();
        let result = remove_preamble(&temp.path().join("CLAUDE.md")).unwrap();
        assert_eq!(result, "not found");
    }

    #[test]
    fn test_claude_md_deinit_no_preamble() {
        let temp = tempfile::TempDir::new().unwrap();
        let claude_md = temp.path().join("CLAUDE.md");
        std::fs::write(&claude_md, "just user content\n").unwrap();

        let result = remove_preamble(&claude_md).unwrap();
        assert_eq!(result, "no preamble");
        // File should be untouched
        let content = std::fs::read_to_string(&claude_md).unwrap();
        assert_eq!(content, "just user content\n");
    }

    #[test]
    fn test_claude_md_is_applicable_user_scope() {
        // Regression: the preamble component must run in user scope so
        // `sah init user` installs the global instructions file.
        assert!(ClaudeMd.is_applicable(&InitScope::User));
        assert!(ClaudeMd.is_applicable(&InitScope::Project));
        assert!(ClaudeMd.is_applicable(&InitScope::Local));
    }

    #[test]
    #[serial_test::serial(home_env)]
    fn test_claude_md_user_scope_writes_global_file() {
        use swissarmyhammer_common::lifecycle::InitStatus;
        use swissarmyhammer_common::reporter::NullReporter;
        use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;

        let env = IsolatedTestEnvironment::new().unwrap();
        // claude-code's global instructions file is ~/.claude/CLAUDE.md, which
        // resolves under the isolated HOME.
        let global_claude_md = env.home_path().join(".claude").join("CLAUDE.md");
        assert!(!global_claude_md.exists());

        let reporter = NullReporter;

        // init in user scope must create ~/.claude/CLAUDE.md with the marker.
        let results = ClaudeMd.init(&InitScope::User, &reporter);
        assert!(
            results.iter().all(|r| r.status != InitStatus::Error),
            "init produced errors: {:?}",
            results
        );
        assert!(
            global_claude_md.exists(),
            "expected {} to exist after init user",
            global_claude_md.display()
        );
        assert_eq!(preamble_file_has_preamble(&global_claude_md), Some(true));

        // Idempotent: running init again makes no change to the marker count.
        ClaudeMd.init(&InitScope::User, &reporter);
        let content = std::fs::read_to_string(&global_claude_md).unwrap();
        assert_eq!(content.matches(CLAUDE_MD_PREAMBLE).count(), 1);

        // deinit removes the preamble-only file.
        let results = ClaudeMd.deinit(&InitScope::User, &reporter);
        assert!(
            results.iter().all(|r| r.status != InitStatus::Error),
            "deinit produced errors: {:?}",
            results
        );
        assert!(
            !global_claude_md.exists(),
            "expected {} to be removed after deinit user",
            global_claude_md.display()
        );
    }

    // ── DenyBash component tests ────────────────────────────────────────

    #[test]
    fn test_deny_bash_is_applicable_all_scopes() {
        // Regression: the DenyBash component must run in user scope so
        // `sah init user` writes permissions.deny to the global settings file.
        assert!(DenyBash.is_applicable(&InitScope::User));
        assert!(DenyBash.is_applicable(&InitScope::Project));
        assert!(DenyBash.is_applicable(&InitScope::Local));
    }

    #[test]
    #[serial_test::serial(home_env)]
    fn test_deny_bash_user_scope_writes_global_settings() {
        use swissarmyhammer_common::lifecycle::InitStatus;
        use swissarmyhammer_common::reporter::NullReporter;
        use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;

        let env = IsolatedTestEnvironment::new().unwrap();
        // claude-code's global settings file is ~/.claude/settings.json, which
        // resolves under the isolated HOME.
        let global_settings = env.home_path().join(".claude").join("settings.json");
        assert!(!global_settings.exists());

        let reporter = NullReporter;

        // init in user scope must create ~/.claude/settings.json with Bash deny.
        let results = DenyBash.init(&InitScope::User, &reporter);
        assert!(
            results.iter().all(|r| r.status != InitStatus::Error),
            "init produced errors: {:?}",
            results
        );
        assert!(
            global_settings.exists(),
            "expected {} to exist after DenyBash init user",
            global_settings.display()
        );
        let content = std::fs::read_to_string(&global_settings).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let deny = parsed
            .pointer("/permissions/deny")
            .and_then(|v| v.as_array())
            .expect("permissions.deny must exist as an array");
        assert!(
            deny.iter().any(|v| v.as_str() == Some("Bash")),
            "expected Bash in permissions.deny, got {:?}",
            deny
        );

        // Idempotent: running init again leaves a single Bash entry.
        DenyBash.init(&InitScope::User, &reporter);
        let content = std::fs::read_to_string(&global_settings).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let deny = parsed
            .pointer("/permissions/deny")
            .and_then(|v| v.as_array())
            .unwrap();
        let bash_count = deny.iter().filter(|v| v.as_str() == Some("Bash")).count();
        assert_eq!(
            bash_count, 1,
            "Bash should appear exactly once after re-init"
        );

        // deinit removes the Bash entry.
        let results = DenyBash.deinit(&InitScope::User, &reporter);
        assert!(
            results.iter().all(|r| r.status != InitStatus::Error),
            "deinit produced errors: {:?}",
            results
        );
        let content = std::fs::read_to_string(&global_settings).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let deny_after = parsed
            .pointer("/permissions/deny")
            .and_then(|v| v.as_array());
        if let Some(arr) = deny_after {
            assert!(
                !arr.iter().any(|v| v.as_str() == Some("Bash")),
                "Bash should be gone from permissions.deny after deinit user"
            );
        }
    }

    // ── Statusline component tests ──────────────────────────────────────

    #[test]
    fn test_statusline_is_applicable_all_scopes() {
        // Regression: the Statusline component must run in user scope so
        // `sah init user` writes the statusLine block to the global settings file.
        assert!(Statusline.is_applicable(&InitScope::User));
        assert!(Statusline.is_applicable(&InitScope::Project));
        assert!(Statusline.is_applicable(&InitScope::Local));
    }

    #[test]
    #[serial_test::serial(home_env)]
    fn test_statusline_user_scope_writes_global_settings() {
        use swissarmyhammer_common::lifecycle::InitStatus;
        use swissarmyhammer_common::reporter::NullReporter;
        use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;

        let env = IsolatedTestEnvironment::new().unwrap();
        let global_settings = env.home_path().join(".claude").join("settings.json");
        assert!(!global_settings.exists());

        let reporter = NullReporter;

        // init in user scope must write statusLine to ~/.claude/settings.json.
        let results = Statusline.init(&InitScope::User, &reporter);
        assert!(
            results.iter().all(|r| r.status != InitStatus::Error),
            "init produced errors: {:?}",
            results
        );
        assert!(
            global_settings.exists(),
            "expected {} to exist after Statusline init user",
            global_settings.display()
        );
        let content = std::fs::read_to_string(&global_settings).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(
            parsed.pointer("/statusLine/type").and_then(|v| v.as_str()),
            Some("command")
        );
        assert_eq!(
            parsed
                .pointer("/statusLine/command")
                .and_then(|v| v.as_str()),
            Some("sah statusline")
        );

        // Idempotent: re-running init keeps the statusLine block intact.
        Statusline.init(&InitScope::User, &reporter);
        let content = std::fs::read_to_string(&global_settings).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed.get("statusLine").is_some());

        // deinit removes the statusLine block.
        let results = Statusline.deinit(&InitScope::User, &reporter);
        assert!(
            results.iter().all(|r| r.status != InitStatus::Error),
            "deinit produced errors: {:?}",
            results
        );
        let content = std::fs::read_to_string(&global_settings).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(
            parsed.get("statusLine").is_none(),
            "statusLine should be gone after deinit user, got {:?}",
            parsed
        );
    }

    // ── Lockfile path scope-awareness regression tests ──────────────────

    /// Regression: `sah init user` must write the lockfile under the user's
    /// home directory (alongside `~/.skills/` and `~/.agents/`), never under
    /// the current working directory.
    ///
    /// The bug being guarded against: `AgentDeployment::init` previously
    /// resolved the lockfile root as `std::env::current_dir()` regardless of
    /// scope, so running `sah init user` from `~/some/project` left a stray
    /// `~/some/project/mirdan-lock.json` behind. The lockfile and the store
    /// it tracks must agree on which directory holds the install.
    #[test]
    #[serial_test::serial(home_env)]
    fn test_agent_deployment_user_scope_lockfile_in_home_not_cwd() {
        use swissarmyhammer_common::lifecycle::InitStatus;
        use swissarmyhammer_common::reporter::NullReporter;
        use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};

        let env = IsolatedTestEnvironment::new().unwrap();
        // Use a non-HOME temp directory as cwd so the bug — writing to
        // current_dir() — would leave behind a detectable stray file.
        let cwd_dir = tempfile::tempdir().unwrap();
        let _cwd_guard = CurrentDirGuard::new(cwd_dir.path()).unwrap();

        let home_lockfile = env.home_path().join("mirdan-lock.json");
        let cwd_lockfile = cwd_dir.path().join("mirdan-lock.json");

        assert!(!home_lockfile.exists());
        assert!(!cwd_lockfile.exists());

        let reporter = NullReporter;
        let results = AgentDeployment.init(&InitScope::User, &reporter);

        // We only assert on the cwd-pollution invariant, not on per-agent
        // success. Whether builtin agents actually deploy depends on which
        // global agent configs exist in the isolated HOME, but the lockfile
        // path resolution must be scope-aware regardless of that.
        assert!(
            results.iter().all(|r| r.status != InitStatus::Error),
            "init user produced errors: {:?}",
            results
        );

        // The bug: a stray mirdan-lock.json appeared in the cwd. Guard
        // against any regression by failing loudly if it shows up.
        assert!(
            !cwd_lockfile.exists(),
            "user-scope init must not write {} (cwd pollution)",
            cwd_lockfile.display()
        );

        // The lockfile must land under the isolated HOME, matching the
        // global skill/agent stores at `~/.skills/` and `~/.agents/`.
        assert!(
            home_lockfile.exists(),
            "user-scope init must write {}",
            home_lockfile.display()
        );
    }

    /// Regression: `sah deinit user` must clean up the global lockfile, not a
    /// cwd-relative one. Pre-populates `~/mirdan-lock.json` and verifies that
    /// `LockfileCleanup::deinit` with `InitScope::User` is what touches it.
    #[test]
    #[serial_test::serial(home_env)]
    fn test_lockfile_cleanup_user_scope_targets_home_not_cwd() {
        use swissarmyhammer_common::lifecycle::InitStatus;
        use swissarmyhammer_common::reporter::NullReporter;
        use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};

        let env = IsolatedTestEnvironment::new().unwrap();
        let cwd_dir = tempfile::tempdir().unwrap();
        let _cwd_guard = CurrentDirGuard::new(cwd_dir.path()).unwrap();

        // Seed both a "global" lockfile and a "cwd" lockfile so we can detect
        // which one deinit touches.
        let home_lockfile = env.home_path().join("mirdan-lock.json");
        let cwd_lockfile = cwd_dir.path().join("mirdan-lock.json");
        let empty_lockfile = r#"{"lockfile_version":1,"packages":{}}"#;
        let sentinel = r#"{"lockfile_version":1,"packages":{},"_sentinel":"do-not-touch"}"#;
        std::fs::write(&home_lockfile, empty_lockfile).unwrap();
        std::fs::write(&cwd_lockfile, sentinel).unwrap();

        let reporter = NullReporter;
        let results = LockfileCleanup.deinit(&InitScope::User, &reporter);
        assert!(
            results.iter().all(|r| r.status != InitStatus::Error),
            "deinit user produced errors: {:?}",
            results
        );

        // The cwd lockfile must be untouched by user-scope deinit. It is
        // someone else's file — the bug wrote it there in the first place,
        // and a scope-aware cleanup would silently delete it or rewrite it on
        // the way out. The sentinel field survives unchanged only if cleanup
        // never reads or writes this file.
        let cwd_content = std::fs::read_to_string(&cwd_lockfile).unwrap();
        assert_eq!(
            cwd_content,
            sentinel,
            "user-scope deinit must not touch {} (cwd pollution)",
            cwd_lockfile.display()
        );

        // And the home lockfile should still exist (it was rewritten after
        // builtin entries were removed, but the file must remain).
        assert!(
            home_lockfile.exists(),
            "user-scope deinit must operate on {}",
            home_lockfile.display()
        );
    }
}
