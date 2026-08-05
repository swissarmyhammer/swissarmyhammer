//! Removal of installed packages: skills, validators, tools, plugins,
//! agents, and MCP server registrations.

use std::path::{Path, PathBuf};

use crate::agents::{
    self, agent_global_agent_dir, agent_global_skill_dir, agent_project_agent_dir,
    agent_project_skill_dir, AgentDef, DetectedAgent,
};
use crate::git_source;
use crate::lockfile::Lockfile;
use crate::mcp_config;
use crate::package_type::PackageType;
use crate::registry::RegistryError;
use crate::store;

use super::package::read_frontmatter;
use super::{remove_empty_dirs_up_to, rooted, safe_dir_name, validators_dir};

/// Find all package names in a lockfile that were installed from a given git URL or shorthand.
///
/// Parses `spec` as a git source, then matches against the `resolved` field
/// of each lockfile entry (which uses the `git+<url>` format).
/// Returns an empty vec if `spec` is not a valid git source or no packages match.
pub fn find_packages_by_git_source(lf: &Lockfile, spec: &str) -> Vec<String> {
    let git_src = match git_source::parse_git_source(spec, None) {
        Ok(src) => src,
        Err(_) => return Vec::new(),
    };
    let resolved_prefix = format!("git+{}", git_src.clone_url);
    lf.packages
        .iter()
        .filter(|(_, pkg)| pkg.resolved == resolved_prefix)
        .map(|(pkg_name, _)| pkg_name.clone())
        .collect()
}

/// Run the uninstall command.
///
/// Accepts a package name or a git URL/shorthand. When given a URL,
/// uninstalls all packages whose lockfile `resolved` field matches.
pub async fn run_uninstall(
    name: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<Vec<crate::DeployResult>, RegistryError> {
    let (project_root, home, mut lf) = setup_lockfile()?;

    // Resolve the lockfile key — try exact match, then display name, then git source
    let lockfile_key = if lf.get_package(name).is_some() {
        Some(name.to_string())
    } else if let Some((key, _)) = lf.find_by_display_name(name) {
        Some(key.to_string())
    } else {
        let matching = find_packages_by_git_source(&lf, name);
        if !matching.is_empty() {
            return uninstall_git_source_matches(
                &mut lf,
                &matching,
                name,
                agent_filter,
                global,
                home.as_deref(),
                &project_root,
            );
        }
        None
    };

    // Use the resolved key, or fall back to the display name for filesystem-only removal
    let key = lockfile_key.as_deref().unwrap_or(name);

    // Determine the display name (last segment) for filesystem operations
    let display_name = key.rsplit('/').next().unwrap_or(key);

    let pkg_type = lf
        .get_package(key)
        .map(|p| p.package_type)
        .unwrap_or_else(|| guess_installed_type(display_name, global));

    let mut results = uninstall_by_type(pkg_type, display_name, agent_filter, global)?;

    // Update lockfile
    lf.remove_package(key);
    save_lockfile(&lf, home.as_deref(), &project_root)?;
    tracing::debug!(key, "uninstalled");

    results.push(crate::DeployResult::message(
        crate::DeployAction::Removed,
        format!("Uninstalled {}", key),
    ));
    Ok(results)
}

/// Resolve the project root, the home directory, and the lockfile (with the
/// home fallback) for one uninstall entry point.
fn setup_lockfile() -> Result<(PathBuf, Option<PathBuf>, Lockfile), RegistryError> {
    let project_root = std::env::current_dir()?;
    let home = dirs::home_dir();
    let lf = load_lockfile_with_home_fallback(&project_root, home.as_deref())?;
    Ok((project_root, home, lf))
}

/// Load the lockfile from `project_root`, falling back to `home` when the
/// CWD lockfile is empty (GUI launches set CWD to HOME, CLI runs may sit in
/// a project directory).
fn load_lockfile_with_home_fallback(
    project_root: &Path,
    home: Option<&Path>,
) -> Result<Lockfile, RegistryError> {
    let lf = Lockfile::load(project_root)?;
    if !lf.packages.is_empty() {
        return Ok(lf);
    }
    let Some(home) = home.filter(|h| *h != project_root) else {
        return Ok(lf);
    };
    if let Ok(home_lf) = Lockfile::load(home) {
        if !home_lf.packages.is_empty() {
            return Ok(home_lf);
        }
    }
    Ok(lf)
}

/// Save the lockfile to `home` when present, else to `project_root`,
/// returning the directory it was saved to.
fn save_lockfile<'a>(
    lf: &Lockfile,
    home: Option<&'a Path>,
    project_root: &'a Path,
) -> Result<&'a Path, RegistryError> {
    let save_dir = home.unwrap_or(project_root);
    lf.save(save_dir)?;
    Ok(save_dir)
}

