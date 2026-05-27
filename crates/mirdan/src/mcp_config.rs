//! MCP configuration file management for Tool deployment.
//!
//! Handles reading, modifying, and writing MCP server entries in agent-specific
//! JSON configuration files (e.g. `.mcp.json`, `.cursor/mcp.json`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::registry::RegistryError;

/// An MCP server entry as written to agent config files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerEntry {
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
}

/// MCP configuration parsed from TOOL.md frontmatter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpFrontmatter {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub transport: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

/// Parse the `mcp` section from TOOL.md YAML frontmatter.
pub fn parse_tool_frontmatter(
    yaml: &serde_yaml_ng::Value,
) -> Result<McpFrontmatter, RegistryError> {
    let mcp = yaml.get("mcp").ok_or_else(|| {
        RegistryError::Validation("TOOL.md frontmatter missing 'mcp' section".to_string())
    })?;

    let command = mcp
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            RegistryError::Validation("TOOL.md mcp section missing 'command'".to_string())
        })?
        .to_string();

    let args: Vec<String> = mcp
        .get("args")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let transport = mcp
        .get("transport")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let env: BTreeMap<String, String> = mcp
        .get("env")
        .and_then(|v| v.as_mapping())
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| {
                    let key = k.as_str()?.to_string();
                    let val = v.as_str()?.to_string();
                    Some((key, val))
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(McpFrontmatter {
        command,
        args,
        transport,
        env,
    })
}

/// Set the MCP server entry for `tool_name` under `servers_key` in
/// `root`, returning `true` if the in-memory value changed.
///
/// Creates the `servers_key` object if it does not yet exist. Returns a
/// `Validation` error when `root` or `servers_key` exists but is not a
/// JSON object. The boolean indicates whether the resulting value differs
/// from what was already there (so `false` means the entry was already
/// equal to `entry`).
pub fn set_mcp_server_entry(
    root: &mut Value,
    servers_key: &str,
    tool_name: &str,
    entry: &McpServerEntry,
) -> Result<bool, RegistryError> {
    let obj = root.as_object_mut().ok_or_else(|| {
        RegistryError::Validation("MCP config root is not a JSON object".to_string())
    })?;

    let servers = obj
        .entry(servers_key)
        .or_insert_with(|| Value::Object(serde_json::Map::new()));

    let servers_map = servers
        .as_object_mut()
        .ok_or_else(|| RegistryError::Validation(format!("'{}' is not an object", servers_key)))?;

    let serialized = serde_json::to_value(entry)
        .map_err(|e| RegistryError::Validation(format!("Failed to serialize MCP entry: {}", e)))?;

    if servers_map.get(tool_name) == Some(&serialized) {
        return Ok(false);
    }
    servers_map.insert(tool_name.to_string(), serialized);
    Ok(true)
}

/// Remove the MCP server entry for `tool_name` from `root[servers_key]`,
/// returning `true` if an entry was removed.
///
/// Returns `false` (no error) when `servers_key` is absent, is not an
/// object, or does not contain `tool_name`.
pub fn remove_mcp_server_entry(root: &mut Value, servers_key: &str, tool_name: &str) -> bool {
    root.as_object_mut()
        .and_then(|obj| obj.get_mut(servers_key))
        .and_then(|servers| servers.as_object_mut())
        .map(|servers_map| servers_map.remove(tool_name).is_some())
        .unwrap_or(false)
}

/// Register an MCP server in a JSON config file.
///
/// Reads the existing config (or creates a new one), merges the entry into
/// the `servers_key` object, and writes back with pretty-printing.
/// Creates parent directories if needed.
pub fn register_mcp_server(
    config_path: &Path,
    servers_key: &str,
    tool_name: &str,
    entry: &McpServerEntry,
) -> Result<(), RegistryError> {
    let mut root = crate::settings::read_json(config_path)?;
    set_mcp_server_entry(&mut root, servers_key, tool_name, entry)?;
    crate::settings::write_json(config_path, &root)
}

/// Unregister an MCP server from a JSON config file.
///
/// Returns `true` if the entry was found and removed, `false` if not found.
pub fn unregister_mcp_server(
    config_path: &Path,
    servers_key: &str,
    tool_name: &str,
) -> Result<bool, RegistryError> {
    if !config_path.exists() {
        return Ok(false);
    }

    let mut root = crate::settings::read_json(config_path)?;
    let removed = remove_mcp_server_entry(&mut root, servers_key, tool_name);
    if removed {
        crate::settings::write_json(config_path, &root)?;
    }
    Ok(removed)
}

