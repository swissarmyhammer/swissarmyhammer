//! MCP configuration file management for Tool deployment.
//!
//! Handles reading, modifying, and writing MCP server entries in agent-specific
//! JSON configuration files (e.g. `.mcp.json`, `.cursor/mcp.json`).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

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
pub fn parse_tool_frontmatter(yaml: &serde_yaml::Value) -> Result<McpFrontmatter, RegistryError> {
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
    let mut root = read_json_config(config_path)?;

    let servers = root
        .as_object_mut()
        .ok_or_else(|| {
            RegistryError::Validation(format!(
                "MCP config is not a JSON object: {}",
                config_path.display()
            ))
        })?
        .entry(servers_key)
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));

    let servers_map = servers.as_object_mut().ok_or_else(|| {
        RegistryError::Validation(format!(
            "'{}' is not an object in {}",
            servers_key,
            config_path.display()
        ))
    })?;

    servers_map.insert(
        tool_name.to_string(),
        serde_json::to_value(entry).map_err(|e| {
            RegistryError::Validation(format!("Failed to serialize MCP entry: {}", e))
        })?,
    );

    write_json_config(config_path, &root)
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

    let mut root = read_json_config(config_path)?;

    let removed = root
        .as_object_mut()
        .and_then(|obj| obj.get_mut(servers_key))
        .and_then(|servers| servers.as_object_mut())
        .map(|servers_map| servers_map.remove(tool_name).is_some())
        .unwrap_or(false);

    if removed {
        write_json_config(config_path, &root)?;
    }

    Ok(removed)
}

/// Read a JSON config file, returning an empty object if the file doesn't exist.
fn read_json_config(path: &Path) -> Result<serde_json::Value, RegistryError> {
    if !path.exists() {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    }

    let content = std::fs::read_to_string(path)?;
    let content = content.trim();
    if content.is_empty() {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    }

    serde_json::from_str(content).map_err(|e| {
        RegistryError::Validation(format!("Invalid JSON in {}: {}", path.display(), e))
    })
}

/// Write a JSON config file with pretty-printing. Creates parent dirs if needed.
fn write_json_config(path: &Path, value: &serde_json::Value) -> Result<(), RegistryError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| RegistryError::Validation(format!("Failed to serialize JSON: {}", e)))?;
    std::fs::write(path, format!("{}\n", json))?;
    Ok(())
}

/// Parse raw YAML frontmatter from a markdown file, returning the YAML Value.
pub fn parse_yaml_frontmatter(path: &Path) -> Result<serde_yaml::Value, RegistryError> {
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
    serde_yaml::from_str(frontmatter)
        .map_err(|e| RegistryError::Validation(format!("Invalid YAML frontmatter: {}", e)))
}

/// Read the plugin name from `.claude-plugin/plugin.json`.
pub fn read_plugin_json(plugin_json_path: &Path) -> Result<String, RegistryError> {
    let content = std::fs::read_to_string(plugin_json_path)?;
    let json: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tool_frontmatter() {
        let yaml: serde_yaml::Value = serde_yaml::from_str(
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
        let yaml: serde_yaml::Value = serde_yaml::from_str("name: no-mcp\n").unwrap();
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
    fn test_read_plugin_json_missing_name() {
        let dir = tempfile::tempdir().unwrap();
        let plugin_json = dir.path().join("plugin.json");
        std::fs::write(&plugin_json, r#"{"description": "no name"}"#).unwrap();

        assert!(read_plugin_json(&plugin_json).is_err());
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
