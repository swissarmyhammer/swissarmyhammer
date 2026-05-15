//! Skill deployment helpers shared across CLI tools.
//!
//! Provides the common pipeline for resolving a builtin skill, formatting a
//! SKILL.md file with YAML frontmatter, and deploying it to agent `.skills/`
//! directories via mirdan.
//!
//! Used by both `shelltool-cli` and `code-context-cli` to avoid duplicating
//! the resolve → format → deploy logic. Template rendering (which depends on
//! `swissarmyhammer-templating`) is left to each CLI's thin wrapper because
//! adding that crate here would create a dependency cycle.

use serde::Serialize;
use std::collections::HashMap;

use crate::{Skill, SkillResolver};

/// YAML frontmatter fields for a SKILL.md file.
///
/// Mirrors the fields expected by [`crate::skill_loader::parse_skill_md`]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    compatibility: Option<&'a str>,
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
pub fn resolve_skill(name: &str) -> Result<Skill, String> {
    let resolver = SkillResolver::new();
    let builtins = resolver.resolve_builtins();
    builtins
        .get(name)
        .cloned()
        .ok_or_else(|| format!("builtin '{name}' skill not found"))
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
/// escaped. The output is compatible with [`crate::skill_loader::parse_skill_md`].
///
/// # Panics
///
/// Panics if `serde_yaml_ng` fails to serialize the frontmatter struct, which
/// should be unreachable since all fields are plain strings.
pub fn format_skill_md(
    skill: &Skill,
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
        compatibility: skill.compatibility.as_deref(),
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
pub fn validate_skill_name(name: &str) -> Result<(), String> {
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
pub fn write_and_deploy(name: &str, skill_content: &str) -> Result<Vec<String>, String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SkillName, SkillResources, SkillSource};

    #[test]
    fn test_resolve_skill_nonexistent_returns_error() {
        let result = resolve_skill("nonexistent-skill-that-does-not-exist");
        assert!(result.is_err(), "nonexistent skill should return Err");
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_validate_skill_name_valid() {
        assert!(validate_skill_name("shell").is_ok());
        assert!(validate_skill_name("code-context").is_ok());
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
        let skill = Skill {
            name: SkillName::new("test-skill").unwrap(),
            description: "description with: colons, \"quotes\", and {braces}".to_string(),
            license: None,
            compatibility: None,
            metadata: HashMap::new(),
            allowed_tools: vec!["tool-a".to_string(), "tool-b".to_string()],
            instructions: "body".to_string(),
            source_path: None,
            source: SkillSource::Builtin,
            resources: SkillResources::default(),
        };

        let md = format_skill_md(&skill, "# Instructions\n\nDo the thing.", &skill.metadata);

        assert!(md.starts_with("---\n"), "should start with frontmatter");
        assert!(
            md.contains("\n---\n"),
            "should have closing frontmatter delimiter"
        );

        let parsed = crate::skill_loader::parse_skill_md(&md, SkillSource::Builtin)
            .expect("format_skill_md output should be parseable as valid SKILL.md");

        assert_eq!(parsed.name.as_str(), "test-skill");
        assert_eq!(parsed.description, skill.description);
        assert_eq!(parsed.allowed_tools, vec!["tool-a", "tool-b"]);
        assert_eq!(parsed.instructions, "# Instructions\n\nDo the thing.");
    }

    #[test]
    fn test_format_skill_md_omits_empty_allowed_tools() {
        let skill = Skill {
            name: SkillName::new("minimal").unwrap(),
            description: "a minimal skill".to_string(),
            license: None,
            compatibility: None,
            metadata: HashMap::new(),
            allowed_tools: vec![],
            instructions: "body".to_string(),
            source_path: None,
            source: SkillSource::Builtin,
            resources: SkillResources::default(),
        };

        let md = format_skill_md(&skill, "body", &skill.metadata);

        assert!(
            !md.contains("allowed-tools"),
            "empty allowed_tools should be omitted from frontmatter"
        );

        let parsed = crate::skill_loader::parse_skill_md(&md, SkillSource::Builtin)
            .expect("output should parse as valid SKILL.md");
        assert_eq!(parsed.name.as_str(), "minimal");
        assert!(parsed.allowed_tools.is_empty());
    }

    #[test]
    fn test_format_skill_md_preserves_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), "swissarmyhammer".to_string());
        metadata.insert("version".to_string(), "1.2.3".to_string());

        let skill = Skill {
            name: SkillName::new("meta-skill").unwrap(),
            description: "a skill with metadata".to_string(),
            license: Some("MIT".to_string()),
            compatibility: None,
            metadata: metadata.clone(),
            allowed_tools: vec![],
            instructions: "body".to_string(),
            source_path: None,
            source: SkillSource::Builtin,
            resources: SkillResources::default(),
        };

        let md = format_skill_md(&skill, "body", &metadata);

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

        let parsed = crate::skill_loader::parse_skill_md(&md, SkillSource::Builtin)
            .expect("output should parse as valid SKILL.md");
        assert_eq!(parsed.name.as_str(), "meta-skill");
        assert_eq!(parsed.metadata.get("author").unwrap(), "swissarmyhammer");
        assert_eq!(parsed.metadata.get("version").unwrap(), "1.2.3");
        assert_eq!(parsed.license.as_deref(), Some("MIT"));
    }

    #[test]
    fn test_format_skill_md_omits_empty_metadata() {
        let skill = Skill {
            name: SkillName::new("no-meta").unwrap(),
            description: "skill without metadata".to_string(),
            license: None,
            compatibility: None,
            metadata: HashMap::new(),
            allowed_tools: vec![],
            instructions: "body".to_string(),
            source_path: None,
            source: SkillSource::Builtin,
            resources: SkillResources::default(),
        };

        let md = format_skill_md(&skill, "body", &skill.metadata);

        assert!(
            !md.contains("metadata:"),
            "empty metadata should be omitted from frontmatter"
        );
        assert!(
            !md.contains("license"),
            "None license should be omitted from frontmatter"
        );
        assert!(
            !md.contains("compatibility"),
            "None compatibility should be omitted from frontmatter"
        );
    }

    /// Regression: `compatibility` round-trips through `format_skill_md` and
    /// `parse_skill_md` so the generated `.skills/` copy matches the builtin
    /// source instead of silently dropping tool-prerequisite metadata.
    #[test]
    fn test_format_skill_md_round_trips_compatibility() {
        let compatibility =
            "Requires the `code_context` MCP tool for symbol lookup and blast-radius analysis.";
        let skill = Skill {
            name: SkillName::new("compat-skill").unwrap(),
            description: "a skill that declares its tool prerequisites".to_string(),
            license: Some("MIT OR Apache-2.0".to_string()),
            compatibility: Some(compatibility.to_string()),
            metadata: HashMap::new(),
            allowed_tools: vec![],
            instructions: "body".to_string(),
            source_path: None,
            source: SkillSource::Builtin,
            resources: SkillResources::default(),
        };

        let md = format_skill_md(&skill, "body", &skill.metadata);

        assert!(
            md.contains("compatibility:"),
            "frontmatter should contain compatibility field, got:\n{md}"
        );

        let parsed = crate::skill_loader::parse_skill_md(&md, SkillSource::Builtin)
            .expect("output should parse as valid SKILL.md");
        assert_eq!(parsed.compatibility.as_deref(), Some(compatibility));
    }
}
