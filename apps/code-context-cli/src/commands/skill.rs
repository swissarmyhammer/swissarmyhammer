//! Skill resolution and deployment for code-context.
//!
//! Resolves builtin skills, renders template variables, writes the rendered
//! SKILL.md to a temp directory, and deploys to all detected agent `.skills/`
//! directories via mirdan.
//!
//! The `code-context skill` command deploys both the `code-context` skill
//! (teaching agents how to use code intelligence ops) and the `lsp` skill
//! (teaching agents how to diagnose and install missing LSP servers).
//!
//! The heavy lifting (resolve, format, validate, deploy) is delegated to
//! [`swissarmyhammer_skills::deploy`]. This module adds only the template
//! rendering step (which depends on `swissarmyhammer-templating`, a crate
//! that cannot be a dependency of `swissarmyhammer-skills` without creating
//! a cycle).

use std::collections::HashMap;

use swissarmyhammer_skills::deploy;

/// Skills deployed by `code-context skill`.
const SKILLS_TO_DEPLOY: &[&str] = &["code-context", "lsp"];

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

/// Resolve, render, and deploy a single builtin skill by name.
///
/// Returns the list of agent directories the skill was deployed to,
/// or an error description.
pub fn deploy_single_skill(name: &str) -> Result<Vec<String>, String> {
    let skill = deploy::resolve_skill(name)?;
    let (instructions, metadata) = render_skill(&skill);
    let content = deploy::format_skill_md(&skill, &instructions, &metadata);
    deploy::write_and_deploy(name, &content)
}

/// Deploy all code-context-related skills to detected agent `.skills/` directories.
///
/// Deploys each skill in [`SKILLS_TO_DEPLOY`] independently — if one fails,
/// the others still proceed. Returns exit code 0 if all succeed, 1 if any fail.
pub fn run_skill() -> i32 {
    let mut had_error = false;
    for name in SKILLS_TO_DEPLOY {
        match deploy_single_skill(name) {
            Ok(targets) => println!("Deployed {name} skill to {}", targets.join(", ")),
            Err(e) => {
                eprintln!("Error deploying {name} skill: {e}");
                had_error = true;
            }
        }
    }
    if had_error {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_exists_in_builtins() {
        let resolver = swissarmyhammer_skills::SkillResolver::new();
        let builtins = resolver.resolve_builtins();
        assert!(
            builtins.contains_key("code-context"),
            "builtin 'code-context' skill should exist"
        );
    }

    #[test]
    fn test_lsp_skill_exists_in_builtins() {
        let resolver = swissarmyhammer_skills::SkillResolver::new();
        let builtins = resolver.resolve_builtins();
        assert!(
            builtins.contains_key("lsp"),
            "builtin 'lsp' skill should exist"
        );
    }

    #[test]
    fn test_skill_has_valid_content() {
        let resolver = swissarmyhammer_skills::SkillResolver::new();
        let builtins = resolver.resolve_builtins();
        let skill = builtins
            .get("code-context")
            .expect("code-context skill should exist");

        assert_eq!(skill.name.as_str(), "code-context");
        assert!(
            !skill.description.is_empty(),
            "description should not be empty"
        );
        assert!(
            !skill.instructions.is_empty(),
            "instructions should not be empty"
        );

        let line_count = skill.instructions.lines().count();
        assert!(
            line_count >= 50,
            "instructions should have >= 50 lines, got {line_count}"
        );
    }

    #[test]
    fn test_lsp_skill_has_valid_content() {
        let resolver = swissarmyhammer_skills::SkillResolver::new();
        let builtins = resolver.resolve_builtins();
        let skill = builtins.get("lsp").expect("lsp skill should exist");

        assert_eq!(skill.name.as_str(), "lsp");
        assert!(
            !skill.description.is_empty(),
            "description should not be empty"
        );
        assert!(
            !skill.instructions.is_empty(),
            "instructions should not be empty"
        );

        let line_count = skill.instructions.lines().count();
        assert!(
            line_count >= 30,
            "instructions should have >= 30 lines, got {line_count}"
        );
    }

    #[test]
    fn test_resolve_render_format_code_context() {
        let skill =
            deploy::resolve_skill("code-context").expect("code-context skill should resolve");
        let (instructions, metadata) = render_skill(&skill);
        assert!(
            instructions.contains("code"),
            "rendered instructions should contain tool content"
        );
        let md = deploy::format_skill_md(&skill, &instructions, &metadata);
        assert!(
            md.starts_with("---\n"),
            "SKILL.md should start with frontmatter"
        );
        assert!(
            md.contains("name: code-context"),
            "frontmatter should contain skill name"
        );
        assert!(
            md.contains("metadata:"),
            "frontmatter should contain metadata block"
        );
    }

    #[test]
    fn test_resolve_render_format_lsp() {
        let skill = deploy::resolve_skill("lsp").expect("lsp skill should resolve");
        let (instructions, metadata) = render_skill(&skill);
        assert!(
            instructions.contains("lsp status"),
            "lsp skill should reference lsp status op"
        );
        let md = deploy::format_skill_md(&skill, &instructions, &metadata);
        assert!(
            md.starts_with("---\n"),
            "SKILL.md should start with frontmatter"
        );
        assert!(
            md.contains("name: lsp"),
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

    #[test]
    fn test_run_skill_returns_valid_exit_code() {
        // run_skill() may fail if there are no agent directories detected,
        // but it should never panic -- it returns 0 or 1.
        let exit_code = run_skill();
        assert!(
            exit_code == 0 || exit_code == 1,
            "exit code should be 0 or 1, got {exit_code}"
        );
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
