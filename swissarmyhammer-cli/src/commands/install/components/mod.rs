//! Composable `Initializable` components for sah init/deinit.
//!
//! Each component encapsulates one aspect of the install/uninstall lifecycle
//! and implements the `Initializable` trait from `swissarmyhammer_common::lifecycle`.

use std::fs;

use swissarmyhammer_common::lifecycle::{InitRegistry, InitResult, InitScope, Initializable};
use swissarmyhammer_common::reporter::{InitEvent, InitReporter};
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_prompts::PromptLibrary;

use super::settings;

/// Register all install/uninstall components into the given registry.
///
/// * `global` - Whether to target user-level (global) paths vs project-level.
/// * `remove_directory` - Whether `ProjectStructure::deinit` should delete directories.
pub fn register_all(registry: &mut InitRegistry, global: bool, remove_directory: bool) {
    registry.register(McpRegistration::new(global));
    registry.register(ClaudeLocalScope);
    registry.register(DenyBash);
    registry.register(ProjectStructure::new(remove_directory));
    registry.register(ClaudeMd);
    registry.register(SkillDeployment::new(global));
    registry.register(AgentDeployment::new(global));
    registry.register(LockfileCleanup);

    // Register tools that have lifecycle operations.
    // Each tool implements Initializable — tools with no-op init/deinit
    // are harmless to include (they'll be skipped automatically).
    registry.register(swissarmyhammer_tools::mcp::tools::kanban::KanbanTool);
}

// ── McpRegistration (priority 10) ────────────────────────────────────

/// Registers/unregisters sah as an MCP server in all detected agent configs.
pub struct McpRegistration {
    global: bool,
}

impl McpRegistration {
    /// Create a new McpRegistration component.
    pub fn new(global: bool) -> Self {
        Self { global }
    }
}

impl Initializable for McpRegistration {
    /// The component name for MCP server registration.
    fn name(&self) -> &str {
        "mcp-registration"
    }

    /// Component category: configuration tasks.
    fn category(&self) -> &str {
        "configuration"
    }

    /// Component priority: 10 (runs early, before other configuration).
    fn priority(&self) -> i32 {
        10
    }

    /// Install sah MCP server to all detected agents using mirdan's mcp_config.
    fn init(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        let mut results = Vec::new();

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

        let entry = mirdan::mcp_config::McpServerEntry {
            command: "sah".to_string(),
            args: vec!["serve".to_string()],
            env: std::collections::BTreeMap::new(),
        };

        let mut installed_count = 0;
        for agent in &agents {
            if register_agent_mcp(&agent.def, &entry, self.global, reporter) {
                installed_count += 1;
            }
        }

        if installed_count == 0 {
            // Fallback to legacy settings.rs for backward compat
            if let Err(e) = install_project_legacy(reporter) {
                results.push(InitResult::error(self.name(), e));
                return results;
            }
        }

        results.push(InitResult::ok(
            self.name(),
            format!("MCP server registered for {} agents", installed_count),
        ));
        results
    }

    /// Remove sah MCP server from all detected agents using mirdan's mcp_config.
    fn deinit(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
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

        let mut removed_count = 0;
        for agent in &agents {
            if unregister_agent_mcp(&agent.def, self.global, reporter) {
                removed_count += 1;
            }
        }

        if removed_count == 0 {
            // Fallback: try legacy project-level removal
            if let Err(e) = uninstall_project_legacy(reporter) {
                return vec![InitResult::error(self.name(), e)];
            }
        }

        vec![InitResult::ok(
            self.name(),
            format!("MCP server unregistered from {} agents", removed_count),
        )]
    }
}

/// Register the sah MCP server for a single agent. Returns true if installed.
fn register_agent_mcp(
    def: &mirdan::agents::AgentDef,
    entry: &mirdan::mcp_config::McpServerEntry,
    global: bool,
    reporter: &dyn InitReporter,
) -> bool {
    let mcp_def = match &def.mcp_config {
        Some(m) => m,
        None => return false,
    };
    let config_path = if global {
        mirdan::agents::agent_global_mcp_config(def)
    } else {
        mirdan::agents::agent_project_mcp_config(def)
    };
    let config_path = match config_path {
        Some(p) => p,
        None => return false,
    };

    match mirdan::mcp_config::register_mcp_server(&config_path, &mcp_def.servers_key, "sah", entry)
    {
        Ok(()) => {
            reporter.emit(&InitEvent::Action {
                verb: "Installed".to_string(),
                message: format!("MCP server for {} ({})", def.name, config_path.display()),
            });
            true
        }
        Err(e) => {
            reporter.emit(&InitEvent::Warning {
                message: format!("failed to install MCP for {}: {}", def.name, e),
            });
            false
        }
    }
}

/// Unregister the sah MCP server from a single agent. Returns true if removed.
fn unregister_agent_mcp(
    def: &mirdan::agents::AgentDef,
    global: bool,
    reporter: &dyn InitReporter,
) -> bool {
    let mcp_def = match &def.mcp_config {
        Some(m) => m,
        None => return false,
    };
    let config_path = if global {
        mirdan::agents::agent_global_mcp_config(def)
    } else {
        mirdan::agents::agent_project_mcp_config(def)
    };
    let config_path = match config_path {
        Some(p) => p,
        None => return false,
    };

    match mirdan::mcp_config::unregister_mcp_server(&config_path, &mcp_def.servers_key, "sah") {
        Ok(true) => {
            reporter.emit(&InitEvent::Action {
                verb: "Removed".to_string(),
                message: format!("MCP server from {} ({})", def.name, config_path.display()),
            });
            true
        }
        Ok(false) => false,
        Err(e) => {
            reporter.emit(&InitEvent::Warning {
                message: format!("failed to remove MCP from {}: {}", def.name, e),
            });
            false
        }
    }
}

