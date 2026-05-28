//! Agent-agnostic install-status detection.
//!
//! This module answers a single question for any detected agent and scope:
//! "is this sah-managed thing installed?" It is the capability the doctor (and
//! optionally `mirdan` itself) consumes instead of hand-coding Claude-specific
//! path checks.
//!
//! The detection is data-driven: it is keyed off [`AgentDef`] path accessors
//! plus an [`InitScope`], not off N bespoke per-agent checks. To check a new
//! agent, populate its `AgentDef` path fields — no new code is required here.

use std::path::{Path, PathBuf};

use swissarmyhammer_common::lifecycle::InitScope;
use swissarmyhammer_doctor::{Check, CheckStatus};

use crate::agents::{
    self, agent_global_agent_dir, agent_global_instructions_file, agent_global_mcp_config,
    agent_global_settings_file, agent_global_skill_dir, agent_project_agent_dir,
    agent_project_instructions_file, agent_project_mcp_config, agent_project_settings_file,
    agent_project_skill_dir, AgentDef, AgentsConfig,
};
use crate::registry::RegistryError;
use crate::table;

/// The preamble marker that must appear at the top of an agent's instructions
/// file (e.g. Claude Code's `CLAUDE.md`).
///
/// This is the single source of truth for the marker; the CLI re-exports it as
/// `CLAUDE_MD_PREAMBLE` so there is exactly one definition.
pub const PREAMBLE_MARKER: &str = "MANDATORY: load the thoughtful skill";

/// A sah-managed component that can be installed into an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Component {
    /// The sah MCP server registration in the agent's MCP config file.
    Mcp,
    /// The sah skills deployed to the agent's skill directory.
    Skills,
    /// The sah subagents deployed to the agent's agent directory.
    Agents,
    /// The sah preamble marker in the agent's instructions file.
    Preamble,
    /// The sah permission entries in the agent's settings file.
    Permissions,
}

impl Component {
    /// Returns a short, stable human-readable label for the component.
    pub fn label(&self) -> &str {
        match self {
            Component::Mcp => "MCP server",
            Component::Skills => "Skills",
            Component::Agents => "Subagents",
            Component::Preamble => "Preamble",
            Component::Permissions => "Permissions",
        }
    }

    /// All components, in display order.
    pub fn all() -> [Component; 5] {
        [
            Component::Mcp,
            Component::Skills,
            Component::Agents,
            Component::Preamble,
            Component::Permissions,
        ]
    }
}

/// Whether a component is installed for a given agent and scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentState {
    /// The component is present on disk.
    Installed,
    /// The component is supported for this agent/scope but is not present.
    Missing,
    /// The agent does not support this component at this scope (no path defined).
    NotApplicable,
}

impl ComponentState {
    /// Returns a short, stable lowercase label for the state.
    ///
    /// Used both for the human-readable `mirdan status` table and as the
    /// machine-readable value in `mirdan status --json`.
    pub fn label(&self) -> &'static str {
        match self {
            ComponentState::Installed => "installed",
            ComponentState::Missing => "missing",
            ComponentState::NotApplicable => "n/a",
        }
    }
}

/// The status of a single component for a single agent at a single scope.
#[derive(Debug, Clone)]
pub struct ComponentStatus {
    /// The agent's stable id (e.g. `claude-code`).
    pub agent_id: String,
    /// The agent's human-readable name (e.g. `Claude Code`).
    pub agent_name: String,
    /// Which component this status describes.
    pub component: Component,
    /// Which scope this status describes.
    pub scope: InitScope,
    /// The on-disk path the component resolves to, if any.
    pub path: Option<PathBuf>,
    /// The detected state.
    pub state: ComponentState,
    /// A human-readable detail describing what was (or was not) found.
    pub detail: String,
}

/// Resolve the on-disk location for a component at a given scope from an [`AgentDef`].
///
/// `User` maps to the agent's global accessors; `Project` and `Local` map to the
/// project accessors (project and local artifacts share the same on-disk
/// location — they differ only in MCP registration, which is out of scope for
/// path resolution here).
///
/// Returns `None` when the agent does not define a path for that component at
/// that scope, which the detectors interpret as [`ComponentState::NotApplicable`].
pub fn component_path(agent: &AgentDef, component: Component, scope: InitScope) -> Option<PathBuf> {
    let is_user = matches!(scope, InitScope::User);
    match component {
        Component::Mcp => {
            if is_user {
                agent_global_mcp_config(agent)
            } else {
                agent_project_mcp_config(agent)
            }
        }
        Component::Skills => Some(if is_user {
            agent_global_skill_dir(agent)
        } else {
            agent_project_skill_dir(agent)
        }),
        Component::Agents => {
            if is_user {
                agent_global_agent_dir(agent)
            } else {
                agent_project_agent_dir(agent)
            }
        }
        Component::Preamble => {
            if is_user {
                agent_global_instructions_file(agent)
            } else {
                agent_project_instructions_file(agent)
            }
        }
        Component::Permissions => {
            if is_user {
                agent_global_settings_file(agent)
            } else {
                agent_project_settings_file(agent)
            }
        }
    }
}

/// Check a single component for a single agent at a single scope.
///
/// Returns a [`ComponentStatus`] whose state is [`ComponentState::NotApplicable`]
/// when the agent defines no path for the component, and otherwise
/// [`ComponentState::Installed`] or [`ComponentState::Missing`] per the
/// component-specific detection rules.
pub fn check_component(
    agent: &AgentDef,
    component: Component,
    scope: InitScope,
) -> ComponentStatus {
    let path = component_path(agent, component, scope);

    let (state, detail) = match &path {
        None => (
            ComponentState::NotApplicable,
            format!(
                "{} not supported for this agent at this scope",
                component.label()
            ),
        ),
        Some(path) => {
            let servers_key = agent.mcp_config.as_ref().map(|c| c.servers_key.as_str());
            detect_component(component, path, servers_key)
        }
    };

    ComponentStatus {
        agent_id: agent.id.clone(),
        agent_name: agent.name.clone(),
        component,
        scope,
        path,
        state,
        detail,
    }
}

/// Run the component-specific detection for a resolved path.
///
/// Returns the detected state plus a human-readable detail. `servers_key`
/// is the agent's configured MCP `servers_key` (e.g. Zed's
/// `"context_servers"`) and is consulted only for the [`Component::Mcp`]
/// branch; the other components ignore it.
fn detect_component(
    component: Component,
    path: &Path,
    servers_key: Option<&str>,
) -> (ComponentState, String) {
    let installed = match component {
        Component::Mcp => mcp_server_installed(path, servers_key),
        Component::Skills | Component::Agents => dir_non_empty(path),
        Component::Preamble => preamble_present(path),
        Component::Permissions => permissions_present(path),
    };

    let detail = if installed {
        format!("found at {}", path.display())
    } else {
        format!("missing at {}", path.display())
    };
    let state = if installed {
        ComponentState::Installed
    } else {
        ComponentState::Missing
    };
    (state, detail)
}

