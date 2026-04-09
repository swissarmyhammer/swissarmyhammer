//! Skill resolution and deployment for code-context.
//!
//! Resolves builtin skills, renders template variables, writes the rendered
//! SKILL.md to a temp directory, and deploys to all detected agent `.skills/`
//! directories via mirdan.
//!
//! The `code-context skill` command deploys both the `code-context` skill
//! (teaching agents how to use code intelligence ops) and the `lsp` skill
//! (teaching agents how to diagnose and install missing LSP servers).

use std::collections::HashMap;

/// Skills deployed by `code-context skill`.
const SKILLS_TO_DEPLOY: &[&str] = &["code-context", "lsp"];

/// Resolve a builtin skill by name from the skill registry.
///
/// Returns the skill definition or an error if the skill is not found.
fn resolve_skill(name: &str) -> Result<swissarmyhammer_skills::Skill, String> {
    let resolver = swissarmyhammer_skills::SkillResolver::new();
    let builtins = resolver.resolve_builtins();
    builtins
        .get(name)
        .cloned()
        .ok_or_else(|| format!("builtin '{name}' skill not found"))
}

/// Render template variables (e.g. `{{version}}`) in skill instructions.
fn render_instructions(skill: &swissarmyhammer_skills::Skill) -> String {
    let engine = swissarmyhammer_templating::TemplateEngine::new();
    let mut vars = HashMap::new();
    vars.insert("version".to_string(), env!("CARGO_PKG_VERSION").to_string());
    engine
        .render(&skill.instructions, &vars)
        .unwrap_or_else(|_| skill.instructions.clone())
}

/// Format a skill as a SKILL.md file with YAML frontmatter.
fn format_skill_md(skill: &swissarmyhammer_skills::Skill, instructions: &str) -> String {
    let mut content = String::from("---\n");
    content.push_str(&format!("name: {}\n", skill.name));
    content.push_str(&format!("description: {}\n", skill.description));
    if !skill.allowed_tools.is_empty() {
        content.push_str(&format!(
            "allowed-tools: \"{}\"\n",
            skill.allowed_tools.join(" ")
        ));
    }
    content.push_str("---\n\n");
    content.push_str(instructions);
    content.push('\n');
    content
}

/// Validate that a skill name is a safe filesystem identifier.
///
/// Accepts only alphanumeric characters, hyphens, and underscores.
/// Rejects path traversal sequences, absolute paths, and empty names.
fn validate_skill_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("skill name must not be empty".to_string());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "skill name '{name}' contains invalid characters (only alphanumeric, hyphens, underscores allowed)"
        ));
    }
    Ok(())
}

/// Write a skill's SKILL.md to a temp directory and deploy to agent `.skills/` dirs.
fn write_and_deploy(name: &str, skill_content: &str) -> Result<Vec<String>, String> {
    validate_skill_name(name)?;
    let temp_dir = tempfile::tempdir().map_err(|e| format!("failed to create temp dir: {e}"))?;
    let skill_dir = temp_dir.path().join(name);
    std::fs::create_dir_all(&skill_dir)
        .map_err(|e| format!("failed to create temp skill dir: {e}"))?;
    std::fs::write(skill_dir.join("SKILL.md"), skill_content)
        .map_err(|e| format!("failed to write SKILL.md: {e}"))?;
    mirdan::install::deploy_skill_to_agents(name, &skill_dir, None, false)
        .map_err(|e| format!("deploying {name} skill: {e}"))
}

/// Resolve, render, and deploy a single builtin skill by name.
///
/// Returns the list of agent directories the skill was deployed to,
/// or an error description.
pub fn deploy_single_skill(name: &str) -> Result<Vec<String>, String> {
    let skill = resolve_skill(name)?;
    let rendered = render_instructions(&skill);
    let content = format_skill_md(&skill, &rendered);
    write_and_deploy(name, &content)
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
            builtins.get("code-context").is_some(),
            "builtin 'code-context' skill should exist"
        );
    }

    #[test]
    fn test_lsp_skill_exists_in_builtins() {
        let resolver = swissarmyhammer_skills::SkillResolver::new();
        let builtins = resolver.resolve_builtins();
        assert!(
            builtins.get("lsp").is_some(),
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
        let skill = resolve_skill("code-context").expect("code-context skill should resolve");
        let rendered = render_instructions(&skill);
        assert!(
            rendered.contains("code"),
            "rendered instructions should contain tool content"
        );
        let md = format_skill_md(&skill, &rendered);
        assert!(
            md.starts_with("---\n"),
            "SKILL.md should start with frontmatter"
        );
        assert!(
            md.contains("name: code-context"),
            "frontmatter should contain skill name"
        );
    }

    #[test]
    fn test_resolve_render_format_lsp() {
        let skill = resolve_skill("lsp").expect("lsp skill should resolve");
        let rendered = render_instructions(&skill);
        assert!(
            rendered.contains("lsp status"),
            "lsp skill should reference lsp status op"
        );
        let md = format_skill_md(&skill, &rendered);
        assert!(
            md.starts_with("---\n"),
            "SKILL.md should start with frontmatter"
        );
        assert!(
            md.contains("name: lsp"),
            "frontmatter should contain skill name"
        );
    }

    #[test]
    fn test_resolve_skill_nonexistent_returns_error() {
        let result = resolve_skill("nonexistent-skill-that-does-not-exist");
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
    fn test_validate_skill_name_valid() {
        assert!(validate_skill_name("code-context").is_ok());
        assert!(validate_skill_name("lsp").is_ok());
        assert!(validate_skill_name("my_skill").is_ok());
        assert!(validate_skill_name("skill123").is_ok());
    }

    #[test]
    fn test_validate_skill_name_rejects_traversal() {
        assert!(validate_skill_name("..").is_err());
        assert!(validate_skill_name("../etc/passwd").is_err());
        assert!(validate_skill_name("/absolute").is_err());
        assert!(validate_skill_name("has spaces").is_err());
        assert!(validate_skill_name("").is_err());
    }
}
