//! Mode definition and management
//!
//! This module provides the core Mode type representing an agent operating mode
//! with its system prompt.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use swissarmyhammer_common::SwissArmyHammerError;

use crate::frontmatter::parse_frontmatter;
use crate::Result;

/// Represents an agent operating mode with system prompt
///
/// A [`Mode`] encapsulates the configuration for a specific agent type,
/// including its identifier, human-readable information, and system prompt.
///
/// # Mode File Format
///
/// Modes can either embed a system prompt directly:
///
/// ```markdown
/// ---
/// name: general-purpose
/// description: General-purpose agent for researching complex questions
/// ---
///
/// You are a general-purpose AI agent capable of researching complex
/// questions, searching for code, and executing multi-step tasks.
/// ```
///
/// Or reference a prompt from the prompts system:
///
/// ```markdown
/// ---
/// name: planner
/// description: Architecture and planning specialist
/// prompt: .system/planner
/// ---
/// ```
///
/// When `prompt` is set, the referenced prompt should be loaded and rendered
/// by the caller to get the system prompt content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mode {
    /// Unique identifier for the mode (e.g., "general-purpose", "Explore")
    id: String,

    /// Human-readable name for the mode
    name: String,

    /// Description of when this mode should be used
    description: String,

    /// Embedded system prompt for this mode (used if `prompt` is None)
    system_prompt: String,

    /// Reference to a prompt path to use for system prompt (e.g., ".system/planner")
    /// When set, the caller should load and render this prompt instead of using `system_prompt`
    prompt: Option<String>,

    /// Reference to an agent name to use for system prompt (e.g., "default", "planner")
    /// When set, the caller should look up the agent and use its instructions as the system prompt.
    /// Takes precedence over `prompt` if both are set.
    agent: Option<String>,

    /// Path to the source file (if loaded from file)
    #[serde(skip)]
    source_path: Option<PathBuf>,
}

