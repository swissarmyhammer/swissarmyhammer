//! Skill resolution and deployment for code-context.
//!
//! Resolves builtin skills, renders template variables, writes the rendered
//! SKILL.md to a temp directory, and deploys to all detected agent `.skills/`
//! directories via mirdan.
//!
//! The `code-context skill` command deploys both the `code-context` skill
//! (teaching agents how to use code intelligence ops) and the `lsp` skill
//! (teaching agents how to diagnose and install missing LSP servers).

use serde::Serialize;
use std::collections::HashMap;

/// Skills deployed by `code-context skill`.
const SKILLS_TO_DEPLOY: &[&str] = &["code-context", "lsp"];

/// YAML frontmatter fields for a SKILL.md file.
///
/// Mirrors the fields expected by `swissarmyhammer_skills::SkillFrontmatter`
/// (the deserialization counterpart in the skill loader). Using `serde_yaml_ng`
/// to serialize this struct produces properly escaped YAML, even when field
/// values contain special characters like colons or quotes.
#[derive(Serialize)]
struct SkillFrontmatter<'a> {
    name: &'a str,
    description: &'a str,
    #[serde(rename = "allowed-tools", skip_serializing_if = "Option::is_none")]
    allowed_tools: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    license: Option<&'a str>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    metadata: HashMap<String, String>,
}

/// Resolve a builtin skill by name from the skill registry.
///
/// Looks up `name` in the set of compiled-in builtin skills.
///
/// # Errors
///
/// Returns an error message if no builtin skill matches `name`.
fn resolve_skill(name: &str) -> Result<swissarmyhammer_skills::Skill, String> {
    let resolver = swissarmyhammer_skills::SkillResolver::new();
    let builtins = resolver.resolve_builtins();
    builtins
        .get(name)
        .cloned()
        .ok_or_else(|| format!("builtin '{name}' skill not found"))
}

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

