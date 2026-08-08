//! Sync command — reconcile `.skills/` with agent directories and verify lockfile.
//!
//! This module provides both a library entry point (`sync()`) for use by
//! other crates (e.g. `sah init`) and a CLI wrapper (`run_sync()`).
//!
//! Sync uses the lockfile as the source of truth. For each skill entry in the
//! lockfile, it verifies the store entry exists and ensures symlinks are present
//! in all detected agent directories. This correctly handles nested store paths
//! (e.g. `anthropics/skills/algorithmic-art`) that arise from URL-based installs.

use std::path::{Path, PathBuf};

use crate::agents::{
    self, agent_global_agent_dir, agent_global_skill_dir, agent_project_agent_dir,
    agent_project_skill_dir, AgentDef, DetectedAgent,
};
use crate::lockfile::Lockfile;
use crate::package_type::PackageType;
use crate::registry::RegistryError;
use crate::store;

/// Report of what `sync` did.
#[derive(Debug, Default, Clone)]
pub struct SyncReport {
    /// Number of symlinks created.
    pub links_created: u32,
    /// Package names in lockfile whose store entries are missing.
    pub missing_packages: Vec<String>,
    /// Number of packages verified in lockfile.
    pub packages_verified: u32,
    /// Agent IDs that were synced.
    pub agents_synced: Vec<String>,
}