/// Check all five components for one agent at one scope.
pub fn check_agent(agent: &AgentDef, scope: InitScope) -> Vec<ComponentStatus> {
    Component::all()
        .into_iter()
        .map(|component| check_component(agent, component, scope))
        .collect()
}

/// Check every component for every detected agent across the given scopes.
///
/// Produces one [`ComponentStatus`] per (detected agent, scope, component).
/// Detected agents are resolved via [`agents::get_detected_agents`], which falls
/// back to `claude-code` when nothing is detected.
pub fn check_all(config: &AgentsConfig, scopes: &[InitScope]) -> Vec<ComponentStatus> {
    let detected = agents::get_detected_agents(config);
    let mut out = Vec::with_capacity(detected.len() * scopes.len() * Component::all().len());
    for agent in &detected {
        for &scope in scopes {
            out.extend(check_agent(&agent.def, scope));
        }
    }
    out
}

/// Check every component for every detected, doctor-enabled agent across the
/// given scopes.
///
/// The contract: this is the install-stack capability that `sah doctor` and
/// `mirdan doctor` consume. It mirrors [`check_all`] but filters the detected
/// agents to those whose [`AgentDef::doctor`] is `true` — i.e. the agents that
/// opt in via the `doctor: true` field in `agents_default.yaml`. The YAML is
/// the single source of truth; this function deliberately does not hard-code
/// any id list.
///
/// `mirdan status` (the table command) intentionally uses [`check_all`]
/// instead: that view is "where are the packages installed across every
/// detected agent" and is not gated by the doctor opt-in.
pub fn check_all_doctored(config: &AgentsConfig, scopes: &[InitScope]) -> Vec<ComponentStatus> {
    let detected = agents::get_detected_agents(config);
    let doctored: Vec<_> = detected.into_iter().filter(|a| a.def.doctor).collect();
    let mut out = Vec::with_capacity(doctored.len() * scopes.len() * Component::all().len());
    for agent in &doctored {
        for &scope in scopes {
            out.extend(check_agent(&agent.def, scope));
        }
    }
    out
}

/// Map a [`ComponentStatus`] into a doctor [`Check`].
///
/// - [`ComponentState::Installed`] → [`CheckStatus::Ok`], no fix.
/// - [`ComponentState::Missing`] → [`CheckStatus::Warning`] with a `sah init` /
///   `sah init user` fix hint derived from the scope.
/// - [`ComponentState::NotApplicable`] → [`CheckStatus::Ok`]; callers decide
///   whether to surface these.
pub fn to_check(status: &ComponentStatus) -> Check {
    let name = format!(
        "{} · {} · {}",
        status.agent_name,
        scope_label(status.scope),
        status.component.label()
    );

    match status.state {
        ComponentState::Installed => Check {
            name,
            status: CheckStatus::Ok,
            message: status.detail.clone(),
            fix: None,
        },
        ComponentState::Missing => Check {
            name,
            status: CheckStatus::Warning,
            message: status.detail.clone(),
            fix: Some(format!("Run `{}` to install", init_command(status.scope))),
        },
        ComponentState::NotApplicable => Check {
            name,
            status: CheckStatus::Ok,
            message: status.detail.clone(),
            fix: None,
        },
    }
}

/// Convert a slice of [`ComponentStatus`] into [`Check`]s, applying the
/// scope-pair policy.
///
/// Statuses are grouped by `(agent_id, component)` so the per-scope rows for
/// the same component can see each other. For each group:
///
/// - Both scopes `Installed` → both rows `Ok` (same as [`to_check`]).
/// - Both scopes `Missing` → both rows `Warning` with `sah init` / `sah init
///   user` fix hints (same as [`to_check`]).
/// - One scope `Installed`, the other `Missing` → the installed-scope row is
///   `Ok` as today; the missing-scope row is demoted to `Ok` with `fix: None`
///   and a message that names where the component was found.
///
/// [`ComponentState::NotApplicable`] statuses are filtered at this layer and
/// produce no [`Check`] — callers receive only actionable rows.
pub fn statuses_to_checks(statuses: &[ComponentStatus]) -> Vec<Check> {
    let mut out = Vec::with_capacity(statuses.len());
    for status in statuses {
        if status.state == ComponentState::NotApplicable {
            continue;
        }
        if status.state == ComponentState::Missing {
            if let Some(peer) = find_installed_peer(statuses, status) {
                out.push(demoted_missing_check(status, peer));
                continue;
            }
        }
        out.push(to_check(status));
    }
    out
}

/// Find an `Installed` peer for `status`: same `agent_id` and `component`,
/// different `scope`.
///
/// Returns `None` when no peer scope has the component installed.
fn find_installed_peer<'a>(
    statuses: &'a [ComponentStatus],
    status: &ComponentStatus,
) -> Option<&'a ComponentStatus> {
    statuses.iter().find(|other| {
        other.agent_id == status.agent_id
            && other.component == status.component
            && other.scope != status.scope
            && other.state == ComponentState::Installed
    })
}

/// Build the demoted [`Check`] for a `Missing` row when a peer scope has the
/// component installed.
///
/// The check is `Ok` with `fix: None` and a message that names both the missing
/// path and the installed peer's scope and path.
fn demoted_missing_check(status: &ComponentStatus, peer: &ComponentStatus) -> Check {
    let name = format!(
        "{} · {} · {}",
        status.agent_name,
        scope_label(status.scope),
        status.component.label()
    );
    let peer_path = peer
        .path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<unknown path>".to_string());
    let message = format!(
        "{}; installed at {} scope ({})",
        status.detail,
        scope_label(peer.scope),
        peer_path,
    );
    Check {
        name,
        status: CheckStatus::Ok,
        message,
        fix: None,
    }
}

/// Human-readable label for a scope, used in check names and the status table.
fn scope_label(scope: InitScope) -> &'static str {
    match scope {
        InitScope::Project => "project",
        InitScope::Local => "local",
        InitScope::User => "user",
    }
}

/// The scopes `mirdan status` and the doctor install-stack check report on.
///
/// Project and user are the two scopes a sah install writes to; `Local` differs
/// only in MCP registration and shares on-disk paths with `Project`, so it is
/// omitted here to avoid duplicate rows.
const STATUS_SCOPES: [InitScope; 2] = [InitScope::Project, InitScope::User];

