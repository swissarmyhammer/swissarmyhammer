//! Per-agent configuration strategies.
//!
//! All agent-specific configuration knowledge — where an agent stores its MCP
//! servers, how it expresses a denied tool, the per-scope settings-file
//! layout — lives here, behind one [`AgentConfigStrategy`] per supported
//! agent. Tools and CLIs never reach into agent config directly; they declare
//! intent ("register me as an MCP server", "deny Bash") and the appliers in
//! [`crate::install`] dispatch to the right strategy via [`strategy_for`].
//!
//! Two strategies cover every agent today:
//!
//! - [`ClaudeCodeStrategy`] owns all of Claude Code's specifics: MCP per scope
//!   (`.mcp.json` for project, `~/.claude.json` `projects.<key>.mcpServers`
//!   for local, global `~/.claude.json` for user), and the `permissions.deny`
//!   array in the scope's settings file (`.claude/settings.json`,
//!   `.claude/settings.local.json`, or `~/.claude/settings.json`).
//! - [`GenericMcpJsonStrategy`] uses only the declarative `mcp_config` fields
//!   from [`AgentDef`] (servers_key / entry_extras / project_path /
//!   global_path). It has no permission or local-scope mechanism, so its
//!   `deny_tool`/`allow_tool` are no-ops and its MCP local scope falls back to
//!   the project config.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::json;
use swissarmyhammer_common::lifecycle::InitScope;

use crate::agents::{self, AgentDef};
use crate::mcp_config::{self, McpServerEntry};
use crate::registry::RegistryError;
use crate::settings;

/// JSON pointer for an agent's denied-tools array (Claude Code shape).
const PERMISSIONS_DENY_POINTER: &str = "/permissions/deny";

/// Per-agent configuration strategy.
///
/// Each method applies one declarative change to one agent at one scope and
/// returns `true` when the on-disk config actually changed (so callers can
/// skip emitting a redundant reporter event). No method emits events itself —
/// that is the applier's job in [`crate::install`].
///
/// Errors are surfaced as [`RegistryError`] so the appliers can downgrade a
/// per-agent failure to a warning without aborting the whole iteration.
pub trait AgentConfigStrategy {
    /// Register `entry` as the MCP server named `server_name` for `agent` at
    /// `scope`. Returns `true` when the config changed.
    fn register_mcp(
        &self,
        agent: &AgentDef,
        scope: InitScope,
        server_name: &str,
        entry: &McpServerEntry,
    ) -> Result<bool, RegistryError>;

    /// Remove the MCP server named `server_name` for `agent` at `scope`.
    /// Returns `true` when the config changed.
    fn unregister_mcp(
        &self,
        agent: &AgentDef,
        scope: InitScope,
        server_name: &str,
    ) -> Result<bool, RegistryError>;

    /// Deny `tool` for `agent` at `scope`. Returns `true` when the config
    /// changed.
    fn deny_tool(
        &self,
        agent: &AgentDef,
        scope: InitScope,
        tool: &str,
    ) -> Result<bool, RegistryError>;

    /// Allow `tool` (remove a prior deny) for `agent` at `scope`. Returns
    /// `true` when the config changed.
    fn allow_tool(
        &self,
        agent: &AgentDef,
        scope: InitScope,
        tool: &str,
    ) -> Result<bool, RegistryError>;
}

/// Select the configuration strategy for `agent`, dispatching on `agent.id`.
///
/// `claude-code` gets the full [`ClaudeCodeStrategy`]; every other agent gets
/// the declarative [`GenericMcpJsonStrategy`] driven by its `mcp_config`
/// fields.
pub fn strategy_for(agent: &AgentDef) -> Box<dyn AgentConfigStrategy> {
    match agent.id.as_str() {
        "claude-code" => Box::new(ClaudeCodeStrategy),
        _ => Box::new(GenericMcpJsonStrategy),
    }
}

// ── Claude Code ──────────────────────────────────────────────────────