/// Dispatch one uninstall to the handler for `pkg_type`, returning the
/// user-visible results the handler produced (empty for handlers that only
/// remove files).
fn uninstall_by_type(
    pkg_type: PackageType,
    name: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<Vec<crate::DeployResult>, RegistryError> {
    match pkg_type {
        PackageType::Skill => uninstall_skill(name, agent_filter, global).map(|()| Vec::new()),
        PackageType::Validator => uninstall_validator(name, global).map(|()| Vec::new()),
        PackageType::Tool => uninstall_tool(name, agent_filter, global),
        PackageType::Plugin => uninstall_plugin(name, agent_filter, global).map(|()| Vec::new()),
        PackageType::Agent => uninstall_agent(name, agent_filter, global).map(|()| Vec::new()),
    }
}

/// Uninstall every package the lockfile resolved from the git `source`,
/// remove each from the lockfile, and save it.
fn uninstall_git_source_matches(
    lf: &mut Lockfile,
    matching: &[String],
    source: &str,
    agent_filter: Option<&str>,
    global: bool,
    home: Option<&Path>,
    project_root: &Path,
) -> Result<Vec<crate::DeployResult>, RegistryError> {
    let mut results = Vec::new();
    for pkg_name in matching {
        let pkg_type = lf.get_package(pkg_name).unwrap().package_type;
        results.extend(uninstall_by_type(pkg_type, pkg_name, agent_filter, global)?);
        lf.remove_package(pkg_name);
        tracing::debug!(pkg_name, "uninstalled");
    }
    save_lockfile(lf, home, project_root)?;
    tracing::debug!(count = matching.len(), source, "uninstalled packages");
    results.push(crate::DeployResult::message(
        crate::DeployAction::Removed,
        format!("Uninstalled {} package(s) from {}", matching.len(), source),
    ));
    Ok(results)
}

/// Uninstall a skill by name from every detected (or filtered) agent.
///
/// Delegates to [`uninstall_skill_at`] with CWD-relative project scope.
pub fn uninstall_skill(
    name: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<(), RegistryError> {
    uninstall_skill_at(name, agent_filter, global, None)
}

/// Load the agents config and resolve the target agents for `agent_filter`.
///
/// The single config-loading step behind every per-agent uninstall loop.
fn load_and_resolve_agents(
    agent_filter: Option<&str>,
) -> Result<(agents::AgentsConfig, Vec<DetectedAgent>), RegistryError> {
    let config = agents::load_agents_config()?;
    let target_agents = agents::resolve_target_agents(&config, agent_filter)?;
    Ok((config, target_agents))
}

/// Build the standard `NotFound` error for an uninstall target that is not
/// installed at the scope.
fn not_found_error(description: String, global: bool) -> RegistryError {
    let scope = if global { "global" } else { "project" };
    RegistryError::NotFound(format!("{description} ({scope} scope)"))
}

/// Remove the symlink for `sanitized` from the directory `agent_dir` yields
/// for each agent, returning the number of symlinks removed.
///
/// The single symlink-removal loop behind [`uninstall_skill_at`] and
/// [`uninstall_agent_at`]: the two differ only in how each agent's directory
/// resolves. `agent_dir` returns `None` for an agent with no directory; that
/// agent is skipped.
fn remove_agent_symlinks(
    agents: &[DetectedAgent],
    sanitized: &str,
    agent_dir: impl Fn(&AgentDef) -> Option<PathBuf>,
) -> Result<usize, RegistryError> {
    let mut removed = 0;
    for agent in agents {
        let Some(base_dir) = agent_dir(&agent.def) else {
            continue;
        };
        let link_name = store::symlink_name(sanitized, &agent.def.symlink_policy);
        let link_path = base_dir.join(&link_name);

        // Check if the path exists (symlink or real dir)
        if std::fs::symlink_metadata(&link_path).is_ok() {
            store::remove_if_exists(&link_path)?;
            tracing::debug!(
                "  Removed from {} ({})",
                link_path.display(),
                agent.def.name
            );
            removed += 1;
        }
    }
    Ok(removed)
}

/// Root-explicit variant of [`uninstall_skill`].
///
/// When `root` is `Some`, project-scope relative paths (the `.skills/` store and
/// each agent's project skill directory) are joined onto `root` instead of being
/// resolved against the process working directory. `None` preserves CWD-relative
/// behavior; global scope ignores `root`.
pub fn uninstall_skill_at(
    name: &str,
    agent_filter: Option<&str>,
    global: bool,
    root: Option<&Path>,
) -> Result<(), RegistryError> {
    let sanitized = safe_dir_name(name)?;
    let (_config, agents) = load_and_resolve_agents(agent_filter)?;

    // 1. Remove symlinks from each agent's skill directory
    let removed = remove_agent_symlinks(&agents, &sanitized, |def| {
        Some(if global {
            agent_global_skill_dir(def)
        } else {
            rooted(root, global, agent_project_skill_dir(def))
        })
    })?;

    // 2. Remove all store entries matching this skill name.
    // Skills can exist at both flat paths (e.g. ~/.skills/explain/) and
    // nested paths (e.g. ~/.skills/owner/repo/explain/) depending on
    // how they were installed (git vs registry). Remove all of them.
    let store_root = rooted(root, global, store::skill_store_dir(global));
    let flat_path = store_root.join(&sanitized);
    let mut store_removed = false;
    if flat_path.exists() {
        std::fs::remove_dir_all(&flat_path)?;
        tracing::debug!(path = %flat_path.display(), "removed store entry");
        store_removed = true;
    }
    // Also scan recursively for nested store entries with matching SKILL.md name
    store_removed |= remove_matching_store_entries(&store_root, name)?;

    if removed == 0 && !store_removed {
        return Err(not_found_error(format!("skill '{name}' not found"), global));
    }

    Ok(())
}

/// Recursively scan the store for directories containing SKILL.md whose
/// frontmatter name matches, and remove them. Also cleans up empty parent
/// dirs. Returns `true` when any entry was removed.
///
/// A missing `dir` is a benign no-op; any other read failure propagates so a
/// scan the process cannot perform is never reported as a clean removal.
fn remove_matching_store_entries(dir: &Path, name: &str) -> Result<bool, RegistryError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(RegistryError::Io(e)),
    };

    let mut any_removed = false;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.join("SKILL.md").exists() {
            any_removed |= remove_store_entry_if_named(&path, name, dir)?;
        } else {
            // Recurse into subdirectories
            any_removed |= remove_matching_store_entries(&path, name)?;
        }
    }

    Ok(any_removed)
}