/// Parse raw YAML frontmatter from a markdown file, returning the YAML Value.
pub fn parse_yaml_frontmatter(path: &Path) -> Result<serde_yaml_ng::Value, RegistryError> {
    let content = std::fs::read_to_string(path)?;
    let content = content.trim();

    if !content.starts_with("---") {
        return Err(RegistryError::Validation(format!(
            "{} must start with YAML frontmatter (---)",
            path.display()
        )));
    }

    let rest = &content[3..];
    let end = rest.find("---").ok_or_else(|| {
        RegistryError::Validation(format!("No closing --- in {} frontmatter", path.display()))
    })?;

    let frontmatter = &rest[..end];
    serde_yaml_ng::from_str(frontmatter)
        .map_err(|e| RegistryError::Validation(format!("Invalid YAML frontmatter: {}", e)))
}

/// Read the plugin name from `.claude-plugin/plugin.json`.
///
/// Accepts JSONC (comments and trailing commas) because plugin authors edit
/// this file by hand and may carry over JSONC conventions from agent configs.
pub fn read_plugin_json(plugin_json_path: &Path) -> Result<String, RegistryError> {
    let content = std::fs::read_to_string(plugin_json_path)?;
    let json: serde_json::Value = crate::parse_jsonc(&content).map_err(|e| {
        RegistryError::Validation(format!(
            "Invalid JSON in {}: {}",
            plugin_json_path.display(),
            e
        ))
    })?;

    json.get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            RegistryError::Validation(format!("Missing 'name' in {}", plugin_json_path.display()))
        })
}

// ── Claude Code local-scope helpers ──────────────────────────────────
//
// Claude Code's local scope stores MCP servers in `~/.claude.json` under
// `projects.<absolute-project-path>.mcpServers`. The helpers below are the
// minimum primitives a caller needs to compose that path and ensure the
// nested project entry exists before mutating its `mcpServers` map.

/// Path to `~/.claude.json` — the file Claude Code reads for user-level
/// and local-scope MCP server configuration.
///
/// Honors `dirs::home_dir()`, which is overridden by `HOME` on Unix and by
/// the `USERPROFILE` family on Windows. Panics if no home directory can be
/// determined.
pub fn claude_json_path() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home directory")
        .join(".claude.json")
}

/// Compute the absolute project path used as the key in
/// `~/.claude.json`'s `projects` map.
///
/// Prefers the git working-tree root (`git rev-parse --show-toplevel`)
/// when run inside a repository; falls back to the current working
/// directory otherwise. Returns a `Validation` error when the current
/// directory cannot be resolved.
pub fn project_key() -> Result<String, RegistryError> {
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
        .map_err(|e| RegistryError::Validation(format!("Failed to get current directory: {}", e)))
}