/// Claude Code's configuration strategy.
///
/// Owns the entirety of Claude Code's on-disk layout: the three MCP targets
/// (project `.mcp.json`, local `~/.claude.json` `projects.<key>.mcpServers`,
/// global `~/.claude.json`) and the per-scope `permissions.deny` settings
/// file. This is the single place those Claude specifics live.
pub struct ClaudeCodeStrategy;

impl ClaudeCodeStrategy {
    /// Resolve the settings file holding `permissions.deny` for `scope`.
    ///
    /// - `Project` → the agent's project settings file (`.claude/settings.json`).
    /// - `Local`   → the project settings file's `settings.local.json` sibling.
    /// - `User`    → the agent's global settings file (`~/.claude/settings.json`).
    ///
    /// Returns `None` only when the agent declares no settings path, which is
    /// not the case for the real `claude-code` definition.
    fn settings_path(agent: &AgentDef, scope: InitScope) -> Option<PathBuf> {
        match scope {
            InitScope::User => agents::agent_global_settings_file(agent),
            InitScope::Project => agents::agent_project_settings_file(agent),
            InitScope::Local => {
                agents::agent_project_settings_file(agent).map(local_settings_sibling)
            }
        }
    }

    /// Apply `mutate` to the `permissions.deny` array in the scope's settings
    /// file, persisting only when the in-memory value changed.
    fn edit_deny(
        agent: &AgentDef,
        scope: InitScope,
        mutate: impl FnOnce(&mut serde_json::Value) -> bool,
    ) -> Result<bool, RegistryError> {
        let Some(path) = Self::settings_path(agent, scope) else {
            return Ok(false);
        };
        let mut root = settings::read_json(&path)?;
        let changed = mutate(&mut root);
        if changed {
            settings::write_json(&path, &root)?;
        }
        Ok(changed)
    }
}

impl AgentConfigStrategy for ClaudeCodeStrategy {
    fn register_mcp(
        &self,
        agent: &AgentDef,
        scope: InitScope,
        server_name: &str,
        entry: &McpServerEntry,
    ) -> Result<bool, RegistryError> {
        match scope {
            InitScope::Local => {
                let key = mcp_config::project_key()?;
                set_mcp_local_scope(
                    &mcp_config::claude_json_path(),
                    &key,
                    server_name,
                    entry,
                    &BTreeMap::new(),
                )
            }
            InitScope::Project | InitScope::User => {
                generic_register_mcp(agent, scope, server_name, entry)
            }
        }
    }

    fn unregister_mcp(
        &self,
        agent: &AgentDef,
        scope: InitScope,
        server_name: &str,
    ) -> Result<bool, RegistryError> {
        match scope {
            InitScope::Local => {
                let key = mcp_config::project_key()?;
                remove_mcp_local_scope(&mcp_config::claude_json_path(), &key, server_name)
            }
            InitScope::Project | InitScope::User => {
                generic_unregister_mcp(agent, scope, server_name)
            }
        }
    }

    fn deny_tool(
        &self,
        agent: &AgentDef,
        scope: InitScope,
        tool: &str,
    ) -> Result<bool, RegistryError> {
        Self::edit_deny(agent, scope, |root| {
            settings::ensure_array_contains(root, PERMISSIONS_DENY_POINTER, &json!(tool))
        })
    }

    fn allow_tool(
        &self,
        agent: &AgentDef,
        scope: InitScope,
        tool: &str,
    ) -> Result<bool, RegistryError> {
        let Some(path) = Self::settings_path(agent, scope) else {
            return Ok(false);
        };
        // Removing from a file that doesn't exist is a no-op, and read_json
        // would synthesize an empty object we'd never need to write back.
        if !path.exists() {
            return Ok(false);
        }
        Self::edit_deny(agent, scope, |root| {
            settings::remove_from_array(root, PERMISSIONS_DENY_POINTER, &json!(tool))
        })
    }
}

