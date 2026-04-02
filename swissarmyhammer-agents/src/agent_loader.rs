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

    let fm: AgentFrontmatter = serde_yaml_ng::from_str(&frontmatter_str)
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
            if path.is_file() && path.file_name().is_some_and(|n| n != "AGENT.md") {
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
pub fn load_agent_from_builtin(_agent_name: &str, files: &[(&str, &str)]) -> Result<Agent, String> {
    let agent_md_content = files
        .iter()
        .find(|(name, _)| name.ends_with("/AGENT.md") || *name == "AGENT.md")
        .map(|(_, content)| *content)
        .ok_or_else(|| "no AGENT.md found in builtin files".to_string())?;

    let mut agent = parse_agent_md(agent_md_content, AgentSource::Builtin)?;

    // Add any additional resource files
    for (name, content) in files {
        if !name.ends_with("/AGENT.md") && *name != "AGENT.md" {
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

    #[test]
    fn test_parse_agent_md_with_path() {
        let content = r#"---
name: my-agent
description: An agent with a path
---

Instructions here.
"#;
        let path = std::path::Path::new("/some/path/AGENT.md");
        let agent = parse_agent_md_with_path(content, AgentSource::Local, Some(path)).unwrap();
        assert_eq!(agent.name.as_str(), "my-agent");
        assert_eq!(agent.source_path, Some(path.to_path_buf()));
        assert_eq!(agent.source, AgentSource::Local);
    }

    #[test]
    fn test_parse_agent_md_no_path() {
        let content = r#"---
name: no-path-agent
description: Agent without path
---

Instructions.
"#;
        let agent = parse_agent_md(content, AgentSource::User).unwrap();
        assert_eq!(agent.source_path, None);
        assert_eq!(agent.source, AgentSource::User);
    }

    #[test]
    fn test_load_agent_from_dir_basic() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let content = r#"---
name: dir-agent
description: Agent loaded from directory
---

Instructions from directory.
"#;
        fs::write(temp_dir.path().join("AGENT.md"), content).unwrap();

        let agent = load_agent_from_dir(temp_dir.path(), AgentSource::Local).unwrap();
        assert_eq!(agent.name.as_str(), "dir-agent");
        assert_eq!(agent.source, AgentSource::Local);
        assert!(agent.source_path.is_some());
    }

    #[test]
    fn test_load_agent_from_dir_with_resources() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let content = r#"---
name: resource-agent
description: Agent with extra resources
---

Instructions.
"#;
        fs::write(temp_dir.path().join("AGENT.md"), content).unwrap();
        fs::write(temp_dir.path().join("extra.md"), "Extra resource content").unwrap();
        fs::write(temp_dir.path().join("config.toml"), "[settings]").unwrap();

        let agent = load_agent_from_dir(temp_dir.path(), AgentSource::Local).unwrap();
        assert!(
            agent.resources.files.contains_key("extra.md"),
            "should have extra.md resource"
        );
        assert!(
            agent.resources.files.contains_key("config.toml"),
            "should have config.toml resource"
        );
        assert!(
            !agent.resources.files.contains_key("AGENT.md"),
            "AGENT.md should not be in resources"
        );
    }

    #[test]
    fn test_load_agent_from_dir_missing_agent_md() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let result = load_agent_from_dir(temp_dir.path(), AgentSource::Local);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no AGENT.md"));
    }

    #[test]
    fn test_load_agent_from_builtin_basic() {
        let files = vec![
            (
                "myagent/AGENT.md",
                "---\nname: myagent\ndescription: Builtin test agent\n---\n\nInstructions.\n",
            ),
            ("myagent/helper.md", "Helper content"),
        ];

        let agent = load_agent_from_builtin("myagent", &files).unwrap();
        assert_eq!(agent.name.as_str(), "myagent");
        assert_eq!(agent.source, AgentSource::Builtin);
        assert!(agent.resources.files.contains_key("helper.md"));
    }

    #[test]
    fn test_load_agent_from_builtin_no_agent_md_fails() {
        let files = vec![("myagent/other.md", "some content")];
        let result = load_agent_from_builtin("myagent", &files);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no AGENT.md"));
    }

    #[test]
    fn test_load_agent_from_builtin_top_level_agent_md() {
        // Some builtins may have AGENT.md at the root (no subdir prefix)
        let files = vec![(
            "AGENT.md",
            "---\nname: rootagent\ndescription: Root-level agent\n---\n\nInstructions.\n",
        )];
        let agent = load_agent_from_builtin("rootagent", &files).unwrap();
        assert_eq!(agent.name.as_str(), "rootagent");
    }

    #[test]
    fn test_split_frontmatter_missing_close() {
        let content = "---\nname: test\n# no closing ---";
        assert!(split_frontmatter(content).is_err());
    }

    #[test]
    fn test_parse_agent_with_background_true() {
        let content = r#"---
name: bg-agent
description: Background agent
background: true
---

Runs in background.
"#;
        let agent = parse_agent_md(content, AgentSource::Builtin).unwrap();
        assert!(agent.background);
    }

    #[test]
    fn test_parse_agent_with_isolation() {
        let content = r#"---
name: isolated-agent
description: Isolated agent
isolation: worktree
---

Runs in isolation.
"#;
        let agent = parse_agent_md(content, AgentSource::Builtin).unwrap();
        assert_eq!(agent.isolation, Some("worktree".to_string()));
    }

    #[test]
    fn test_parse_agent_with_metadata() {
        let content = r#"---
name: meta-agent
description: Agent with metadata
metadata:
  version: "1.0"
  author: test
---

Instructions.
"#;
        let agent = parse_agent_md(content, AgentSource::Builtin).unwrap();
        assert_eq!(agent.metadata.get("version"), Some(&"1.0".to_string()));
        assert_eq!(agent.metadata.get("author"), Some(&"test".to_string()));
    }
}
