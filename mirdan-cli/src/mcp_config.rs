//! MCP server configuration management for AI coding agents.
//!
//! Generalizes MCP config read/write/merge/remove across all agents that
//! support MCP servers (identified by `AgentDef::mcp_config`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::agents::{self, AgentDef};
use crate::registry::RegistryError;

/// An MCP server entry to install into agent config files.
#[derive(Debug, Clone)]
pub struct McpServerEntry {
    /// Key in the mcpServers object (e.g. "sah").
    pub name: String,
    /// Binary command (e.g. "sah").
    pub command: String,
    /// Command arguments (e.g. ["serve"]).
    pub args: Vec<String>,
    /// Optional environment variables.
    pub env: Option<HashMap<String, String>>,
}

impl McpServerEntry {
    /// Convert to JSON value for the config file.
    pub fn to_json(&self) -> Value {
        let mut entry = json!({
            "command": self.command,
            "args": self.args,
        });
        if let Some(ref env) = self.env {
            entry
                .as_object_mut()
                .unwrap()
                .insert("env".to_string(), json!(env));
        }
        entry
    }
}

/// Read a JSON config file, returning empty object if it doesn't exist.
pub fn read_config(path: &Path) -> Result<Value, RegistryError> {
    if path.exists() {
        let content = std::fs::read_to_string(path)?;
        if content.trim().is_empty() {
            return Ok(json!({}));
        }
        serde_json::from_str(&content).map_err(|e| {
            RegistryError::Validation(format!("Failed to parse {}: {}", path.display(), e))
        })
    } else {
        Ok(json!({}))
    }
}

/// Write a JSON config file with pretty formatting.
/// Creates parent directories if needed.
pub fn write_config(path: &Path, settings: &Value) -> Result<(), RegistryError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let content = serde_json::to_string_pretty(settings)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Merge an MCP server entry into a settings Value.
/// Returns true if a change was made.
pub fn merge_mcp_server(settings: &mut Value, servers_key: &str, entry: &McpServerEntry) -> bool {
    if settings.get(servers_key).is_none() {
        settings
            .as_object_mut()
            .unwrap()
            .insert(servers_key.to_string(), json!({}));
    }

    let servers = settings
        .get_mut(servers_key)
        .unwrap()
        .as_object_mut()
        .unwrap();

    if servers.contains_key(&entry.name) {
        return false;
    }

    servers.insert(entry.name.clone(), entry.to_json());
    true
}

/// Remove an MCP server entry from a settings Value by name.
/// Returns true if a change was made.
pub fn remove_mcp_server(settings: &mut Value, servers_key: &str, name: &str) -> bool {
    if let Some(servers) = settings
        .get_mut(servers_key)
        .and_then(|m| m.as_object_mut())
    {
        if servers.remove(name).is_some() {
            return true;
        }
    }
    false
}

/// Install an MCP server entry for a single agent.
/// Returns the config file path that was written, or None if the agent doesn't support MCP.
pub fn install_mcp_for_agent(
    agent: &AgentDef,
    entry: &McpServerEntry,
    global: bool,
) -> Result<Option<PathBuf>, RegistryError> {
    let mcp_def = match &agent.mcp_config {
        Some(c) => c,
        None => return Ok(None),
    };

    let config_path = if global {
        match agents::mcp_global_config_path(agent) {
            Some(p) => p,
            None => return Ok(None),
        }
    } else {
        match agents::mcp_project_config_path(agent) {
            Some(p) => p,
            None => return Ok(None),
        }
    };

    let mut settings = read_config(&config_path)?;
    let changed = merge_mcp_server(&mut settings, &mcp_def.servers_key, entry);

    if changed {
        write_config(&config_path, &settings)?;
    }

    Ok(Some(config_path))
}