/// Derive Claude Code's local-scope settings sibling from a project settings
/// path by swapping the file name `settings.json` → `settings.local.json`.
///
/// Falls back to joining `settings.local.json` onto the parent when the path
/// has no recognizable file name.
fn local_settings_sibling(project_settings: PathBuf) -> PathBuf {
    let parent = project_settings.parent().map(Path::to_path_buf);
    match parent {
        Some(dir) => dir.join("settings.local.json"),
        None => PathBuf::from("settings.local.json"),
    }
}

// ── Claude Code local scope (`~/.claude.json`) ───────────────────────
//
// Claude Code's local scope stores MCP servers in `~/.claude.json` under
// `projects.<absolute-project-path>.mcpServers`. These helpers are Claude's
// local mechanism — folded in from the former `mcp_config` public API — and
// are intentionally private to the strategy layer.

/// Register an MCP server in Claude Code's local scope: `claude_json` under
/// `projects.<project_key>.mcpServers.<server_name>`.
///
/// Reads `claude_json` (missing file = empty object), ensures the nested
/// `projects.<project_key>` entry, writes the server entry under that entry's
/// `mcpServers` map, and persists only when the content changed. Returns
/// whether a change occurred.
fn set_mcp_local_scope(
    claude_json: &Path,
    project_key: &str,
    server_name: &str,
    entry: &McpServerEntry,
    extras: &BTreeMap<String, serde_json::Value>,
) -> Result<bool, RegistryError> {
    let mut root = settings::read_json(claude_json)?;
    let entry_value = mcp_config::ensure_project_entry(&mut root, project_key);
    let changed =
        mcp_config::set_mcp_server_entry(entry_value, "mcpServers", server_name, entry, extras)?;
    if changed {
        settings::write_json(claude_json, &root)?;
    }
    Ok(changed)
}

/// Remove an MCP server from Claude Code's local scope and prune the
/// now-empty `mcpServers` object.
///
/// Returns `Ok(false)` without touching the filesystem when `claude_json`
/// does not exist. Otherwise removes
/// `projects.<project_key>.mcpServers.<server_name>` and — when that empties
/// `mcpServers` — deletes the `mcpServers` key so a dangling empty map is not
/// left behind, preserving any sibling keys under the project entry. Writes
/// back only when something changed.
fn remove_mcp_local_scope(
    claude_json: &Path,
    project_key: &str,
    server_name: &str,
) -> Result<bool, RegistryError> {
    if !claude_json.exists() {
        return Ok(false);
    }
    let mut root = settings::read_json(claude_json)?;
    let changed = prune_local_scope_entry(&mut root, project_key, server_name);
    if changed {
        settings::write_json(claude_json, &root)?;
    }
    Ok(changed)
}

/// Remove `projects.<key>.mcpServers.<server_name>` from `root` and prune the
/// `mcpServers` map when it becomes empty. Returns `true` when the server
/// entry was found and removed.
fn prune_local_scope_entry(root: &mut serde_json::Value, key: &str, server_name: &str) -> bool {
    let entry = match root.get_mut("projects").and_then(|p| p.get_mut(key)) {
        Some(e) => e,
        None => return false,
    };

    let changed = mcp_config::remove_mcp_server_entry(entry, "mcpServers", server_name);

    let should_remove = entry
        .get("mcpServers")
        .and_then(|m| m.as_object())
        .map(|m| m.is_empty())
        .unwrap_or(false);
    if should_remove {
        if let Some(obj) = entry.as_object_mut() {
            obj.remove("mcpServers");
        }
    }

    changed
}

// ── Generic MCP JSON ─────────────────────────────────────────────────

/// Strategy for agents that store MCP servers in a JSON/TOML config using the
/// declarative `mcp_config` fields (servers_key, entry_extras, project_path,
/// global_path) and have no permission or special local-scope mechanism.
pub struct GenericMcpJsonStrategy;

impl AgentConfigStrategy for GenericMcpJsonStrategy {
    fn register_mcp(
        &self,
        agent: &AgentDef,
        scope: InitScope,
        server_name: &str,
        entry: &McpServerEntry,
    ) -> Result<bool, RegistryError> {
        generic_register_mcp(agent, scope, server_name, entry)
    }