/// Remove one store entry directory when its SKILL.md frontmatter name or its
/// directory name matches `name`, then clean up now-empty parent directories
/// up to (and excluding) `boundary`. Returns `true` when the entry was removed.
fn remove_store_entry_if_named(
    path: &Path,
    name: &str,
    boundary: &Path,
) -> Result<bool, RegistryError> {
    let fm_name = read_skill_frontmatter_name(&path.join("SKILL.md"));
    let dir_name = path.file_name().map(|n| n.to_string_lossy().to_string());
    if fm_name.as_deref() != Some(name) && dir_name.as_deref() != Some(name) {
        return Ok(false);
    }
    std::fs::remove_dir_all(path)?;
    tracing::debug!(path = %path.display(), "removed nested store entry");
    if let Some(parent) = path.parent() {
        remove_empty_dirs_up_to(parent, boundary);
    }
    Ok(true)
}

/// Read the name field from a SKILL.md frontmatter.
///
/// Delegates to the shared [`read_frontmatter`] parser; any parse failure
/// (missing file, malformed frontmatter, absent name) yields `None`.
fn read_skill_frontmatter_name(path: &Path) -> Option<String> {
    read_frontmatter(path).ok().map(|(name, _version)| name)
}

/// Uninstall a validator: remove its directory from the validators store.
pub(crate) fn uninstall_validator(name: &str, global: bool) -> Result<(), RegistryError> {
    let target_dir = validators_dir(global).join(safe_dir_name(name)?);

    if !target_dir.exists() {
        return Err(not_found_error(
            format!("validator '{name}' not found"),
            global,
        ));
    }

    std::fs::remove_dir_all(&target_dir)?;
    tracing::debug!("  Removed from {}", target_dir.display());
    Ok(())
}

