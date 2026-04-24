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
    let fm: SkillFrontmatter = serde_yaml_ng::from_str(&frontmatter_str)
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

/// Parse a skill from a directory on disk.
///
/// Reads the skill's `SKILL.md` from `dir` and walks the directory recursively
/// to collect any additional resource files (e.g. `references/RUST_REVIEW.md`).
/// Resource keys in [`Skill::resources`] are stored as forward-slash-separated
/// paths relative to the skill root, preserving subdirectory structure so the
/// deploy layer can recreate it under each agent's `.skills/` directory.
pub fn load_skill_from_dir(dir: &Path, source: SkillSource) -> Result<Skill, String> {
    let skill_md_path = dir.join("SKILL.md");

    if !skill_md_path.exists() {
        return Err(format!("no SKILL.md found in {}", dir.display()));
    }

    let content = std::fs::read_to_string(&skill_md_path)
        .map_err(|e| format!("failed to read {}: {}", skill_md_path.display(), e))?;

    let mut skill = parse_skill_md_with_path(&content, source, Some(&skill_md_path))?;

    // Walk the skill directory recursively so files under subdirectories
    // (e.g. `references/`) are picked up. Resource keys are stored as paths
    // relative to `dir`, normalized to forward slashes, so that deployment
    // can recreate the subdirectory layout on disk.
    for entry in walkdir::WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .flatten()
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        // Skip the top-level SKILL.md — it is already parsed above.
        if path == skill_md_path {
            continue;
        }

        let rel_path = match path.strip_prefix(dir) {
            Ok(p) => p,
            Err(_) => continue,
        };

        // Normalize to forward slashes so keys match the builtin-path format
        // and remain platform-independent.
        let key = rel_path
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");

        if let Ok(file_content) = std::fs::read_to_string(path) {
            skill.resources.files.insert(key, file_content);
        }
    }

    Ok(skill)
}