/// Ensure that `root["projects"][key]` exists as an empty object and
/// return a mutable reference to it.
///
/// Creates the `projects` map and the nested project entry if either is
/// missing; preserves any existing fields under the project entry when it
/// already exists. `root` must be a JSON object.
pub fn ensure_project_entry<'a>(root: &'a mut Value, key: &str) -> &'a mut Value {
    if root.get("projects").is_none() {
        root.as_object_mut()
            .expect("ensure_project_entry requires root to be a JSON object")
            .insert("projects".to_string(), json!({}));
    }

    let projects = root
        .get_mut("projects")
        .expect("just inserted")
        .as_object_mut()
        .expect("projects must be an object");

    if !projects.contains_key(key) {
        projects.insert(key.to_string(), json!({}));
    }

    root.get_mut("projects")
        .expect("projects exists")
        .get_mut(key)
        .expect("just ensured")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tool_frontmatter() {
        let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
name: my-tool
mcp:
  command: npx
  args:
    - "-y"
    - "@my/mcp-server"
  transport: stdio
  env:
    API_KEY: "${API_KEY}"
"#,
        )
        .unwrap();

        let fm = parse_tool_frontmatter(&yaml).unwrap();
        assert_eq!(fm.command, "npx");
        assert_eq!(fm.args, vec!["-y", "@my/mcp-server"]);
        assert_eq!(fm.transport, Some("stdio".to_string()));
        assert_eq!(fm.env.get("API_KEY").unwrap(), "${API_KEY}");
    }

    #[test]
    fn test_parse_tool_frontmatter_missing_mcp() {
        let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str("name: no-mcp\n").unwrap();
        assert!(parse_tool_frontmatter(&yaml).is_err());
    }

    #[test]
    fn test_register_mcp_server_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(".mcp.json");

        let entry = McpServerEntry {
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@my/server".to_string()],
            env: BTreeMap::new(),
        };

        register_mcp_server(&config_path, "mcpServers", "my-tool", &entry).unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(json["mcpServers"]["my-tool"]["command"]
            .as_str()
            .unwrap()
            .contains("npx"));
    }

    #[test]
    fn test_register_mcp_server_preserves_existing() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(".mcp.json");

        // Write existing config
        std::fs::write(
            &config_path,
            r#"{"mcpServers": {"existing": {"command": "node"}}, "otherKey": true}"#,
        )
        .unwrap();

        let entry = McpServerEntry {
            command: "npx".to_string(),
            args: vec![],
            env: BTreeMap::new(),
        };

        register_mcp_server(&config_path, "mcpServers", "new-tool", &entry).unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        // Existing entry preserved
        assert_eq!(json["mcpServers"]["existing"]["command"], "node");
        // New entry added
        assert_eq!(json["mcpServers"]["new-tool"]["command"], "npx");
        // Other keys preserved
        assert_eq!(json["otherKey"], true);
    }

    #[test]
    fn test_unregister_mcp_server() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(".mcp.json");

        std::fs::write(
            &config_path,
            r#"{"mcpServers": {"my-tool": {"command": "npx"}, "other": {"command": "node"}}}"#,
        )
        .unwrap();

        let removed = unregister_mcp_server(&config_path, "mcpServers", "my-tool").unwrap();
        assert!(removed);

        let content = std::fs::read_to_string(&config_path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(json["mcpServers"]["my-tool"].is_null());
        assert_eq!(json["mcpServers"]["other"]["command"], "node");
    }

    #[test]
    fn test_unregister_mcp_server_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(".mcp.json");

        std::fs::write(&config_path, r#"{"mcpServers": {}}"#).unwrap();

        let removed = unregister_mcp_server(&config_path, "mcpServers", "nonexistent").unwrap();
        assert!(!removed);
    }

    #[test]
    fn test_unregister_mcp_server_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("nonexistent.json");

        let removed = unregister_mcp_server(&config_path, "mcpServers", "tool").unwrap();
        assert!(!removed);
    }

    #[test]
    fn test_read_plugin_json() {
        let dir = tempfile::tempdir().unwrap();
        let plugin_json = dir.path().join("plugin.json");
        std::fs::write(
            &plugin_json,
            r#"{"name": "my-plugin", "description": "A plugin"}"#,
        )
        .unwrap();

        let name = read_plugin_json(&plugin_json).unwrap();
        assert_eq!(name, "my-plugin");
    }

    #[test]
    fn test_read_plugin_json_accepts_jsonc() {
        // Plugin authors may carry JSONC conventions over from their agent
        // settings. Trailing commas and line comments must parse.
        let dir = tempfile::tempdir().unwrap();
        let plugin_json = dir.path().join("plugin.json");
        std::fs::write(
            &plugin_json,
            "// My plugin\n{\n  \"name\": \"my-plugin\",\n  \"description\": \"A plugin\",\n}",
        )
        .unwrap();

        let name = read_plugin_json(&plugin_json).unwrap();
        assert_eq!(name, "my-plugin");
    }

    #[test]
    fn test_register_mcp_server_into_jsonc_settings() {
        // End-to-end regression for the Zed `init user` failure: when the
        // existing settings file is JSONC (header comment, trailing commas),
        // `register_mcp_server` must read it via the lenient parser, mutate,
        // and write strict JSON back. Mirrors the install path that failed
        // for `~/.config/zed/settings.json`.
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("settings.json");
        std::fs::write(
            &config_path,
            "// Settings for Zed AI\n{\n  \"context_servers\": {},\n}",
        )
        .unwrap();

        let entry = McpServerEntry {
            command: "sah".to_string(),
            args: vec!["serve".to_string()],
            env: BTreeMap::new(),
        };

        register_mcp_server(&config_path, "context_servers", "sah", &entry).unwrap();

        // Written file is now strict JSON; round-trips with `read_plugin_json`-
        // style strict parsing.
        let content = std::fs::read_to_string(&config_path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(json["context_servers"]["sah"]["command"], "sah");
        assert_eq!(json["context_servers"]["sah"]["args"][0], "serve");
    }

    #[test]
    fn test_read_plugin_json_missing_name() {
        let dir = tempfile::tempdir().unwrap();
        let plugin_json = dir.path().join("plugin.json");
        std::fs::write(&plugin_json, r#"{"description": "no name"}"#).unwrap();

        assert!(read_plugin_json(&plugin_json).is_err());
    }

    #[test]
    fn test_set_mcp_server_entry_creates_servers_key_when_missing() {
        let mut root = json!({});
        let entry = McpServerEntry {
            command: "sah".to_string(),
            args: vec!["serve".to_string()],
            env: BTreeMap::new(),
        };
        let changed = set_mcp_server_entry(&mut root, "mcpServers", "sah", &entry).unwrap();
        assert!(changed);
        assert_eq!(root["mcpServers"]["sah"]["command"], "sah");
        assert_eq!(root["mcpServers"]["sah"]["args"], json!(["serve"]));
    }

    #[test]
    fn test_set_mcp_server_entry_is_idempotent_for_equal_value() {
        let entry = McpServerEntry {
            command: "sah".to_string(),
            args: vec!["serve".to_string()],
            env: BTreeMap::new(),
        };
        let mut root = json!({});
        set_mcp_server_entry(&mut root, "mcpServers", "sah", &entry).unwrap();
        let changed = set_mcp_server_entry(&mut root, "mcpServers", "sah", &entry).unwrap();
        assert!(!changed);
    }

    #[test]
    fn test_set_mcp_server_entry_preserves_siblings() {
        let mut root = json!({
            "mcpServers": { "other": { "command": "node" } },
            "otherKey": "value"
        });
        let entry = McpServerEntry {
            command: "sah".to_string(),
            args: vec![],
            env: BTreeMap::new(),
        };
        set_mcp_server_entry(&mut root, "mcpServers", "sah", &entry).unwrap();
        assert_eq!(root["mcpServers"]["other"]["command"], "node");
        assert_eq!(root["mcpServers"]["sah"]["command"], "sah");
        assert_eq!(root["otherKey"], "value");
    }

    #[test]
    fn test_remove_mcp_server_entry_returns_true_when_present() {
        let mut root = json!({
            "mcpServers": { "sah": { "command": "sah" }, "other": { "command": "n" } }
        });
        let removed = remove_mcp_server_entry(&mut root, "mcpServers", "sah");
        assert!(removed);
        assert!(!root["mcpServers"].as_object().unwrap().contains_key("sah"));
        assert!(root["mcpServers"]["other"].is_object());
    }

    #[test]
    fn test_remove_mcp_server_entry_returns_false_when_absent() {
        let mut root = json!({"mcpServers": {"other": {"command": "node"}}});
        assert!(!remove_mcp_server_entry(&mut root, "mcpServers", "sah"));
    }

    #[test]
    fn test_remove_mcp_server_entry_returns_false_when_servers_key_missing() {
        let mut root = json!({"otherKey": "value"});
        assert!(!remove_mcp_server_entry(&mut root, "mcpServers", "sah"));
    }

    #[test]
    fn test_claude_json_path_is_absolute_and_named() {
        let path = claude_json_path();
        assert!(path.is_absolute(), "claude_json_path should be absolute");
        assert!(
            path.ends_with(".claude.json"),
            "claude_json_path should end in .claude.json: {}",
            path.display()
        );
    }

    #[test]
    fn test_project_key_returns_nonempty_path() {
        let key = project_key().expect("project_key should not fail in a real cwd");
        assert!(!key.is_empty(), "project key should not be empty");
    }

    #[test]
    fn test_ensure_project_entry_creates_missing_structure() {
        let mut root = json!({});
        let entry = ensure_project_entry(&mut root, "/abs/project/path");
        assert_eq!(entry, &json!({}));
        assert!(root["projects"]["/abs/project/path"].is_object());
    }

    #[test]
    fn test_ensure_project_entry_preserves_existing_entry_fields() {
        let mut root = json!({
            "projects": {
                "/abs/project/path": { "allowedTools": ["Read"], "mcpServers": {} }
            },
            "otherKey": "value"
        });
        let entry = ensure_project_entry(&mut root, "/abs/project/path");
        assert_eq!(entry["allowedTools"], json!(["Read"]));
        assert!(entry["mcpServers"].is_object());
        // Sibling keys preserved.
        assert_eq!(root["otherKey"], json!("value"));
    }

    #[test]
    fn test_ensure_project_entry_adds_only_requested_key() {
        let mut root = json!({
            "projects": {
                "/other/project": { "allowedTools": [] }
            }
        });
        ensure_project_entry(&mut root, "/new/project");
        assert!(root["projects"]["/other/project"].is_object());
        assert!(root["projects"]["/new/project"].is_object());
    }

    #[test]
    fn test_parse_yaml_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("TOOL.md");
        std::fs::write(
            &md,
            "---\nname: my-tool\nmcp:\n  command: npx\n---\n# Tool\n",
        )
        .unwrap();

        let yaml = parse_yaml_frontmatter(&md).unwrap();
        assert_eq!(yaml["name"].as_str().unwrap(), "my-tool");
        assert_eq!(yaml["mcp"]["command"].as_str().unwrap(), "npx");
    }
}
