//! Parses AGENT.md files from directories or embedded content

use crate::agent::{Agent, AgentName, AgentResources, AgentSource};
use crate::validation::validate_frontmatter;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// YAML frontmatter structure for AGENT.md
#[derive(Debug, Deserialize)]
struct AgentFrontmatter {
    name: Option<String>,
    description: Option<String>,
    model: Option<String>,
    #[serde(default)]
    tools: Option<String>,
    #[serde(default, rename = "allowed-tools")]
    allowed_tools_str: Option<String>,
    #[serde(default, rename = "disallowed-tools")]
    disallowed_tools_str: Option<String>,
    isolation: Option<String>,
    #[serde(default, rename = "max-turns")]
    max_turns: Option<u32>,
    #[serde(default)]
    background: bool,
    #[serde(default)]
    metadata: HashMap<String, String>,
}

/// Parse an AGENT.md file content into an Agent
pub fn parse_agent_md(content: &str, source: AgentSource) -> Result<Agent, String> {
    parse_agent_md_with_path(content, source, None)
}

/// Parse an AGENT.md file content into an Agent with optional source path
pub fn parse_agent_md_with_path(
    content: &str,
    source: AgentSource,
    source_path: Option<&Path>,
) -> Result<Agent, String> {
    let (frontmatter_str, body) = split_frontmatter(content)?;

    let fm: AgentFrontmatter = serde_yaml::from_str(&frontmatter_str)
        .map_err(|e| format!("failed to parse AGENT.md frontmatter: {}", e))?;

    validate_frontmatter(&fm.name, &fm.description).map_err(|errors| errors.join("; "))?;

    let name = AgentName::new(fm.name.as_deref().unwrap())?;

    // Parse tools from space-separated string (tools field or allowed-tools fallback)
    let tools_str = fm.tools.or(fm.allowed_tools_str);
    let tools = tools_str
        .map(|s| s.split_whitespace().map(String::from).collect())
        .unwrap_or_default();

    let disallowed_tools = fm
        .disallowed_tools_str
        .map(|s| s.split_whitespace().map(String::from).collect())
        .unwrap_or_default();

    Ok(Agent {
        name,
        description: fm.description.unwrap_or_default(),
        model: fm.model,
        tools,
        disallowed_tools,
        isolation: fm.isolation,
        max_turns: fm.max_turns,
        background: fm.background,
        metadata: fm.metadata,
        instructions: body.trim().to_string(),
        source_path: source_path.map(|p| p.to_path_buf()),
        source,
        resources: AgentResources::default(),
    })
}

/// Parse an agent from a directory on disk
pub fn load_agent_from_dir(dir: &Path, source: AgentSource) -> Result<Agent, String> {
    let agent_md_path = dir.join("AGENT.md");

    if !agent_md_path.exists() {
        return Err(format!("no AGENT.md found in {}", dir.display()));
    }

    let content = std::fs::read_to_string(&agent_md_path)
        .map_err(|e| format!("failed to read {}: {}", agent_md_path.display(), e))?;

    let mut agent = parse_agent_md_with_path(&content, source, Some(&agent_md_path))?;

    // Load additional resource files from the directory
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.file_name().map_or(false, |n| n != "AGENT.md") {
                if let Ok(file_content) = std::fs::read_to_string(&path) {
                    let filename = path.file_name().unwrap().to_string_lossy().to_string();
                    agent.resources.files.insert(filename, file_content);
                }
            }
        }
    }

    Ok(agent)
}

/// Parse an agent from embedded builtin content
pub fn load_agent_from_builtin(
    _agent_name: &str,
    files: &[(&str, &str)],
) -> Result<Agent, String> {
    let agent_md_content = files
        .iter()
        .find(|(name, _)| name.ends_with("/AGENT") || *name == "AGENT")
        .map(|(_, content)| *content)
        .ok_or_else(|| "no AGENT.md found in builtin files".to_string())?;

    let mut agent = parse_agent_md(agent_md_content, AgentSource::Builtin)?;

    // Add any additional resource files
    for (name, content) in files {
        if !name.ends_with("/AGENT") && *name != "AGENT" {
            let filename = name.rsplit('/').next().unwrap_or(name);
            agent
                .resources
                .files
                .insert(filename.to_string(), content.to_string());
        }
    }

    Ok(agent)
}

/// Split YAML frontmatter from markdown body
fn split_frontmatter(content: &str) -> Result<(String, String), String> {
    let content = content.trim();

    if !content.starts_with("---") {
        return Err("AGENT.md must start with YAML frontmatter (---)".to_string());
    }

    let after_first = &content[3..];
    let end_pos = after_first
        .find("\n---")
        .ok_or_else(|| "AGENT.md frontmatter not terminated (missing closing ---)".to_string())?;

    let frontmatter = after_first[..end_pos].trim().to_string();
    let body = after_first[end_pos + 4..].to_string();

    Ok((frontmatter, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_agent_md() {
        let content = r#"---
name: test
description: Test execution subagent
model: default
tools: "*"
max-turns: 25
---

# Test Agent

Run the test suite and report results.
"#;

        let agent = parse_agent_md(content, AgentSource::Builtin).unwrap();
        assert_eq!(agent.name.as_str(), "test");
        assert_eq!(agent.description, "Test execution subagent");
        assert_eq!(agent.model, Some("default".to_string()));
        assert_eq!(agent.tools, vec!["*"]);
        assert_eq!(agent.max_turns, Some(25));
        assert!(!agent.background);
        assert!(agent.instructions.contains("# Test Agent"));
    }

    #[test]
    fn test_parse_agent_md_with_disallowed_tools() {
        let content = r#"---
name: reviewer
description: Code review agent
disallowed-tools: "bash write"
---

Review code changes.
"#;

        let agent = parse_agent_md(content, AgentSource::Builtin).unwrap();
        assert_eq!(agent.disallowed_tools, vec!["bash", "write"]);
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
    fn test_allowed_tools_alias() {
        let content = r#"---
name: explore
description: Exploration agent
allowed-tools: "read glob grep"
---

Explore the codebase.
"#;

        let agent = parse_agent_md(content, AgentSource::Builtin).unwrap();
        assert_eq!(agent.tools, vec!["read", "glob", "grep"]);
    }
}
