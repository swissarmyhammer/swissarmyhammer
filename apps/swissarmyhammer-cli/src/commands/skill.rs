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

use super::install::components::{
    is_safe_name, is_safe_relative_path, remove_store_entries, save_lockfile_and_report,
};

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

    write_skill_contents(&skill_dir, &rendered_skill)?;

    mirdan::install::deploy_skill_to_agents(name, &skill_dir, None, global)
        .map_err(|e| format!("Failed to deploy skill '{}': {}", name, e))
}

/// Write a rendered skill's `SKILL.md` and any bundled resource files into
/// `skill_dir`, preserving the subdirectory structure of resource keys.
///
/// Resource keys may be multi-segment relative paths (e.g.
/// `references/helper.md`) so skills can deploy progressive-disclosure
/// content under subdirectories. Each path is validated with
/// [`is_safe_relative_path`] and its parent directory is created before the
/// file is written, so links like `[...](./references/FOO.md)` in `SKILL.md`
/// resolve correctly after deployment.
///
/// # Errors
///
/// Returns an error if `SKILL.md` cannot be written, if any resource path
/// fails the relative-path safety check, or if a resource's parent directory
/// cannot be created or file cannot be written.
fn write_skill_contents(
    skill_dir: &std::path::Path,
    skill: &swissarmyhammer_skills::Skill,
) -> Result<(), String> {
    let skill_md_path = skill_dir.join("SKILL.md");
    let content = format_skill_md(skill);
    fs::write(&skill_md_path, &content)
        .map_err(|e| format!("Failed to write {}: {}", skill_md_path.display(), e))?;

    for (resource_path, file_content) in &skill.resources.files {
        if !is_safe_relative_path(resource_path) {
            return Err(format!("Unsafe resource path: {:?}", resource_path));
        }
        let file_path = skill_dir.join(resource_path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "Failed to create resource directory {}: {}",
                    parent.display(),
                    e
                )
            })?;
        }
        fs::write(&file_path, file_content)
            .map_err(|e| format!("Failed to write {}: {}", file_path.display(), e))?;
    }

    Ok(())
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
    let template_context = build_version_template_context();

    let (project_root, mut lockfile) = load_project_lockfile()?;

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
        lockfile.add_package(name.clone(), locked_builtin_skill_package(targets));
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

fn build_version_template_context() -> TemplateContext {
    let mut ctx = TemplateContext::new();
    ctx.set(
        "version".to_string(),
        serde_json::json!(env!("CARGO_PKG_VERSION")),
    );
    ctx
}

fn load_project_lockfile() -> Result<(std::path::PathBuf, mirdan::lockfile::Lockfile), String> {
    let project_root =
        std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
    let lockfile = mirdan::lockfile::Lockfile::load(&project_root)
        .map_err(|e| format!("Failed to load lockfile: {}", e))?;
    Ok((project_root, lockfile))
}

fn locked_builtin_skill_package(targets: Vec<String>) -> mirdan::lockfile::LockedPackage {
    mirdan::lockfile::LockedPackage {
        package_type: mirdan::package_type::PackageType::Skill,
        version: "0.0.0".to_string(),
        resolved: "builtin".to_string(),
        integrity: String::new(),
        installed_at: chrono::Utc::now().to_rfc3339(),
        targets,
    }
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
    let frontmatter = build_skill_frontmatter(skill);
    let yaml = serde_yaml_ng::to_string(&frontmatter).unwrap_or_default();
    format!("---\n{}---\n\n{}\n", yaml, skill.instructions)
}

fn build_skill_frontmatter(skill: &swissarmyhammer_skills::Skill) -> serde_yaml_ng::Mapping {
    let mut fm = serde_yaml_ng::Mapping::new();
    insert_yaml_string(&mut fm, "name", skill.name.as_str());
    insert_yaml_string(&mut fm, "description", &skill.description);

    if !skill.allowed_tools.is_empty() {
        insert_yaml_string(&mut fm, "allowed-tools", &skill.allowed_tools.join(" "));
    }
    if let Some(ref license) = skill.license {
        insert_yaml_string(&mut fm, "license", license);
    }
    if let Some(ref compatibility) = skill.compatibility {
        insert_yaml_string(&mut fm, "compatibility", compatibility);
    }
    if !skill.metadata.is_empty() {
        fm.insert(
            serde_yaml_ng::Value::String("metadata".to_string()),
            build_metadata_mapping(&skill.metadata),
        );
    }
    fm
}

fn insert_yaml_string(map: &mut serde_yaml_ng::Mapping, key: &str, value: &str) {
    map.insert(
        serde_yaml_ng::Value::String(key.to_string()),
        serde_yaml_ng::Value::String(value.to_string()),
    );
}

