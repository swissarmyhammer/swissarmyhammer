//! Skill resolution and deployment for kanban.
//!
//! Resolves the builtin `kanban` skill, renders template variables, writes
//! the rendered SKILL.md to a temp directory, and deploys to all detected
//! agent `.skills/` directories via mirdan.
//!
//! `KanbanSkillDeployment` implements `Initializable` so that skill
//! deployment runs as part of `kanban init` / `kanban deinit`.
//!
//! The heavy lifting (resolve, format, validate, deploy) is delegated to
//! [`swissarmyhammer_skills::deploy`]. This module adds only the template
//! rendering step (which depends on `swissarmyhammer-templating`, a crate
//! that cannot be a dependency of `swissarmyhammer-skills` without creating
//! a cycle) and the `Initializable` impl.

use std::collections::HashMap;

use swissarmyhammer_common::lifecycle::{InitResult, InitScope, Initializable};
use swissarmyhammer_common::reporter::{InitEvent, InitReporter};
use swissarmyhammer_skills::deploy;

/// The init profile whose tagged builtin skills kanban deploys.
const KANBAN_PROFILE: &str = "kanban";

/// Render template variables (e.g. `{{version}}`) in skill instructions and metadata.
///
/// Substitutes known placeholders — currently only `{{version}}` (set to this
/// crate's `CARGO_PKG_VERSION`). Renders both `skill.instructions` and any
/// metadata values containing template syntax.
///
/// Falls back to the raw text if template rendering fails, logging a warning
/// via `tracing`.
fn render_skill(skill: &swissarmyhammer_skills::Skill) -> (String, HashMap<String, String>) {
    let engine = swissarmyhammer_templating::TemplateEngine::new();
    let mut vars = HashMap::new();
    vars.insert("version".to_string(), env!("CARGO_PKG_VERSION").to_string());

    let instructions = engine
        .render(&skill.instructions, &vars)
        .unwrap_or_else(|err| {
            tracing::warn!(
                skill = skill.name.as_str(),
                error = %err,
                "template rendering failed, falling back to raw instructions"
            );
            skill.instructions.clone()
        });

    // Render template variables in metadata values (e.g., version: "{{version}}")
    let mut metadata = skill.metadata.clone();
    for value in metadata.values_mut() {
        if value.contains("{{") {
            if let Ok(rendered_value) = engine.render(value, &vars) {
                *value = rendered_value;
            }
        }
    }

    (instructions, metadata)
}

/// Resolve, render, and deploy every builtin skill tagged with the `kanban`
/// init profile.
///
/// Returns the deduplicated list of agent directories the skills were deployed
/// to, or an error description on the first deployment failure.
pub fn deploy_kanban_skills() -> Result<Vec<String>, String> {
    let skills = deploy::resolve_profile_skills(KANBAN_PROFILE);
    if skills.is_empty() {
        return Err(format!(
            "no builtin skills are tagged with the '{KANBAN_PROFILE}' profile"
        ));
    }

    let mut targets: Vec<String> = Vec::new();
    for skill in &skills {
        let name = skill.name.as_str();
        let (instructions, metadata) = render_skill(skill);
        let content = deploy::format_skill_md(skill, &instructions, &metadata);
        for target in deploy::write_and_deploy(name, &content)? {
            if !targets.contains(&target) {
                targets.push(target);
            }
        }
    }
    Ok(targets)
}

// ── KanbanSkillDeployment (Initializable) ────────────────────────────────────

/// Deploys/removes the `kanban`-profile skills as part of `kanban init` /
/// `kanban deinit`.
///
/// Resolves every builtin skill tagged with the `kanban` init profile, renders
/// template variables, formats the SKILL.md, and deploys each to all detected
/// agent `.skills/` directories.
pub struct KanbanSkillDeployment;

impl Initializable for KanbanSkillDeployment {
    /// The component name shown in init/deinit output.
    fn name(&self) -> &str {
        "kanban-skill-deployment"
    }

    /// Component category: skills.
    fn category(&self) -> &str {
        "skills"
    }

    /// Priority 20 — the skill-deployment step. MCP registration is owned by
    /// `KanbanTool` (priority 55), so it actually runs *after* this; the two
    /// are independent — deploying the `kanban`-profile skills writes SKILL.md
    /// to agent `.skills/` dirs and does not depend on the MCP entry being
    /// present.
    fn priority(&self) -> i32 {
        20
    }

    /// Only applies in project and local scopes — not user/global scope.
    fn is_applicable(&self, scope: &InitScope) -> bool {
        matches!(scope, InitScope::Project | InitScope::Local)
    }

    /// Deploy the kanban-profile skills to all detected agent `.skills/` directories.
    fn init(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        match deploy_kanban_skills() {
            Ok(targets) => {
                reporter.emit(&InitEvent::Action {
                    verb: "Deployed".to_string(),
                    message: format!("kanban skills to {}", targets.join(", ")),
                });
                vec![InitResult::ok(
                    self.name(),
                    format!("Kanban skills deployed to {}", targets.join(", ")),
                )]
            }
            Err(e) => {
                vec![InitResult::error(
                    self.name(),
                    format!("Failed to deploy kanban skills: {e}"),
                )]
            }
        }
    }

