//! Tool enable/disable configuration with YAML persistence.
//!
//! Supports two layers of configuration:
//! 1. **Global** — `~/.sah/tools.yaml`
//! 2. **Project** — `.sah/tools.yaml` (at git root)
//!
//! Project overrides global.  Unlisted tools default to enabled.
//!
//! # Format
//!
//! ```yaml
//! # tools.yaml — tool enable/disable configuration
//! # Unlisted tools default to enabled.
//! shell:
//!   enabled: true
//! kanban:
//!   enabled: false
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::warn;

/// Canonical list of all known MCP tool names, sorted alphabetically.
///
/// This is the single source of truth for valid tool names used both in the
/// CLI (`sah tools` commands) and in validation logic.  Add a new name here
/// whenever a new top-level tool is registered with the [`ToolRegistry`].
pub const KNOWN_TOOL_NAMES: &[&str] = &[
    "agent",
    "code_context",
    "files",
    "git",
    "kanban",
    "question",
    "ralph",
    "shell",
    "skill",
    "web",
];

/// Per-tool configuration entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolEntry {
    /// Whether the tool is enabled (`true`) or disabled (`false`).
    enabled: bool,
}

impl ToolEntry {
    /// Create a new `ToolEntry` with the given enabled state.
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Return `true` if this tool is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// Tool configuration — a map of tool names to their settings.
///
/// Serializes/deserializes as a flat YAML mapping:
/// ```yaml
/// shell:
///   enabled: true
/// kanban:
///   enabled: false
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ToolConfig {
    #[serde(flatten)]
    entries: HashMap<String, ToolEntry>,
}

impl ToolConfig {
    /// Return a reference to the inner map of tool name → entry.
    pub fn entries(&self) -> &HashMap<String, ToolEntry> {
        &self.entries
    }

    /// Return a mutable reference to the inner map.
    pub fn entries_mut(&mut self) -> &mut HashMap<String, ToolEntry> {
        &mut self.entries
    }

    /// Merge another config on top of this one.
    ///
    /// Entries in `other` override entries with the same key in `self`.
    /// This implements the "later layer wins" rule used throughout the config stack.
    pub fn merge(&mut self, other: ToolConfig) {
        for (name, entry) in other.entries {
            self.entries.insert(name, entry);
        }
    }

    /// Return the names of all tools that are explicitly disabled.
    ///
    /// Tools not present in the config are considered enabled by default and
    /// will not appear in this list.
    pub fn disabled_tools(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|(_, entry)| !entry.enabled)
            .map(|(name, _)| name.clone())
            .collect()
    }
}

const TOOLS_CONFIG_FILENAME: &str = "tools.yaml";

/// Load tool config from a single YAML file.
///
/// Returns `None` if the file does not exist or cannot be read, and logs a
/// warning if the file exists but cannot be parsed.
pub fn load_tool_config_from_path(path: &Path) -> Option<ToolConfig> {
    match std::fs::read_to_string(path) {
        Ok(content) => match serde_yaml_ng::from_str(&content) {
            Ok(config) => Some(config),
            Err(e) => {
                warn!("Failed to parse {}: {}", path.display(), e);
                None
            }
        },
        Err(_) => None, // File not found is not an error
    }
}

/// Save tool config to a YAML file.
///
/// Creates parent directories as needed.
///
/// # Errors
///
/// Returns an `io::Error` if directory creation, YAML serialization, or file
/// writing fails.
pub fn save_tool_config(config: &ToolConfig, path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let yaml = serde_yaml_ng::to_string(config)
        .map_err(|e| std::io::Error::other(format!("YAML serialization failed: {}", e)))?;
    std::fs::write(path, yaml)
}

/// Resolve the global tools.yaml path (`~/.sah/tools.yaml`).
///
/// Returns `None` if the home directory cannot be determined.
pub fn global_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".sah").join(TOOLS_CONFIG_FILENAME))
}