/// Library entry point — reconcile `.skills/` store with agent directories.
///
/// Called by both `mirdan sync` and `sah init`.
///
/// Uses the lockfile as the source of truth rather than scanning the filesystem,
/// which correctly handles nested store paths from URL-based installs.
///
/// # Errors
///
/// Returns [`RegistryError`] when the agents configuration cannot be loaded,
/// the `--agent` filter names no known agent, the lockfile cannot be read or
/// parsed, or a missing symlink cannot be created.
pub fn sync(
    project_root: &Path,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<SyncReport, RegistryError> {
    let config = agents::load_agents_config()?;
    let agents = agents::resolve_target_agents(&config, agent_filter)?;

    let mut report = SyncReport::default();

    // Load lockfile — this is the source of truth for what's installed
    let lf = Lockfile::load(project_root)?;

    for (name, pkg) in &lf.packages {
        verify_package(name, pkg.package_type, &agents, global, &mut report)?;
    }

    // Record which agents we synced
    for agent in &agents {
        report.agents_synced.push(agent.def.id.clone());
    }

    Ok(report)
}

/// Verify one lockfile entry, and create any symlink it is missing.
///
/// # Errors
///
/// Returns [`RegistryError`] when a missing symlink cannot be created.
fn verify_package(
    name: &str,
    package_type: PackageType,
    agents: &[DetectedAgent],
    global: bool,
    report: &mut SyncReport,
) -> Result<(), RegistryError> {
    match package_type {
        PackageType::Skill => verify_linked_package(
            name,
            &store::skill_store_dir(global),
            agents,
            |def| {
                Some(scoped_agent_path(
                    def,
                    global,
                    agent_global_skill_dir,
                    agent_project_skill_dir,
                ))
            },
            report,
        ),
        PackageType::Agent => verify_linked_package(
            name,
            &store::agent_store_dir(global),
            agents,
            |def| scoped_agent_path(def, global, agent_global_agent_dir, agent_project_agent_dir),
            report,
        ),
        PackageType::Tool => {
            record_presence(report, name, tool_is_configured(name, agents, global));
            Ok(())
        }
        PackageType::Plugin => {
            record_presence(report, name, plugin_is_installed(name, agents, global));
            Ok(())
        }
        PackageType::Validator => {
            record_presence(report, name, validator_is_installed(name, global));
            Ok(())
        }
    }
}

/// Count a package as verified, or record it as missing.
fn record_presence(report: &mut SyncReport, name: &str, present: bool) {
    if present {
        report.packages_verified += 1;
    } else {
        report.missing_packages.push(name.to_string());
    }
}

/// Resolve one of an agent's paths by scope: `global_path` when `global`, and
/// `project_path` otherwise.
///
/// Every agent path this module reads — skill directory, subagent directory,
/// MCP config, plugin directory — comes as a global/project pair, and every one
/// is chosen the same way. This is that choice, written once for all of them.
fn scoped_agent_path<T>(
    def: &AgentDef,
    global: bool,
    global_path: fn(&AgentDef) -> T,
    project_path: fn(&AgentDef) -> T,
) -> T {
    if global {
        global_path(def)
    } else {
        project_path(def)
    }
}

/// Verify a store-backed package, and link it into every agent directory that
/// does not hold it yet.
///
/// `store_root` is the store the package lives in. `agent_dir` resolves the
/// per-agent directory the link belongs in, and returns `None` for an agent
/// that has no such directory.
///
/// # Errors
///
/// Returns [`RegistryError`] when a missing symlink cannot be created.
fn verify_linked_package(
    name: &str,
    store_root: &Path,
    agents: &[DetectedAgent],
    agent_dir: impl Fn(&AgentDef) -> Option<PathBuf>,
    report: &mut SyncReport,
) -> Result<(), RegistryError> {
    let sanitized = store::sanitize_dir_name(name);
    let store_path = store_root.join(&sanitized);

    if !store_path.exists() {
        report.missing_packages.push(name.to_string());
        return Ok(());
    }

    report.packages_verified += 1;

    for agent in agents {
        let Some(base_dir) = agent_dir(&agent.def) else {
            continue;
        };
        let link_name = store::symlink_name(&sanitized, &agent.def.symlink_policy);
        let link_path = base_dir.join(&link_name);

        // Skip if link already exists and is valid
        if std::fs::symlink_metadata(&link_path).is_ok() {
            continue;
        }

        // Create missing symlink
        store::create_skill_link(&store_path, &link_path)?;
        report.links_created += 1;
    }

    Ok(())
}

/// Whether any target agent's MCP config already declares the tool `name`.
fn tool_is_configured(name: &str, agents: &[DetectedAgent], global: bool) -> bool {
    agents
        .iter()
        .any(|agent| agent_declares_mcp_server(agent, name, global))
}

/// Whether one agent's MCP config file declares a server named `name`.
fn agent_declares_mcp_server(agent: &DetectedAgent, name: &str, global: bool) -> bool {
    let Some(mcp_def) = &agent.def.mcp_config else {
        return false;
    };
    let config_path = scoped_agent_path(
        &agent.def,
        global,
        agents::agent_global_mcp_config,
        agents::agent_project_mcp_config,
    );
    let Some(path) = config_path else {
        return false;
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return false;
    };
    // Agent MCP configs are user-written (Zed/VS Code ship JSONC). Mirror the
    // lenient input format we accept on install.
    let Ok(settings) = crate::parse_jsonc(&content) else {
        return false;
    };
    settings
        .get(&mcp_def.servers_key)
        .and_then(|servers| servers.get(name))
        .is_some()
}

/// Whether any target agent already holds a plugin directory named `name`.
fn plugin_is_installed(name: &str, agents: &[DetectedAgent], global: bool) -> bool {
    let sanitized = store::sanitize_dir_name(name);
    agents.iter().any(|agent| {
        let plugin_dir = scoped_agent_path(
            &agent.def,
            global,
            agents::agent_global_plugin_dir,
            agents::agent_project_plugin_dir,
        );
        plugin_dir.is_some_and(|base_dir| base_dir.join(&sanitized).exists())
    })
}

/// Whether the validator `name` is deployed to the validators directory.
fn validator_is_installed(name: &str, global: bool) -> bool {
    crate::install::validators_dir(global)
        .join(store::sanitize_dir_name(name))
        .exists()
}

/// CLI wrapper for `mirdan sync`.
///
/// # Errors
///
/// Returns [`RegistryError`] when the current directory cannot be read, or
/// when [`sync`] fails.
pub fn run_sync(agent_filter: Option<&str>, global: bool) -> Result<(), RegistryError> {
    let project_root = std::env::current_dir()?;
    let report = sync(&project_root, agent_filter, global)?;

    println!("Sync complete:");
    println!("  Agents synced: {}", report.agents_synced.len());
    println!("  Links created: {}", report.links_created);
    println!("  Packages verified: {}", report.packages_verified);

    if !report.missing_packages.is_empty() {
        println!("  Missing packages:");
        for name in &report.missing_packages {
            println!("    - {}", name);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lockfile::LockedPackage;
    use serial_test::serial;
    use swissarmyhammer_common::test_utils::CurrentDirGuard;

    #[test]
    #[serial]
    fn test_sync_empty_project() {
        let dir = tempfile::tempdir().unwrap();
        // `sync(.., global=false)` resolves every store and agent path relative
        // to the process working directory, so pin it to the tempdir.
        let _cwd = CurrentDirGuard::new(dir.path()).unwrap();

        let report = sync(dir.path(), None, false).unwrap();
        assert_eq!(report.links_created, 0);
        assert_eq!(report.packages_verified, 0);
        assert!(report.missing_packages.is_empty());
    }

    #[test]
    fn test_sync_report_default() {
        let report = SyncReport::default();
        assert_eq!(report.links_created, 0);
        assert!(report.missing_packages.is_empty());
        assert_eq!(report.packages_verified, 0);
        assert!(report.agents_synced.is_empty());
    }

    #[test]
    #[serial]
    fn test_sync_skill_missing_from_store() {
        let dir = tempfile::tempdir().unwrap();
        // The skill store lookup goes through `skill_store_dir(false)`, which
        // is CWD-relative. Pin the CWD so a `.skills/ghost-skill` directory
        // under the repo root cannot make this test see the skill.
        let _cwd = CurrentDirGuard::new(dir.path()).unwrap();

        // Write a lockfile with a skill that's not in the store
        let mut lf = Lockfile::default();
        lf.add_package(
            "ghost-skill".to_string(),
            LockedPackage {
                package_type: PackageType::Skill,
                version: "1.0.0".to_string(),
                resolved: "file:somewhere".to_string(),
                integrity: String::new(),
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                targets: vec![],
            },
        );
        lf.save(dir.path()).unwrap();

        let report = sync(dir.path(), None, false).unwrap();
        assert_eq!(report.packages_verified, 0);
        assert_eq!(report.missing_packages, vec!["ghost-skill"]);
    }

    #[test]
    #[serial]
    fn test_sync_skill_present_in_store() {
        // `skill_store_dir(false)` returns the relative ".skills/", which the
        // OS resolves against the process working directory, not against the
        // `project_root` argument. Pin the CWD to the tempdir so the store
        // this test creates is the store sync reads.
        let dir = tempfile::tempdir().unwrap();
        let _cwd = CurrentDirGuard::new(dir.path()).unwrap();

        // Create a skill in the store relative to tempdir
        let store = dir.path().join(".skills/my-skill");
        std::fs::create_dir_all(&store).unwrap();
        std::fs::write(store.join("SKILL.md"), "# test").unwrap();

        // Write a lockfile referencing it
        let mut lf = Lockfile::default();
        lf.add_package(
            "my-skill".to_string(),
            LockedPackage {
                package_type: PackageType::Skill,
                version: "1.0.0".to_string(),
                resolved: "file:my-skill".to_string(),
                integrity: String::new(),
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                targets: vec![],
            },
        );
        lf.save(dir.path()).unwrap();

        let report = sync(dir.path(), None, false).unwrap();
        assert!(!report.agents_synced.is_empty());
        assert_eq!(report.packages_verified, 1);
        assert!(report.missing_packages.is_empty());
    }

    #[test]
    #[serial]
    fn test_sync_nested_store_path() {
        // Verify that URL-based package names with nested store paths
        // are resolved correctly through sanitize_dir_name
        let dir = tempfile::tempdir().unwrap();
        // The store lookup is CWD-relative, and this test asserts the package
        // is missing, so pin the CWD away from any real `.skills/` tree.
        let _cwd = CurrentDirGuard::new(dir.path()).unwrap();

        // Write lockfile with URL-based package name
        let mut lf = Lockfile::default();
        lf.add_package(
            "https://github.com/anthropics/skills/algorithmic-art".to_string(),
            LockedPackage {
                package_type: PackageType::Skill,
                version: "1.0.0".to_string(),
                resolved: "git+https://github.com/anthropics/skills.git".to_string(),
                integrity: String::new(),
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                targets: vec!["claude-code".to_string()],
            },
        );
        lf.save(dir.path()).unwrap();

        // Without the store entry, it should report as missing
        let report = sync(dir.path(), None, false).unwrap();
        assert_eq!(
            report.missing_packages,
            vec!["https://github.com/anthropics/skills/algorithmic-art"]
        );

        // Verify sanitize_dir_name produces nested path
        let sanitized =
            store::sanitize_dir_name("https://github.com/anthropics/skills/algorithmic-art");
        assert_eq!(sanitized, "anthropics/skills/algorithmic-art");
    }

    #[test]
    #[serial]
    fn test_sync_validator_missing() {
        let dir = tempfile::tempdir().unwrap();
        // Isolate CWD so the project-relative `.validators/` lookup resolves to
        // the (empty) tempdir rather than whatever directory the test runner
        // happens to be in.
        let _cwd = CurrentDirGuard::new(dir.path()).unwrap();

        let mut lf = Lockfile::default();
        lf.add_package(
            "my-validator".to_string(),
            LockedPackage {
                package_type: PackageType::Validator,
                version: "1.0.0".to_string(),
                resolved: "file:my-validator".to_string(),
                integrity: String::new(),
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                targets: vec![],
            },
        );
        lf.save(dir.path()).unwrap();

        let report = sync(dir.path(), None, false).unwrap();

        assert_eq!(report.packages_verified, 0);
        assert_eq!(report.missing_packages, vec!["my-validator"]);
    }

    #[test]
    #[serial]
    fn test_sync_validator_present_in_project_dir() {
        // A validator deployed to the project `.validators/` directory (where
        // `deploy_validator` writes via `validators_dir(false)`) must be
        // verified by sync — not reported missing. This guards against the
        // validator branch checking the wrong (old avp) layout.
        let dir = tempfile::tempdir().unwrap();
        let _cwd = CurrentDirGuard::new(dir.path()).unwrap();

        // Deploy a validator where `validators_dir(false)` resolves it.
        let val_dir = crate::install::validators_dir(false).join("my-validator");
        std::fs::create_dir_all(&val_dir).unwrap();
        std::fs::write(val_dir.join("VALIDATOR.md"), "# test").unwrap();

        let mut lf = Lockfile::default();
        lf.add_package(
            "my-validator".to_string(),
            LockedPackage {
                package_type: PackageType::Validator,
                version: "1.0.0".to_string(),
                resolved: "file:my-validator".to_string(),
                integrity: String::new(),
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                targets: vec![],
            },
        );
        lf.save(dir.path()).unwrap();

        let report = sync(dir.path(), None, false).unwrap();

        assert_eq!(report.packages_verified, 1);
        assert!(report.missing_packages.is_empty());
    }

    #[test]
    #[serial]
    fn test_sync_mcp_missing() {
        let dir = tempfile::tempdir().unwrap();
        // `agent_project_mcp_config` is CWD-relative, and this test asserts
        // the tool is missing, so pin the CWD away from any real agent config
        // that already declares a `sah` server.
        let _cwd = CurrentDirGuard::new(dir.path()).unwrap();

        let mut lf = Lockfile::default();
        lf.add_package(
            "sah".to_string(),
            LockedPackage {
                package_type: PackageType::Tool,
                version: "0.0.0".to_string(),
                resolved: "mcp:sah".to_string(),
                integrity: String::new(),
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                targets: vec!["claude-code".to_string()],
            },
        );
        lf.save(dir.path()).unwrap();

        let report = sync(dir.path(), None, false).unwrap();
        // MCP entry exists in lockfile but no agent config file has it
        assert_eq!(report.packages_verified, 0);
        assert_eq!(report.missing_packages, vec!["sah"]);
    }
}
