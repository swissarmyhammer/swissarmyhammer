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
    registry.register(SkillDeployment::new(global));
    registry.register(AgentDeployment::new(global));
    registry.register(LockfileCleanup);
}

// ── McpRegistration (priority 10) ────────────────────────────────────

/// Registers/unregisters sah as an MCP server in all detected agent configs.
pub struct McpRegistration {
    global: bool,
}

impl McpRegistration {
    pub fn new(global: bool) -> Self {
        Self { global }
    }
}

impl Initializable for McpRegistration {
    fn name(&self) -> &str {
        "mcp-registration"
    }

    fn category(&self) -> &str {
        "configuration"
    }

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
            if let Some(mcp_def) = &agent.def.mcp_config {
                let config_path = if self.global {
                    mirdan::agents::agent_global_mcp_config(&agent.def)
                } else {
                    mirdan::agents::agent_project_mcp_config(&agent.def)
                };
                if let Some(config_path) = config_path {
                    match mirdan::mcp_config::register_mcp_server(
                        &config_path,
                        &mcp_def.servers_key,
                        "sah",
                        &entry,
                    ) {
                        Ok(()) => {
                            reporter.emit(&InitEvent::Action {
                                verb: "Installed".to_string(),
                                message: format!(
                                    "MCP server for {} ({})",
                                    agent.def.name,
                                    config_path.display()
                                ),
                            });
                            installed_count += 1;
                        }
                        Err(e) => {
                            reporter.emit(&InitEvent::Warning {
                                message: format!(
                                    "failed to install MCP for {}: {}",
                                    agent.def.name, e
                                ),
                            });
                        }
                    }
                }
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
            if let Some(mcp_def) = &agent.def.mcp_config {
                let config_path = if self.global {
                    mirdan::agents::agent_global_mcp_config(&agent.def)
                } else {
                    mirdan::agents::agent_project_mcp_config(&agent.def)
                };
                if let Some(config_path) = config_path {
                    match mirdan::mcp_config::unregister_mcp_server(
                        &config_path,
                        &mcp_def.servers_key,
                        "sah",
                    ) {
                        Ok(true) => {
                            reporter.emit(&InitEvent::Action {
                                verb: "Removed".to_string(),
                                message: format!(
                                    "MCP server from {} ({})",
                                    agent.def.name,
                                    config_path.display()
                                ),
                            });
                            removed_count += 1;
                        }
                        Ok(false) => {}
                        Err(e) => {
                            reporter.emit(&InitEvent::Warning {
                                message: format!(
                                    "failed to remove MCP from {}: {}",
                                    agent.def.name, e
                                ),
                            });
                        }
                    }
                }
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
        reporter.emit(&InitEvent::Action {
            verb: "Unchanged".to_string(),
            message: format!("MCP server already configured in {}", path.display()),
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

    if let Some(mcp_servers) = mcp_settings.get("mcpServers").and_then(|m| m.as_object()) {
        if mcp_servers.is_empty() {
            mcp_settings.as_object_mut().unwrap().remove("mcpServers");
        }
    }

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
    fn name(&self) -> &str {
        "claude-local-scope"
    }

    fn category(&self) -> &str {
        "configuration"
    }

    fn priority(&self) -> i32 {
        11
    }

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
            reporter.emit(&InitEvent::Action {
                verb: "Unchanged".to_string(),
                message: format!(
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

        let changed = if let Some(projects) = root.get_mut("projects") {
            if let Some(entry) = projects.get_mut(&key) {
                let changed = settings::remove_mcp_server(entry);
                if let Some(mcp_servers) = entry.get("mcpServers").and_then(|m| m.as_object()) {
                    if mcp_servers.is_empty() {
                        entry.as_object_mut().unwrap().remove("mcpServers");
                    }
                }
                changed
            } else {
                false
            }
        } else {
            false
        };

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

// ── DenyBash (priority 15) ───────────────────────────────────────────

/// Manages the "Bash" deny rule in `.claude/settings.json`.
pub struct DenyBash;

impl Initializable for DenyBash {
    fn name(&self) -> &str {
        "deny-bash"
    }

    fn category(&self) -> &str {
        "configuration"
    }

    fn priority(&self) -> i32 {
        15
    }

    fn is_applicable(&self, scope: &InitScope) -> bool {
        matches!(scope, InitScope::Project | InitScope::Local)
    }

    /// Add "Bash" to permissions.deny in .claude/settings.json.
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

/// Creates/removes the `.swissarmyhammer/` and `.prompts/` project directories.
pub struct ProjectStructure {
    remove_directory: bool,
}

impl ProjectStructure {
    pub fn new(remove_directory: bool) -> Self {
        Self { remove_directory }
    }
}

impl Initializable for ProjectStructure {
    fn name(&self) -> &str {
        "project-structure"
    }

    fn category(&self) -> &str {
        "structure"
    }

    fn priority(&self) -> i32 {
        20
    }

    fn is_applicable(&self, scope: &InitScope) -> bool {
        matches!(scope, InitScope::Project | InitScope::Local)
    }

    /// Create the project directory structure with .prompts, .swissarmyhammer, and workflows.
    fn init(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        use swissarmyhammer_common::SwissarmyhammerDirectory;

        let sah_dir = match SwissarmyhammerDirectory::from_git_root().or_else(|_| {
            let cwd = std::env::current_dir()
                .map_err(|e| format!("Failed to get current directory: {}", e))?;
            SwissarmyhammerDirectory::from_custom_root(cwd)
                .map_err(|e| format!("Failed to create .swissarmyhammer directory: {}", e))
        }) {
            Ok(d) => d,
            Err(e) => return vec![InitResult::error(self.name(), e)],
        };

        // Create .prompts/ as a sibling to .swissarmyhammer/ (dot-directory path for PromptResolver)
        let project_root = match sah_dir.root().parent() {
            Some(p) => p,
            None => {
                return vec![InitResult::error(
                    self.name(),
                    "Failed to determine project root from .swissarmyhammer directory".to_string(),
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

    /// Remove `.swissarmyhammer/` and `.prompts/` directories if `remove_directory` is true.
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

        let sah_dir = cwd.join(".swissarmyhammer");
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
    pub fn new(global: bool) -> Self {
        Self { global }
    }
}

impl Initializable for SkillDeployment {
    fn name(&self) -> &str {
        "skill-deployment"
    }

    fn category(&self) -> &str {
        "deployment"
    }

    fn priority(&self) -> i32 {
        30
    }

    /// Install builtin skills via mirdan's deploy + lockfile.
    ///
    /// 1. Write each builtin to a temp dir (they're embedded in the binary)
    /// 2. Call `deploy_skill_to_agents()` for each -- handles store copy + symlinks
    /// 3. Write lockfile entries via `Lockfile`
    ///
    /// Skill instructions are rendered through the prompt library's Liquid template
    /// engine before writing to disk, so `{% include %}` partials are expanded.
    fn init(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        use swissarmyhammer_skills::SkillResolver;

        let resolver = SkillResolver::new();
        let skills = resolver.resolve_builtins();

        let prompt_library = PromptLibrary::default();
        let template_context = TemplateContext::new();

        let project_root = match std::env::current_dir() {
            Ok(d) => d,
            Err(e) => {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to get current directory: {}", e),
                )];
            }
        };
        let mut lockfile = match mirdan::lockfile::Lockfile::load(&project_root) {
            Ok(l) => l,
            Err(e) => {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to load lockfile: {}", e),
                )];
            }
        };

        let mut installed_count = 0;
        let mut installed_names: Vec<String> = Vec::new();
        let mut agent_names: Vec<String> = Vec::new();
        for (name, skill) in &skills {
            let temp_dir = match tempfile::tempdir() {
                Ok(d) => d,
                Err(e) => {
                    return vec![InitResult::error(
                        self.name(),
                        format!("Failed to create temp dir: {}", e),
                    )];
                }
            };
            let skill_dir = temp_dir.path().join(name);
            if let Err(e) = fs::create_dir_all(&skill_dir) {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to create temp skill dir: {}", e),
                )];
            }

            // Render instructions through the prompt library's Liquid engine
            let rendered_skill =
                render_skill_instructions(skill, &prompt_library, &template_context);

            let skill_md_path = skill_dir.join("SKILL.md");
            let content = format_skill_md(&rendered_skill);
            if let Err(e) = fs::write(&skill_md_path, &content) {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to write {}: {}", skill_md_path.display(), e),
                )];
            }

            // Write any additional resource files
            for (filename, file_content) in &skill.resources.files {
                let file_path = skill_dir.join(filename);
                if let Err(e) = fs::write(&file_path, file_content) {
                    return vec![InitResult::error(
                        self.name(),
                        format!("Failed to write {}: {}", file_path.display(), e),
                    )];
                }
            }

            // Deploy via mirdan: store copy + agent symlinks
            let targets = match mirdan::install::deploy_skill_to_agents(
                name,
                &skill_dir,
                None,
                self.global,
            ) {
                Ok(t) => t,
                Err(e) => {
                    return vec![InitResult::error(
                        self.name(),
                        format!("Failed to deploy skill '{}': {}", name, e),
                    )];
                }
            };

            // Collect agent names from first skill (same for all)
            if agent_names.is_empty() {
                agent_names = targets.clone();
            }

            // Record in lockfile
            let version = skill
                .metadata
                .get("version")
                .cloned()
                .unwrap_or_else(|| "0.0.0".to_string());
            lockfile.add_package(
                name.clone(),
                mirdan::lockfile::LockedPackage {
                    package_type: mirdan::package_type::PackageType::Skill,
                    version,
                    resolved: "builtin".to_string(),
                    integrity: String::new(),
                    installed_at: chrono::Utc::now().to_rfc3339(),
                    targets,
                },
            );

            installed_names.push(name.clone());
            installed_count += 1;
        }

        if installed_count > 0 {
            if let Err(e) = lockfile.save(&project_root) {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to save lockfile: {}", e),
                )];
            }
            reporter.emit(&InitEvent::Action {
                verb: "Installed".to_string(),
                message: format!(
                    "{} skills → {}",
                    installed_count,
                    agent_names.join(", ")
                ),
            });
        }

        vec![InitResult::ok(
            self.name(),
            format!("Deployed {} builtin skills", installed_count),
        )]
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

/// Render skill instructions through the prompt library's Liquid template engine.
///
/// This expands `{% include %}` partials so the installed SKILL.md contains
/// the full rendered content rather than raw Liquid tags.
fn render_skill_instructions(
    skill: &swissarmyhammer_skills::Skill,
    prompt_library: &PromptLibrary,
    template_context: &TemplateContext,
) -> swissarmyhammer_skills::Skill {
    let rendered_instructions =
        match prompt_library.render_text(&skill.instructions, template_context) {
            Ok(rendered) => rendered,
            Err(e) => {
                // render_skill_instructions does not have reporter access; leave as eprintln
                eprintln!(
                    "Warning: failed to render partials for skill '{}': {}",
                    skill.name, e
                );
                skill.instructions.clone()
            }
        };

    let mut rendered = skill.clone();
    rendered.instructions = rendered_instructions;
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
    pub fn new(global: bool) -> Self {
        Self { global }
    }
}

impl Initializable for AgentDeployment {
    fn name(&self) -> &str {
        "agent-deployment"
    }

    fn category(&self) -> &str {
        "deployment"
    }

    fn priority(&self) -> i32 {
        31
    }

    /// Install builtin agents via mirdan's deploy + lockfile.
    ///
    /// 1. Write each builtin agent to a temp dir (they're embedded in the binary)
    /// 2. Call `deploy_agent_to_agents()` for each -- handles store copy + symlinks
    /// 3. Write lockfile entries via `Lockfile`
    ///
    /// Agent instructions are rendered through the prompt library's Liquid template
    /// engine before writing to disk, so `{% include %}` partials are expanded.
    fn init(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        use swissarmyhammer_agents::AgentResolver;

        let resolver = AgentResolver::new();
        let agents = resolver.resolve_builtins();

        let prompt_library = PromptLibrary::default();
        let template_context = TemplateContext::new();

        let project_root = match std::env::current_dir() {
            Ok(d) => d,
            Err(e) => {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to get current directory: {}", e),
                )];
            }
        };
        let mut lockfile = match mirdan::lockfile::Lockfile::load(&project_root) {
            Ok(l) => l,
            Err(e) => {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to load lockfile: {}", e),
                )];
            }
        };

        let mut installed_count = 0;
        let mut agent_targets: Vec<String> = Vec::new();
        for (name, agent) in &agents {
            let temp_dir = match tempfile::tempdir() {
                Ok(d) => d,
                Err(e) => {
                    return vec![InitResult::error(
                        self.name(),
                        format!("Failed to create temp dir: {}", e),
                    )];
                }
            };
            let agent_dir = temp_dir.path().join(name);
            if let Err(e) = fs::create_dir_all(&agent_dir) {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to create temp agent dir: {}", e),
                )];
            }

            // Render instructions through the prompt library's Liquid engine
            let rendered_instructions =
                match prompt_library.render_text(&agent.instructions, &template_context) {
                    Ok(rendered) => rendered,
                    Err(e) => {
                        reporter.emit(&InitEvent::Warning {
                            message: format!(
                                "failed to render partials for agent '{}': {}",
                                name, e
                            ),
                        });
                        agent.instructions.clone()
                    }
                };

            let agent_md_path = agent_dir.join("AGENT.md");
            let content = format_agent_md(agent, &rendered_instructions);
            if let Err(e) = fs::write(&agent_md_path, &content) {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to write {}: {}", agent_md_path.display(), e),
                )];
            }