/// Legacy project-level install via .mcp.json (backward compat fallback).
fn install_project_legacy(reporter: &dyn InitReporter) -> Result<(), String> {
    let path = settings::mcp_json_path();
    let mut mcp_settings = settings::read_settings(&path)?;
    let changed = settings::merge_mcp_server(&mut mcp_settings);
    settings::write_settings(&path, &mcp_settings)?;

    if changed {
        reporter.emit(&InitEvent::Action {
            verb: "Installed".to_string(),
            message: format!("MCP server to {}", path.display()),
        });
    } else {
        reporter.emit(&InitEvent::Skipped {
            component: "MCP".to_string(),
            reason: format!("MCP server already configured in {}", path.display()),
        });
    }
    Ok(())
}

/// Legacy project-level uninstall via .mcp.json (backward compat fallback).
fn uninstall_project_legacy(reporter: &dyn InitReporter) -> Result<(), String> {
    let path = settings::mcp_json_path();
    if !path.exists() {
        reporter.emit(&InitEvent::Skipped {
            component: "mcp-registration".to_string(),
            reason: format!("No {} file found, nothing to uninstall", path.display()),
        });
        return Ok(());
    }

    let mut mcp_settings = settings::read_settings(&path)?;
    let changed = settings::remove_mcp_server(&mut mcp_settings);
    cleanup_empty_mcp_servers(&mut mcp_settings);

    if mcp_settings == serde_json::json!({}) {
        fs::remove_file(&path)
            .map_err(|e| format!("Failed to remove {}: {}", path.display(), e))?;
        reporter.emit(&InitEvent::Action {
            verb: "Removed".to_string(),
            message: format!("MCP server, removed empty {}", path.display()),
        });
    } else if changed {
        settings::write_settings(&path, &mcp_settings)?;
        reporter.emit(&InitEvent::Action {
            verb: "Removed".to_string(),
            message: format!("MCP server from {}", path.display()),
        });
    } else {
        reporter.emit(&InitEvent::Skipped {
            component: "mcp-registration".to_string(),
            reason: format!("MCP server was not configured in {}", path.display()),
        });
    }

    Ok(())
}

// ── ClaudeLocalScope (priority 11) ───────────────────────────────────

/// Manages Claude Code local-scope config in `~/.claude.json`.
pub struct ClaudeLocalScope;

impl Initializable for ClaudeLocalScope {
    /// The component name for Claude Code local scope configuration.
    fn name(&self) -> &str {
        "claude-local-scope"
    }

    /// Component category: configuration tasks.
    fn category(&self) -> &str {
        "configuration"
    }

    /// Component priority: 11 (runs after global MCP registration).
    fn priority(&self) -> i32 {
        11
    }

    /// Only applicable to local scope installations.
    fn is_applicable(&self, scope: &InitScope) -> bool {
        matches!(scope, InitScope::Local)
    }

    /// Install to local scope: `~/.claude.json` under `projects.<project-path>.mcpServers`.
    fn init(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        let path = settings::claude_json_path();
        let key = match settings::project_key() {
            Ok(k) => k,
            Err(e) => return vec![InitResult::error(self.name(), e)],
        };

        let mut root = match settings::read_settings(&path) {
            Ok(r) => r,
            Err(e) => return vec![InitResult::error(self.name(), e)],
        };
        let entry = settings::ensure_project_entry(&mut root, &key);
        let changed = settings::merge_mcp_server(entry);
        if let Err(e) = settings::write_settings(&path, &root) {
            return vec![InitResult::error(self.name(), e)];
        }

        if changed {
            reporter.emit(&InitEvent::Action {
                verb: "Installed".to_string(),
                message: format!(
                    "MCP server to {} (local scope, project: {})",
                    path.display(),
                    key
                ),
            });
        } else {
            reporter.emit(&InitEvent::Skipped {
                component: "MCP".to_string(),
                reason: format!(
                    "MCP server already configured in {} (local scope, project: {})",
                    path.display(),
                    key
                ),
            });
        }
        vec![InitResult::ok(self.name(), "Claude local scope configured")]
    }

    /// Uninstall from local scope: `~/.claude.json` under `projects.<project-path>.mcpServers`.
    fn deinit(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        let path = settings::claude_json_path();
        let key = match settings::project_key() {
            Ok(k) => k,
            Err(e) => return vec![InitResult::error(self.name(), e)],
        };

        if !path.exists() {
            reporter.emit(&InitEvent::Skipped {
                component: self.name().to_string(),
                reason: format!("No {} file found, nothing to uninstall", path.display()),
            });
            return vec![InitResult::ok(self.name(), "Nothing to uninstall")];
        }

        let mut root = match settings::read_settings(&path) {
            Ok(r) => r,
            Err(e) => return vec![InitResult::error(self.name(), e)],
        };

        let changed = remove_mcp_from_project_entry(&mut root, &key);

        if changed {
            if let Err(e) = settings::write_settings(&path, &root) {
                return vec![InitResult::error(self.name(), e)];
            }
            reporter.emit(&InitEvent::Action {
                verb: "Removed".to_string(),
                message: format!(
                    "MCP server from {} (local scope, project: {})",
                    path.display(),
                    key
                ),
            });
        } else {
            reporter.emit(&InitEvent::Skipped {
                component: self.name().to_string(),
                reason: format!(
                    "MCP server was not configured in {} (local scope, project: {})",
                    path.display(),
                    key
                ),
            });
        }

        vec![InitResult::ok(
            self.name(),
            "Claude local scope unconfigured",
        )]
    }
}

