//! Claude Code settings file manipulation for MCP server configuration.
//!
//! Claude Code stores MCP server config in different locations by scope:
//! - **project**: `<project-root>/.mcp.json`
//! - **user**: `~/.claude.json` (top-level `mcpServers`)
//! - **local**: `~/.claude.json` (under `projects.<project-path>.mcpServers`)

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

/// Path to the project-level MCP config file.
pub fn mcp_json_path() -> PathBuf {
    PathBuf::from(".mcp.json")
}

/// Path to `~/.claude.json` (used for user and local scopes).
pub fn claude_json_path() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home directory")
        .join(".claude.json")
}

/// Get the absolute project path (used as key in `~/.claude.json` `projects` map).
/// Uses git root if available, otherwise the current directory.
pub fn project_key() -> Result<String, String> {
    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
    {
        if output.status.success() {
            let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !root.is_empty() {
                return Ok(root);
            }
        }
    }
    std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| format!("Failed to get current directory: {}", e))
}

/// Generate the sah MCP server configuration.
pub fn sah_mcp_server_config() -> Value {
    json!({
        "command": "sah",
        "args": ["serve"]
    })
}

/// Check if an MCP server entry is the sah server.
pub fn is_sah_server(config: &Value) -> bool {
    if let Some(cmd) = config.get("command").and_then(|c| c.as_str()) {
        cmd == "sah" || cmd.ends_with("/sah")
    } else {
        false
    }
}

/// Merge sah MCP server into a Value that contains (or should contain) `mcpServers`.
/// Works for both .mcp.json and ~/.claude.json (and project entries within it).
/// Returns true if a change was made.
pub fn merge_mcp_server(settings: &mut Value) -> bool {
    if settings.get("mcpServers").is_none() {
        settings
            .as_object_mut()
            .unwrap()
            .insert("mcpServers".to_string(), json!({}));
    }

    let mcp_servers = settings
        .get_mut("mcpServers")
        .unwrap()
        .as_object_mut()
        .unwrap();

    if let Some(existing) = mcp_servers.get("sah") {
        if is_sah_server(existing) {
            return false;
        }
    }

    mcp_servers.insert("sah".to_string(), sah_mcp_server_config());
    true
}

/// Remove sah MCP server from a Value that contains `mcpServers`.
/// Returns true if a change was made.
pub fn remove_mcp_server(settings: &mut Value) -> bool {
    if let Some(mcp_servers) = settings
        .get_mut("mcpServers")
        .and_then(|m| m.as_object_mut())
    {
        if mcp_servers.remove("sah").is_some() {
            return true;
        }
    }
    false
}

/// Ensure a project entry exists in `~/.claude.json` under `projects.<key>`.
/// Returns a mutable reference to the project entry.
pub fn ensure_project_entry<'a>(root: &'a mut Value, key: &str) -> &'a mut Value {
    if root.get("projects").is_none() {
        root.as_object_mut()
            .unwrap()
            .insert("projects".to_string(), json!({}));
    }

    let projects = root.get_mut("projects").unwrap().as_object_mut().unwrap();

    if !projects.contains_key(key) {
        projects.insert(key.to_string(), json!({}));
    }

    root.get_mut("projects").unwrap().get_mut(key).unwrap()
}

/// Read settings from file, returning empty object if file doesn't exist.
pub fn read_settings(path: &Path) -> Result<Value, String> {
    if path.exists() {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        if content.trim().is_empty() {
            return Ok(json!({}));
        }
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))
    } else {
        Ok(json!({}))
    }
}