fn build_metadata_mapping(
    metadata: &std::collections::HashMap<String, String>,
) -> serde_yaml_ng::Value {
    let mut meta_map = serde_yaml_ng::Mapping::new();
    let mut keys: Vec<_> = metadata.keys().collect();
    keys.sort();
    for key in keys {
        insert_yaml_string(&mut meta_map, key, &metadata[key]);
    }
    serde_yaml_ng::Value::Mapping(meta_map)
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
            !md.contains("compatibility"),
            "None compatibility should be omitted"
        );
        assert!(
            !md.contains("metadata:"),
            "empty metadata should be omitted"
        );
    }

    /// Regression: `compatibility` survives the serialize/parse round-trip used
    /// by `sah init`, so tool-prerequisite metadata declared in `builtin/` is
    /// preserved in the generated agent `.skills/` copies.
    #[test]
    fn test_format_skill_md_round_trips_compatibility() {
        use std::collections::HashMap;
        use swissarmyhammer_skills::{Skill, SkillName, SkillResources, SkillSource};

        let compatibility = "Requires the `kanban` MCP tool .";
        let skill = Skill {
            name: SkillName::new("compat-skill").unwrap(),
            description: "skill that declares tool prerequisites".to_string(),
            license: Some("MIT OR Apache-2.0".to_string()),
            compatibility: Some(compatibility.to_string()),
            metadata: HashMap::new(),
            allowed_tools: vec![],
            instructions: "body".to_string(),
            source_path: None,
            source: SkillSource::Builtin,
            resources: SkillResources::default(),
        };

        let md = format_skill_md(&skill);

        assert!(
            md.contains("compatibility:"),
            "frontmatter should contain compatibility field, got:\n{md}"
        );

        let parsed =
            swissarmyhammer_skills::skill_loader::parse_skill_md(&md, SkillSource::Builtin)
                .expect("output should parse as valid SKILL.md");
        assert_eq!(parsed.compatibility.as_deref(), Some(compatibility));
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

    /// Regression for the `.skills/<name>/references/*.md` deploy bug: a skill
    /// whose resource key includes a `references/` subdirectory must land at
    /// `<skill_dir>/references/<name>` after deployment, with `SKILL.md` at the
    /// skill root. Previously, resource keys were flattened and/or rejected by
    /// `is_safe_name`, so bundled references wound up at the skill root and
    /// `[...](./references/FOO.md)` links in `SKILL.md` broke.
    #[test]
    fn test_write_skill_contents_preserves_references_subdir() {
        use std::collections::HashMap;
        use swissarmyhammer_skills::{Skill, SkillName, SkillResources, SkillSource};

        let mut resources = SkillResources::default();
        resources.files.insert(
            "references/helper.md".to_string(),
            "# Helper\n\nReference body.".to_string(),
        );

        let skill = Skill {
            name: SkillName::new("refs-skill").unwrap(),
            description: "skill with a references/ resource".to_string(),
            license: None,
            compatibility: None,
            metadata: HashMap::new(),
            allowed_tools: vec![],
            instructions: "See [helper](./references/helper.md).".to_string(),
            source_path: None,
            source: SkillSource::Builtin,
            resources,
        };

        let temp_dir = tempfile::tempdir().unwrap();
        let skill_dir = temp_dir.path().join("refs-skill");
        fs::create_dir_all(&skill_dir).unwrap();

        write_skill_contents(&skill_dir, &skill).expect("write_skill_contents should succeed");

        let skill_md_path = skill_dir.join("SKILL.md");
        assert!(
            skill_md_path.exists(),
            "SKILL.md should exist at the skill root, expected: {}",
            skill_md_path.display()
        );

        let helper_path = skill_dir.join("references").join("helper.md");
        assert!(
            helper_path.exists(),
            "resource should land at <skill>/references/<name>, expected: {}",
            helper_path.display()
        );

        let helper_content = std::fs::read_to_string(&helper_path).unwrap();
        assert_eq!(helper_content, "# Helper\n\nReference body.");
    }

    /// Parent-directory traversal must be refused at deploy time even if
    /// somehow a resource key sneaks past the earlier validation layers.
    #[test]
    fn test_write_skill_contents_rejects_parent_traversal() {
        use std::collections::HashMap;
        use swissarmyhammer_skills::{Skill, SkillName, SkillResources, SkillSource};

        let mut resources = SkillResources::default();
        resources
            .files
            .insert("../escape.md".to_string(), "bad".to_string());

        let skill = Skill {
            name: SkillName::new("bad-skill").unwrap(),
            description: "skill with an unsafe resource path".to_string(),
            license: None,
            compatibility: None,
            metadata: HashMap::new(),
            allowed_tools: vec![],
            instructions: "body".to_string(),
            source_path: None,
            source: SkillSource::Builtin,
            resources,
        };

        let temp_dir = tempfile::tempdir().unwrap();
        let skill_dir = temp_dir.path().join("bad-skill");
        fs::create_dir_all(&skill_dir).unwrap();

        let err = write_skill_contents(&skill_dir, &skill)
            .expect_err("write_skill_contents should reject `..` traversal");
        assert!(
            err.contains("Unsafe resource path"),
            "error should mention unsafe resource path, got: {err}"
        );
    }
}