/// Remove the MCP server from a specific project entry in `~/.claude.json`.
///
/// Returns true if the server was found and removed.
fn remove_mcp_from_project_entry(root: &mut serde_json::Value, key: &str) -> bool {
    let entry = match root.get_mut("projects").and_then(|p| p.get_mut(key)) {
        Some(e) => e,
        None => return false,
    };

    let changed = settings::remove_mcp_server(entry);

    // Clean up empty mcpServers object
    let should_remove = entry
        .get("mcpServers")
        .and_then(|m| m.as_object())
        .map(|m| m.is_empty())
        .unwrap_or(false);
    if should_remove {
        if let Some(obj) = entry.as_object_mut() {
            obj.remove("mcpServers");
        }
    }

    changed
}

// ── DenyBash (priority 15) ───────────────────────────────────────────

/// Manages the "Bash" deny rule in `.claude/settings.json`.
pub struct DenyBash;

impl Initializable for DenyBash {
    /// The component name for Bash denial rule configuration.
    fn name(&self) -> &str {
        "deny-bash"
    }

    /// Component category: configuration tasks.
    fn category(&self) -> &str {
        "configuration"
    }

    /// Component priority: 15 (runs after global MCP registration).
    fn priority(&self) -> i32 {
        15
    }

    /// This component only applies in user (global) scope.
    fn is_applicable(&self, scope: &InitScope) -> bool {
        matches!(scope, InitScope::Project | InitScope::Local)
    }

    /// Add "Bash" to permissions.deny in .claude/settings.json.
    #[allow(deprecated)]
    fn init(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        let path = settings::claude_settings_path();
        let mut claude_settings = match settings::read_settings(&path) {
            Ok(s) => s,
            Err(e) => return vec![InitResult::error(self.name(), e)],
        };
        let changed = settings::merge_deny_bash(&mut claude_settings);
        if let Err(e) = settings::write_settings(&path, &claude_settings) {
            return vec![InitResult::error(self.name(), e)];
        }

        if changed {
            reporter.emit(&InitEvent::Action {
                verb: "Configured".to_string(),
                message: format!(
                    "Bash tool denied in {} (use shell tool instead)",
                    path.display()
                ),
            });
        }
        vec![InitResult::ok(self.name(), "Bash deny rule configured")]
    }

    /// Remove "Bash" from permissions.deny in .claude/settings.json.
    #[allow(deprecated)]
    fn deinit(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        let path = settings::claude_settings_path();
        if !path.exists() {
            return vec![InitResult::ok(self.name(), "Settings file not found")];
        }

        let mut claude_settings = match settings::read_settings(&path) {
            Ok(s) => s,
            Err(e) => return vec![InitResult::error(self.name(), e)],
        };
        let changed = settings::remove_deny_bash(&mut claude_settings);

        if changed {
            if let Err(e) = settings::write_settings(&path, &claude_settings) {
                return vec![InitResult::error(self.name(), e)];
            }
            reporter.emit(&InitEvent::Action {
                verb: "Removed".to_string(),
                message: format!("Bash tool deny rule from {}", path.display()),
            });
        }
        vec![InitResult::ok(self.name(), "Bash deny rule removed")]
    }
}

// ── ProjectStructure (priority 20) ───────────────────────────────────

