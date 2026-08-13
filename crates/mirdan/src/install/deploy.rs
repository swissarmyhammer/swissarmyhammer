//! Deployment of skills, agents, tools, plugins, and validators to the
//! central stores and each detected agent's directories.

use std::path::{Path, PathBuf};

use crate::agents::{
    self, agent_global_agent_dir, agent_global_skill_dir, agent_project_agent_dir,
    agent_project_skill_dir, AgentDef,
};
use crate::mcp_config::{self, ServersKey, ToolName};
use crate::registry::RegistryError;
use crate::store;

use super::{
    copy_dir_recursive, rooted, sanitize_dir_name, temp_dir_error, temp_subdir_error,
    validators_dir,
};

/// Deploy a skill to the central store, then symlink into each agent's skill directory.
///
/// This is the public, synchronous API. All filesystem operations are sync.
///
/// In project scope the store directory (`.skills/`) and each agent's skill
/// directory are resolved relative to the current working directory. Use
/// [`deploy_skill_to_agents_at`] to root them at an explicit directory instead
/// (e.g. for a long-running process that must not depend on CWD).
pub fn deploy_skill_to_agents(
    name: &str,
    source_dir: &Path,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<Vec<String>, RegistryError> {
    deploy_skill_to_agents_at(name, source_dir, agent_filter, global, None)
}

/// Root-explicit variant of [`deploy_skill_to_agents`].
///
/// When `root` is `Some`, project-scope relative paths (the `.skills/` store and
/// each agent's project skill directory) are joined onto `root` instead of being
/// resolved against the process working directory. When `root` is `None`,
/// behavior is identical to [`deploy_skill_to_agents`]. Global scope ignores
/// `root` because its paths are absolute (`~/.skills`, the agent's
/// tilde-expanded global directory).
pub fn deploy_skill_to_agents_at(
    name: &str,
    source_dir: &Path,
    agent_filter: Option<&str>,
    global: bool,
    root: Option<&Path>,
) -> Result<Vec<String>, RegistryError> {
    deploy_to_agent_dirs(
        name,
        source_dir,
        agent_filter,
        global,
        root,
        store::skill_store_dir,
        |def| Some(agent_global_skill_dir(def)),
        |def| Some(agent_project_skill_dir(def)),
    )
}

/// Deploy through [`deploy_via_store`], resolving the store root and each
/// agent's target directory from the kind's resolvers.
///
/// The single kind-dispatch behind [`deploy_skill_to_agents_at`] and
/// [`deploy_agent_to_agents_at`]: the two differ only in the store root and
/// the global/project directory resolvers.
#[allow(clippy::too_many_arguments)]
fn deploy_to_agent_dirs(
    name: &str,
    source_dir: &Path,
    agent_filter: Option<&str>,
    global: bool,
    root: Option<&Path>,
    store_dir: fn(bool) -> PathBuf,
    global_dir: fn(&AgentDef) -> Option<PathBuf>,
    project_dir: fn(&AgentDef) -> Option<PathBuf>,
) -> Result<Vec<String>, RegistryError> {
    deploy_via_store(
        name,
        source_dir,
        agent_filter,
        rooted(root, global, store_dir(global)),
        |def| {
            if global {
                global_dir(def)
            } else {
                project_dir(def).map(|d| rooted(root, global, d))
            }
        },
    )
}

/// Copy `source_dir` into `store_root/<sanitized name>` and symlink the store
/// entry into the directory `agent_dir` yields for each detected agent.
///
/// The single deployment mechanism behind [`deploy_skill_to_agents_at`] and
/// [`deploy_agent_to_agents_at`]: the two differ only in the store root and in
/// how each agent's target directory resolves. `agent_dir` returns `None` for
/// an agent with no target directory; that agent is skipped.
fn deploy_via_store(
    name: &str,
    source_dir: &Path,
    agent_filter: Option<&str>,
    store_root: PathBuf,
    agent_dir: impl Fn(&AgentDef) -> Option<PathBuf>,
) -> Result<Vec<String>, RegistryError> {
    let config = agents::load_agents_config()?;
    let agents = agents::resolve_target_agents(&config, agent_filter)?;

    if agents.is_empty() {
        return Err(RegistryError::Validation(
            "no agents detected. Run 'mirdan agents' to check".to_string(),
        ));
    }

    // 1. Copy source into the central store
    let sanitized = sanitize_dir_name(name);
    let store_path = store_root.join(&sanitized);

    // Remove existing store entry
    store::remove_if_exists(&store_path)?;

    copy_dir_recursive(source_dir, &store_path)?;
    tracing::debug!("  Stored in {}", store_path.display());

    // 2. Create symlinks from each agent's target directory
    let mut targets = Vec::new();

    for agent in &agents {
        let Some(base_dir) = agent_dir(&agent.def) else {
            tracing::debug!(
                "Agent {} has no agent directory configured, skipping",
                agent.def.id
            );
            continue;
        };
        let link_name = store::symlink_name(&sanitized, &agent.def.symlink_policy);
        let link_path = base_dir.join(&link_name);

        // Remove existing (real dir or stale symlink)
        store::remove_if_exists(&link_path)?;

        store::create_skill_link(&store_path, &link_path)?;
        tracing::debug!(
            "  Linked {} -> {} ({})",
            link_path.display(),
            store_path.display(),
            agent.def.name
        );
        targets.push(agent.def.id.clone());
    }

    Ok(targets)
}

/// Write a rendered SKILL.md to a temp directory and deploy it to all agents.
///
/// Stages `skill_content` as `<tmpdir>/<name>/SKILL.md`, then delegates to
/// [`deploy_skill_to_agents`] to store it centrally and symlink it into every
/// detected agent's skill directory. This is the deployment step that callers
/// reach for after rendering a skill's content with their own template engine;
/// mirdan owns the filesystem staging so `swissarmyhammer-skills` stays
/// deployment-free.
///
/// # Errors
///
/// Returns an error if `name` is not a safe filesystem identifier, the temp
/// directory cannot be created, the file cannot be written, or deployment
/// fails.
pub fn stage_and_deploy_skill(
    name: &str,
    skill_content: &str,
) -> Result<Vec<String>, RegistryError> {
    if !store::is_safe_name(name) {
        return Err(RegistryError::Validation(format!(
            "unsafe skill name: {name:?}"
        )));
    }

    let temp_dir = tempfile::tempdir().map_err(temp_dir_error)?;
    let skill_dir = temp_dir.path().join(name);
    std::fs::create_dir_all(&skill_dir).map_err(|e| temp_subdir_error("skill", e))?;
    std::fs::write(skill_dir.join("SKILL.md"), skill_content)
        .map_err(|e| RegistryError::Validation(format!("failed to write SKILL.md: {e}")))?;

    deploy_skill_to_agents(name, &skill_dir, None, false)
}

/// Deploy an agent (subagent) to the central store, then symlink into each coding agent's agent directory.
///
/// This is the public, synchronous API. Mirrors [`deploy_skill_to_agents`] but uses
/// the `.agents/` store and agent-specific agent directories (e.g. `.claude/agents/`).
pub fn deploy_agent_to_agents(
    name: &str,
    source_dir: &Path,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<Vec<String>, RegistryError> {
    deploy_agent_to_agents_at(name, source_dir, agent_filter, global, None)
}

/// Root-explicit variant of [`deploy_agent_to_agents`].
///
/// Mirrors [`deploy_skill_to_agents_at`]: when `root` is `Some`, project-scope
/// relative paths (the `.agents/` store and each agent's project agent
/// directory) are joined onto `root` instead of the process working directory.
/// `None` preserves CWD-relative behavior; global scope ignores `root`.
pub fn deploy_agent_to_agents_at(
    name: &str,
    source_dir: &Path,
    agent_filter: Option<&str>,
    global: bool,
    root: Option<&Path>,
) -> Result<Vec<String>, RegistryError> {
    deploy_to_agent_dirs(
        name,
        source_dir,
        agent_filter,
        global,
        root,
        store::agent_store_dir,
        agent_global_agent_dir,
        agent_project_agent_dir,
    )
}

/// Deploy a validator to ./.validators/.
pub(crate) fn deploy_validator(
    name: &str,
    source_dir: &Path,
    global: bool,
) -> Result<Vec<String>, RegistryError> {
    let target_dir = validators_dir(global).join(sanitize_dir_name(name));

    store::remove_if_exists(&target_dir)?;

    copy_dir_recursive(source_dir, &target_dir)?;
    let target_path = target_dir.display().to_string();
    tracing::debug!("  Deployed to {}", target_path);

    Ok(vec![target_path])
}

/// Deploy a tool to the central tool store and register in agent MCP configs.
pub(crate) fn deploy_tool(
    name: &str,
    source_dir: &Path,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<Vec<String>, RegistryError> {
    let config = agents::load_agents_config()?;
    let agents = agents::resolve_target_agents(&config, agent_filter)?;

    // 1. Parse MCP frontmatter from TOOL.md
    let tool_md = source_dir.join("TOOL.md");
    let yaml = mcp_config::parse_yaml_frontmatter(&tool_md)?;
    let mcp_fm = mcp_config::parse_tool_frontmatter(&yaml)?;

    // 2. Copy source into the central tool store
    let sanitized = sanitize_dir_name(name);
    let store_path = store::tool_store_dir(global).join(&sanitized);

    store::remove_if_exists(&store_path)?;
    copy_dir_recursive(source_dir, &store_path)?;
    tracing::debug!("  Stored in {}", store_path.display());

    // 3. Register in each agent's MCP config
    let entry = mcp_config::McpServerEntry {
        command: mcp_fm.command,
        args: mcp_fm.args,
        env: mcp_fm.env,
    };

    let mut targets = Vec::new();

    for agent in &agents {
        if let Some(ref mcp_cfg) = agent.def.mcp_config {
            let config_path = if global {
                agents::agent_global_mcp_config(&agent.def)
            } else {
                agents::agent_project_mcp_config(&agent.def)
            };

            if let Some(config_path) = config_path {
                mcp_config::register_mcp_server(
                    &config_path,
                    &ServersKey::new(&mcp_cfg.servers_key),
                    &ToolName::new(name),
                    &entry,
                    &mcp_cfg.entry_extras,
                )?;
                tracing::debug!(
                    "  Registered in {} ({})",
                    config_path.display(),
                    agent.def.name
                );
                targets.push(agent.def.id.clone());
            }
        } else {
            tracing::debug!("Agent {} has no MCP config, skipping", agent.def.id);
        }
    }

    if targets.is_empty() {
        return Err(RegistryError::Validation(
            "no agents with MCP config detected.".to_string(),
        ));
    }

    Ok(targets)
}

/// Deploy a plugin to agent plugin directories.
pub(crate) fn deploy_plugin(
    name: &str,
    source_dir: &Path,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<Vec<String>, RegistryError> {
    let config = agents::load_agents_config()?;
    let agents = agents::resolve_target_agents(&config, agent_filter)?;

    let sanitized = sanitize_dir_name(name);
    let mut targets = Vec::new();

    for agent in &agents {
        let plugin_dir = if global {
            agents::agent_global_plugin_dir(&agent.def)
        } else {
            agents::agent_project_plugin_dir(&agent.def)
        };

        if let Some(base_dir) = plugin_dir {
            let target = base_dir.join(&sanitized);
            store::remove_if_exists(&target)?;
            copy_dir_recursive(source_dir, &target)?;
            tracing::debug!("  Deployed to {} ({})", target.display(), agent.def.name);
            targets.push(agent.def.id.clone());

            register_plugin_mcp_servers(&agent.def, &target, global);
        } else {
            tracing::debug!("Agent {} has no plugin path, skipping", agent.def.id);
        }
    }

    if targets.is_empty() {
        return Err(RegistryError::Validation(
            "no agents with plugin support detected. Plugins are currently supported by Claude Code"
                .to_string(),
        ));
    }

    Ok(targets)
}

/// Register the MCP servers a deployed plugin declares in its `.mcp.json`.
///
/// A plugin with no `.mcp.json`, an agent with no MCP config, and an entry
/// that fails to parse are each a silent skip: plugin MCP registration is
/// best-effort on top of the plugin file deployment. Plugin authors edit
/// `.mcp.json` by hand, so the file is parsed as JSONC.
fn register_plugin_mcp_servers(agent: &AgentDef, target: &Path, global: bool) {
    let plugin_mcp = target.join(".mcp.json");
    if !plugin_mcp.exists() {
        return;
    }
    let Some(ref mcp_cfg) = agent.mcp_config else {
        return;
    };
    let config_path = if global {
        agents::agent_global_mcp_config(agent)
    } else {
        agents::agent_project_mcp_config(agent)
    };
    let Some(config_path) = config_path else {
        return;
    };
    let Ok(content) = std::fs::read_to_string(&plugin_mcp) else {
        return;
    };
    let Ok(json) = crate::parse_jsonc(&content) else {
        return;
    };
    let Some(servers) = json.get(&mcp_cfg.servers_key).and_then(|s| s.as_object()) else {
        return;
    };
    for (server_name, server_def) in servers {
        let Ok(entry) = serde_json::from_value::<mcp_config::McpServerEntry>(server_def.clone())
        else {
            continue;
        };
        let _ = mcp_config::register_mcp_server(
            &config_path,
            &ServersKey::new(&mcp_cfg.servers_key),
            &ToolName::new(server_name),
            &entry,
            &mcp_cfg.entry_extras,
        );
        tracing::debug!("  Registered MCP server '{}' from plugin", server_name);
    }
}
