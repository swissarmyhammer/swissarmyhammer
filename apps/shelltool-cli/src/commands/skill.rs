//! Skill resolution and deployment for shelltool.
//!
//! Resolves the builtin `shell` skill, renders template variables, writes the
//! rendered SKILL.md to a temp directory, and deploys to all detected agent
//! `.skills/` directories via mirdan.
//!
//! `ShelltoolSkillDeployment` implements `Initializable` so that skill
//! deployment runs as part of `shelltool init` / `shelltool deinit`.
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

/// The builtin skill name deployed by shelltool.
const SKILL_NAME: &str = "shell";

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

/// Resolve, render, and deploy the shell builtin skill.
///
/// Returns the list of agent directories the skill was deployed to,
/// or an error description.
pub fn deploy_shell_skill() -> Result<Vec<String>, String> {
    let skill = deploy::resolve_skill(SKILL_NAME)?;
    let (instructions, metadata) = render_skill(&skill);
    let content = deploy::format_skill_md(&skill, &instructions, &metadata);
    deploy::write_and_deploy(SKILL_NAME, &content)
}

// ── ShelltoolSkillDeployment (Initializable) ────────────────────────────────

/// Deploys/removes the `shell` skill as part of `shelltool init` / `shelltool deinit`.
///
/// Resolves the builtin `shell` skill, renders template variables, formats
/// the SKILL.md, and deploys it to all detected agent `.skills/` directories.
pub struct ShelltoolSkillDeployment;

impl Initializable for ShelltoolSkillDeployment {
    /// The component name shown in init/deinit output.
    fn name(&self) -> &str {
        "shelltool-skill-deployment"
    }

    /// Component category: skills.
    fn category(&self) -> &str {
        "skills"
    }

    /// Priority 30 — runs after ShelltoolMcpRegistration (priority 10) and
    /// ShellExecuteTool (priority 0, the default) so that config and Bash
    /// denial are in place before the skill is deployed.
    fn priority(&self) -> i32 {
        30
    }

    /// Only applies in project and local scopes — not user/global scope.
    fn is_applicable(&self, scope: &InitScope) -> bool {
        matches!(scope, InitScope::Project | InitScope::Local)
    }

    /// Deploy the shell skill to all detected agent `.skills/` directories.
    fn init(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        match deploy_shell_skill() {
            Ok(targets) => {
                reporter.emit(&InitEvent::Action {
                    verb: "Deployed".to_string(),
                    message: format!("shell skill to {}", targets.join(", ")),
                });
                vec![InitResult::ok(
                    self.name(),
                    format!("Shell skill deployed to {}", targets.join(", ")),
                )]
            }
            Err(e) => {
                vec![InitResult::error(
                    self.name(),
                    format!("Failed to deploy shell skill: {e}"),
                )]
            }
        }
    }

    /// Remove the shell skill from all detected agents.
    fn deinit(&self, _scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        if let Err(e) = mirdan::install::uninstall_skill(SKILL_NAME, None, false) {
            reporter.emit(&InitEvent::Warning {
                message: format!("Failed to uninstall shell skill: {e}"),
            });
        } else {
            reporter.emit(&InitEvent::Action {
                verb: "Removed".to_string(),
                message: "shell skill from agents".to_string(),
            });
        }

        vec![InitResult::ok(
            self.name(),
            "Shell skill deployment removed",
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_common::reporter::NullReporter;

    #[test]
    fn test_skill_exists_in_builtins() {
        let resolver = swissarmyhammer_skills::SkillResolver::new();
        let builtins = resolver.resolve_builtins();
        assert!(
            builtins.contains_key("shell"),
            "builtin 'shell' skill should exist"
        );
    }

    #[test]
    fn test_skill_has_valid_content() {
        let resolver = swissarmyhammer_skills::SkillResolver::new();
        let builtins = resolver.resolve_builtins();
        let skill = builtins.get("shell").expect("shell skill should exist");

        assert_eq!(skill.name.as_str(), "shell");
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
    fn test_resolve_skill_returns_shell() {
        let skill = deploy::resolve_skill("shell").expect("shell skill should resolve");
        assert_eq!(skill.name.as_str(), "shell");
    }

    #[test]
    fn test_resolve_render_format_shell() {
        let skill = deploy::resolve_skill("shell").expect("shell skill should resolve");
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
            md.contains("name: shell"),
            "frontmatter should contain skill name"
        );
    }

    #[test]
    fn test_deploy_shell_skill_returns_valid_result() {
        // deploy_shell_skill() may fail if there are no agent directories detected,
        // but it should never panic.
        let _result = deploy_shell_skill();
    }

    #[test]
    fn test_shelltool_skill_deployment_name_and_priority() {
        let component = ShelltoolSkillDeployment;
        assert_eq!(
            Initializable::name(&component),
            "shelltool-skill-deployment"
        );
        assert_eq!(component.category(), "skills");
        assert_eq!(component.priority(), 30);
    }

    #[test]
    fn test_shelltool_skill_deployment_is_applicable() {
        let component = ShelltoolSkillDeployment;
        assert!(component.is_applicable(&InitScope::Project));
        assert!(component.is_applicable(&InitScope::Local));
        assert!(!component.is_applicable(&InitScope::User));
    }

    #[test]
    fn test_shelltool_skill_deployment_init() {
        let component = ShelltoolSkillDeployment;
        let reporter = NullReporter;
        let results = component.init(&InitScope::Project, &reporter);
        // Should return exactly one result (Ok or Error depending on env)
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_shelltool_skill_deployment_deinit() {
        let component = ShelltoolSkillDeployment;
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
}