/// Serialize a slice of [`ComponentStatus`] into the `mirdan status --json` shape.
///
/// The output is an object with a `components` array — one entry per status —
/// plus a `total` count. Each entry carries the agent id/name, scope, component
/// label, state label, path (or null), and detail. This mirrors the structure of
/// [`run_agents`](crate::agents::run_agents)'s JSON output.
///
/// Kept separate from [`run_status`] so the JSON shape can be tested against a
/// synthetic config without touching the live filesystem.
///
/// Accepts any iterator of borrowed [`ComponentStatus`] so callers that already
/// hold a `Vec<&ComponentStatus>` (the `--json` branch of [`run_status`]) can
/// pass references without cloning, while a `&[ComponentStatus]` slice is
/// equally accepted.
pub fn status_json<'a>(
    statuses: impl IntoIterator<Item = &'a ComponentStatus>,
) -> serde_json::Value {
    let components: Vec<serde_json::Value> = statuses
        .into_iter()
        .map(|s| {
            serde_json::json!({
                "agent_id": s.agent_id,
                "agent_name": s.agent_name,
                "scope": scope_label(s.scope),
                "component": s.component.label(),
                "state": s.state.label(),
                "path": s.path.as_ref().map(|p| p.display().to_string()),
                "detail": s.detail,
            })
        })
        .collect();

    let total = components.len();
    serde_json::json!({
        "components": components,
        "total": total,
    })
}

/// Run the `mirdan status` command.
///
/// Detects the install-status of every sah-managed component for every detected
/// agent across the project and user scopes, then prints either a table (Agent /
/// Scope / Component / State / Path) or, when `json` is set, the structured JSON
/// from [`status_json`].
///
/// When `all` is false, [`ComponentState::NotApplicable`] rows are hidden — they
/// describe components an agent simply does not support at a scope and are noise
/// in the common case. `--all` surfaces them.
pub fn run_status(all: bool, json: bool) -> Result<(), RegistryError> {
    let config = agents::load_agents_config()?;
    let statuses = check_all(&config, &STATUS_SCOPES);

    let visible: Vec<&ComponentStatus> = statuses
        .iter()
        .filter(|s| all || s.state != ComponentState::NotApplicable)
        .collect();

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&status_json(visible.iter().copied())).unwrap()
        );
        return Ok(());
    }

    println!("Install Status:\n");

    let mut tbl = table::new_table();
    tbl.set_header(vec!["Agent", "Scope", "Component", "State", "Path"]);

    for status in &visible {
        let path = status
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        tbl.add_row(vec![
            status.agent_name.clone(),
            scope_label(status.scope).to_string(),
            status.component.label().to_string(),
            status.state.label().to_string(),
            path,
        ]);
    }

    println!("{tbl}");

    Ok(())
}

/// The `sah init` command that installs components for a given scope.
fn init_command(scope: InitScope) -> &'static str {
    match scope {
        InitScope::User => "sah init user",
        InitScope::Project | InitScope::Local => "sah init",
    }
}

/// True when the MCP config at `path` registers the sah server.
///
/// Installed when a `sah` entry exists under either the agent's configured
/// `servers_key` (when one is supplied) or one of the hardcoded fallback
/// keys, and its `command` is `sah` or ends with `/sah`. The fallback list —
/// `["mcpServers", "servers", "mcp_servers"]` — is always probed in addition
/// to `servers_key` so legacy configs (and agents whose `AgentDef` predates
/// the `servers_key` field) still detect correctly.
///
/// `servers_key` is the JSON key under which the agent stores its MCP
/// servers map. For agents like Zed this is `"context_servers"`, which the
/// hardcoded list does not cover. Pass `None` when no agent-specific key is
/// known (e.g. detection against a synthetic path with no agent context).
///
/// Supports both JSON and TOML — for files with a `.toml` extension the
/// content is parsed as TOML and converted to a `serde_json::Value` so the
/// downstream probing is identical to the JSON case.
///
/// This is the **single source of truth** for "is the sah MCP server installed
/// at this path?" and is consumed by both `mirdan::status` and the sah-cli
/// install layer so detection and installation cannot drift.
pub fn mcp_server_installed(path: &Path, servers_key: Option<&str>) -> bool {
    let Some(root) = read_config_doc(path) else {
        return false;
    };
    // Probe the agent's configured servers_key first when known. We then fall
    // back to the conventional keys — `"mcpServers"` (the JSON default),
    // `"servers"` (vscode-style), and `"mcp_servers"` (Codex's TOML
    // convention) — so legacy installs and agents whose definition lacks an
    // `mcp_config` still detect.
    let mut keys: Vec<&str> = Vec::with_capacity(4);
    if let Some(key) = servers_key {
        keys.push(key);
    }
    for fallback in ["mcpServers", "servers", "mcp_servers"] {
        if !keys.contains(&fallback) {
            keys.push(fallback);
        }
    }
    for key in keys {
        if let Some(server) = root.get(key).and_then(|s| s.get("sah")) {
            if is_sah_command(server) {
                return true;
            }
        }
    }
    false
}

/// True when an MCP server entry's `command` is `sah` or ends with `/sah`.
fn is_sah_command(server: &serde_json::Value) -> bool {
    server
        .get("command")
        .and_then(|c| c.as_str())
        .is_some_and(|cmd| cmd == "sah" || cmd.ends_with("/sah"))
}

/// True when `path` is a directory that exists and contains at least one entry.
fn dir_non_empty(path: &Path) -> bool {
    std::fs::read_dir(path)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false)
}

/// True when the instructions file at `path` exists and its first non-empty
/// line contains [`PREAMBLE_MARKER`].
///
/// This is the **single source of truth** for "is the sah preamble present at
/// this path?" and is consumed by both `mirdan::status` and the sah-cli install
/// layer so detection and installation cannot drift.
pub fn preamble_present(path: &Path) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    preamble_present_in(&content)
}

/// True when `content`'s first non-empty line contains [`PREAMBLE_MARKER`].
///
/// Companion to [`preamble_present`] for callers that have already read the
/// file (e.g. the install layer's `ensure`/`merge` paths, which need both the
/// detection result and the original content). Keeping a single string-based
/// predicate avoids reading the file twice and guarantees the install layer
/// and `mirdan status` apply identical "is the preamble there?" logic.
pub fn preamble_present_in(content: &str) -> bool {
    content
        .lines()
        .find(|l| !l.trim().is_empty())
        .is_some_and(|line| line.contains(PREAMBLE_MARKER))
}

/// True when the settings JSON at `path` lists `"Bash"` under `permissions.deny`.
///
/// This is the **single source of truth** for "is the sah Bash-deny permission
/// installed at this path?" and is consumed by both `mirdan::status` and the
/// sah-cli install layer so detection and installation cannot drift.
pub fn permissions_present(path: &Path) -> bool {
    let Some(root) = read_json(path) else {
        return false;
    };
    root.get("permissions")
        .and_then(|p| p.get("deny"))
        .and_then(|d| d.as_array())
        .is_some_and(|deny| deny.iter().filter_map(|v| v.as_str()).any(|s| s == "Bash"))
}

/// Read and parse JSON at `path`, returning `None` on any error or missing file.
///
/// Accepts JSONC (comments and trailing commas) so detection of an agent's
/// installed components mirrors the lenient input format we accept on install.
fn read_json(path: &Path) -> Option<serde_json::Value> {
    let content = std::fs::read_to_string(path).ok()?;
    crate::parse_jsonc(&content).ok()
}