    fn unregister_mcp(
        &self,
        agent: &AgentDef,
        scope: InitScope,
        server_name: &str,
    ) -> Result<bool, RegistryError> {
        generic_unregister_mcp(agent, scope, server_name)
    }

    /// Generic agents have no permission mechanism — denying a tool is a no-op.
    fn deny_tool(
        &self,
        _agent: &AgentDef,
        _scope: InitScope,
        _tool: &str,
    ) -> Result<bool, RegistryError> {
        Ok(false)
    }

    /// Generic agents have no permission mechanism — allowing a tool is a no-op.
    fn allow_tool(
        &self,
        _agent: &AgentDef,
        _scope: InitScope,
        _tool: &str,
    ) -> Result<bool, RegistryError> {
        Ok(false)
    }
}

/// Resolve the MCP config file for `agent` at `scope` (User → global path,
/// Project/Local → project path), returning `None` when the agent declares no
/// MCP config for that scope.
fn agent_mcp_config_path(agent: &AgentDef, scope: InitScope) -> Option<PathBuf> {
    if matches!(scope, InitScope::User) {
        agents::agent_global_mcp_config(agent)
    } else {
        agents::agent_project_mcp_config(agent)
    }
}

/// Register `entry` into the agent's MCP JSON config for `scope` using the
/// agent's declared `servers_key` and `entry_extras`. No-op (Ok(false)) when
/// the agent has no MCP config or no config path for the scope.
fn generic_register_mcp(
    agent: &AgentDef,
    scope: InitScope,
    server_name: &str,
    entry: &McpServerEntry,
) -> Result<bool, RegistryError> {
    let Some(mcp_cfg) = agent.mcp_config.as_ref() else {
        return Ok(false);
    };
    let Some(path) = agent_mcp_config_path(agent, scope) else {
        return Ok(false);
    };
    let mut root = settings::read_json(&path)?;
    let changed = mcp_config::set_mcp_server_entry(
        &mut root,
        &mcp_cfg.servers_key,
        server_name,
        entry,
        &mcp_cfg.entry_extras,
    )?;
    if changed {
        settings::write_json(&path, &root)?;
    }
    Ok(changed)
}

