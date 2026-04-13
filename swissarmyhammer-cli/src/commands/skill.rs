//! Builtin skill deployment for sah.
//!
//! Resolves all compiled-in builtin skills, renders their Liquid templates
//! (expanding `{% include %}` partials via the prompt library), formats each
//! as a SKILL.md with YAML frontmatter, and deploys the results to every
//! detected agent's `.skills/` directory via mirdan.
//!
//! [`SkillDeployment`] implements [`Initializable`] so that skill deployment
//! runs as part of `sah init` / `sah deinit`, registered from
//! [`crate::commands::registry::register_all`].
//!
//! This module mirrors the dedicated `commands/skill.rs` module used by
//! sibling CLIs (`code-context-cli`, `shelltool-cli`), while retaining the
//! Liquid-based rendering and mirdan lockfile tracking specific to sah —
//! sah's builtin skills rely on `{% include %}` partials that the simple
//! `swissarmyhammer-templating` engine used by other CLIs does not support.
//!
//! The filesystem helpers ([`is_safe_name`], [`save_lockfile_and_report`],
//! [`remove_store_entries`]) live in [`super::install::components`] because
//! they are shared with the sibling `AgentDeployment` component.

use std::fs;

use swissarmyhammer_common::lifecycle::{InitResult, InitScope, Initializable};
use swissarmyhammer_common::reporter::{InitEvent, InitReporter};
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_prompts::PromptLibrary;

use super::install::components::{is_safe_name, remove_store_entries, save_lockfile_and_report};

// ── SkillDeployment (priority 30) ────────────────────────────────────

/// Deploys/removes builtin skills via mirdan's store + lockfile.
///
/// Derives global-vs-project behavior from the `InitScope` parameter passed
/// to `init`/`deinit` — `InitScope::User` targets global agent configs,
/// all other scopes target project-level configs.
pub struct SkillDeployment;

impl Initializable for SkillDeployment {
    /// The component name for skill deployment.
    fn name(&self) -> &str {
        "skill-deployment"
    }

    /// Component category: deployment tasks.
    fn category(&self) -> &str {
        "deployment"
    }

    /// Component priority: 30 (runs after structure setup at priority 20).
    fn priority(&self) -> i32 {
        30
    }

    /// Install builtin skills via mirdan's deploy + lockfile.
    ///
    /// Skill instructions are rendered through the prompt library's Liquid
    /// template engine before writing to disk, so `{% include %}` partials
    /// and `{{version}}` variables are expanded.
    fn init(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        let global = matches!(scope, InitScope::User);
        match deploy_all_skills(global, reporter) {
            Ok(msg) => vec![InitResult::ok(self.name(), msg)],
            Err(e) => vec![InitResult::error(self.name(), e)],
        }
    }