    /// Remove the kanban-profile skills from all detected agents.
    fn deinit(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        for skill in deploy::resolve_profile_skills(KANBAN_PROFILE) {
            let name = skill.name.as_str();
            if let Err(e) = mirdan::install::uninstall_skill(name, None, false) {
                reporter.emit(&InitEvent::Warning {
                    message: format!("Failed to uninstall {name} skill: {e}"),
                });
            } else {
                reporter.emit(&InitEvent::Action {
                    verb: "Removed".to_string(),
                    message: format!("{name} skill from agents"),
                });
            }
        }

        vec![InitResult::ok(
            self.name(),
            "Kanban skill deployment removed",
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_common::reporter::NullReporter;
    use swissarmyhammer_common::test_utils::CurrentDirGuard;

    /// Deploying a skill writes the central `.skills/` store and per-agent
    /// `.claude/skills`, `.zed/skills`, etc. directories relative to the
    /// process working directory. During `cargo test` the working directory
    /// is the crate manifest dir, so any test that runs real deployment must
    /// first chdir into an isolated temp dir or it pollutes the source tree.
    /// Returns the guard (restores cwd on drop) and the owning `TempDir`.
    fn isolated_deploy_dir() -> (CurrentDirGuard, tempfile::TempDir) {
        let temp = tempfile::tempdir().expect("create temp dir for skill deployment");
        let guard = CurrentDirGuard::new(temp.path()).expect("chdir into isolated temp dir");
        (guard, temp)
    }

    #[test]
    fn test_skill_exists_in_builtins() {
        let resolver = swissarmyhammer_skills::SkillResolver::new();
        let builtins = resolver.resolve_builtins();
        assert!(
            builtins.contains_key("kanban"),
            "builtin 'kanban' skill should exist"
        );
    }

    #[test]
    fn test_skill_has_valid_content() {
        let resolver = swissarmyhammer_skills::SkillResolver::new();
        let builtins = resolver.resolve_builtins();
        let skill = builtins.get("kanban").expect("kanban skill should exist");

        assert_eq!(skill.name.as_str(), "kanban");
        assert!(
            !skill.description.is_empty(),
            "description should not be empty"
        );
        assert!(
            !skill.instructions.is_empty(),
            "instructions should not be empty"
        );
    }

    #[test]
    fn test_resolve_skill_returns_kanban() {
        let skill = deploy::resolve_skill("kanban").expect("kanban skill should resolve");
        assert_eq!(skill.name.as_str(), "kanban");
    }

    #[test]
    fn test_resolve_render_format_kanban() {
        let skill = deploy::resolve_skill("kanban").expect("kanban skill should resolve");
        let (instructions, metadata) = render_skill(&skill);
        assert!(
            !instructions.is_empty(),
            "rendered instructions should not be empty"
        );
        let md = deploy::format_skill_md(&skill, &instructions, &metadata);
        assert!(
            md.starts_with("---\n"),
            "SKILL.md should start with frontmatter"
        );
        assert!(
            md.contains("name: kanban"),
            "frontmatter should contain skill name"
        );
        assert!(
            md.contains("metadata:"),
            "frontmatter should contain metadata block"
        );
    }

    #[test]
    fn test_resolve_skill_nonexistent_returns_error() {
        let result = deploy::resolve_skill("nonexistent-skill-that-does-not-exist");
        assert!(result.is_err(), "nonexistent skill should return Err");
        assert!(result.unwrap_err().contains("not found"));
    }

    /// The locked `kanban`-profile membership — the workflow cluster. The
    /// `explore`/`code-context` exploration skills now belong to the separate
    /// `code-context` profile, not `kanban`. The filter must select exactly
    /// this subset and nothing else.
    const EXPECTED_KANBAN_PROFILE_SKILLS: &[&str] =
        &["kanban", "plan", "task", "finish", "implement", "review"];

    #[test]
    fn test_resolve_profile_skills_selects_exact_subset() {
        let mut got: Vec<String> = deploy::resolve_profile_skills("kanban")
            .into_iter()
            .map(|s| s.name.as_str().to_string())
            .collect();
        got.sort();

        let mut expected: Vec<String> = EXPECTED_KANBAN_PROFILE_SKILLS
            .iter()
            .map(|s| s.to_string())
            .collect();
        expected.sort();

        assert_eq!(
            got, expected,
            "kanban profile filter must select exactly the tagged subset"
        );
    }

    #[test]
    fn test_resolve_profile_skills_unknown_profile_is_empty() {
        assert!(
            deploy::resolve_profile_skills("no-such-profile").is_empty(),
            "an unknown profile should match no skills"
        );
    }

    #[test]
    fn test_kanban_profile_excludes_untagged_skill() {
        // `commit` is a builtin skill that is NOT in the kanban profile, so the
        // filter must not pick it up.
        let resolver = swissarmyhammer_skills::SkillResolver::new();
        let builtins = resolver.resolve_builtins();
        assert!(
            builtins.contains_key("commit"),
            "builtin 'commit' skill should exist (sanity check)"
        );

        let selected: Vec<String> = deploy::resolve_profile_skills("kanban")
            .into_iter()
            .map(|s| s.name.as_str().to_string())
            .collect();
        assert!(
            !selected.contains(&"commit".to_string()),
            "untagged 'commit' skill must not be selected by the kanban profile"
        );
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_deploy_kanban_skills_returns_valid_result() {
        // deploy_kanban_skills() may fail if there are no agent directories detected,
        // but it should never panic. Run it inside an isolated temp dir so the
        // deployed `.skills/`, `.claude/skills`, etc. land there, not in the
        // real crate source tree.
        //
        // `#[serial_test::serial(cwd)]` joins the crate-wide `cwd` serialization
        // group — the single mutex shared by EVERY CWD-touching test in this
        // crate (`doctor.rs`, `logging.rs`, `serve.rs`). `isolated_deploy_dir()`
        // mutates process-global CWD via `CurrentDirGuard`, so it must not run
        // concurrently with any other test that reads or mutates CWD.
        let (_guard, _temp) = isolated_deploy_dir();
        let _result = deploy_kanban_skills();
    }

    #[test]
    fn test_kanban_skill_deployment_name_and_priority() {
        let component = KanbanSkillDeployment;
        assert_eq!(Initializable::name(&component), "kanban-skill-deployment");
        assert_eq!(component.category(), "skills");
        assert_eq!(component.priority(), 20);
    }

    #[test]
    fn test_kanban_skill_deployment_is_applicable() {
        let component = KanbanSkillDeployment;
        assert!(component.is_applicable(&InitScope::Project));
        assert!(component.is_applicable(&InitScope::Local));
        assert!(!component.is_applicable(&InitScope::User));
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_kanban_skill_deployment_init() {
        // init() deploys the kanban skill via mirdan into cwd-relative agent
        // directories — isolate cwd to a temp dir so it does not pollute the
        // crate source tree. `#[serial_test::serial(cwd)]` joins the crate-wide
        // `cwd` group (see `test_deploy_kanban_skill_returns_valid_result`).
        let (_guard, _temp) = isolated_deploy_dir();
        let component = KanbanSkillDeployment;
        let reporter = NullReporter;
        let results = component.init(&InitScope::Project, &reporter);
        // Should return exactly one result (Ok or Error depending on env)
        assert_eq!(results.len(), 1);
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_kanban_skill_deployment_deinit() {
        // deinit() removes skill symlinks from cwd-relative agent directories;
        // isolate cwd to a temp dir so the call targets the temp dir only.
        // `#[serial_test::serial(cwd)]` joins the crate-wide `cwd` group (see
        // `test_deploy_kanban_skill_returns_valid_result`).
        let (_guard, _temp) = isolated_deploy_dir();
        let component = KanbanSkillDeployment;
        let reporter = NullReporter;
        let results = component.deinit(&InitScope::Project, &reporter);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_render_skill_expands_version() {
        let mut metadata = HashMap::new();
        metadata.insert("version".to_string(), "{{version}}".to_string());
        metadata.insert("author".to_string(), "swissarmyhammer".to_string());

        let skill = swissarmyhammer_skills::Skill {
            name: swissarmyhammer_skills::SkillName::new("tmpl-skill").unwrap(),
            description: "skill with template metadata".to_string(),
            license: None,
            compatibility: None,
            metadata,
            allowed_tools: vec![],
            profiles: vec![],
            instructions: "body with {{version}}".to_string(),
            source_path: None,
            source: swissarmyhammer_skills::SkillSource::Builtin,
            resources: swissarmyhammer_skills::SkillResources::default(),
        };

        let (instructions, rendered_metadata) = render_skill(&skill);

        assert!(
            !instructions.contains("{{version}}"),
            "instructions should have {{{{version}}}} expanded"
        );
        assert!(
            instructions.contains(env!("CARGO_PKG_VERSION")),
            "instructions should contain the actual version"
        );

        let version_val = rendered_metadata.get("version").unwrap();
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
            rendered_metadata.get("author").unwrap(),
            "swissarmyhammer",
            "non-template metadata should be preserved"
        );
    }

    #[test]
    fn test_render_skill_preserves_non_template_metadata() {
        let skill = deploy::resolve_skill("kanban").expect("kanban skill should resolve");
        let original_metadata = skill.metadata.clone();
        let (_instructions, rendered_metadata) = render_skill(&skill);

        // Keys should be preserved exactly
        assert_eq!(
            rendered_metadata.keys().collect::<Vec<_>>(),
            original_metadata.keys().collect::<Vec<_>>(),
            "metadata keys should round-trip through render"
        );

        // Non-template values (those without `{{`) should be unchanged
        for (k, v) in &original_metadata {
            if !v.contains("{{") {
                assert_eq!(
                    rendered_metadata.get(k),
                    Some(v),
                    "non-template metadata value for '{k}' should be preserved"
                );
            }
        }
    }
}