/// Write settings to file with pretty formatting.
/// Creates parent directories if needed.
pub fn write_settings(path: &Path, settings: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
        }
    }

    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    fs::write(path, content).map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sah_mcp_server_config_structure() {
        let config = sah_mcp_server_config();
        assert_eq!(config.get("command").unwrap(), "sah");
        assert_eq!(config.get("args").unwrap(), &json!(["serve"]));
    }

    #[test]
    fn test_is_sah_server() {
        let sah = json!({"command": "sah", "args": ["serve"]});
        assert!(is_sah_server(&sah));

        let full_path = json!({"command": "/usr/local/bin/sah", "args": ["serve"]});
        assert!(is_sah_server(&full_path));

        let other = json!({"command": "other-tool", "args": ["run"]});
        assert!(!is_sah_server(&other));
    }

    #[test]
    fn test_merge_mcp_server_empty() {
        let mut settings = json!({});
        let changed = merge_mcp_server(&mut settings);
        assert!(changed);
        assert_eq!(settings["mcpServers"]["sah"]["command"], "sah");
        assert_eq!(settings["mcpServers"]["sah"]["args"], json!(["serve"]));
    }

    #[test]
    fn test_merge_mcp_server_idempotent() {
        let mut settings = json!({});
        merge_mcp_server(&mut settings);
        let first = settings.clone();

        let changed = merge_mcp_server(&mut settings);
        assert!(!changed);
        assert_eq!(settings, first);
    }

    #[test]
    fn test_merge_mcp_server_preserves_existing() {
        let mut settings = json!({
            "mcpServers": {
                "other-tool": { "command": "other", "args": ["run"] }
            }
        });
        merge_mcp_server(&mut settings);
        assert!(settings["mcpServers"]["other-tool"].is_object());
        assert!(settings["mcpServers"]["sah"].is_object());
    }

    #[test]
    fn test_merge_mcp_server_preserves_other_settings() {
        let mut settings = json!({
            "permissions": { "allow": ["Read"] },
            "mcpServers": {}
        });
        merge_mcp_server(&mut settings);
        assert_eq!(settings["permissions"]["allow"], json!(["Read"]));
        assert!(settings["mcpServers"]["sah"].is_object());
    }

    #[test]
    fn test_remove_mcp_server() {
        let mut settings = json!({
            "mcpServers": {
                "sah": { "command": "sah", "args": ["serve"] },
                "other": { "command": "other", "args": ["run"] }
            },
            "other_setting": "value"
        });
        let changed = remove_mcp_server(&mut settings);
        assert!(changed);
        assert!(!settings["mcpServers"]
            .as_object()
            .unwrap()
            .contains_key("sah"));
        assert!(settings["mcpServers"]["other"].is_object());
        assert_eq!(settings["other_setting"], "value");
    }

    #[test]
    fn test_remove_mcp_server_not_present() {
        let mut settings = json!({"mcpServers": {"other": {"command": "other"}}});
        let changed = remove_mcp_server(&mut settings);
        assert!(!changed);
    }

    #[test]
    fn test_remove_mcp_server_no_mcp_servers_key() {
        let mut settings = json!({"other": "value"});
        let changed = remove_mcp_server(&mut settings);
        assert!(!changed);
    }

    #[test]
    fn test_install_uninstall_roundtrip() {
        let mut settings = json!({});
        merge_mcp_server(&mut settings);
        assert!(settings["mcpServers"]["sah"].is_object());

        remove_mcp_server(&mut settings);
        assert!(!settings["mcpServers"]
            .as_object()
            .unwrap()
            .contains_key("sah"));
    }

    #[test]
    fn test_mcp_json_path() {
        let path = mcp_json_path();
        assert_eq!(path, PathBuf::from(".mcp.json"));
    }

    #[test]
    fn test_claude_json_path() {
        let path = claude_json_path();
        assert!(path.ends_with(".claude.json"));
        assert!(path.is_absolute());
    }

    #[test]
    fn test_project_key_returns_nonempty() {
        let key = project_key().unwrap();
        assert!(!key.is_empty());
    }

    #[test]
    fn test_ensure_project_entry_creates_structure() {
        let mut root = json!({});
        let entry = ensure_project_entry(&mut root, "/some/project");
        assert!(entry.is_object());
        assert!(root["projects"]["/some/project"].is_object());
    }

    #[test]
    fn test_ensure_project_entry_preserves_existing() {
        let mut root = json!({
            "projects": {
                "/some/project": { "allowedTools": [] }
            }
        });
        let entry = ensure_project_entry(&mut root, "/some/project");
        assert!(entry.get("allowedTools").is_some());
    }

    #[test]
    fn test_merge_into_project_entry() {
        let mut root = json!({
            "projects": {
                "/my/proj": { "allowedTools": [], "mcpServers": {} }
            },
            "numStartups": 100
        });
        let entry = ensure_project_entry(&mut root, "/my/proj");
        let changed = merge_mcp_server(entry);
        assert!(changed);
        assert_eq!(
            root["projects"]["/my/proj"]["mcpServers"]["sah"]["command"],
            "sah"
        );
        // Preserves other data
        assert_eq!(root["numStartups"], 100);
        assert_eq!(root["projects"]["/my/proj"]["allowedTools"], json!([]));
    }

    #[test]
    fn test_read_settings_nonexistent() {
        let path = PathBuf::from("/tmp/nonexistent-sah-test/settings.json");
        let result = read_settings(&path).unwrap();
        assert_eq!(result, json!({}));
    }

    #[test]
    fn test_read_write_roundtrip() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("settings.json");

        let settings = json!({"mcpServers": {"sah": {"command": "sah", "args": ["serve"]}}});
        write_settings(&path, &settings).unwrap();

        let read_back = read_settings(&path).unwrap();
        assert_eq!(read_back, settings);
    }

    #[test]
    fn test_write_settings_creates_parent_dirs() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("nested/dir/settings.json");

        let settings = json!({"test": true});
        write_settings(&path, &settings).unwrap();

        assert!(path.exists());
    }
}