            // Deploy via mirdan: store copy + coding agent symlinks
            let targets = match mirdan::install::deploy_agent_to_agents(
                name,
                &agent_dir,
                None,
                self.global,
            ) {
                Ok(t) => t,
                Err(e) => {
                    return vec![InitResult::error(
                        self.name(),
                        format!("Failed to deploy agent '{}': {}", name, e),
                    )];
                }
            };

            // Collect agent targets from first deploy (same for all)
            if agent_targets.is_empty() {
                agent_targets = targets.clone();
            }

            // Record in lockfile
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

        if installed_count > 0 {
            if let Err(e) = lockfile.save(&project_root) {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to save lockfile: {}", e),
                )];
            }
            reporter.emit(&InitEvent::Action {
                verb: "Installed".to_string(),
                message: format!(
                    "{} agents → {}",
                    installed_count,
                    agent_targets.join(", ")
                ),
            });
        }

        vec![InitResult::ok(
            self.name(),
            format!("Deployed {} builtin agents", installed_count),
        )]
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
    fn name(&self) -> &str {
        "lockfile-cleanup"
    }

    fn category(&self) -> &str {
        "deployment"
    }

    fn priority(&self) -> i32 {
        32
    }

    fn init(&self, _scope: &InitScope, _reporter: &dyn InitReporter) -> Vec<InitResult> {
        // Lockfile entries are written by SkillDeployment and AgentDeployment
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

// ── Shared helpers ───────────────────────────────────────────────────

/// Map mirdan `DeployResult` entries to reporter events.
///
/// Each `DeployResult` is translated to an `InitEvent::Action` or `InitEvent::Warning`
/// depending on the action type. `Skipped` results are silently ignored.
///
/// This bridges the mirdan deploy layer (which returns structured results) with the
/// sah lifecycle reporter (which formats output for the user).
///
/// Currently unused because `deploy_skill_to_agents` and `deploy_agent_to_agents`
/// still return `Vec<String>` (agent IDs). Once those functions are updated to
/// return `Vec<DeployResult>`, the call sites should switch from per-target
/// emissions to calling this function instead.
#[allow(dead_code)]
fn map_deploy_results(results: &[mirdan::DeployResult], reporter: &dyn InitReporter) {
    for r in results {
        match r.action {
            mirdan::DeployAction::Created => reporter.emit(&InitEvent::Action {
                verb: "Created".to_string(),
                message: r.message.clone(),
            }),
            mirdan::DeployAction::Updated => reporter.emit(&InitEvent::Action {
                verb: "Updated".to_string(),
                message: r.message.clone(),
            }),
            mirdan::DeployAction::Removed => reporter.emit(&InitEvent::Action {
                verb: "Removed".to_string(),
                message: r.message.clone(),
            }),
            mirdan::DeployAction::Linked => reporter.emit(&InitEvent::Action {
                verb: "Linked".to_string(),
                message: r.message.clone(),
            }),
            mirdan::DeployAction::Skipped => {}
            mirdan::DeployAction::Warning => reporter.emit(&InitEvent::Warning {
                message: r.message.clone(),
            }),
        }
    }
}

/// Remove named entries from a store directory and their symlinks from link directories.
///
/// This is the shared filesystem logic for both skill and agent uninstall.
/// The caller resolves names and directories, this function does the filesystem work.
pub(crate) fn remove_store_entries(
    store_dir: &std::path::Path,
    names: &[String],
    link_dirs: &[std::path::PathBuf],
    symlink_policies: &[mirdan::agents::SymlinkPolicy],
    kind: &str,
    reporter: &dyn InitReporter,
) {
    for name in names {
        let store_path = store_dir.join(name);

        // Remove symlinks from each link directory
        for (dir, policy) in link_dirs.iter().zip(symlink_policies.iter()) {
            let link_name = mirdan::store::symlink_name(name, policy);
            let link_path = dir.join(&link_name);
            remove_if_symlink(&link_path, reporter);
        }

        // Remove entry from the store
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

    // Remove the store directory if empty
    if store_dir.exists() {
        if let Ok(entries) = fs::read_dir(store_dir) {
            if entries.count() == 0 {
                let _ = fs::remove_dir(store_dir);
            }
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