/// Uninstall an MCP server from all detected (or filtered) agents.
pub async fn run_uninstall_mcp(
    name: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<Vec<crate::DeployResult>, RegistryError> {
    let (_config, target_agents) = load_and_resolve_agents(agent_filter)?;

    let mut results = Vec::new();

    let removed = unregister_and_report_mcp(&target_agents, name, global, "Removed", &mut results)?;

    // Update lockfile (same home fallback as `run_uninstall`, so a global
    // MCP server recorded in the HOME lockfile is found and cleaned up).
    let (project_root, home, mut lf) = setup_lockfile()?;
    lf.remove_package(name);
    let save_dir = save_lockfile(&lf, home.as_deref(), &project_root)?;
    results.push(crate::DeployResult::updated(
        save_dir.join("mirdan-lock.json"),
        "Updated mirdan-lock.json".to_string(),
    ));

    let summary = if removed == 0 {
        format!(
            "MCP server '{}' not found in any agent (removed from lockfile)",
            name
        )
    } else {
        format!(
            "Uninstalled MCP server '{}' from {} agent(s)",
            name, removed
        )
    };
    results.push(crate::DeployResult::message(
        crate::DeployAction::Removed,
        summary,
    ));
    Ok(results)
}

/// Unregister `name` from every agent's MCP config and push one user-visible
/// [`crate::DeployResult`] per config changed, with `action_verb` naming the
/// change (`"Removed"` or `"Unregistered"`). Returns the number of configs
/// changed.
///
/// The single unregister-and-report site behind [`run_uninstall_mcp`] and
/// [`uninstall_tool`]: the two differ only in the verb of the message.
fn unregister_and_report_mcp(
    agents: &[DetectedAgent],
    name: &str,
    global: bool,
    action_verb: &str,
    results: &mut Vec<crate::DeployResult>,
) -> Result<usize, RegistryError> {
    unregister_mcp_from_agents(agents, name, global, |agent, config_path| {
        results.push(crate::DeployResult::removed(
            config_path,
            format!(
                "{action_verb} MCP server '{name}' from {} ({})",
                agent.def.name,
                config_path.display()
            ),
        ));
    })
}

/// Unregister `name` from each agent's MCP config file, calling `on_removed`
/// with the agent and its config path for each config that changed. Returns
/// the number of configs changed.
///
/// The single unregister loop behind [`unregister_and_report_mcp`].
fn unregister_mcp_from_agents(
    agents: &[DetectedAgent],
    name: &str,
    global: bool,
    mut on_removed: impl FnMut(&DetectedAgent, &Path),
) -> Result<usize, RegistryError> {
    let mut removed = 0;
    for agent in agents {
        let Some(ref mcp_cfg) = agent.def.mcp_config else {
            continue;
        };
        let config_path = if global {
            agents::agent_global_mcp_config(&agent.def)
        } else {
            agents::agent_project_mcp_config(&agent.def)
        };
        let Some(config_path) = config_path else {
            continue;
        };
        if mcp_config::unregister_mcp_server(&config_path, &mcp_cfg.servers_key, name)? {
            on_removed(agent, &config_path);
            removed += 1;
        }
    }
    Ok(removed)
}

/// A store-root resolver keyed by the `global` flag.
type StoreDirFn = fn(bool) -> PathBuf;

/// Guess the package type based on what's installed.
fn guess_installed_type(name: &str, global: bool) -> PackageType {
    // An unsafe name is never installed in any store. Fall through to the
    // Skill default so the dispatched uninstall rejects it with a
    // Validation error instead of probing a traversal path.
    let Ok(sanitized) = safe_dir_name(name) else {
        return PackageType::Skill;
    };
    // Check the stores in precedence order: validator, tool, agent.
    let stores: [(StoreDirFn, PackageType); 3] = [
        (validators_dir, PackageType::Validator),
        (store::tool_store_dir, PackageType::Tool),
        (store::agent_store_dir, PackageType::Agent),
    ];
    for (store_dir, pkg_type) in stores {
        if store_dir(global).join(&sanitized).exists() {
            return pkg_type;
        }
    }
    // Check plugin dirs
    if plugin_installed(name, global) {
        return PackageType::Plugin;
    }
    // Default to skill
    PackageType::Skill
}

/// Check whether any agent's plugin directory holds an entry named `name`.
///
/// An unsafe name is never installed, so it reports `false` instead of
/// probing a traversal path.
fn plugin_installed(name: &str, global: bool) -> bool {
    let Ok(sanitized) = safe_dir_name(name) else {
        return false;
    };
    let Ok(config) = agents::load_agents_config() else {
        return false;
    };
    config.agents.iter().any(|agent| {
        let plugin_dir = if global {
            agents::agent_global_plugin_dir(agent)
        } else {
            agents::agent_project_plugin_dir(agent)
        };
        plugin_dir.is_some_and(|dir| dir.join(&sanitized).exists())
    })
}

/// Uninstall a tool: unregister its MCP server from every agent config, then
/// remove its store entry. Returns one user-visible result per MCP config
/// changed, so the caller can show that agent configuration was modified.
pub(crate) fn uninstall_tool(
    name: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<Vec<crate::DeployResult>, RegistryError> {
    let sanitized = safe_dir_name(name)?;
    let (_config, agents) = load_and_resolve_agents(agent_filter)?;

    let mut results = Vec::new();

    // 1. Unregister from each agent's MCP config
    let mut removed =
        unregister_and_report_mcp(&agents, name, global, "Unregistered", &mut results)?;

    // 2. Remove from tool store
    let store_path = store::tool_store_dir(global).join(&sanitized);
    if store_path.exists() {
        remove_and_log_store_entry(&store_path)?;
        removed += 1;
    }

    if removed == 0 {
        return Err(not_found_error(format!("tool '{name}' not found"), global));
    }

    Ok(results)
}

/// Uninstall a plugin: remove its directory from each agent's plugin
/// directory. Returns `NotFound` when no agent held the plugin.
pub(crate) fn uninstall_plugin(
    name: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<(), RegistryError> {
    let sanitized = safe_dir_name(name)?;
    let (_config, agents) = load_and_resolve_agents(agent_filter)?;

    let mut removed = 0;

    for agent in &agents {
        let plugin_dir = if global {
            agents::agent_global_plugin_dir(&agent.def)
        } else {
            agents::agent_project_plugin_dir(&agent.def)
        };

        if let Some(base_dir) = plugin_dir {
            let target = base_dir.join(&sanitized);
            if target.exists() {
                std::fs::remove_dir_all(&target)?;
                tracing::debug!("  Removed from {} ({})", target.display(), agent.def.name);
                removed += 1;
            }
        }
    }

    if removed == 0 {
        return Err(not_found_error(
            format!("plugin '{name}' not found"),
            global,
        ));
    }

    Ok(())
}

fn uninstall_agent(
    name: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<(), RegistryError> {
    let removed = uninstall_agent_at(name, agent_filter, global, None)?;
    if removed == 0 {
        return Err(not_found_error(
            format!("agent '{name}' not found in any coding agent"),
            global,
        ));
    }
    Ok(())
}

/// Root-explicit agent uninstall shared by [`uninstall_agent`] and
/// [`deinit_profile`].
///
/// Removes the agent's symlink from each coding agent's directory and cleans up
/// the `.agents/` store entry when no symlink still references it. When `root`
/// is `Some`, project-scope relative paths are joined onto `root`. Returns the
/// number of symlinks removed so callers can decide whether "not found" is an
/// error (single uninstall) or a benign no-op (profile deinit).
pub(crate) fn uninstall_agent_at(
    name: &str,
    agent_filter: Option<&str>,
    global: bool,
    root: Option<&Path>,
) -> Result<usize, RegistryError> {
    let sanitized = safe_dir_name(name)?;
    let (config, target_agents) = load_and_resolve_agents(agent_filter)?;

    // 1. Remove symlinks from each coding agent's agent directory
    let removed = remove_agent_symlinks(&target_agents, &sanitized, |def| {
        if global {
            agent_global_agent_dir(def)
        } else {
            agent_project_agent_dir(def).map(|d| rooted(root, global, d))
        }
    })?;

    // 2. Remove store entry if no remaining symlinks reference it
    remove_agent_store_entry_if_unreferenced(
        &agents::get_detected_agents(&config),
        &sanitized,
        global,
        root,
    )?;

    Ok(removed)
}

/// Remove the `.agents/` store entry for `sanitized` when no detected
/// agent's directory still references it.
fn remove_agent_store_entry_if_unreferenced(
    all_agents: &[DetectedAgent],
    sanitized: &str,
    global: bool,
    root: Option<&Path>,
) -> Result<(), RegistryError> {
    let store_path = rooted(root, global, store::agent_store_dir(global)).join(sanitized);
    if !store_path.exists() {
        return Ok(());
    }
    let all_agent_dirs: Vec<PathBuf> = all_agents
        .iter()
        .filter_map(|a| {
            if global {
                agent_global_agent_dir(&a.def)
            } else {
                agent_project_agent_dir(&a.def).map(|d| rooted(root, global, d))
            }
        })
        .collect();

    if !store::store_entry_still_referenced(&store_path, &all_agent_dirs) {
        remove_and_log_store_entry(&store_path)?;
    }
    Ok(())
}

/// Remove one store entry directory and log the removal.
fn remove_and_log_store_entry(path: &Path) -> Result<(), RegistryError> {
    std::fs::remove_dir_all(path)?;
    tracing::debug!("  Removed store entry {}", path.display());
    Ok(())
}