/// Parse a skill from embedded builtin content.
///
/// `skill_name` is the leading directory segment used by the build-time
/// generator to group files under a skill (e.g. `review`). `files` is the
/// list of `(name, content)` pairs for that skill, where each `name` is a
/// forward-slash path rooted at the builtin `skills/` directory
/// (e.g. `review/SKILL.md`, `review/references/RUST_REVIEW.md`).
///
/// Resource keys in [`Skill::resources`] are stored as paths relative to the
/// skill root — only the leading `<skill_name>/` segment is stripped, so
/// subdirectory structure (e.g. `references/RUST_REVIEW.md`) is preserved for
/// the deploy layer.
pub fn load_skill_from_builtin(skill_name: &str, files: &[(&str, &str)]) -> Result<Skill, String> {
    // Find the SKILL.md content — names include the .md extension
    let skill_md_content = files
        .iter()
        .find(|(name, _)| name.ends_with("/SKILL.md") || *name == "SKILL.md")
        .map(|(_, content)| *content)
        .ok_or_else(|| "no SKILL.md found in builtin files".to_string())?;

    let mut skill = parse_skill_md(skill_md_content, SkillSource::Builtin)?;

    // Strip only the leading skill-name segment so the remaining relative path
    // (e.g. `references/RUST_REVIEW.md`) becomes the resource key. This
    // preserves subdirectory structure for the deploy layer, matching what
    // `load_skill_from_dir` produces when walking the disk.
    let prefix = format!("{skill_name}/");
    for (name, content) in files {
        if name.ends_with("/SKILL.md") || *name == "SKILL.md" {
            continue;
        }

        let key = name.strip_prefix(&prefix).unwrap_or(name);
        skill
            .resources
            .files
            .insert(key.to_string(), content.to_string());
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
    fn test_load_skill_from_dir_missing_skill_md() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        // Directory exists but has no SKILL.md
        let result = load_skill_from_dir(temp_dir.path(), SkillSource::Local);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("no SKILL.md found"),
            "expected 'no SKILL.md found' error, got: {}",
            err
        );
    }

    #[test]
    fn test_load_skill_from_dir_with_resources() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let skill_dir = temp_dir.path().join("my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();

        // Write a valid SKILL.md
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: my-skill\ndescription: A test skill\n---\nInstructions here.",
        )
        .unwrap();

        // Write an extra resource file
        std::fs::write(skill_dir.join("helper.md"), "Extra resource content").unwrap();

        let skill = load_skill_from_dir(&skill_dir, SkillSource::Local).unwrap();
        assert_eq!(skill.name.as_str(), "my-skill");
        assert!(
            skill.resources.files.contains_key("helper.md"),
            "should have loaded helper.md resource"
        );
        assert_eq!(
            skill.resources.files.get("helper.md").unwrap(),
            "Extra resource content"
        );
    }

    #[test]
    fn test_load_skill_from_dir_source_path() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let skill_dir = temp_dir.path().join("path-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();

        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: path-skill\ndescription: test\n---\nBody.",
        )
        .unwrap();

        let skill = load_skill_from_dir(&skill_dir, SkillSource::User).unwrap();
        assert_eq!(skill.source, SkillSource::User);
        assert!(
            skill.source_path.is_some(),
            "should have source_path set for disk-loaded skill"
        );
        assert!(skill
            .source_path
            .unwrap()
            .to_string_lossy()
            .contains("SKILL.md"));
    }

    #[test]
    fn test_split_frontmatter_unterminated() {
        let content = "---\nname: test\nNo closing delimiter";
        let result = split_frontmatter(content);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("not terminated"));
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

    /// Regression: disk-loaded skills preserve subdirectory structure for
    /// progressive-disclosure resources (e.g. `references/RUST_REVIEW.md`)
    /// instead of flattening them to bare filenames.
    #[test]
    fn test_load_skill_from_dir_preserves_subdirectories() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let skill_dir = temp_dir.path().join("my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();

        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: my-skill\ndescription: A test skill\n---\nBody.",
        )
        .unwrap();

        let references_dir = skill_dir.join("references");
        std::fs::create_dir_all(&references_dir).unwrap();
        std::fs::write(references_dir.join("helper.md"), "Helper content").unwrap();

        let skill = load_skill_from_dir(&skill_dir, SkillSource::Local).unwrap();

        assert!(
            skill.resources.files.contains_key("references/helper.md"),
            "resource key should retain the `references/` prefix, got keys: {:?}",
            skill.resources.files.keys().collect::<Vec<_>>()
        );
        assert_eq!(
            skill.resources.files.get("references/helper.md").unwrap(),
            "Helper content"
        );
        assert!(
            !skill.resources.files.contains_key("helper.md"),
            "resource should not be stored under a flattened `helper.md` key"
        );
    }

    /// Regression: builtin skills preserve subdirectory structure when the
    /// build-time generator emits keys like `my-skill/references/helper.md`.
    /// Only the leading skill-name segment is stripped; the remainder becomes
    /// the resource key.
    #[test]
    fn test_load_skill_from_builtin_preserves_subdirectories() {
        let files: Vec<(&str, &str)> = vec![
            (
                "my-skill/SKILL.md",
                "---\nname: my-skill\ndescription: A test skill\n---\nBody.",
            ),
            ("my-skill/references/helper.md", "Helper content"),
        ];

        let skill = load_skill_from_builtin("my-skill", &files).unwrap();

        assert!(
            skill.resources.files.contains_key("references/helper.md"),
            "resource key should retain the `references/` prefix, got keys: {:?}",
            skill.resources.files.keys().collect::<Vec<_>>()
        );
        assert_eq!(
            skill.resources.files.get("references/helper.md").unwrap(),
            "Helper content"
        );
        assert!(
            !skill.resources.files.contains_key("helper.md"),
            "resource should not be stored under a flattened `helper.md` key"
        );
    }
}