/// Creates/removes the `.sah/` and `.prompts/` project directories.
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

    /// Component category: structural setup tasks.
    fn category(&self) -> &str {
        "structure"
    }

    /// Component priority: 20 (runs after configuration setup).
    fn priority(&self) -> i32 {
        20
    }

    /// Only applicable to project and local scope installations.
    fn is_applicable(&self, scope: &InitScope) -> bool {
        matches!(scope, InitScope::Project | InitScope::Local)
    }

    /// Create the project directory structure with .prompts, .sah, and workflows.
    fn init(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        use swissarmyhammer_common::SwissarmyhammerDirectory;

        let sah_dir = match SwissarmyhammerDirectory::from_git_root().or_else(|_| {
            let cwd = std::env::current_dir()
                .map_err(|e| format!("Failed to get current directory: {}", e))?;
            SwissarmyhammerDirectory::from_custom_root(cwd)
                .map_err(|e| format!("Failed to create .sah directory: {}", e))
        }) {
            Ok(d) => d,
            Err(e) => return vec![InitResult::error(self.name(), e)],
        };

        // Create .prompts/ as a sibling to .sah/ (dot-directory path for PromptResolver)
        let project_root = match sah_dir.root().parent() {
            Some(p) => p,
            None => {
                return vec![InitResult::error(
                    self.name(),
                    "Failed to determine project root from .sah directory".to_string(),
                )];
            }
        };
        let prompts_dir = project_root.join(".prompts");
        if let Err(e) = fs::create_dir_all(&prompts_dir) {
            return vec![InitResult::error(
                self.name(),
                format!("Failed to create .prompts directory: {}", e),
            )];
        }

        if let Err(e) = sah_dir.ensure_subdir("workflows") {
            return vec![InitResult::error(
                self.name(),
                format!("Failed to create workflows directory: {}", e),
            )];
        }

        reporter.emit(&InitEvent::Action {
            verb: "Created".to_string(),
            message: format!("project structure at {}", sah_dir.root().display()),
        });

        vec![InitResult::ok(self.name(), "Project structure initialized")]
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

// ── SkillDeployment (priority 30) ────────────────────────────────────

/// Deploys/removes builtin skills via mirdan's store + lockfile.
pub struct SkillDeployment {
    global: bool,
}

impl SkillDeployment {
    /// Create a new SkillDeployment component.
    pub fn new(global: bool) -> Self {
        Self { global }
    }
}

impl Initializable for SkillDeployment {
    /// The component name for skill deployment.
    fn name(&self) -> &str {
        "skill-deployment"
    }

    /// Component category: deployment tasks.
    fn category(&self) -> &str {
        "deployment"
    }

    /// Component priority: 30 (runs after structure setup).
    fn priority(&self) -> i32 {
        30
    }

    /// Install builtin skills via mirdan's deploy + lockfile.
    ///
    /// Skill instructions are rendered through the prompt library's Liquid template
    /// engine before writing to disk, so `{% include %}` partials are expanded.
    fn init(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        match deploy_all_skills(self.global, reporter) {
            Ok(msg) => vec![InitResult::ok(self.name(), msg)],
            Err(e) => vec![InitResult::error(self.name(), e)],
        }
    }

    /// Remove builtin skill symlinks from agent directories and clean up the .skills/ store.
    fn deinit(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        use swissarmyhammer_skills::SkillResolver;

        let store_dir = mirdan::store::skill_store_dir(self.global);

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

        let resolver = SkillResolver::new();
        let builtins = resolver.resolve_builtins();
        let builtin_names: Vec<String> = builtins.keys().cloned().collect();

        let link_dirs: Vec<std::path::PathBuf> = agents
            .iter()
            .map(|agent| {
                if self.global {
                    mirdan::agents::agent_global_skill_dir(&agent.def)
                } else {
                    mirdan::agents::agent_project_skill_dir(&agent.def)
                }
            })
            .collect();

        let symlink_policies: Vec<_> = agents
            .iter()
            .map(|agent| agent.def.symlink_policy.clone())
            .collect();

        let agent_names: Vec<String> = agents.iter().map(|a| a.def.id.clone()).collect();

        remove_store_entries(
            &store_dir,
            &builtin_names,
            &link_dirs,
            &symlink_policies,
            "skill",
            reporter,
        );

        reporter.emit(&InitEvent::Action {
            verb: "Removed".to_string(),
            message: format!(
                "{} skills from {}",
                builtin_names.len(),
                agent_names.join(", ")
            ),
        });

        vec![InitResult::ok(self.name(), "Builtin skills removed")]
    }
}

/// Deploy a single builtin skill to a temp dir and then to agents.
///
/// Returns the list of agent targets on success, or an error message.
fn deploy_single_skill(
    name: &str,
    skill: &swissarmyhammer_skills::Skill,
    prompt_library: &PromptLibrary,
    template_context: &TemplateContext,
    global: bool,
    reporter: &dyn InitReporter,
) -> Result<Vec<String>, String> {
    if !is_safe_name(name) {
        return Err(format!("Unsafe skill name: {:?}", name));
    }

    let temp_dir = tempfile::tempdir().map_err(|e| format!("Failed to create temp dir: {}", e))?;
    let skill_dir = temp_dir.path().join(name);
    fs::create_dir_all(&skill_dir)
        .map_err(|e| format!("Failed to create temp skill dir: {}", e))?;

    let rendered_skill =
        render_skill_instructions(skill, prompt_library, template_context, reporter);

    let skill_md_path = skill_dir.join("SKILL.md");
    let content = format_skill_md(&rendered_skill);
    fs::write(&skill_md_path, &content)
        .map_err(|e| format!("Failed to write {}: {}", skill_md_path.display(), e))?;

    for (filename, file_content) in &skill.resources.files {
        if !is_safe_name(filename) {
            return Err(format!("Unsafe resource filename: {:?}", filename));
        }
        let file_path = skill_dir.join(filename);
        fs::write(&file_path, file_content)
            .map_err(|e| format!("Failed to write {}: {}", file_path.display(), e))?;
    }

    mirdan::install::deploy_skill_to_agents(name, &skill_dir, None, global)
        .map_err(|e| format!("Failed to deploy skill '{}': {}", name, e))
}

/// Deploy all builtin skills, update lockfile, and report results.
fn deploy_all_skills(global: bool, reporter: &dyn InitReporter) -> Result<String, String> {
    use swissarmyhammer_skills::SkillResolver;

    let resolver = SkillResolver::new();
    let skills = resolver.resolve_builtins();

    let prompt_library = PromptLibrary::default();
    let mut template_context = TemplateContext::new();
    template_context.set(
        "version".to_string(),
        serde_json::json!(env!("CARGO_PKG_VERSION")),
    );

    let project_root =
        std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
    let mut lockfile = mirdan::lockfile::Lockfile::load(&project_root)
        .map_err(|e| format!("Failed to load lockfile: {}", e))?;

    let mut installed_count = 0;
    let mut skill_targets: Vec<String> = Vec::new();

    for (name, skill) in &skills {
        let targets = deploy_single_skill(
            name,
            skill,
            &prompt_library,
            &template_context,
            global,
            reporter,
        )?;
        if skill_targets.is_empty() {
            skill_targets = targets.clone();
        }
        lockfile.add_package(
            name.clone(),
            mirdan::lockfile::LockedPackage {
                package_type: mirdan::package_type::PackageType::Skill,
                version: "0.0.0".to_string(),
                resolved: "builtin".to_string(),
                integrity: String::new(),
                installed_at: chrono::Utc::now().to_rfc3339(),
                targets,
            },
        );
        installed_count += 1;
    }

    save_lockfile_and_report(
        &lockfile,
        &project_root,
        installed_count,
        "skills",
        &skill_targets,
        reporter,
    )?;
    Ok(format!("Deployed {} builtin skills", installed_count))
}

/// Render skill instructions and metadata through the prompt library's Liquid template engine.
///
/// This expands `{% include %}` partials and `{{version}}` variables so the
/// installed SKILL.md contains the full rendered content rather than raw Liquid tags.
fn render_skill_instructions(
    skill: &swissarmyhammer_skills::Skill,
    prompt_library: &PromptLibrary,
    template_context: &TemplateContext,
    reporter: &dyn InitReporter,
) -> swissarmyhammer_skills::Skill {
    let rendered_instructions =
        match prompt_library.render_text(&skill.instructions, template_context) {
            Ok(rendered) => rendered,
            Err(e) => {
                reporter.emit(&InitEvent::Warning {
                    message: format!(
                        "Failed to render partials for skill '{}': {}",
                        skill.name, e
                    ),
                });
                skill.instructions.clone()
            }
        };

    let mut rendered = skill.clone();
    rendered.instructions = rendered_instructions;

    // Render template variables in metadata values (e.g., version: "{{version}}")
    for value in rendered.metadata.values_mut() {
        if value.contains("{{") {
            if let Ok(rendered_value) = prompt_library.render_text(value, template_context) {
                *value = rendered_value;
            }
        }
    }

    rendered
}

/// Format a Skill back into SKILL.md content (frontmatter + body).
fn format_skill_md(skill: &swissarmyhammer_skills::Skill) -> String {
    let mut content = String::from("---\n");
    content.push_str(&format!("name: {}\n", skill.name));
    content.push_str(&format!("description: {}\n", skill.description));

    if !skill.allowed_tools.is_empty() {
        let tools = skill.allowed_tools.join(" ");
        content.push_str(&format!("allowed-tools: \"{}\"\n", tools));
    }

    if let Some(ref license) = skill.license {
        content.push_str(&format!("license: {}\n", license));
    }

    if !skill.metadata.is_empty() {
        content.push_str("metadata:\n");
        let mut keys: Vec<_> = skill.metadata.keys().collect();
        keys.sort();
        for key in keys {
            content.push_str(&format!("  {}: \"{}\"\n", key, skill.metadata[key]));
        }
    }

    content.push_str("---\n\n");
    content.push_str(&skill.instructions);
    content.push('\n');

    content
}

// ── AgentDeployment (priority 31) ────────────────────────────────────

/// Deploys/removes builtin agents via mirdan's store + lockfile.
pub struct AgentDeployment {
    global: bool,
}

impl AgentDeployment {
    /// Create a new AgentDeployment component.
    pub fn new(global: bool) -> Self {
        Self { global }
    }
}

impl Initializable for AgentDeployment {
    /// The component name for agent deployment.
    fn name(&self) -> &str {
        "agent-deployment"
    }

    /// Component category: deployment tasks.
    fn category(&self) -> &str {
        "deployment"
    }

    /// Component priority: 31 (runs after skill deployment).
    fn priority(&self) -> i32 {
        31
    }

    /// Install builtin agents via mirdan's deploy + lockfile.
    ///
    /// Agent instructions are rendered through the prompt library's Liquid template
    /// engine before writing to disk, so `{% include %}` partials are expanded.
    fn init(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        match init_all_agents(self.global, reporter) {
            Ok(msg) => vec![InitResult::ok(self.name(), msg)],
            Err(e) => vec![InitResult::error(self.name(), e)],
        }
    }

    /// Remove builtin agent symlinks from coding agent directories and clean up the .agents/ store.
    fn deinit(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        use swissarmyhammer_agents::AgentResolver;

        let store_dir = mirdan::store::agent_store_dir(self.global);

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
            let agent_dir = if self.global {
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

        remove_store_entries(
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
    if !is_safe_name(name) {
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
    let content = format_agent_md(&rendered_agent, &rendered_instructions);
    fs::write(&agent_md_path, &content)
        .map_err(|e| format!("Failed to write {}: {}", agent_md_path.display(), e))?;

    mirdan::install::deploy_agent_to_agents(name, &agent_dir, None, global)
        .map_err(|e| format!("Failed to deploy agent '{}': {}", name, e))
}

/// Deploy all builtin agents, update lockfile, and report results.
fn init_all_agents(global: bool, reporter: &dyn InitReporter) -> Result<String, String> {
    use swissarmyhammer_agents::AgentResolver;

    let resolver = AgentResolver::new();
    let agents = resolver.resolve_builtins();

    let prompt_library = PromptLibrary::default();
    let mut template_context = TemplateContext::new();
    template_context.set(
        "version".to_string(),
        serde_json::json!(env!("CARGO_PKG_VERSION")),
    );

    let project_root =
        std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
    let mut lockfile = mirdan::lockfile::Lockfile::load(&project_root)
        .map_err(|e| format!("Failed to load lockfile: {}", e))?;

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
        lockfile.add_package(
            name.clone(),
            mirdan::lockfile::LockedPackage {
                package_type: mirdan::package_type::PackageType::Agent,
                version: "0.0.0".to_string(),
                resolved: "builtin".to_string(),
                integrity: String::new(),
                installed_at: chrono::Utc::now().to_rfc3339(),
                targets,
            },
        );
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

/// Format an Agent back into AGENT.md content (frontmatter + rendered body).
fn format_agent_md(agent: &swissarmyhammer_agents::Agent, rendered_instructions: &str) -> String {
    let mut content = String::from("---\n");
    content.push_str(&format!("name: {}\n", agent.name));
    content.push_str(&format!("description: {}\n", agent.description));

    if let Some(ref model) = agent.model {
        content.push_str(&format!("model: {}\n", model));
    }

    if !agent.tools.is_empty() {
        if agent.tools.len() == 1 && agent.tools[0] == "*" {
            content.push_str("tools: \"*\"\n");
        } else {
            let tools = agent.tools.join(" ");
            content.push_str(&format!("tools: \"{}\"\n", tools));
        }
    }

    if !agent.disallowed_tools.is_empty() {
        let tools = agent.disallowed_tools.join(" ");
        content.push_str(&format!("disallowed-tools: \"{}\"\n", tools));
    }

    if let Some(ref isolation) = agent.isolation {
        content.push_str(&format!("isolation: {}\n", isolation));
    }

    if let Some(max_turns) = agent.max_turns {
        content.push_str(&format!("max-turns: {}\n", max_turns));
    }

    if agent.background {
        content.push_str("background: true\n");
    }

    if !agent.metadata.is_empty() {
        content.push_str("metadata:\n");
        let mut keys: Vec<_> = agent.metadata.keys().collect();
        keys.sort();
        for key in keys {
            content.push_str(&format!("  {}: \"{}\"\n", key, agent.metadata[key]));
        }
    }

    content.push_str("---\n\n");
    content.push_str(rendered_instructions);
    content.push('\n');

    content
}

// ── LockfileCleanup (priority 32) ────────────────────────────────────

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

    /// Component category: deployment tasks.
    fn category(&self) -> &str {
        "deployment"
    }

    /// Component priority: 32 (runs after skill and agent deployment).
    fn priority(&self) -> i32 {
        32
    }

    /// Lockfile entries are written by SkillDeployment and AgentDeployment during their init phases.
    /// This component does not need to do anything during initialization.
    fn init(&self, _scope: &InitScope, _reporter: &dyn InitReporter) -> Vec<InitResult> {
        vec![]
    }

    /// Remove lockfile entries for all builtin skills and agents.
    fn deinit(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        use swissarmyhammer_agents::AgentResolver;
        use swissarmyhammer_skills::SkillResolver;

        let project_root = match std::env::current_dir() {
            Ok(d) => d,
            Err(e) => {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to get current directory: {}", e),
                )];
            }
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

// ── ClaudeMd (priority 22) ──────────────────────────────────────────

/// The preamble line that must appear at the top of CLAUDE.md.
pub const CLAUDE_MD_PREAMBLE: &str = "MANDATORY: load the thoughtful skill";

/// Ensures a `CLAUDE.md` file exists at the git root with the required preamble.
pub struct ClaudeMd;

/// Check if `CLAUDE.md` at the given root has the required preamble as its first non-empty line.
///
/// Returns `None` if the file does not exist, `Some(true)` if the preamble is present,
/// and `Some(false)` if it is missing.
#[cfg(test)]
fn claude_md_has_preamble(root: &std::path::Path) -> Option<bool> {
    let path = root.join("CLAUDE.md");
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    let first_non_empty = content.lines().find(|l| !l.trim().is_empty());
    Some(first_non_empty.map_or(false, |line| line.contains(CLAUDE_MD_PREAMBLE)))
}

/// Ensure `CLAUDE.md` at the given root has the required preamble.
///
/// Returns `"created"` if the file was created, `"already present"` if
/// no change was needed, or `"prepended"` if the preamble was prepended.
fn ensure_claude_md_preamble(root: &std::path::Path) -> Result<&'static str, String> {
    let path = root.join("CLAUDE.md");
    if !path.exists() {
        std::fs::write(&path, format!("{}\n", CLAUDE_MD_PREAMBLE))
            .map_err(|e| format!("Failed to create CLAUDE.md: {}", e))?;
        return Ok("created");
    }
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read CLAUDE.md: {}", e))?;
    let first_non_empty = content.lines().find(|l| !l.trim().is_empty());
    if first_non_empty.map_or(false, |line| line.contains(CLAUDE_MD_PREAMBLE)) {
        return Ok("already present");
    }
    let new_content = format!("{}\n\n{}", CLAUDE_MD_PREAMBLE, content);
    std::fs::write(&path, new_content).map_err(|e| format!("Failed to update CLAUDE.md: {}", e))?;
    Ok("prepended")
}

/// Remove the preamble from `CLAUDE.md`. Deletes the file if it becomes empty.
///
/// Returns `"removed"` if the preamble was stripped, `"deleted"` if the file
/// was deleted (only contained the preamble), `"not found"` if no file exists,
/// or `"no preamble"` if the file exists but has no preamble.
fn remove_claude_md_preamble(root: &std::path::Path) -> Result<&'static str, String> {
    let path = root.join("CLAUDE.md");
    if !path.exists() {
        return Ok("not found");
    }
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read CLAUDE.md: {}", e))?;
    let first_non_empty = content.lines().find(|l| !l.trim().is_empty());
    if !first_non_empty.map_or(false, |line| line.contains(CLAUDE_MD_PREAMBLE)) {
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
    while after_preamble
        .first()
        .map_or(false, |l| l.trim().is_empty())
    {
        after_preamble.remove(0);
    }
    if after_preamble.is_empty() {
        std::fs::remove_file(&path).map_err(|e| format!("Failed to delete CLAUDE.md: {}", e))?;
        return Ok("deleted");
    }
    let new_content = after_preamble.join("\n") + "\n";
    std::fs::write(&path, new_content).map_err(|e| format!("Failed to update CLAUDE.md: {}", e))?;
    Ok("removed")
}

impl Initializable for ClaudeMd {
    /// The component name for CLAUDE.md preamble management.
    fn name(&self) -> &str {
        "claude-md"
    }

    /// Component category: configuration tasks.
    fn category(&self) -> &str {
        "configuration"
    }

    /// Component priority: 22 (runs after ProjectStructure, before KanbanTool).
    fn priority(&self) -> i32 {
        22
    }

    /// Only applicable to project and local scope installations.
    fn is_applicable(&self, scope: &InitScope) -> bool {
        matches!(scope, InitScope::Project | InitScope::Local)
    }

    /// Ensure CLAUDE.md exists at the git root with the required preamble.
    fn init(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        let root = match swissarmyhammer_common::utils::find_git_repository_root() {
            Some(r) => r,
            None => {
                return vec![InitResult::error(
                    self.name(),
                    "No git repository found".to_string(),
                )];
            }
        };

        match ensure_claude_md_preamble(&root) {
            Ok("created") => {
                reporter.emit(&InitEvent::Action {
                    verb: "Created".to_string(),
                    message: format!("CLAUDE.md at {}", root.display()),
                });
                vec![InitResult::ok(
                    self.name(),
                    "CLAUDE.md created with preamble",
                )]
            }
            Ok("prepended") => {
                reporter.emit(&InitEvent::Action {
                    verb: "Updated".to_string(),
                    message: format!("CLAUDE.md at {}", root.display()),
                });
                vec![InitResult::ok(
                    self.name(),
                    "Preamble prepended to CLAUDE.md",
                )]
            }
            Ok(_) => {
                reporter.emit(&InitEvent::Skipped {
                    component: self.name().to_string(),
                    reason: "CLAUDE.md already has the required preamble".to_string(),
                });
                vec![InitResult::ok(
                    self.name(),
                    "CLAUDE.md already has preamble",
                )]
            }
            Err(e) => vec![InitResult::error(self.name(), e)],
        }
    }

    /// Remove the preamble from CLAUDE.md (or delete the file if only preamble).
    fn deinit(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        let root = match swissarmyhammer_common::utils::find_git_repository_root() {
            Some(r) => r,
            None => {
                return vec![InitResult::error(
                    self.name(),
                    "No git repository found".to_string(),
                )];
            }
        };

        match remove_claude_md_preamble(&root) {
            Ok("deleted") => {
                reporter.emit(&InitEvent::Action {
                    verb: "Removed".to_string(),
                    message: format!("CLAUDE.md from {}", root.display()),
                });
                vec![InitResult::ok(self.name(), "CLAUDE.md deleted")]
            }
            Ok("removed") => {
                reporter.emit(&InitEvent::Action {
                    verb: "Updated".to_string(),
                    message: format!("removed preamble from CLAUDE.md at {}", root.display()),
                });
                vec![InitResult::ok(
                    self.name(),
                    "Preamble removed from CLAUDE.md",
                )]
            }
            Ok(_) => {
                reporter.emit(&InitEvent::Skipped {
                    component: self.name().to_string(),
                    reason: "CLAUDE.md not found or has no preamble".to_string(),
                });
                vec![InitResult::ok(self.name(), "Nothing to remove")]
            }
            Err(e) => vec![InitResult::error(self.name(), e)],
        }
    }
}

// ── Shared helpers ───────────────────────────────────────────────────

/// Remove the `mcpServers` key from a JSON value if it is an empty object.
fn cleanup_empty_mcp_servers(settings: &mut serde_json::Value) {
    let is_empty = settings
        .get("mcpServers")
        .and_then(|m| m.as_object())
        .map(|m| m.is_empty())
        .unwrap_or(false);
    if is_empty {
        if let Some(obj) = settings.as_object_mut() {
            obj.remove("mcpServers");
        }
    }
}

/// Save lockfile and emit a reporter event if any packages were installed.
fn save_lockfile_and_report(
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

/// Validate that a name is safe to use as a filesystem path component.
///
/// Rejects names containing path separators, parent-directory references,
/// or absolute paths to prevent path traversal attacks.
fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && !std::path::Path::new(name).is_absolute()
}

/// Remove named entries from a store directory and their symlinks from link directories.
///
/// This is the shared filesystem logic for both skill and agent uninstall.
/// The caller resolves names and directories, this function does the filesystem work.
/// Names are validated to prevent path traversal — any name containing `/`, `\`, or `..`
/// is skipped with a warning.
pub(crate) fn remove_store_entries(
    store_dir: &std::path::Path,
    names: &[String],
    link_dirs: &[std::path::PathBuf],
    symlink_policies: &[mirdan::agents::SymlinkPolicy],
    kind: &str,
    reporter: &dyn InitReporter,
) {
    for name in names {
        if !is_safe_name(name) {
            reporter.emit(&InitEvent::Warning {
                message: format!("skipping unsafe {} name: {:?}", kind, name),
            });
            continue;
        }

        remove_single_store_entry(store_dir, name, link_dirs, symlink_policies, kind, reporter);
    }

    // Remove the store directory if empty
    if store_dir.exists() {
        if let Ok(entries) = fs::read_dir(store_dir) {
            if entries.count() == 0 {
                let _ = fs::remove_dir(store_dir);
            }
        }
    }
}

/// Remove a single named entry from the store and its symlinks from link directories.
fn remove_single_store_entry(
    store_dir: &std::path::Path,
    name: &str,
    link_dirs: &[std::path::PathBuf],
    symlink_policies: &[mirdan::agents::SymlinkPolicy],
    kind: &str,
    reporter: &dyn InitReporter,
) {
    let store_path = store_dir.join(name);

    for (dir, policy) in link_dirs.iter().zip(symlink_policies.iter()) {
        let link_name = mirdan::store::symlink_name(name, policy);
        let link_path = dir.join(&link_name);
        remove_if_symlink(&link_path, reporter);
    }

    if store_path.exists() {
        if let Err(e) = fs::remove_dir_all(&store_path) {
            reporter.emit(&InitEvent::Warning {
                message: format!(
                    "failed to remove store entry {}: {}",
                    store_path.display(),
                    e
                ),
            });
        } else {
            tracing::debug!("Removed {} store: {}", kind, store_path.display());
        }
    }
}

/// Remove a path only if it is a symlink. Returns true if removed.
///
/// This is the safety-critical function: it ensures deinit never deletes
/// real directories or files that weren't created by `sah init`.
pub(crate) fn remove_if_symlink(path: &std::path::Path, reporter: &dyn InitReporter) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            if let Err(e) = std::fs::remove_file(path) {
                reporter.emit(&InitEvent::Warning {
                    message: format!("failed to remove {}: {}", path.display(), e),
                });
                false
            } else {
                tracing::debug!("Removed link: {}", path.display());
                true
            }
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_safe_name() {
        assert!(is_safe_name("my-skill"));
        assert!(is_safe_name("agent_v2"));
        assert!(!is_safe_name("../escape"));
        assert!(!is_safe_name("foo/bar"));
        assert!(!is_safe_name("foo\\bar"));
        assert!(!is_safe_name(""));
    }

    #[test]
    fn test_claude_md_creates_file_when_absent() {
        let temp = tempfile::TempDir::new().unwrap();
        let result = ensure_claude_md_preamble(temp.path()).unwrap();
        assert_eq!(result, "created");

        let content = std::fs::read_to_string(temp.path().join("CLAUDE.md")).unwrap();
        assert!(content.starts_with(CLAUDE_MD_PREAMBLE));
        assert!(content.ends_with('\n'));
    }

    #[test]
    fn test_claude_md_prepends_preamble_to_existing() {
        let temp = tempfile::TempDir::new().unwrap();
        let claude_md = temp.path().join("CLAUDE.md");
        std::fs::write(&claude_md, "existing content\n").unwrap();

        let result = ensure_claude_md_preamble(temp.path()).unwrap();
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

        let result = ensure_claude_md_preamble(temp.path()).unwrap();
        assert_eq!(result, "already present");

        let content = std::fs::read_to_string(&claude_md).unwrap();
        // Should not have doubled the preamble
        assert_eq!(content.matches(CLAUDE_MD_PREAMBLE).count(), 1);
    }

    #[test]
    fn test_claude_md_has_preamble_absent() {
        let temp = tempfile::TempDir::new().unwrap();
        assert_eq!(claude_md_has_preamble(temp.path()), None);
    }

    #[test]
    fn test_claude_md_has_preamble_present() {
        let temp = tempfile::TempDir::new().unwrap();
        let claude_md = temp.path().join("CLAUDE.md");
        std::fs::write(&claude_md, format!("{}\nother stuff\n", CLAUDE_MD_PREAMBLE)).unwrap();
        assert_eq!(claude_md_has_preamble(temp.path()), Some(true));
    }

    #[test]
    fn test_claude_md_has_preamble_missing() {
        let temp = tempfile::TempDir::new().unwrap();
        let claude_md = temp.path().join("CLAUDE.md");
        std::fs::write(&claude_md, "some other content\n").unwrap();
        assert_eq!(claude_md_has_preamble(temp.path()), Some(false));
    }

    #[test]
    fn test_claude_md_deinit_deletes_preamble_only_file() {
        let temp = tempfile::TempDir::new().unwrap();
        let claude_md = temp.path().join("CLAUDE.md");
        std::fs::write(&claude_md, format!("{}\n", CLAUDE_MD_PREAMBLE)).unwrap();

        let result = remove_claude_md_preamble(temp.path()).unwrap();
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

        let result = remove_claude_md_preamble(temp.path()).unwrap();
        assert_eq!(result, "removed");
        let content = std::fs::read_to_string(&claude_md).unwrap();
        assert!(!content.contains(CLAUDE_MD_PREAMBLE));
        assert!(content.contains("my project notes"));
        assert!(content.contains("more stuff"));
    }

    #[test]
    fn test_claude_md_deinit_no_file() {
        let temp = tempfile::TempDir::new().unwrap();
        let result = remove_claude_md_preamble(temp.path()).unwrap();
        assert_eq!(result, "not found");
    }

    #[test]
    fn test_claude_md_deinit_no_preamble() {
        let temp = tempfile::TempDir::new().unwrap();
        let claude_md = temp.path().join("CLAUDE.md");
        std::fs::write(&claude_md, "just user content\n").unwrap();

        let result = remove_claude_md_preamble(temp.path()).unwrap();
        assert_eq!(result, "no preamble");
        // File should be untouched
        let content = std::fs::read_to_string(&claude_md).unwrap();
        assert_eq!(content, "just user content\n");
    }
}