/// Resolve the project tools.yaml path (`.sah/tools.yaml` at the git root).
///
/// Returns `None` if the current directory is not inside a git repository.
pub fn project_config_path() -> Option<PathBuf> {
    swissarmyhammer_common::utils::find_git_repository_root()
        .map(|root| root.join(".sah").join(TOOLS_CONFIG_FILENAME))
}

/// Load and merge tool config from the global and project layers.
///
/// Layer order (later wins):
/// 1. Global: `~/.sah/tools.yaml`
/// 2. Project: `.sah/tools.yaml` at the git root
///
/// Missing files are silently skipped.  Parse errors are logged as warnings
/// and the affected layer is skipped.
pub fn load_merged_tool_config() -> ToolConfig {
    let mut config = ToolConfig::default();

    // Layer 1: Global (~/.sah/tools.yaml)
    if let Some(global_path) = global_config_path() {
        if let Some(global_config) = load_tool_config_from_path(&global_path) {
            config.merge(global_config);
        }
    }

    // Layer 2: Project (.sah/tools.yaml) — overrides global
    if let Some(project_path) = project_config_path() {
        if let Some(project_config) = load_tool_config_from_path(&project_path) {
            config.merge(project_config);
        }
    }

    config
}

/// Apply a loaded [`ToolConfig`] to a [`ToolRegistry`].
///
/// Each entry in the config calls [`ToolRegistry::set_tool_enabled`].  Tools
/// not mentioned in the config are left in their current state (enabled by
/// default).
pub fn apply_tool_config(
    registry: &mut crate::mcp::tool_registry::ToolRegistry,
    config: &ToolConfig,
) {
    for (name, entry) in &config.entries {
        registry.set_tool_enabled(name, entry.enabled);
    }
}

/// Watches tool config files for changes and reloads when modified.
///
/// Tracks the mtime of both global and project `tools.yaml` files. On each
/// call to [`check_and_reload`], compares the current mtime to the last-seen
/// value and re-reads only when the file has actually changed on disk.
///
/// A deleted file is treated as "all tools enabled" (empty config).
pub struct ToolConfigWatcher {
    global_path: Option<PathBuf>,
    project_path: Option<PathBuf>,
    global_mtime: Option<SystemTime>,
    project_mtime: Option<SystemTime>,
}

impl Default for ToolConfigWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolConfigWatcher {
    /// Create a new watcher resolving config paths from the current environment.
    pub fn new() -> Self {
        let global_path = global_config_path();
        let project_path = project_config_path();
        let global_mtime = global_path.as_ref().and_then(|p| file_mtime(p));
        let project_mtime = project_path.as_ref().and_then(|p| file_mtime(p));
        Self {
            global_path,
            project_path,
            global_mtime,
            project_mtime,
        }
    }

    /// Check if config files have changed and reload into the registry if so.
    ///
    /// Returns `true` if the config was reloaded, `false` if no change detected.
    pub fn check_and_reload(
        &mut self,
        registry: &mut crate::mcp::tool_registry::ToolRegistry,
    ) -> bool {
        let new_global_mtime = self.global_path.as_ref().and_then(|p| file_mtime(p));
        let new_project_mtime = self.project_path.as_ref().and_then(|p| file_mtime(p));

        if new_global_mtime == self.global_mtime && new_project_mtime == self.project_mtime {
            return false;
        }

        // Something changed — reload full merged config
        self.global_mtime = new_global_mtime;
        self.project_mtime = new_project_mtime;

        let config = load_merged_tool_config();

        // Reset all tools to enabled, then apply config
        registry.set_all_enabled(true);
        apply_tool_config(registry, &config);

        let disabled = config.disabled_tools();
        if disabled.is_empty() {
            tracing::debug!("Tool config reloaded: all tools enabled");
        } else {
            tracing::info!("Tool config reloaded: {} tools disabled", disabled.len());
        }

        true
    }
}

