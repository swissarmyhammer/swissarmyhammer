//! Parses SKILL.md files from directories or embedded content

use crate::skill::{Skill, SkillName, SkillResources, SkillSource};
use crate::validation::validate_frontmatter;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// YAML frontmatter structure for SKILL.md
#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    name: Option<String>,
    description: Option<String>,
    license: Option<String>,
    compatibility: Option<String>,
    #[serde(default)]
    metadata: HashMap<String, String>,
    #[serde(default, rename = "allowed-tools")]
    allowed_tools_str: Option<String>,
}

/// Parse a SKILL.md file content into a Skill
pub fn parse_skill_md(content: &str, source: SkillSource) -> Result<Skill, String> {
    parse_skill_md_with_path(content, source, None)
}

/// Parse a SKILL.md file content into a Skill with optional source path
pub fn parse_skill_md_with_path(
    content: &str,
    source: SkillSource,
    source_path: Option<&Path>,
) -> Result<Skill, String> {
    // Split frontmatter from body
    let (frontmatter_str, body) = split_frontmatter(content)?;

    // Parse YAML frontmatter
    let fm: SkillFrontmatter = serde_yaml::from_str(&frontmatter_str)
        .map_err(|e| format!("failed to parse SKILL.md frontmatter: {}", e))?;

    // Validate required fields
    validate_frontmatter(&fm.name, &fm.description).map_err(|errors| errors.join("; "))?;

    let name = SkillName::new(fm.name.as_deref().unwrap())?;

    // Parse allowed-tools from space-separated string
    let allowed_tools = fm
        .allowed_tools_str
        .map(|s| s.split_whitespace().map(String::from).collect())
        .unwrap_or_default();

    Ok(Skill {
        name,
        description: fm.description.unwrap_or_default(),
        license: fm.license,
        compatibility: fm.compatibility,
        metadata: fm.metadata,
        allowed_tools,
        instructions: body.trim().to_string(),
        source_path: source_path.map(|p| p.to_path_buf()),
        source,
        resources: SkillResources::default(),
    })
}

/// Parse a skill from a directory on disk
pub fn load_skill_from_dir(dir: &Path, source: SkillSource) -> Result<Skill, String> {
    let skill_md_path = dir.join("SKILL.md");

    if !skill_md_path.exists() {
        return Err(format!("no SKILL.md found in {}", dir.display()));
    }

    let content = std::fs::read_to_string(&skill_md_path)
        .map_err(|e| format!("failed to read {}: {}", skill_md_path.display(), e))?;

    let mut skill = parse_skill_md_with_path(&content, source, Some(&skill_md_path))?;

    // Load additional resource files from the directory
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.file_name().is_some_and(|n| n != "SKILL.md") {
                if let Ok(file_content) = std::fs::read_to_string(&path) {
                    let filename = path.file_name().unwrap().to_string_lossy().to_string();
                    skill.resources.files.insert(filename, file_content);
                }
            }
        }
    }

    Ok(skill)
}

/// Parse a skill from embedded builtin content (name, list of (filename, content) pairs)
pub fn load_skill_from_builtin(_skill_name: &str, files: &[(&str, &str)]) -> Result<Skill, String> {
    // Find the SKILL.md content — names include the .md extension
    let skill_md_content = files
        .iter()
        .find(|(name, _)| name.ends_with("/SKILL.md") || *name == "SKILL.md")
        .map(|(_, content)| *content)
        .ok_or_else(|| "no SKILL.md found in builtin files".to_string())?;

    let mut skill = parse_skill_md(skill_md_content, SkillSource::Builtin)?;

    // Add any additional resource files
    for (name, content) in files {
        if !name.ends_with("/SKILL.md") && *name != "SKILL.md" {
            let filename = name.rsplit('/').next().unwrap_or(name);
            skill
                .resources
                .files
                .insert(filename.to_string(), content.to_string());
        }
    }

    Ok(skill)
}

/// Split YAML frontmatter from markdown body
fn split_frontmatter(content: &str) -> Result<(String, String), String> {
    let content = content.trim();

    if !content.starts_with("---") {
        return Err("SKILL.md must start with YAML frontmatter (---)".to_string());
    }

    let after_first = &content[3..];
    let end_pos = after_first
        .find("\n---")
        .ok_or_else(|| "SKILL.md frontmatter not terminated (missing closing ---)".to_string())?;

    let frontmatter = after_first[..end_pos].trim().to_string();
    let body = after_first[end_pos + 4..].to_string();

    Ok((frontmatter, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_skill_md() {
        let content = r#"---
name: plan
description: Turn specs into plans
allowed-tools: mcp__sah__flow mcp__sah__files
metadata:
  author: swissarmyhammer
  version: "1.0"
---

# Plan

Create a plan from a spec file.

## How to Execute

Use the flow tool.
"#;

        let skill = parse_skill_md(content, SkillSource::Builtin).unwrap();
        assert_eq!(skill.name.as_str(), "plan");
        assert_eq!(skill.description, "Turn specs into plans");
        assert_eq!(skill.allowed_tools.len(), 2);
        assert_eq!(skill.allowed_tools[0], "mcp__sah__flow");
        assert!(skill.instructions.contains("# Plan"));
        assert_eq!(skill.metadata.get("author").unwrap(), "swissarmyhammer");
    }

    #[test]
    fn test_split_frontmatter() {
        let content = "---\nname: test\n---\n\n# Body";
        let (fm, body) = split_frontmatter(content).unwrap();
        assert_eq!(fm, "name: test");
        assert!(body.contains("# Body"));
    }

    #[test]
    fn test_missing_frontmatter() {
        let content = "# No frontmatter";
        assert!(split_frontmatter(content).is_err());
    }

    #[test]
    fn test_parse_skill_md_with_star_allowed_tools() {
        let content = r#"---
name: plan
description: Turn specs into plans
allowed-tools: "*"
metadata:
  author: swissarmyhammer
  version: "1.0"
---

# Plan

Create a plan from a spec file.
"#;

        let skill = parse_skill_md(content, SkillSource::Builtin).unwrap();
        assert_eq!(skill.name.as_str(), "plan");
        assert_eq!(skill.allowed_tools.len(), 1);
        assert_eq!(skill.allowed_tools[0], "*");
    }
}