/// Uninstall an MCP server entry from a single agent.
/// Returns the config file path that was modified, or None if unchanged.
pub fn uninstall_mcp_for_agent(
    agent: &AgentDef,
    name: &str,
    global: bool,
) -> Result<Option<PathBuf>, RegistryError> {
    let mcp_def = match &agent.mcp_config {
        Some(c) => c,
        None => return Ok(None),
    };

    let config_path = if global {
        match agents::mcp_global_config_path(agent) {
            Some(p) => p,
            None => return Ok(None),
        }
    } else {
        match agents::mcp_project_config_path(agent) {
            Some(p) => p,
            None => return Ok(None),
        }
    };

    if !config_path.exists() {
        return Ok(None);
    }

    let mut settings = read_config(&config_path)?;
    let changed = remove_mcp_server(&mut settings, &mcp_def.servers_key, name);

    if changed {
        // Clean up empty servers object
        if let Some(servers) = settings.get(&mcp_def.servers_key).and_then(|m| m.as_object()) {
            if servers.is_empty() {
                settings
                    .as_object_mut()
                    .unwrap()
                    .remove(&mcp_def.servers_key);
            }
        }

        // Delete file if empty, otherwise write back
        if settings == json!({}) {
            std::fs::remove_file(&config_path)?;
        } else {
            write_config(&config_path, &settings)?;
        }
        Ok(Some(config_path))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_entry() -> McpServerEntry {
        McpServerEntry {
            name: "sah".to_string(),
            command: "sah".to_string(),
            args: vec!["serve".to_string()],
            env: None,
        }
    }

    #[test]
    fn test_entry_to_json() {
        let entry = test_entry();
        let json = entry.to_json();
        assert_eq!(json["command"], "sah");
        assert_eq!(json["args"], json!(["serve"]));
        assert!(json.get("env").is_none());
    }

    #[test]
    fn test_entry_to_json_with_env() {
        let entry = McpServerEntry {
            name: "test".to_string(),
            command: "test-cmd".to_string(),
            args: vec![],
            env: Some(HashMap::from([("KEY".to_string(), "VALUE".to_string())])),
        };
        let json = entry.to_json();
        assert_eq!(json["env"]["KEY"], "VALUE");
    }

    #[test]
    fn test_merge_mcp_server_empty() {
        let mut settings = json!({});
        let entry = test_entry();
        let changed = merge_mcp_server(&mut settings, "mcpServers", &entry);
        assert!(changed);
        assert_eq!(settings["mcpServers"]["sah"]["command"], "sah");
    }

    #[test]
    fn test_merge_mcp_server_idempotent() {
        let mut settings = json!({});
        let entry = test_entry();
        merge_mcp_server(&mut settings, "mcpServers", &entry);
        let changed = merge_mcp_server(&mut settings, "mcpServers", &entry);
        assert!(!changed);
    }

    #[test]
    fn test_merge_mcp_server_preserves_existing() {
        let mut settings = json!({
            "mcpServers": {
                "other": { "command": "other", "args": ["run"] }
            }
        });
        let entry = test_entry();
        merge_mcp_server(&mut settings, "mcpServers", &entry);
        assert!(settings["mcpServers"]["other"].is_object());
        assert!(settings["mcpServers"]["sah"].is_object());
    }

    #[test]
    fn test_remove_mcp_server() {
        let mut settings = json!({
            "mcpServers": {
                "sah": { "command": "sah", "args": ["serve"] },
                "other": { "command": "other" }
            }
        });
        let changed = remove_mcp_server(&mut settings, "mcpServers", "sah");
        assert!(changed);
        assert!(!settings["mcpServers"]
            .as_object()
            .unwrap()
            .contains_key("sah"));
        assert!(settings["mcpServers"]["other"].is_object());
    }

    #[test]
    fn test_remove_mcp_server_not_present() {
        let mut settings = json!({"mcpServers": {"other": {}}});
        let changed = remove_mcp_server(&mut settings, "mcpServers", "sah");
        assert!(!changed);
    }

    #[test]
    fn test_read_write_config_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        let settings = json!({"mcpServers": {"sah": {"command": "sah"}}});
        write_config(&path, &settings).unwrap();
        let read_back = read_config(&path).unwrap();
        assert_eq!(read_back, settings);
    }

    #[test]
    fn test_read_config_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        let result = read_config(&path).unwrap();
        assert_eq!(result, json!({}));
    }

    #[test]
    fn test_read_config_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("invalid.json");
        std::fs::write(&path, "{not valid json!!}").unwrap();
        let result = read_config(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_write_config_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/dir/config.json");
        write_config(&path, &json!({"test": true})).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_install_mcp_for_agent_no_mcp_support() {
        use crate::agents::{AgentDef, SymlinkPolicy};
        let agent = AgentDef {
            id: "test".to_string(),
            name: "Test".to_string(),
            project_path: ".test/skills".to_string(),
            global_path: "~/.test/skills".to_string(),
            detect: vec![],
            symlink_policy: SymlinkPolicy::default(),
            mcp_config: None,
        };
        let entry = test_entry();
        let result = install_mcp_for_agent(&agent, &entry, false).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_install_and_uninstall_mcp_for_agent() {
        use crate::agents::{AgentDef, McpConfigDef, SymlinkPolicy};

        let dir = tempfile::tempdir().unwrap();
        let config_file = dir.path().join("mcp.json");

        let agent = AgentDef {
            id: "test".to_string(),
            name: "Test".to_string(),
            project_path: ".test/skills".to_string(),
            global_path: "~/.test/skills".to_string(),
            detect: vec![],
            symlink_policy: SymlinkPolicy::default(),
            mcp_config: Some(McpConfigDef {
                project_file: Some(config_file.to_string_lossy().to_string()),
                global_file: None,
                servers_key: "mcpServers".to_string(),
            }),
        };

        let entry = test_entry();

        // Install
        let result = install_mcp_for_agent(&agent, &entry, false).unwrap();
        assert!(result.is_some());

        // Verify file contents
        let settings = read_config(&config_file).unwrap();
        assert_eq!(settings["mcpServers"]["sah"]["command"], "sah");

        // Uninstall
        let result = uninstall_mcp_for_agent(&agent, "sah", false).unwrap();
        assert!(result.is_some());

        // File should be removed since it's now empty
        assert!(!config_file.exists());
    }

    #[test]
    fn test_install_mcp_preserves_existing_entries() {
        use crate::agents::{AgentDef, McpConfigDef, SymlinkPolicy};

        let dir = tempfile::tempdir().unwrap();
        let config_file = dir.path().join("mcp.json");

        // Pre-populate config with an existing server
        let existing = json!({
            "mcpServers": {
                "other-tool": { "command": "other", "args": ["run"] }
            }
        });
        write_config(&config_file, &existing).unwrap();

        let agent = AgentDef {
            id: "test".to_string(),
            name: "Test".to_string(),
            project_path: ".test/skills".to_string(),
            global_path: "~/.test/skills".to_string(),
            detect: vec![],
            symlink_policy: SymlinkPolicy::default(),
            mcp_config: Some(McpConfigDef {
                project_file: Some(config_file.to_string_lossy().to_string()),
                global_file: None,
                servers_key: "mcpServers".to_string(),
            }),
        };

        // Install our server
        install_mcp_for_agent(&agent, &test_entry(), false).unwrap();

        // Both entries should exist
        let settings = read_config(&config_file).unwrap();
        assert!(settings["mcpServers"]["other-tool"].is_object());
        assert!(settings["mcpServers"]["sah"].is_object());
    }

    #[test]
    fn test_uninstall_mcp_preserves_other_entries() {
        use crate::agents::{AgentDef, McpConfigDef, SymlinkPolicy};

        let dir = tempfile::tempdir().unwrap();
        let config_file = dir.path().join("mcp.json");

        // Pre-populate config with two servers
        let existing = json!({
            "mcpServers": {
                "sah": { "command": "sah", "args": ["serve"] },
                "other-tool": { "command": "other", "args": ["run"] }
            }
        });
        write_config(&config_file, &existing).unwrap();

        let agent = AgentDef {
            id: "test".to_string(),
            name: "Test".to_string(),
            project_path: ".test/skills".to_string(),
            global_path: "~/.test/skills".to_string(),
            detect: vec![],
            symlink_policy: SymlinkPolicy::default(),
            mcp_config: Some(McpConfigDef {
                project_file: Some(config_file.to_string_lossy().to_string()),
                global_file: None,
                servers_key: "mcpServers".to_string(),
            }),
        };

        // Uninstall sah
        uninstall_mcp_for_agent(&agent, "sah", false).unwrap();

        // File should still exist with other-tool
        assert!(config_file.exists());
        let settings = read_config(&config_file).unwrap();
        assert!(settings["mcpServers"]["other-tool"].is_object());
        assert!(!settings["mcpServers"]
            .as_object()
            .unwrap()
            .contains_key("sah"));
    }

    #[test]
    fn test_install_mcp_global_scope() {
        use crate::agents::{AgentDef, McpConfigDef, SymlinkPolicy};

        let dir = tempfile::tempdir().unwrap();
        let global_config = dir.path().join("global-mcp.json");

        let agent = AgentDef {
            id: "test".to_string(),
            name: "Test".to_string(),
            project_path: ".test/skills".to_string(),
            global_path: "~/.test/skills".to_string(),
            detect: vec![],
            symlink_policy: SymlinkPolicy::default(),
            mcp_config: Some(McpConfigDef {
                project_file: Some("project-mcp.json".to_string()),
                global_file: Some(global_config.to_string_lossy().to_string()),
                servers_key: "mcpServers".to_string(),
            }),
        };

        // Install globally
        let result = install_mcp_for_agent(&agent, &test_entry(), true).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), global_config);

        // Global config should have the entry
        let settings = read_config(&global_config).unwrap();
        assert_eq!(settings["mcpServers"]["sah"]["command"], "sah");

        // Project config should NOT exist (we installed globally)
        assert!(!dir.path().join("project-mcp.json").exists());
    }

    #[test]
    fn test_install_mcp_agent_no_project_file() {
        use crate::agents::{AgentDef, McpConfigDef, SymlinkPolicy};

        let dir = tempfile::tempdir().unwrap();
        let global_config = dir.path().join("windsurf-global.json");

        // Agent like Windsurf with no project-level MCP support
        let agent = AgentDef {
            id: "windsurf".to_string(),
            name: "Windsurf".to_string(),
            project_path: ".windsurf/skills".to_string(),
            global_path: "~/.windsurf/skills".to_string(),
            detect: vec![],
            symlink_policy: SymlinkPolicy::default(),
            mcp_config: Some(McpConfigDef {
                project_file: None,
                global_file: Some(global_config.to_string_lossy().to_string()),
                servers_key: "mcpServers".to_string(),
            }),
        };

        // Project-level install should return None (no project_file)
        let result = install_mcp_for_agent(&agent, &test_entry(), false).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_install_mcp_idempotent() {
        use crate::agents::{AgentDef, McpConfigDef, SymlinkPolicy};

        let dir = tempfile::tempdir().unwrap();
        let config_file = dir.path().join("mcp.json");

        let agent = AgentDef {
            id: "test".to_string(),
            name: "Test".to_string(),
            project_path: ".test/skills".to_string(),
            global_path: "~/.test/skills".to_string(),
            detect: vec![],
            symlink_policy: SymlinkPolicy::default(),
            mcp_config: Some(McpConfigDef {
                project_file: Some(config_file.to_string_lossy().to_string()),
                global_file: None,
                servers_key: "mcpServers".to_string(),
            }),
        };

        let entry = test_entry();

        // Install twice
        install_mcp_for_agent(&agent, &entry, false).unwrap();
        install_mcp_for_agent(&agent, &entry, false).unwrap();

        // Should still have exactly one entry
        let settings = read_config(&config_file).unwrap();
        let servers = settings["mcpServers"].as_object().unwrap();
        assert_eq!(servers.len(), 1);
    }

    #[test]
    fn test_mcp_entry_with_env() {
        let entry = McpServerEntry {
            name: "test".to_string(),
            command: "test-cmd".to_string(),
            args: vec!["--flag".to_string()],
            env: Some(HashMap::from([
                ("API_KEY".to_string(), "secret".to_string()),
                ("DEBUG".to_string(), "1".to_string()),
            ])),
        };

        let mut settings = json!({});
        merge_mcp_server(&mut settings, "mcpServers", &entry);

        let server = &settings["mcpServers"]["test"];
        assert_eq!(server["command"], "test-cmd");
        assert_eq!(server["args"], json!(["--flag"]));
        assert_eq!(server["env"]["API_KEY"], "secret");
        assert_eq!(server["env"]["DEBUG"], "1");
    }
}