    /// Remove builtin skill symlinks from agent directories and clean up
    /// the `.skills/` store.
    fn deinit(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        use swissarmyhammer_skills::SkillResolver;

        let global = matches!(scope, InitScope::User);
        let store_dir = mirdan::store::skill_store_dir(global);

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
                if global {
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
/// Renders Liquid templates in the skill's instructions and metadata, writes
/// the resulting SKILL.md plus any bundled resource files to a unique temp
/// directory, and delegates installation to mirdan.
///
/// Returns the list of agent targets on success, or an error description.
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

/// Deploy all builtin skills, update the mirdan lockfile, and report results.
///
/// Resolves the full set of builtin skills, renders each through the prompt
/// library's Liquid engine, writes them to temp directories, and deploys
/// them to every detected agent. Lockfile entries are added for each skill
/// so that `deinit` can tear them down cleanly.
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

/// Render skill instructions and metadata through the prompt library's
/// Liquid template engine.
///
/// Expands `{% include %}` partials and `{{version}}` variables so the
/// installed SKILL.md contains the full rendered content rather than raw
/// Liquid tags. Falls back to the raw text and emits a warning through
/// `reporter` when rendering fails.
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
///
/// Builds YAML frontmatter using `serde_yaml_ng` so values containing special
/// characters (colons, quotes, newlines) are correctly escaped. The resulting
/// document is compatible with `swissarmyhammer_skills::skill_loader::parse_skill_md`.
fn format_skill_md(skill: &swissarmyhammer_skills::Skill) -> String {
    // Build a frontmatter map and let serde_yaml_ng handle proper escaping/quoting
    let mut frontmatter = serde_yaml_ng::Mapping::new();
    frontmatter.insert(
        serde_yaml_ng::Value::String("name".to_string()),
        serde_yaml_ng::Value::String(skill.name.to_string()),
    );
    frontmatter.insert(
        serde_yaml_ng::Value::String("description".to_string()),
        serde_yaml_ng::Value::String(skill.description.clone()),
    );

    if !skill.allowed_tools.is_empty() {
        let tools = skill.allowed_tools.join(" ");
        frontmatter.insert(
            serde_yaml_ng::Value::String("allowed-tools".to_string()),
            serde_yaml_ng::Value::String(tools),
        );
    }

    if let Some(ref license) = skill.license {
        frontmatter.insert(
            serde_yaml_ng::Value::String("license".to_string()),
            serde_yaml_ng::Value::String(license.clone()),
        );
    }

    if !skill.metadata.is_empty() {
        let mut meta_map = serde_yaml_ng::Mapping::new();
        let mut keys: Vec<_> = skill.metadata.keys().collect();
        keys.sort();
        for key in keys {
            meta_map.insert(
                serde_yaml_ng::Value::String(key.clone()),
                serde_yaml_ng::Value::String(skill.metadata[key].clone()),
            );
        }
        frontmatter.insert(
            serde_yaml_ng::Value::String("metadata".to_string()),
            serde_yaml_ng::Value::Mapping(meta_map),
        );
    }

    let yaml = serde_yaml_ng::to_string(&frontmatter).unwrap_or_default();
    format!("---\n{}---\n\n{}\n", yaml, skill.instructions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_common::reporter::NullReporter;

    #[test]
    fn test_skill_deployment_name_and_priority() {
        let component = SkillDeployment;
        assert_eq!(Initializable::name(&component), "skill-deployment");
        assert_eq!(component.category(), "deployment");
        assert_eq!(component.priority(), 30);
    }

    #[test]
    fn test_builtin_skills_exist() {
        let resolver = swissarmyhammer_skills::SkillResolver::new();
        let builtins = resolver.resolve_builtins();
        assert!(
            !builtins.is_empty(),
            "at least one builtin skill should exist"
        );
    }

    #[test]
    fn test_format_skill_md_has_frontmatter() {
        let resolver = swissarmyhammer_skills::SkillResolver::new();
        let builtins = resolver.resolve_builtins();
        let (_, skill) = builtins.iter().next().expect("at least one builtin skill");

        let md = format_skill_md(skill);
        assert!(md.starts_with("---\n"), "should start with frontmatter");
        assert!(
            md.contains("\n---\n"),
            "should have closing frontmatter delimiter"
        );
        assert!(
            md.contains(&format!("name: {}", skill.name)),
            "frontmatter should contain skill name"
        );
    }

    #[test]
    fn test_format_skill_md_preserves_metadata() {
        use std::collections::HashMap;
        use swissarmyhammer_skills::{Skill, SkillName, SkillResources, SkillSource};

        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), "swissarmyhammer".to_string());
        metadata.insert("version".to_string(), "1.2.3".to_string());

        let skill = Skill {
            name: SkillName::new("meta-skill").unwrap(),
            description: "skill with metadata".to_string(),
            license: Some("MIT".to_string()),
            compatibility: None,
            metadata,
            allowed_tools: vec![],
            instructions: "body".to_string(),
            source_path: None,
            source: SkillSource::Builtin,
            resources: SkillResources::default(),
        };

        let md = format_skill_md(&skill);

        assert!(
            md.contains("metadata:"),
            "frontmatter should contain metadata block"
        );
        assert!(
            md.contains("author: swissarmyhammer"),
            "metadata should contain author"
        );
        assert!(
            md.contains("version: 1.2.3"),
            "metadata should contain version"
        );
        assert!(
            md.contains("license: MIT"),
            "frontmatter should contain license"
        );

        // Round-trip: output must be parseable by the skill loader
        let parsed =
            swissarmyhammer_skills::skill_loader::parse_skill_md(&md, SkillSource::Builtin)
                .expect("output should parse as valid SKILL.md");
        assert_eq!(parsed.metadata.get("author").unwrap(), "swissarmyhammer");
        assert_eq!(parsed.metadata.get("version").unwrap(), "1.2.3");
        assert_eq!(parsed.license.as_deref(), Some("MIT"));
    }

    #[test]
    fn test_format_skill_md_omits_empty_frontmatter_fields() {
        use std::collections::HashMap;
        use swissarmyhammer_skills::{Skill, SkillName, SkillResources, SkillSource};

        let skill = Skill {
            name: SkillName::new("minimal").unwrap(),
            description: "minimal skill".to_string(),
            license: None,
            compatibility: None,
            metadata: HashMap::new(),
            allowed_tools: vec![],
            instructions: "body".to_string(),
            source_path: None,
            source: SkillSource::Builtin,
            resources: SkillResources::default(),
        };

        let md = format_skill_md(&skill);
        assert!(
            !md.contains("allowed-tools"),
            "empty allowed_tools should be omitted"
        );
        assert!(!md.contains("license"), "None license should be omitted");
        assert!(
            !md.contains("metadata:"),
            "empty metadata should be omitted"
        );
    }

    #[test]
    fn test_render_skill_instructions_expands_version() {
        use std::collections::HashMap;
        use swissarmyhammer_skills::{Skill, SkillName, SkillResources, SkillSource};

        let mut metadata = HashMap::new();
        metadata.insert("version".to_string(), "{{version}}".to_string());
        metadata.insert("author".to_string(), "swissarmyhammer".to_string());

        let skill = Skill {
            name: SkillName::new("tmpl-skill").unwrap(),
            description: "skill with templated metadata".to_string(),
            license: None,
            compatibility: None,
            metadata,
            allowed_tools: vec![],
            instructions: "body with {{version}}".to_string(),
            source_path: None,
            source: SkillSource::Builtin,
            resources: SkillResources::default(),
        };

        let prompt_library = PromptLibrary::default();
        let mut template_context = TemplateContext::new();
        template_context.set(
            "version".to_string(),
            serde_json::json!(env!("CARGO_PKG_VERSION")),
        );

        let reporter = NullReporter;
        let rendered =
            render_skill_instructions(&skill, &prompt_library, &template_context, &reporter);

        assert!(
            !rendered.instructions.contains("{{version}}"),
            "instructions should have {{{{version}}}} expanded"
        );
        assert!(
            rendered.instructions.contains(env!("CARGO_PKG_VERSION")),
            "instructions should contain the actual version"
        );

        let version_val = rendered.metadata.get("version").unwrap();
        assert!(
            !version_val.contains("{{version}}"),
            "metadata version should have {{{{version}}}} expanded"
        );
        assert_eq!(
            version_val,
            env!("CARGO_PKG_VERSION"),
            "metadata version should be the crate version"
        );
        assert_eq!(
            rendered.metadata.get("author").unwrap(),
            "swissarmyhammer",
            "non-template metadata should be preserved"
        );
    }

    #[test]
    fn test_skill_deployment_init_returns_one_result() {
        // init() should return exactly one result, either Ok or Error depending
        // on the environment (e.g., whether any agents are detected).
        let component = SkillDeployment;
        let reporter = NullReporter;
        let results = component.init(&InitScope::Project, &reporter);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_skill_deployment_deinit_returns_one_result() {
        let component = SkillDeployment;
        let reporter = NullReporter;
        let results = component.deinit(&InitScope::Project, &reporter);
        assert_eq!(results.len(), 1);
    }
}