/// Read and parse an MCP config document at `path` as a `serde_json::Value`.
///
/// Picks the parser from the file extension: `.toml` paths are parsed as TOML
/// and converted to a `serde_json::Value` so downstream probing (the
/// `mcpServers.sah.command` walk) is identical regardless of input format;
/// every other extension is parsed as JSONC (JSON with comments and trailing
/// commas) so detection mirrors the lenient input format the installer
/// accepts. Returns `None` for missing files and parse errors so the detector
/// reports `Missing` rather than panicking on malformed user config.
fn read_config_doc(path: &Path) -> Option<serde_json::Value> {
    let content = std::fs::read_to_string(path).ok()?;
    let is_toml = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"));
    if is_toml {
        let value: toml::Value = toml::from_str(&content).ok()?;
        serde_json::to_value(value).ok()
    } else {
        crate::parse_jsonc(&content).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{DetectMethod, SymlinkPolicy};
    use tempfile::TempDir;

    /// Build an `AgentDef` whose every path field points inside `dir`.
    ///
    /// The agent is `claude-code` so it is the fallback detected agent, and its
    /// detect method points at a path that never exists so detection relies on
    /// the fallback.
    fn temp_agent(dir: &Path) -> AgentDef {
        let p = |name: &str| dir.join(name).to_string_lossy().to_string();
        AgentDef {
            id: "claude-code".to_string(),
            name: "Claude Code".to_string(),
            project_path: p("skills"),
            global_path: p("global-skills"),
            detect: vec![DetectMethod::Dir {
                dir: "/nonexistent/path/that/should/not/exist".to_string(),
            }],
            symlink_policy: SymlinkPolicy::default(),
            mcp_config: Some(crate::agents::McpConfigDef {
                project_path: p("mcp.json"),
                global_path: Some(p("global-mcp.json")),
                servers_key: "mcpServers".to_string(),
                entry_extras: std::collections::BTreeMap::new(),
            }),
            plugin_path: None,
            global_plugin_path: None,
            agent_path: Some(p("agents")),
            global_agent_path: Some(p("global-agents")),
            instructions_path: Some(p("CLAUDE.md")),
            global_instructions_path: Some(p("global-CLAUDE.md")),
            settings_path: Some(p("settings.json")),
            global_settings_path: Some(p("global-settings.json")),
            doctor: false,
        }
    }

    /// Build an `AgentDef` with no optional component paths defined.
    fn bare_agent(dir: &Path) -> AgentDef {
        AgentDef {
            id: "bare".to_string(),
            name: "Bare".to_string(),
            project_path: dir.join("skills").to_string_lossy().to_string(),
            global_path: dir.join("global-skills").to_string_lossy().to_string(),
            detect: vec![],
            symlink_policy: SymlinkPolicy::default(),
            mcp_config: None,
            plugin_path: None,
            global_plugin_path: None,
            agent_path: None,
            global_agent_path: None,
            instructions_path: None,
            global_instructions_path: None,
            settings_path: None,
            global_settings_path: None,
            doctor: false,
        }
    }

    fn state_of(agent: &AgentDef, component: Component) -> ComponentState {
        check_component(agent, component, InitScope::Project).state
    }

    #[test]
    fn test_mcp_missing_then_installed() {
        let dir = TempDir::new().unwrap();
        let agent = temp_agent(dir.path());
        assert_eq!(state_of(&agent, Component::Mcp), ComponentState::Missing);

        std::fs::write(
            dir.path().join("mcp.json"),
            r#"{"mcpServers": {"sah": {"command": "sah", "args": ["serve"]}}}"#,
        )
        .unwrap();
        assert_eq!(state_of(&agent, Component::Mcp), ComponentState::Installed);
    }

    #[test]
    fn test_mcp_installed_with_absolute_path_command() {
        let dir = TempDir::new().unwrap();
        let agent = temp_agent(dir.path());
        std::fs::write(
            dir.path().join("mcp.json"),
            r#"{"mcpServers": {"sah": {"command": "/usr/local/bin/sah"}}}"#,
        )
        .unwrap();
        assert_eq!(state_of(&agent, Component::Mcp), ComponentState::Installed);
    }

    #[test]
    fn test_mcp_installed_jsonc_json_branch() {
        // Agents like Zed ship JSONC settings.json files (line comments and
        // trailing commas). The detector must read them via the same lenient
        // parser the installer uses, otherwise install would silently succeed
        // while detection reports Missing.
        let dir = TempDir::new().unwrap();
        let agent = temp_agent(dir.path());
        std::fs::write(
            dir.path().join("mcp.json"),
            "// Zed-style header comment\n{\n  \"mcpServers\": {\n    \"sah\": {\n      \"command\": \"sah\",\n      \"args\": [\"serve\",],\n    },\n  },\n}",
        )
        .unwrap();
        assert_eq!(state_of(&agent, Component::Mcp), ComponentState::Installed);
    }

    #[test]
    fn test_mcp_other_server_is_missing() {
        let dir = TempDir::new().unwrap();
        let agent = temp_agent(dir.path());
        std::fs::write(
            dir.path().join("mcp.json"),
            r#"{"mcpServers": {"other": {"command": "node"}}}"#,
        )
        .unwrap();
        assert_eq!(state_of(&agent, Component::Mcp), ComponentState::Missing);
    }

    #[test]
    fn test_mcp_wrong_command_is_missing() {
        let dir = TempDir::new().unwrap();
        let agent = temp_agent(dir.path());
        std::fs::write(
            dir.path().join("mcp.json"),
            r#"{"mcpServers": {"sah": {"command": "not-sah"}}}"#,
        )
        .unwrap();
        assert_eq!(state_of(&agent, Component::Mcp), ComponentState::Missing);
    }

    #[test]
    fn test_mcp_installed_respects_servers_key() {
        // Regression for the Zed detection bug: when an agent declares
        // `servers_key: context_servers`, an installed entry under that key
        // must be detected even though it is not in the hardcoded fallback
        // list (`mcpServers`, `servers`, `mcp_servers`).
        let dir = TempDir::new().unwrap();
        let mut agent = temp_agent(dir.path());
        agent.mcp_config.as_mut().unwrap().servers_key = "context_servers".to_string();
        std::fs::write(
            dir.path().join("mcp.json"),
            r#"{"context_servers": {"sah": {"command": "sah", "source": "custom"}}}"#,
        )
        .unwrap();
        assert_eq!(state_of(&agent, Component::Mcp), ComponentState::Installed);
    }

    #[test]
    fn test_mcp_installed_falls_back_to_default_keys() {
        // When no servers_key is provided (or the agent's key is the legacy
        // default), the detector must still find a sah entry under
        // `mcpServers`. This covers legacy JSON configs and agents whose
        // AgentDef predates the entry_extras work.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("mcp.json");
        std::fs::write(
            &path,
            r#"{"mcpServers": {"sah": {"command": "sah", "args": ["serve"]}}}"#,
        )
        .unwrap();
        // Direct call with no servers_key — exercises the fallback path.
        assert!(mcp_server_installed(&path, None));
    }

    #[test]
    fn test_mcp_installed_toml_basic() {
        let dir = TempDir::new().unwrap();
        let mut agent = temp_agent(dir.path());
        // Point the MCP config at a .toml file so the detector recognizes
        // the format from the path extension.
        let toml_path = dir.path().join("config.toml").to_string_lossy().to_string();
        agent.mcp_config.as_mut().unwrap().project_path = toml_path.clone();
        std::fs::write(&toml_path, "[mcp_servers.sah]\ncommand = \"sah\"\n").unwrap();
        assert_eq!(state_of(&agent, Component::Mcp), ComponentState::Installed);
    }

    #[test]
    fn test_mcp_installed_toml_absolute_path() {
        let dir = TempDir::new().unwrap();
        let mut agent = temp_agent(dir.path());
        let toml_path = dir.path().join("config.toml").to_string_lossy().to_string();
        agent.mcp_config.as_mut().unwrap().project_path = toml_path.clone();
        std::fs::write(
            &toml_path,
            "[mcp_servers.sah]\ncommand = \"/usr/local/bin/sah\"\n",
        )
        .unwrap();
        assert_eq!(state_of(&agent, Component::Mcp), ComponentState::Installed);
    }

    #[test]
    fn test_mcp_installed_toml_wrong_command() {
        let dir = TempDir::new().unwrap();
        let mut agent = temp_agent(dir.path());
        let toml_path = dir.path().join("config.toml").to_string_lossy().to_string();
        agent.mcp_config.as_mut().unwrap().project_path = toml_path.clone();
        std::fs::write(&toml_path, "[mcp_servers.sah]\ncommand = \"not-sah\"\n").unwrap();
        assert_eq!(state_of(&agent, Component::Mcp), ComponentState::Missing);
    }

    #[test]
    fn test_mcp_installed_toml_other_server() {
        let dir = TempDir::new().unwrap();
        let mut agent = temp_agent(dir.path());
        let toml_path = dir.path().join("config.toml").to_string_lossy().to_string();
        agent.mcp_config.as_mut().unwrap().project_path = toml_path.clone();
        std::fs::write(&toml_path, "[mcp_servers.other]\ncommand = \"node\"\n").unwrap();
        assert_eq!(state_of(&agent, Component::Mcp), ComponentState::Missing);
    }

    #[test]
    fn test_mcp_installed_toml_malformed_returns_false() {
        let dir = TempDir::new().unwrap();
        let mut agent = temp_agent(dir.path());
        let toml_path = dir.path().join("config.toml").to_string_lossy().to_string();
        agent.mcp_config.as_mut().unwrap().project_path = toml_path.clone();
        // Not valid TOML — an unterminated table header.
        std::fs::write(&toml_path, "[mcp_servers.sah\ncommand = ").unwrap();
        assert_eq!(state_of(&agent, Component::Mcp), ComponentState::Missing);
    }

    #[test]
    fn test_skills_missing_then_installed() {
        let dir = TempDir::new().unwrap();
        let agent = temp_agent(dir.path());
        assert_eq!(state_of(&agent, Component::Skills), ComponentState::Missing);

        let skills = dir.path().join("skills");
        std::fs::create_dir_all(&skills).unwrap();
        // Empty dir is still missing.
        assert_eq!(state_of(&agent, Component::Skills), ComponentState::Missing);

        std::fs::write(skills.join("a-skill"), "x").unwrap();
        assert_eq!(
            state_of(&agent, Component::Skills),
            ComponentState::Installed
        );
    }

    #[test]
    fn test_agents_missing_then_installed() {
        let dir = TempDir::new().unwrap();
        let agent = temp_agent(dir.path());
        assert_eq!(state_of(&agent, Component::Agents), ComponentState::Missing);

        let agent_dir = dir.path().join("agents");
        std::fs::create_dir_all(&agent_dir).unwrap();
        std::fs::write(agent_dir.join("sub"), "x").unwrap();
        assert_eq!(
            state_of(&agent, Component::Agents),
            ComponentState::Installed
        );
    }

    #[test]
    fn test_preamble_missing_then_installed() {
        let dir = TempDir::new().unwrap();
        let agent = temp_agent(dir.path());
        assert_eq!(
            state_of(&agent, Component::Preamble),
            ComponentState::Missing
        );

        let claude_md = dir.path().join("CLAUDE.md");
        // File without the marker is missing.
        std::fs::write(&claude_md, "# notes\n").unwrap();
        assert_eq!(
            state_of(&agent, Component::Preamble),
            ComponentState::Missing
        );

        std::fs::write(&claude_md, format!("\n{}\n\nnotes\n", PREAMBLE_MARKER)).unwrap();
        assert_eq!(
            state_of(&agent, Component::Preamble),
            ComponentState::Installed
        );
    }

    #[test]
    fn test_permissions_missing_then_installed() {
        let dir = TempDir::new().unwrap();
        let agent = temp_agent(dir.path());
        assert_eq!(
            state_of(&agent, Component::Permissions),
            ComponentState::Missing
        );

        let settings = dir.path().join("settings.json");
        std::fs::write(&settings, r#"{"permissions": {"deny": ["Other"]}}"#).unwrap();
        assert_eq!(
            state_of(&agent, Component::Permissions),
            ComponentState::Missing
        );

        std::fs::write(
            &settings,
            r#"{"permissions": {"deny": ["Bash", "WebFetch"]}}"#,
        )
        .unwrap();
        assert_eq!(
            state_of(&agent, Component::Permissions),
            ComponentState::Installed
        );
    }

    #[test]
    fn test_not_applicable_when_path_field_none() {
        let dir = TempDir::new().unwrap();
        let agent = bare_agent(dir.path());
        // Skills always resolves a path (project_path/global_path are required),
        // so the remaining four are NotApplicable when the field is None.
        assert_eq!(
            state_of(&agent, Component::Mcp),
            ComponentState::NotApplicable
        );
        assert_eq!(
            state_of(&agent, Component::Agents),
            ComponentState::NotApplicable
        );
        assert_eq!(
            state_of(&agent, Component::Preamble),
            ComponentState::NotApplicable
        );
        assert_eq!(
            state_of(&agent, Component::Permissions),
            ComponentState::NotApplicable
        );
    }

    #[test]
    fn test_component_path_user_vs_project() {
        let dir = TempDir::new().unwrap();
        let agent = temp_agent(dir.path());
        let project = component_path(&agent, Component::Skills, InitScope::Project).unwrap();
        let user = component_path(&agent, Component::Skills, InitScope::User).unwrap();
        assert!(project.ends_with("skills"));
        assert!(user.ends_with("global-skills"));

        // Local maps to project accessors, same as Project.
        let local = component_path(&agent, Component::Skills, InitScope::Local).unwrap();
        assert_eq!(local, project);
    }

    #[test]
    fn test_check_agent_returns_all_components() {
        let dir = TempDir::new().unwrap();
        let agent = temp_agent(dir.path());
        let statuses = check_agent(&agent, InitScope::Project);
        assert_eq!(statuses.len(), 5);
        let components: Vec<Component> = statuses.iter().map(|s| s.component).collect();
        assert_eq!(components, Component::all().to_vec());
    }

    #[test]
    fn test_check_all_covers_agents_scopes_components() {
        let dir = TempDir::new().unwrap();
        // Synthetic config whose only entry is claude-code with a never-matching
        // detect method; get_detected_agents falls back to it.
        let config = AgentsConfig {
            agents: vec![temp_agent(dir.path())],
        };
        let scopes = [InitScope::Project, InitScope::User];
        let statuses = check_all(&config, &scopes);

        // 1 agent × 2 scopes × 5 components.
        assert_eq!(statuses.len(), 10);

        for scope in scopes {
            for component in Component::all() {
                assert!(
                    statuses
                        .iter()
                        .any(|s| s.scope == scope && s.component == component),
                    "missing status for scope {:?} component {:?}",
                    scope,
                    component
                );
            }
        }
        // All rows are for claude-code.
        assert!(statuses.iter().all(|s| s.agent_id == "claude-code"));
    }

    #[test]
    fn test_to_check_maps_installed_to_ok() {
        let dir = TempDir::new().unwrap();
        let agent = temp_agent(dir.path());
        std::fs::write(
            dir.path().join("mcp.json"),
            r#"{"mcpServers": {"sah": {"command": "sah"}}}"#,
        )
        .unwrap();
        let status = check_component(&agent, Component::Mcp, InitScope::Project);
        let check = to_check(&status);
        assert_eq!(check.status, CheckStatus::Ok);
        assert!(check.fix.is_none());
    }

    #[test]
    fn test_to_check_maps_missing_to_warning_with_fix() {
        let dir = TempDir::new().unwrap();
        let agent = temp_agent(dir.path());
        let project = check_component(&agent, Component::Mcp, InitScope::Project);
        let project_check = to_check(&project);
        assert_eq!(project_check.status, CheckStatus::Warning);
        let fix = project_check.fix.expect("missing should carry a fix");
        assert!(!fix.is_empty());
        assert!(fix.contains("sah init"));
        assert!(!fix.contains("sah init user"));

        let user = check_component(&agent, Component::Mcp, InitScope::User);
        let user_check = to_check(&user);
        assert_eq!(user_check.status, CheckStatus::Warning);
        let user_fix = user_check.fix.expect("missing should carry a fix");
        assert!(user_fix.contains("sah init user"));
    }

    #[test]
    fn test_to_check_maps_not_applicable_to_ok() {
        let dir = TempDir::new().unwrap();
        let agent = bare_agent(dir.path());
        let status = check_component(&agent, Component::Mcp, InitScope::Project);
        assert_eq!(status.state, ComponentState::NotApplicable);
        let check = to_check(&status);
        assert_eq!(check.status, CheckStatus::Ok);
        assert!(check.fix.is_none());
    }

    #[test]
    fn test_component_labels_are_non_empty() {
        for component in Component::all() {
            assert!(!component.label().is_empty());
        }
    }

    #[test]
    fn test_component_state_labels() {
        assert_eq!(ComponentState::Installed.label(), "installed");
        assert_eq!(ComponentState::Missing.label(), "missing");
        assert_eq!(ComponentState::NotApplicable.label(), "n/a");
    }

    #[test]
    fn test_status_json_shape_covers_agents_scopes_components() {
        let dir = TempDir::new().unwrap();
        // Synthetic config: claude-code with a never-matching detect method, so
        // get_detected_agents falls back to it. No filesystem state is created,
        // so every applicable component reports "missing".
        let config = AgentsConfig {
            agents: vec![temp_agent(dir.path())],
        };
        let scopes = [InitScope::Project, InitScope::User];
        let statuses = check_all(&config, &scopes);
        let json = status_json(&statuses);

        // Top-level shape: a components array plus a total count.
        assert_eq!(json["total"], 10); // 1 agent × 2 scopes × 5 components
        let components = json["components"].as_array().expect("components array");
        assert_eq!(components.len(), 10);

        // Every (scope, component) pair is represented for the one agent.
        for scope in scopes {
            for component in Component::all() {
                let found = components.iter().any(|c| {
                    c["scope"] == scope_label(scope) && c["component"] == component.label()
                });
                assert!(
                    found,
                    "missing JSON entry for scope {:?} component {:?}",
                    scope, component
                );
            }
        }

        // Each entry carries the full set of fields with the expected types.
        for entry in components {
            assert_eq!(entry["agent_id"], "claude-code");
            assert_eq!(entry["agent_name"], "Claude Code");
            assert!(entry["scope"].is_string());
            assert!(entry["component"].is_string());
            assert!(entry["state"].is_string());
            // Path is a string (these components all resolve a path) and detail
            // is always present.
            assert!(entry["path"].is_string());
            assert!(entry["detail"].is_string());
        }
    }

    #[test]
    fn test_status_json_path_is_null_when_not_applicable() {
        let dir = TempDir::new().unwrap();
        let agent = bare_agent(dir.path());
        // MCP is NotApplicable for a bare agent: no path resolves.
        let status = check_component(&agent, Component::Mcp, InitScope::Project);
        assert_eq!(status.state, ComponentState::NotApplicable);

        let json = status_json(std::slice::from_ref(&status));
        let entry = &json["components"][0];
        assert_eq!(entry["state"], "n/a");
        assert!(entry["path"].is_null());
    }

    /// Build a synthetic `codex` `AgentDef` whose every probed path lands
    /// inside `dir`. This mirrors the `temp_agent` helper but pins the agent
    /// to `codex` so the same shape the real `agents_default.yaml` entry
    /// produces (no `agent_path`, but MCP + Preamble paths set) is exercised
    /// end-to-end.
    fn codex_temp_agent(dir: &Path) -> AgentDef {
        let p = |name: &str| dir.join(name).to_string_lossy().to_string();
        AgentDef {
            id: "codex".to_string(),
            name: "Codex".to_string(),
            project_path: p("skills"),
            global_path: p("global-skills"),
            // detect via a directory inside dir so this agent is detected
            // (and selected by get_detected_agents) without depending on a
            // real $HOME layout.
            detect: vec![DetectMethod::Dir { dir: p("detect") }],
            symlink_policy: SymlinkPolicy::default(),
            mcp_config: Some(crate::agents::McpConfigDef {
                project_path: p("config.toml"),
                global_path: Some(p("global-config.toml")),
                servers_key: "mcp_servers".to_string(),
                entry_extras: std::collections::BTreeMap::new(),
            }),
            plugin_path: None,
            global_plugin_path: None,
            // codex has no subagents directory.
            agent_path: None,
            global_agent_path: None,
            instructions_path: Some(p("AGENTS.md")),
            global_instructions_path: Some(p("global-AGENTS.md")),
            settings_path: None,
            global_settings_path: None,
            doctor: true,
        }
    }

    #[test]
    fn codex_full_stack() {
        let dir = TempDir::new().unwrap();
        // Make the detect dir exist so get_detected_agents picks this entry up.
        std::fs::create_dir_all(dir.path().join("detect")).unwrap();

        let agent = codex_temp_agent(dir.path());
        let config = AgentsConfig {
            agents: vec![agent],
        };

        let scopes = [InitScope::Project, InitScope::User];
        let statuses = check_all(&config, &scopes);

        // With both Preamble and MCP path fields populated for both scopes,
        // the four (component × scope) cells we care about must resolve to a
        // concrete on-disk state (Installed or Missing), never NotApplicable.
        for &component in &[Component::Mcp, Component::Preamble] {
            for &scope in &scopes {
                let status = statuses
                    .iter()
                    .find(|s| s.component == component && s.scope == scope)
                    .unwrap_or_else(|| {
                        panic!(
                            "expected codex status row for component {:?} at scope {:?}",
                            component, scope
                        )
                    });
                assert_ne!(
                    status.state,
                    ComponentState::NotApplicable,
                    "codex {:?} at {:?} must not be NotApplicable",
                    component,
                    scope
                );
            }
        }

        // With nothing on disk yet, both MCP and Preamble report Missing at
        // both scopes — the rows are reachable but the artifacts are not
        // installed.
        let project_mcp = statuses
            .iter()
            .find(|s| s.component == Component::Mcp && s.scope == InitScope::Project)
            .unwrap();
        assert_eq!(project_mcp.state, ComponentState::Missing);
        let user_mcp = statuses
            .iter()
            .find(|s| s.component == Component::Mcp && s.scope == InitScope::User)
            .unwrap();
        assert_eq!(user_mcp.state, ComponentState::Missing);

        let project_preamble = statuses
            .iter()
            .find(|s| s.component == Component::Preamble && s.scope == InitScope::Project)
            .unwrap();
        assert_eq!(project_preamble.state, ComponentState::Missing);
        let user_preamble = statuses
            .iter()
            .find(|s| s.component == Component::Preamble && s.scope == InitScope::User)
            .unwrap();
        assert_eq!(user_preamble.state, ComponentState::Missing);

        // Write a preamble file at the user scope path and re-check: that one
        // cell flips to Installed, proving the row genuinely participates in
        // detection rather than being permanently NotApplicable.
        std::fs::write(
            dir.path().join("global-AGENTS.md"),
            format!("{}\n", PREAMBLE_MARKER),
        )
        .unwrap();
        let statuses = check_all(&config, &scopes);
        let user_preamble = statuses
            .iter()
            .find(|s| s.component == Component::Preamble && s.scope == InitScope::User)
            .unwrap();
        assert_eq!(user_preamble.state, ComponentState::Installed);
    }

    /// `check_all_doctored` must filter agents by `AgentDef.doctor` before
    /// running the per-component sweep. Given a config that contains one
    /// doctor-enabled agent and one doctor-disabled agent (both detectable),
    /// every emitted `ComponentStatus` must belong to the doctor-enabled
    /// agent — the disabled one contributes nothing.
    #[test]
    fn test_check_all_doctored_filters_by_doctor_field() {
        let dir = TempDir::new().unwrap();
        // Make both agents detectable so `get_detected_agents` returns both.
        let detect_a = dir.path().join("detect-a");
        let detect_b = dir.path().join("detect-b");
        std::fs::create_dir_all(&detect_a).unwrap();
        std::fs::create_dir_all(&detect_b).unwrap();

        let p = |sub: &str, name: &str| {
            dir.path()
                .join(sub)
                .join(name)
                .to_string_lossy()
                .to_string()
        };

        // Doctor-enabled agent: claude-code shape, opts in via `doctor: true`.
        let doctored = AgentDef {
            id: "claude-code".to_string(),
            name: "Claude Code".to_string(),
            project_path: p("a", "skills"),
            global_path: p("a", "global-skills"),
            detect: vec![DetectMethod::Dir {
                dir: detect_a.to_string_lossy().to_string(),
            }],
            symlink_policy: SymlinkPolicy::default(),
            mcp_config: None,
            plugin_path: None,
            global_plugin_path: None,
            agent_path: None,
            global_agent_path: None,
            instructions_path: None,
            global_instructions_path: None,
            settings_path: None,
            global_settings_path: None,
            doctor: true,
        };
        // Doctor-disabled agent: also detected, but the install-stack must skip it.
        let undoctored = AgentDef {
            id: "cursor".to_string(),
            name: "Cursor".to_string(),
            project_path: p("b", "skills"),
            global_path: p("b", "global-skills"),
            detect: vec![DetectMethod::Dir {
                dir: detect_b.to_string_lossy().to_string(),
            }],
            symlink_policy: SymlinkPolicy::default(),
            mcp_config: None,
            plugin_path: None,
            global_plugin_path: None,
            agent_path: None,
            global_agent_path: None,
            instructions_path: None,
            global_instructions_path: None,
            settings_path: None,
            global_settings_path: None,
            doctor: false,
        };
        let config = AgentsConfig {
            agents: vec![doctored, undoctored],
        };

        let statuses = check_all_doctored(&config, &[InitScope::Project, InitScope::User]);

        assert!(
            !statuses.is_empty(),
            "expected at least one row for the doctor-enabled agent"
        );
        for status in &statuses {
            assert_eq!(
                status.agent_id, "claude-code",
                "every install-stack row must belong to a doctor: true agent; got '{}'",
                status.agent_id
            );
        }
        assert!(
            statuses.iter().all(|s| s.agent_id != "cursor"),
            "doctor: false agent 'cursor' must not appear in check_all_doctored output"
        );
    }

    /// Build a synthetic `ComponentStatus` for tests of `statuses_to_checks`.
    ///
    /// Lets each test compose the (agent_id, component, scope, state, path)
    /// tuple it needs without going through the filesystem detector.
    fn synthetic_status(
        agent_id: &str,
        component: Component,
        scope: InitScope,
        state: ComponentState,
        path: Option<PathBuf>,
    ) -> ComponentStatus {
        let detail = match (&state, &path) {
            (ComponentState::Installed, Some(p)) => format!("found at {}", p.display()),
            (ComponentState::Missing, Some(p)) => format!("missing at {}", p.display()),
            (ComponentState::NotApplicable, _) => {
                format!(
                    "{} not supported for this agent at this scope",
                    component.label()
                )
            }
            (_, None) => String::new(),
        };
        ComponentStatus {
            agent_id: agent_id.to_string(),
            agent_name: "Claude Code".to_string(),
            component,
            scope,
            path,
            state,
            detail,
        }
    }

    #[test]
    fn test_statuses_to_checks_demotes_project_missing_when_user_installed() {
        let user_path = PathBuf::from("/Users/test/.claude/CLAUDE.md");
        let project_path = PathBuf::from("/work/repo/CLAUDE.md");
        let statuses = vec![
            synthetic_status(
                "claude-code",
                Component::Preamble,
                InitScope::Project,
                ComponentState::Missing,
                Some(project_path.clone()),
            ),
            synthetic_status(
                "claude-code",
                Component::Preamble,
                InitScope::User,
                ComponentState::Installed,
                Some(user_path.clone()),
            ),
        ];

        let checks = statuses_to_checks(&statuses);
        assert_eq!(checks.len(), 2);

        let project_check = checks
            .iter()
            .find(|c| c.name == "Claude Code · project · Preamble")
            .expect("project preamble row");
        assert_eq!(
            project_check.status,
            CheckStatus::Ok,
            "project-missing with user-installed should demote to Ok"
        );
        assert!(
            project_check.fix.is_none(),
            "demoted row should have no fix"
        );
        assert!(
            project_check
                .message
                .contains(&user_path.display().to_string()),
            "message should mention the user-scope path; got: {}",
            project_check.message
        );
        assert!(
            project_check.message.contains("user"),
            "message should name the user scope; got: {}",
            project_check.message
        );

        let user_check = checks
            .iter()
            .find(|c| c.name == "Claude Code · user · Preamble")
            .expect("user preamble row");
        assert_eq!(user_check.status, CheckStatus::Ok);
        assert!(user_check.fix.is_none());
    }

    #[test]
    fn test_statuses_to_checks_demotes_user_missing_when_project_installed() {
        let user_path = PathBuf::from("/Users/test/.claude/CLAUDE.md");
        let project_path = PathBuf::from("/work/repo/CLAUDE.md");
        let statuses = vec![
            synthetic_status(
                "claude-code",
                Component::Preamble,
                InitScope::Project,
                ComponentState::Installed,
                Some(project_path.clone()),
            ),
            synthetic_status(
                "claude-code",
                Component::Preamble,
                InitScope::User,
                ComponentState::Missing,
                Some(user_path.clone()),
            ),
        ];

        let checks = statuses_to_checks(&statuses);
        let user_check = checks
            .iter()
            .find(|c| c.name == "Claude Code · user · Preamble")
            .expect("user preamble row");
        assert_eq!(
            user_check.status,
            CheckStatus::Ok,
            "user-missing with project-installed should demote to Ok"
        );
        assert!(user_check.fix.is_none());
        assert!(
            user_check
                .message
                .contains(&project_path.display().to_string()),
            "message should reference the project path; got: {}",
            user_check.message
        );
        assert!(
            user_check.message.contains("project"),
            "message should name the project scope; got: {}",
            user_check.message
        );
    }

    #[test]
    fn test_statuses_to_checks_both_missing_stays_warning() {
        let statuses = vec![
            synthetic_status(
                "claude-code",
                Component::Preamble,
                InitScope::Project,
                ComponentState::Missing,
                Some(PathBuf::from("/work/repo/CLAUDE.md")),
            ),
            synthetic_status(
                "claude-code",
                Component::Preamble,
                InitScope::User,
                ComponentState::Missing,
                Some(PathBuf::from("/Users/test/.claude/CLAUDE.md")),
            ),
        ];

        let checks = statuses_to_checks(&statuses);
        assert_eq!(checks.len(), 2);

        let project_check = checks
            .iter()
            .find(|c| c.name == "Claude Code · project · Preamble")
            .unwrap();
        assert_eq!(project_check.status, CheckStatus::Warning);
        let project_fix = project_check
            .fix
            .as_ref()
            .expect("missing should carry fix");
        assert!(project_fix.contains("sah init"));
        assert!(!project_fix.contains("sah init user"));

        let user_check = checks
            .iter()
            .find(|c| c.name == "Claude Code · user · Preamble")
            .unwrap();
        assert_eq!(user_check.status, CheckStatus::Warning);
        assert!(user_check
            .fix
            .as_ref()
            .expect("missing should carry fix")
            .contains("sah init user"));
    }

    #[test]
    fn test_statuses_to_checks_both_installed_stays_ok() {
        let statuses = vec![
            synthetic_status(
                "claude-code",
                Component::Preamble,
                InitScope::Project,
                ComponentState::Installed,
                Some(PathBuf::from("/work/repo/CLAUDE.md")),
            ),
            synthetic_status(
                "claude-code",
                Component::Preamble,
                InitScope::User,
                ComponentState::Installed,
                Some(PathBuf::from("/Users/test/.claude/CLAUDE.md")),
            ),
        ];

        let checks = statuses_to_checks(&statuses);
        assert_eq!(checks.len(), 2);
        for check in &checks {
            assert_eq!(check.status, CheckStatus::Ok);
            assert!(check.fix.is_none());
        }
    }

    #[test]
    fn test_statuses_to_checks_filters_not_applicable() {
        let statuses = vec![
            synthetic_status(
                "claude-code",
                Component::Mcp,
                InitScope::Project,
                ComponentState::NotApplicable,
                None,
            ),
            synthetic_status(
                "claude-code",
                Component::Mcp,
                InitScope::User,
                ComponentState::NotApplicable,
                None,
            ),
            synthetic_status(
                "claude-code",
                Component::Preamble,
                InitScope::Project,
                ComponentState::Missing,
                Some(PathBuf::from("/work/repo/CLAUDE.md")),
            ),
        ];

        let checks = statuses_to_checks(&statuses);
        assert_eq!(
            checks.len(),
            1,
            "NotApplicable statuses should produce no checks"
        );
        assert_eq!(checks[0].name, "Claude Code · project · Preamble");
    }

    #[test]
    fn test_status_json_reflects_installed_state() {
        let dir = TempDir::new().unwrap();
        let agent = temp_agent(dir.path());
        std::fs::write(
            dir.path().join("mcp.json"),
            r#"{"mcpServers": {"sah": {"command": "sah"}}}"#,
        )
        .unwrap();
        let status = check_component(&agent, Component::Mcp, InitScope::Project);
        assert_eq!(status.state, ComponentState::Installed);

        let json = status_json(std::slice::from_ref(&status));
        let entry = &json["components"][0];
        assert_eq!(entry["state"], "installed");
        assert_eq!(entry["component"], "MCP server");
        assert_eq!(entry["scope"], "project");
        assert!(entry["path"].as_str().unwrap().ends_with("mcp.json"));
    }
}