impl Mode {
    /// Create a new mode with embedded system prompt
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        system_prompt: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            system_prompt: system_prompt.into(),
            prompt: None,
            agent: None,
            source_path: None,
        }
    }

    /// Create a mode that references a prompt for its system prompt
    pub fn with_prompt(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        prompt_path: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            system_prompt: String::new(),
            prompt: Some(prompt_path.into()),
            agent: None,
            source_path: None,
        }
    }

    /// Create a mode that references an agent for its system prompt
    pub fn with_agent(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        agent_name: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            system_prompt: String::new(),
            prompt: None,
            agent: Some(agent_name.into()),
            source_path: None,
        }
    }

    /// Create a mode from a markdown file with frontmatter
    ///
    /// # Format
    ///
    /// With embedded system prompt:
    /// ```markdown
    /// ---
    /// name: mode-name
    /// description: Mode description
    /// ---
    /// System prompt content
    /// ```
    ///
    /// Or with prompt reference:
    /// ```markdown
    /// ---
    /// name: mode-name
    /// description: Mode description
    /// prompt: .system/mode-name
    /// ---
    /// ```
    pub fn from_markdown(content: &str, file_id: impl Into<String>) -> Result<Self> {
        let parsed = parse_frontmatter(content)?;

        let metadata = parsed.metadata.ok_or_else(|| SwissArmyHammerError::Other {
            message: "Mode file must have frontmatter with name and description".to_string(),
        })?;

        let name = metadata
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SwissArmyHammerError::Other {
                message: "Mode frontmatter must have 'name' field".to_string(),
            })?
            .to_string();

        let description = metadata
            .get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SwissArmyHammerError::Other {
                message: "Mode frontmatter must have 'description' field".to_string(),
            })?
            .to_string();

        // Check for prompt reference in frontmatter
        let prompt = metadata
            .get("prompt")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Check for agent reference in frontmatter
        let agent = metadata
            .get("agent")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let system_prompt = parsed.content.trim().to_string();

        // Must have either a prompt reference, agent reference, or embedded content
        if prompt.is_none() && agent.is_none() && system_prompt.is_empty() {
            return Err(SwissArmyHammerError::Other {
                message: "Mode must have a 'prompt' field, 'agent' field, or system prompt content"
                    .to_string(),
            });
        }

        Ok(Self {
            id: file_id.into(),
            name,
            description,
            system_prompt,
            prompt,
            agent,
            source_path: None,
        })
    }

    /// Create a mode from a file path
    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| SwissArmyHammerError::Other {
            message: format!("Failed to read mode file: {}", e),
        })?;

        // Extract file ID from filename (without extension)
        let file_id = path.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
            SwissArmyHammerError::Other {
                message: "Invalid mode filename".to_string(),
            }
        })?;

        let mut mode = Self::from_markdown(&content, file_id)?;
        mode.source_path = Some(path.to_path_buf());
        Ok(mode)
    }

    /// Get the mode ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the mode name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the mode description
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Get the embedded system prompt
    ///
    /// Note: This may be empty if the mode uses a prompt reference instead.
    /// Check `prompt()` first to see if a prompt path should be loaded.
    pub fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    /// Get the prompt reference path (e.g., ".system/planner")
    ///
    /// When this is Some, the caller should load and render the referenced
    /// prompt to get the system prompt content instead of using `system_prompt()`.
    pub fn prompt(&self) -> Option<&str> {
        self.prompt.as_deref()
    }

    /// Get the agent reference name (e.g., "default", "planner")
    ///
    /// When this is Some, the caller should look up the agent and use its
    /// instructions as the system prompt. Takes precedence over `prompt()`.
    pub fn agent(&self) -> Option<&str> {
        self.agent.as_deref()
    }

    /// Check if this mode uses a prompt reference
    pub fn uses_prompt_reference(&self) -> bool {
        self.prompt.is_some()
    }

    /// Check if this mode uses an agent reference
    pub fn uses_agent_reference(&self) -> bool {
        self.agent.is_some()
    }

    /// Get the source file path
    pub fn source_path(&self) -> Option<&PathBuf> {
        self.source_path.as_ref()
    }

    /// Set the source file path
    pub fn with_source_path(mut self, path: PathBuf) -> Self {
        self.source_path = Some(path);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_new() {
        let mode = Mode::new("test", "Test Mode", "A test mode", "You are a test agent.");

        assert_eq!(mode.id(), "test");
        assert_eq!(mode.name(), "Test Mode");
        assert_eq!(mode.description(), "A test mode");
        assert_eq!(mode.system_prompt(), "You are a test agent.");
        assert!(mode.source_path().is_none());
    }

    #[test]
    fn test_mode_from_markdown() {
        let content = r#"---
name: example-mode
description: An example mode for testing
---
You are an example agent designed for testing.

Your role is to demonstrate mode functionality.
"#;

        let mode = Mode::from_markdown(content, "example").unwrap();
        assert_eq!(mode.id(), "example");
        assert_eq!(mode.name(), "example-mode");
        assert_eq!(mode.description(), "An example mode for testing");
        assert!(mode.system_prompt().contains("You are an example agent"));
    }

    #[test]
    fn test_mode_from_markdown_missing_name() {
        let content = r#"---
description: Missing name field
---
System prompt
"#;

        let result = Mode::from_markdown(content, "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_mode_from_markdown_missing_description() {
        let content = r#"---
name: test-mode
---
System prompt
"#;

        let result = Mode::from_markdown(content, "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_mode_from_markdown_empty_system_prompt() {
        let content = r#"---
name: test-mode
description: A test mode
---

"#;

        let result = Mode::from_markdown(content, "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_mode_from_markdown_no_frontmatter() {
        let content = "Just a system prompt without frontmatter";

        let result = Mode::from_markdown(content, "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_mode_from_markdown_with_agent_ref() {
        let content = "---\nname: Test\ndescription: Test mode\nagent: planner\n---\n";
        let mode = Mode::from_markdown(content, "test").unwrap();
        assert!(mode.uses_agent_reference());
        assert_eq!(mode.agent(), Some("planner"));
        assert!(mode.system_prompt().is_empty());
    }

    #[test]
    fn test_mode_with_source_path() {
        let mode = Mode::new("test", "Test", "Description", "Prompt")
            .with_source_path(PathBuf::from("/path/to/mode.md"));

        assert!(mode.source_path().is_some());
        assert_eq!(
            mode.source_path().unwrap(),
            &PathBuf::from("/path/to/mode.md")
        );
    }

    #[test]
    fn test_mode_with_prompt_constructor() {
        let mode = Mode::with_prompt(
            "ref-mode",
            "Reference Mode",
            "Uses a prompt reference",
            ".system/planner",
        );

        assert_eq!(mode.id(), "ref-mode");
        assert_eq!(mode.name(), "Reference Mode");
        assert_eq!(mode.description(), "Uses a prompt reference");
        assert_eq!(mode.system_prompt(), "");
        assert_eq!(mode.prompt(), Some(".system/planner"));
        assert!(mode.agent().is_none());
        assert!(mode.uses_prompt_reference());
        assert!(!mode.uses_agent_reference());
        assert!(mode.source_path().is_none());
    }

    #[test]
    fn test_mode_with_agent_constructor() {
        let mode = Mode::with_agent(
            "agent-mode",
            "Agent Mode",
            "Uses an agent reference",
            "planner",
        );

        assert_eq!(mode.id(), "agent-mode");
        assert_eq!(mode.name(), "Agent Mode");
        assert_eq!(mode.description(), "Uses an agent reference");
        assert_eq!(mode.system_prompt(), "");
        assert!(mode.prompt().is_none());
        assert_eq!(mode.agent(), Some("planner"));
        assert!(!mode.uses_prompt_reference());
        assert!(mode.uses_agent_reference());
        assert!(mode.source_path().is_none());
    }

    #[test]
    fn test_mode_new_accessors() {
        let mode = Mode::new("id", "Name", "Desc", "Prompt");

        assert!(mode.prompt().is_none());
        assert!(mode.agent().is_none());
        assert!(!mode.uses_prompt_reference());
        assert!(!mode.uses_agent_reference());
    }

    #[test]
    fn test_mode_from_markdown_with_prompt_ref() {
        let content =
            "---\nname: Planner\ndescription: Planning mode\nprompt: .system/planner\n---\n";
        let mode = Mode::from_markdown(content, "planner").unwrap();

        assert_eq!(mode.id(), "planner");
        assert_eq!(mode.name(), "Planner");
        assert_eq!(mode.description(), "Planning mode");
        assert_eq!(mode.prompt(), Some(".system/planner"));
        assert!(mode.uses_prompt_reference());
        assert!(mode.system_prompt().is_empty());
    }

    #[test]
    fn test_mode_from_markdown_with_both_prompt_and_agent() {
        let content =
            "---\nname: Both\ndescription: Has both\nprompt: .system/foo\nagent: bar\n---\n";
        let mode = Mode::from_markdown(content, "both").unwrap();

        assert_eq!(mode.prompt(), Some(".system/foo"));
        assert_eq!(mode.agent(), Some("bar"));
        assert!(mode.uses_prompt_reference());
        assert!(mode.uses_agent_reference());
    }

    #[test]
    fn test_mode_from_file_success() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test-mode.md");
        std::fs::write(
            &file_path,
            "---\nname: File Mode\ndescription: Loaded from file\n---\nFile system prompt content.\n",
        )
        .unwrap();

        let mode = Mode::from_file(&file_path).unwrap();
        assert_eq!(mode.id(), "test-mode");
        assert_eq!(mode.name(), "File Mode");
        assert_eq!(mode.description(), "Loaded from file");
        assert!(mode.system_prompt().contains("File system prompt content"));
        assert!(mode.source_path().is_some());
        assert_eq!(mode.source_path().unwrap(), &file_path);
    }

    #[test]
    fn test_mode_from_file_not_found() {
        let result = Mode::from_file(std::path::Path::new("/nonexistent/path/mode.md"));
        assert!(result.is_err());
    }

    #[test]
    fn test_mode_from_file_invalid_content() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("bad-mode.md");
        std::fs::write(&file_path, "no frontmatter at all").unwrap();

        let result = Mode::from_file(&file_path);
        // No frontmatter means metadata is None, so this should error
        assert!(result.is_err());
    }

    #[test]
    fn test_mode_from_file_invalid_filename() {
        // Create a file with no stem (just an extension won't work on most OS,
        // but a path ending in ".." or similar edge case)
        // On Unix, a file named just ".md" has stem "" which is still valid to_str
        // We'll test with a directory path which has no file_stem
        let result = Mode::from_file(std::path::Path::new("/some/directory/"));
        assert!(result.is_err());
    }

    #[test]
    fn test_mode_from_markdown_with_prompt_and_content() {
        // Both prompt reference and embedded content are present
        let content =
            "---\nname: Hybrid\ndescription: Has prompt ref and content\nprompt: .system/foo\n---\nSome fallback content\n";
        let mode = Mode::from_markdown(content, "hybrid").unwrap();

        assert_eq!(mode.prompt(), Some(".system/foo"));
        assert!(!mode.system_prompt().is_empty());
        assert!(mode.system_prompt().contains("fallback"));
    }
}