/// Remove `server_name` from the agent's MCP JSON config for `scope`. No-op
/// (Ok(false)) when the agent has no MCP config, no path, or the file is
/// absent.
fn generic_unregister_mcp(
    agent: &AgentDef,
    scope: InitScope,
    server_name: &str,
) -> Result<bool, RegistryError> {
    let Some(mcp_cfg) = agent.mcp_config.as_ref() else {
        return Ok(false);
    };
    let Some(path) = agent_mcp_config_path(agent, scope) else {
        return Ok(false);
    };
    mcp_config::unregister_mcp_server(&path, &mcp_cfg.servers_key, server_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a Claude-like AgentDef whose MCP/settings paths live under `root`.
    fn claude_agent(root: &Path) -> AgentDef {
        AgentDef {
            id: "claude-code".to_string(),
            name: "Claude Code".to_string(),
            project_path: ".claude/skills".to_string(),
            global_path: "~/.claude/skills".to_string(),
            detect: vec![],
            symlink_policy: agents::SymlinkPolicy::default(),
            mcp_config: Some(agents::McpConfigDef {
                project_path: root.join(".mcp.json").to_string_lossy().to_string(),
                global_path: Some(
                    root.join("global.claude.json")
                        .to_string_lossy()
                        .to_string(),
                ),
                servers_key: "mcpServers".to_string(),
                entry_extras: BTreeMap::new(),
            }),
            plugin_path: None,
            global_plugin_path: None,
            agent_path: None,
            global_agent_path: None,
            instructions_path: None,
            global_instructions_path: None,
            settings_path: Some(
                root.join(".claude/settings.json")
                    .to_string_lossy()
                    .to_string(),
            ),
            global_settings_path: Some(
                root.join("global-settings.json")
                    .to_string_lossy()
                    .to_string(),
            ),
            doctor: true,
        }
    }

    /// Build a generic (Zed-like) AgentDef using only declarative mcp_config.
    fn generic_agent(root: &Path) -> AgentDef {
        let mut extras = BTreeMap::new();
        extras.insert(
            "source".to_string(),
            serde_json::Value::String("custom".to_string()),
        );
        AgentDef {
            id: "zed-ai".to_string(),
            name: "Zed".to_string(),
            project_path: ".zed/skills".to_string(),
            global_path: "~/.zed/skills".to_string(),
            detect: vec![],
            symlink_policy: agents::SymlinkPolicy::default(),
            mcp_config: Some(agents::McpConfigDef {
                project_path: root
                    .join(".zed/settings.json")
                    .to_string_lossy()
                    .to_string(),
                global_path: None,
                servers_key: "context_servers".to_string(),
                entry_extras: extras,
            }),
            plugin_path: None,
            global_plugin_path: None,
            agent_path: None,
            global_agent_path: None,
            instructions_path: None,
            global_instructions_path: None,
            settings_path: None,
            global_settings_path: None,
            doctor: true,
        }
    }

    fn entry() -> McpServerEntry {
        McpServerEntry {
            command: "sah".to_string(),
            args: vec!["serve".to_string()],
            env: BTreeMap::new(),
        }
    }

    fn read(path: &Path) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn strategy_for_dispatches_on_id() {
        let dir = tempfile::tempdir().unwrap();
        // Behavior, not type: only ClaudeCodeStrategy denies a tool.
        let claude = claude_agent(dir.path());
        let generic = generic_agent(dir.path());
        assert!(strategy_for(&claude)
            .deny_tool(&claude, InitScope::Project, "Bash")
            .unwrap());
        assert!(!strategy_for(&generic)
            .deny_tool(&generic, InitScope::Project, "Bash")
            .unwrap());
    }

    #[test]
    fn claude_register_mcp_project_writes_mcp_json() {
        let dir = tempfile::tempdir().unwrap();
        let agent = claude_agent(dir.path());
        let changed = ClaudeCodeStrategy
            .register_mcp(&agent, InitScope::Project, "sah", &entry())
            .unwrap();
        assert!(changed);
        let json = read(&dir.path().join(".mcp.json"));
        assert_eq!(json["mcpServers"]["sah"]["command"], "sah");
    }

    #[test]
    fn claude_register_mcp_user_writes_global_claude_json() {
        let dir = tempfile::tempdir().unwrap();
        let agent = claude_agent(dir.path());
        ClaudeCodeStrategy
            .register_mcp(&agent, InitScope::User, "sah", &entry())
            .unwrap();
        let json = read(&dir.path().join("global.claude.json"));
        assert_eq!(json["mcpServers"]["sah"]["command"], "sah");
    }

    #[test]
    fn claude_unregister_mcp_project_removes_entry() {
        let dir = tempfile::tempdir().unwrap();
        let agent = claude_agent(dir.path());
        ClaudeCodeStrategy
            .register_mcp(&agent, InitScope::Project, "sah", &entry())
            .unwrap();
        let changed = ClaudeCodeStrategy
            .unregister_mcp(&agent, InitScope::Project, "sah")
            .unwrap();
        assert!(changed);
        let json = read(&dir.path().join(".mcp.json"));
        assert!(json["mcpServers"]["sah"].is_null());
    }

    #[test]
    fn claude_deny_and_allow_tool_project() {
        let dir = tempfile::tempdir().unwrap();
        let agent = claude_agent(dir.path());
        let settings_path = dir.path().join(".claude/settings.json");

        assert!(ClaudeCodeStrategy
            .deny_tool(&agent, InitScope::Project, "Bash")
            .unwrap());
        let json = read(&settings_path);
        assert!(json["permissions"]["deny"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "Bash"));

        // Idempotent deny: no change the second time.
        assert!(!ClaudeCodeStrategy
            .deny_tool(&agent, InitScope::Project, "Bash")
            .unwrap());

        assert!(ClaudeCodeStrategy
            .allow_tool(&agent, InitScope::Project, "Bash")
            .unwrap());
        let json = read(&settings_path);
        assert!(!json["permissions"]["deny"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "Bash"));
    }

    #[test]
    fn claude_deny_tool_user_writes_global_settings() {
        let dir = tempfile::tempdir().unwrap();
        let agent = claude_agent(dir.path());
        ClaudeCodeStrategy
            .deny_tool(&agent, InitScope::User, "Bash")
            .unwrap();
        let json = read(&dir.path().join("global-settings.json"));
        assert!(json["permissions"]["deny"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "Bash"));
    }

    #[test]
    fn claude_deny_tool_local_writes_settings_local_json() {
        let dir = tempfile::tempdir().unwrap();
        let agent = claude_agent(dir.path());
        ClaudeCodeStrategy
            .deny_tool(&agent, InitScope::Local, "Bash")
            .unwrap();
        // Local denies live in the settings.local.json sibling, not settings.json.
        let local = dir.path().join(".claude/settings.local.json");
        assert!(local.exists(), "local settings file should be written");
        assert!(
            !dir.path().join(".claude/settings.json").exists(),
            "project settings.json must not be touched by local-scope deny"
        );
        let json = read(&local);
        assert!(json["permissions"]["deny"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "Bash"));
    }

    #[test]
    fn claude_allow_tool_missing_file_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let agent = claude_agent(dir.path());
        assert!(!ClaudeCodeStrategy
            .allow_tool(&agent, InitScope::Project, "Bash")
            .unwrap());
        assert!(!dir.path().join(".claude/settings.json").exists());
    }

    #[test]
    fn claude_local_unregister_prunes_empty_mcp_servers() {
        let dir = tempfile::tempdir().unwrap();
        let claude_json = dir.path().join(".claude.json");
        set_mcp_local_scope(
            &claude_json,
            "/abs/project",
            "sah",
            &entry(),
            &BTreeMap::new(),
        )
        .unwrap();

        let changed = remove_mcp_local_scope(&claude_json, "/abs/project", "sah").unwrap();
        assert!(changed);
        let json = read(&claude_json);
        assert!(
            json["projects"]["/abs/project"]
                .as_object()
                .unwrap()
                .get("mcpServers")
                .is_none(),
            "empty mcpServers should be pruned: {json}"
        );
        assert!(json["projects"]["/abs/project"].is_object());
    }

    #[test]
    fn generic_register_uses_yaml_fields_and_extras() {
        let dir = tempfile::tempdir().unwrap();
        let agent = generic_agent(dir.path());
        let changed = GenericMcpJsonStrategy
            .register_mcp(&agent, InitScope::Project, "sah", &entry())
            .unwrap();
        assert!(changed);
        let json = read(&dir.path().join(".zed/settings.json"));
        // servers_key and entry_extras from the YAML def are honored.
        assert_eq!(json["context_servers"]["sah"]["command"], "sah");
        assert_eq!(json["context_servers"]["sah"]["source"], "custom");
    }

    #[test]
    fn generic_unregister_removes_entry() {
        let dir = tempfile::tempdir().unwrap();
        let agent = generic_agent(dir.path());
        GenericMcpJsonStrategy
            .register_mcp(&agent, InitScope::Project, "sah", &entry())
            .unwrap();
        let changed = GenericMcpJsonStrategy
            .unregister_mcp(&agent, InitScope::Project, "sah")
            .unwrap();
        assert!(changed);
        let json = read(&dir.path().join(".zed/settings.json"));
        assert!(json["context_servers"]["sah"].is_null());
    }

    #[test]
    fn generic_deny_and_allow_are_noops() {
        let dir = tempfile::tempdir().unwrap();
        let agent = generic_agent(dir.path());
        assert!(!GenericMcpJsonStrategy
            .deny_tool(&agent, InitScope::Project, "Bash")
            .unwrap());
        assert!(!GenericMcpJsonStrategy
            .allow_tool(&agent, InitScope::Project, "Bash")
            .unwrap());
    }
}