/// Format a skill as a complete SKILL.md file with YAML frontmatter.
///
/// Combines the skill's frontmatter fields (`name`, `description`,
/// `allowed_tools`, `license`, `metadata`) into YAML frontmatter and appends
/// the already-rendered `instructions` as the body. The `metadata` parameter
/// is passed separately because it may have had template variables rendered.
///
/// Uses `serde_yaml_ng` to serialize the frontmatter, ensuring that values
/// containing YAML special characters (colons, quotes, newlines) are properly
/// escaped. The output is compatible with `swissarmyhammer_skills::parse_skill_md`.
///
/// # Panics
///
/// Panics if `serde_yaml_ng` fails to serialize the frontmatter struct, which
/// should be unreachable since all fields are plain strings.
fn format_skill_md(
    skill: &swissarmyhammer_skills::Skill,
    instructions: &str,
    metadata: &HashMap<String, String>,
) -> String {
    let allowed_tools = if skill.allowed_tools.is_empty() {
        None
    } else {
        Some(skill.allowed_tools.join(" "))
    };

    let frontmatter = SkillFrontmatter {
        name: skill.name.as_str(),
        description: &skill.description,
        allowed_tools,
        license: skill.license.as_deref(),
        metadata: metadata.clone(),
    };

    let yaml = serde_yaml_ng::to_string(&frontmatter)
        .expect("SkillFrontmatter serialization should not fail");

    format!("---\n{yaml}---\n\n{instructions}\n")
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

/// Write a skill's rendered SKILL.md to a temp directory and deploy it.
///
/// Creates a temporary directory structure `<tmpdir>/<name>/SKILL.md` containing
/// `skill_content`, then delegates to `mirdan::install::deploy_skill_to_agents`
/// to copy it into every detected agent's `.skills/` directory.
///
/// # Errors
///
/// Returns an error if `name` fails validation, the temp directory cannot be
/// created, the file cannot be written, or mirdan deployment fails.
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
    let (instructions, metadata) = render_skill(&skill);
    let content = format_skill_md(&skill, &instructions, &metadata);
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
        let skill = resolve_skill("code-context").expect("code-context skill should resolve");
        let (instructions, metadata) = render_skill(&skill);
        assert!(
            instructions.contains("code"),
            "rendered instructions should contain tool content"
        );
        let md = format_skill_md(&skill, &instructions, &metadata);
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
        let skill = resolve_skill("lsp").expect("lsp skill should resolve");
        let (instructions, metadata) = render_skill(&skill);
        assert!(
            instructions.contains("lsp status"),
            "lsp skill should reference lsp status op"
        );
        let md = format_skill_md(&skill, &instructions, &metadata);
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

    #[test]
    fn test_format_skill_md_escapes_yaml_special_chars() {
        // Build a Skill with a description containing YAML special characters
        // that would break hand-rolled string concatenation.
        let skill = swissarmyhammer_skills::Skill {
            name: swissarmyhammer_skills::SkillName::new("test-skill").unwrap(),
            description: "description with: colons, \"quotes\", and {braces}".to_string(),
            license: None,
            compatibility: None,
            metadata: HashMap::new(),
            allowed_tools: vec!["tool-a".to_string(), "tool-b".to_string()],
            instructions: "body".to_string(),
            source_path: None,
            source: swissarmyhammer_skills::SkillSource::Builtin,
            resources: swissarmyhammer_skills::SkillResources::default(),
        };

        let md = format_skill_md(&skill, "# Instructions\n\nDo the thing.", &skill.metadata);

        // The output must start and end with proper frontmatter delimiters.
        assert!(md.starts_with("---\n"), "should start with frontmatter");
        assert!(
            md.contains("\n---\n"),
            "should have closing frontmatter delimiter"
        );

        // The output must round-trip through the skill loader's parser.
        let parsed = swissarmyhammer_skills::skill_loader::parse_skill_md(
            &md,
            swissarmyhammer_skills::SkillSource::Builtin,
        )
        .expect("format_skill_md output should be parseable as valid SKILL.md");

        assert_eq!(parsed.name.as_str(), "test-skill");
        assert_eq!(parsed.description, skill.description);
        assert_eq!(parsed.allowed_tools, vec!["tool-a", "tool-b"]);
        assert_eq!(parsed.instructions, "# Instructions\n\nDo the thing.");
    }

    #[test]
    fn test_format_skill_md_omits_empty_allowed_tools() {
        let skill = swissarmyhammer_skills::Skill {
            name: swissarmyhammer_skills::SkillName::new("minimal").unwrap(),
            description: "a minimal skill".to_string(),
            license: None,
            compatibility: None,
            metadata: HashMap::new(),
            allowed_tools: vec![],
            instructions: "body".to_string(),
            source_path: None,
            source: swissarmyhammer_skills::SkillSource::Builtin,
            resources: swissarmyhammer_skills::SkillResources::default(),
        };

        let md = format_skill_md(&skill, "body", &skill.metadata);

        // allowed-tools should not appear in the output when empty.
        assert!(
            !md.contains("allowed-tools"),
            "empty allowed_tools should be omitted from frontmatter"
        );

        // Must still round-trip.
        let parsed = swissarmyhammer_skills::skill_loader::parse_skill_md(
            &md,
            swissarmyhammer_skills::SkillSource::Builtin,
        )
        .expect("output should parse as valid SKILL.md");
        assert_eq!(parsed.name.as_str(), "minimal");
        assert!(parsed.allowed_tools.is_empty());
    }

    #[test]
    fn test_format_skill_md_preserves_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), "swissarmyhammer".to_string());
        metadata.insert("version".to_string(), "1.2.3".to_string());

        let skill = swissarmyhammer_skills::Skill {
            name: swissarmyhammer_skills::SkillName::new("meta-skill").unwrap(),
            description: "a skill with metadata".to_string(),
            license: Some("MIT".to_string()),
            compatibility: None,
            metadata: metadata.clone(),
            allowed_tools: vec![],
            instructions: "body".to_string(),
            source_path: None,
            source: swissarmyhammer_skills::SkillSource::Builtin,
            resources: swissarmyhammer_skills::SkillResources::default(),
        };

        let md = format_skill_md(&skill, "body", &metadata);

        // Metadata and license should appear in the frontmatter.
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
            "metadata should contain rendered version"
        );
        assert!(
            md.contains("license: MIT"),
            "frontmatter should contain license"
        );

        // Must round-trip through the skill loader's parser.
        let parsed = swissarmyhammer_skills::skill_loader::parse_skill_md(
            &md,
            swissarmyhammer_skills::SkillSource::Builtin,
        )
        .expect("output should parse as valid SKILL.md");
        assert_eq!(parsed.name.as_str(), "meta-skill");
        assert_eq!(parsed.metadata.get("author").unwrap(), "swissarmyhammer");
        assert_eq!(parsed.metadata.get("version").unwrap(), "1.2.3");
        assert_eq!(parsed.license.as_deref(), Some("MIT"));
    }

    #[test]
    fn test_render_skill_expands_version_in_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), "swissarmyhammer".to_string());
        metadata.insert("version".to_string(), "{{version}}".to_string());

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

        // Template variable should be expanded in instructions.
        assert!(
            !instructions.contains("{{version}}"),
            "instructions should have {{{{version}}}} expanded"
        );
        assert!(
            instructions.contains(env!("CARGO_PKG_VERSION")),
            "instructions should contain the actual version"
        );

        // Template variable should be expanded in metadata values.
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

        // Non-template metadata should pass through unchanged.
        assert_eq!(
            rendered_metadata.get("author").unwrap(),
            "swissarmyhammer",
            "non-template metadata should be preserved"
        );
    }

    #[test]
    fn test_format_skill_md_omits_empty_metadata() {
        let skill = swissarmyhammer_skills::Skill {
            name: swissarmyhammer_skills::SkillName::new("no-meta").unwrap(),
            description: "skill without metadata".to_string(),
            license: None,
            compatibility: None,
            metadata: HashMap::new(),
            allowed_tools: vec![],
            instructions: "body".to_string(),
            source_path: None,
            source: swissarmyhammer_skills::SkillSource::Builtin,
            resources: swissarmyhammer_skills::SkillResources::default(),
        };

        let md = format_skill_md(&skill, "body", &skill.metadata);

        // Empty metadata should not produce a metadata block in the output.
        assert!(
            !md.contains("metadata:"),
            "empty metadata should be omitted from frontmatter"
        );
        assert!(
            !md.contains("license"),
            "None license should be omitted from frontmatter"
        );
    }
}