/// Get the mtime of a file, or `None` if it doesn't exist or can't be read.
fn file_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_config(entries: &[(&str, bool)]) -> ToolConfig {
        let mut config = ToolConfig::default();
        for (name, enabled) in entries {
            config
                .entries_mut()
                .insert(name.to_string(), ToolEntry::new(*enabled));
        }
        config
    }

    #[test]
    fn test_yaml_round_trip() {
        let config = make_config(&[("shell", true), ("kanban", false)]);
        let yaml = serde_yaml_ng::to_string(&config).expect("serialize");
        let decoded: ToolConfig = serde_yaml_ng::from_str(&yaml).expect("deserialize");
        assert_eq!(config, decoded);
    }

    #[test]
    fn test_merge_project_overrides_global() {
        // Global disables shell; project re-enables it.
        let mut global = make_config(&[("shell", false)]);
        let project = make_config(&[("shell", true)]);
        global.merge(project);
        assert!(global.entries()["shell"].is_enabled());
    }

    #[test]
    fn test_disabled_tools_extraction() {
        let config = make_config(&[("shell", true), ("kanban", false), ("git", false)]);
        let mut disabled = config.disabled_tools();
        disabled.sort();
        assert_eq!(disabled, vec!["git".to_string(), "kanban".to_string()]);
    }

    #[test]
    fn test_load_missing_file_returns_none() {
        let result = load_tool_config_from_path(Path::new("/nonexistent/path/tools.yaml"));
        assert!(result.is_none());
    }

    #[test]
    fn test_save_and_load_round_trip() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("tools.yaml");

        let original = make_config(&[("shell", true), ("kanban", false)]);
        save_tool_config(&original, &path).expect("save");

        let loaded = load_tool_config_from_path(&path).expect("load");
        assert_eq!(original, loaded);
    }

    #[test]
    fn test_save_creates_parent_dirs() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("nested").join("dir").join("tools.yaml");

        let config = make_config(&[("shell", true)]);
        save_tool_config(&config, &path).expect("save with nested dirs");
        assert!(path.exists());
    }

    #[test]
    fn test_merge_empty_other_leaves_self_unchanged() {
        let mut config = make_config(&[("shell", false)]);
        config.merge(ToolConfig::default());
        assert!(!config.entries()["shell"].is_enabled());
    }

    /// Validate that every name in KNOWN_TOOL_NAMES exists in a fully-registered
    /// ToolRegistry.  This guards against KNOWN_TOOL_NAMES drifting out of sync
    /// with the actual tool implementations.
    ///
    /// `#[tokio::test]` is required because several register helpers and tool
    /// constructors internally touch async resources (e.g. ShellState, kanban
    /// storage) that need a Tokio runtime to be present.
    #[tokio::test]
    async fn test_known_tool_names_matches_registry() {
        use std::sync::Arc;
        use tokio::sync::RwLock;

        use crate::mcp::tool_registry::{
            register_code_context_tools, register_file_tools, register_git_tools,
            register_kanban_tools, register_questions_tools, register_ralph_tools,
            register_shell_tools, register_web_tools, ToolRegistry,
        };
        use crate::mcp::tools::{agent::register_agent_tools, skill::register_skill_tools};

        let mut registry = ToolRegistry::new();
        register_shell_tools(&mut registry);
        register_file_tools(&mut registry).await;
        register_git_tools(&mut registry);
        register_kanban_tools(&mut registry);
        register_questions_tools(&mut registry);
        register_ralph_tools(&mut registry);
        register_web_tools(&mut registry);
        register_code_context_tools(&mut registry);

        // agent and skill tools require library handles
        let agent_lib = Arc::new(RwLock::new(swissarmyhammer_agents::AgentLibrary::new()));
        let skill_lib = Arc::new(RwLock::new(swissarmyhammer_skills::SkillLibrary::new()));
        let prompt_lib = Arc::new(RwLock::new(
            swissarmyhammer_prompts::PromptLibrary::default(),
        ));
        register_agent_tools(&mut registry, agent_lib, prompt_lib.clone());
        register_skill_tools(&mut registry, skill_lib, prompt_lib);

        for &name in KNOWN_TOOL_NAMES {
            assert!(
                registry.get_tool(name).is_some(),
                "KNOWN_TOOL_NAMES contains '{}' but it is not registered in the ToolRegistry",
                name
            );
        }
    }

    // NOTE: The watcher tests use `#[tokio::test]` even though `check_and_reload`
    // is a synchronous function.  This is required because `ShellExecuteTool::new()`
    // internally accesses shared async state (ShellState) that requires a Tokio
    // runtime to be active at construction time.
    #[tokio::test]
    async fn test_watcher_detects_file_change() {
        use crate::mcp::tool_registry::ToolRegistry;

        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("tools.yaml");

        // Create initial config with shell disabled
        let config = make_config(&[("shell", false)]);
        save_tool_config(&config, &path).expect("save");

        // Create a watcher pointing at our test file
        let mut watcher = ToolConfigWatcher {
            global_path: None,
            project_path: Some(path.clone()),
            global_mtime: None,
            project_mtime: file_mtime(&path),
        };

        // Create a registry with a mock tool
        let mut registry = ToolRegistry::new();
        registry.register(crate::mcp::tools::shell::ShellExecuteTool::new());

        // First check — mtimes match, no reload
        assert!(!watcher.check_and_reload(&mut registry));

        // Modify the file (enable shell)
        std::thread::sleep(std::time::Duration::from_millis(50));
        let new_config = make_config(&[("shell", true)]);
        save_tool_config(&new_config, &path).expect("save updated");

        // Second check — mtime changed, should reload
        assert!(watcher.check_and_reload(&mut registry));
        assert!(registry.is_tool_enabled("shell"));
    }

    // NOTE: #[tokio::test] is required here — see explanation on test_watcher_detects_file_change.
    #[tokio::test]
    async fn test_watcher_no_reload_when_unchanged() {
        use crate::mcp::tool_registry::ToolRegistry;

        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("tools.yaml");

        let config = make_config(&[("shell", false)]);
        save_tool_config(&config, &path).expect("save");

        let mut watcher = ToolConfigWatcher {
            global_path: None,
            project_path: Some(path.clone()),
            global_mtime: None,
            project_mtime: file_mtime(&path),
        };

        let mut registry = ToolRegistry::new();
        registry.register(crate::mcp::tools::shell::ShellExecuteTool::new());

        // Check twice with no file change — both should return false
        assert!(!watcher.check_and_reload(&mut registry));
        assert!(!watcher.check_and_reload(&mut registry));
    }

    // NOTE: #[tokio::test] is required here — see explanation on test_watcher_detects_file_change.
    #[tokio::test]
    async fn test_watcher_deleted_file_reverts_to_all_enabled() {
        use crate::mcp::tool_registry::ToolRegistry;

        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("tools.yaml");

        // Create config disabling shell
        let config = make_config(&[("shell", false)]);
        save_tool_config(&config, &path).expect("save");

        let mut watcher = ToolConfigWatcher {
            global_path: None,
            project_path: Some(path.clone()),
            global_mtime: None,
            project_mtime: file_mtime(&path),
        };

        let mut registry = ToolRegistry::new();
        registry.register(crate::mcp::tools::shell::ShellExecuteTool::new());

        // Apply initial config
        apply_tool_config(&mut registry, &config);
        assert!(!registry.is_tool_enabled("shell"));

        // Delete the file
        std::fs::remove_file(&path).expect("delete");

        // Watcher should detect deletion (mtime goes from Some to None) and reload
        assert!(watcher.check_and_reload(&mut registry));
        assert!(registry.is_tool_enabled("shell"));
    }
}
