//! Mirdan Install/Uninstall - Type-aware package deployment.
//!
//! Skills -> agent skill directories (one copy per detected agent)
//! Validators -> .avp/validators/ (project) or ~/.avp/validators/ (global)
//! Tools -> .tools/ store + agent MCP config files
//! Plugins -> agent plugin directories (e.g. .claude/plugins/)

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use indicatif::{ProgressBar, ProgressStyle};

use crate::agents::{self, agent_global_skill_dir, agent_project_skill_dir};
use crate::git_source::{self, InstallSource};
use crate::lockfile::{self, LockedPackage, Lockfile};
use crate::mcp_config;
use crate::package_type::{self, PackageType};
use crate::registry::{RegistryClient, RegistryError};
use crate::store;

/// Sanitize a package name for use as a filesystem directory name.
///
/// Delegates to [`store::sanitize_dir_name`].
fn sanitize_dir_name(name: &str) -> String {
    store::sanitize_dir_name(name)
}

/// Run the install command.
///
/// Accepts multiple forms:
/// - `name` or `name@version` — download from registry
/// - `./local-path` — install from a local directory
/// - `owner/repo` or git URL — clone from git (with `--git` flag or as fallback)
///
/// Auto-detects type from contents:
/// - SKILL.md -> deploy to each detected agent's skill directory
/// - VALIDATOR.md + rules/ -> deploy to .avp/validators/
pub async fn run_install(
    package_spec: &str,
    agent_filter: Option<&str>,
    global: bool,
    git: bool,
    skill_select: Option<&str>,
) -> Result<(), RegistryError> {
    match git_source::classify_source(package_spec, git) {
        InstallSource::LocalPath(path) => run_install_local(&path, agent_filter, global).await,
        InstallSource::GitRepo(source) => {
            run_install_git(&source, agent_filter, global, skill_select).await
        }
        InstallSource::Registry(spec) => {
            match run_install_registry(&spec, agent_filter, global).await {
                Ok(()) => Ok(()),
                Err(RegistryError::NotFound(_)) => {
                    // Registry miss — try as git source before giving up
                    match git_source::parse_git_source(package_spec, skill_select) {
                        Ok(source) => {
                            println!("  Not found in registry, trying as git repository...");
                            run_install_git(&source, agent_filter, global, skill_select).await
                        }
                        Err(_) => {
                            // Git parse also failed — report the original registry error
                            Err(RegistryError::NotFound(format!(
                                "Package '{}' not found in registry",
                                spec
                            )))
                        }
                    }
                }
                Err(e) => Err(e),
            }
        }
    }
}

/// Install a package from a local directory path.
async fn run_install_local(
    local_path: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<(), RegistryError> {
    let dir = Path::new(local_path).canonicalize().map_err(|e| {
        RegistryError::Validation(format!("Cannot resolve path '{}': {}", local_path, e))
    })?;

    if !dir.is_dir() {
        return Err(RegistryError::Validation(format!(
            "'{}' is not a directory",
            local_path
        )));
    }

    // Detect package type
    let pkg_type = package_type::detect_package_type(&dir).ok_or_else(|| {
        RegistryError::Validation(format!(
            "Cannot determine package type in '{}'. Expected SKILL.md, VALIDATOR.md + rules/, TOOL.md, or .claude-plugin/plugin.json",
            local_path
        ))
    })?;

    // Read name and version from frontmatter (or plugin.json for plugins)
    let (name, version) = match pkg_type {
        PackageType::Skill => read_frontmatter(&dir.join("SKILL.md"))?,
        PackageType::Validator => read_frontmatter(&dir.join("VALIDATOR.md"))?,
        PackageType::Tool => read_frontmatter(&dir.join("TOOL.md"))?,
        PackageType::Plugin => {
            let plugin_name =
                mcp_config::read_plugin_json(&dir.join(".claude-plugin/plugin.json"))?;
            (plugin_name, "0.0.0".to_string())
        }
    };

    println!("Installing {} from local path ({})...", name, pkg_type);

    let targets = match pkg_type {
        PackageType::Skill => deploy_skill(&name, &dir, agent_filter, global).await?,
        PackageType::Validator => deploy_validator(&name, &dir, global)?,
        PackageType::Tool => deploy_tool(&name, &dir, agent_filter, global)?,
        PackageType::Plugin => deploy_plugin(&name, &dir, agent_filter, global)?,
    };

    // Update lockfile
    let project_root = std::env::current_dir()?;
    let mut lf = Lockfile::load(&project_root)?;
    lf.add_package(
        name.clone(),
        LockedPackage {
            package_type: pkg_type,
            version: version.clone(),
            resolved: format!("file:{}", dir.display()),
            integrity: String::new(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            targets: targets.clone(),
        },
    );
    lf.save(&project_root)?;
    println!("  Updated mirdan-lock.json");

    println!(
        "\nInstalled {}@{} ({}) from local path",
        name, version, pkg_type
    );
    for target in &targets {
        println!("  -> {}", target);
    }

    Ok(())
}

/// Install packages from a git repository.
///
/// Clones the repo, discovers packages, and deploys each one.
async fn run_install_git(
    source: &git_source::GitSource,
    agent_filter: Option<&str>,
    global: bool,
    skill_select: Option<&str>,
) -> Result<(), RegistryError> {
    println!("Cloning {}...", source.display_name);

    let temp_dir = git_source::git_clone(source)?;

    // Merge select from GitSource and the --skill flag (--skill takes precedence)
    let select = skill_select.or(source.select.as_deref());

    let packages =
        git_source::discover_packages(temp_dir.path(), source.subpath.as_deref(), select)?;

    println!(
        "  Found {} package(s) in {}",
        packages.len(),
        source.display_name
    );
    for pkg in &packages {
        println!("    - {} ({})", pkg.name, pkg.package_type);
    }

    let project_root = std::env::current_dir()?;
    let mut lf = Lockfile::load(&project_root)?;

    for pkg in &packages {
        println!("\nInstalling {} ({})...", pkg.name, pkg.package_type);

        let targets = match pkg.package_type {
            PackageType::Skill => deploy_skill(&pkg.name, &pkg.path, agent_filter, global).await?,
            PackageType::Validator => deploy_validator(&pkg.name, &pkg.path, global)?,
            PackageType::Tool => deploy_tool(&pkg.name, &pkg.path, agent_filter, global)?,
            PackageType::Plugin => deploy_plugin(&pkg.name, &pkg.path, agent_filter, global)?,
        };

        // Read version from frontmatter (or plugin.json for plugins)
        let version = match pkg.package_type {
            PackageType::Skill => read_frontmatter(&pkg.path.join("SKILL.md"))
                .map(|(_, v)| v)
                .unwrap_or_else(|_| "0.0.0".to_string()),
            PackageType::Validator => read_frontmatter(&pkg.path.join("VALIDATOR.md"))
                .map(|(_, v)| v)
                .unwrap_or_else(|_| "0.0.0".to_string()),
            PackageType::Tool => read_frontmatter(&pkg.path.join("TOOL.md"))
                .map(|(_, v)| v)
                .unwrap_or_else(|_| "0.0.0".to_string()),
            PackageType::Plugin => "0.0.0".to_string(),
        };

        lf.add_package(
            pkg.name.clone(),
            LockedPackage {
                package_type: pkg.package_type,
                version: version.clone(),
                resolved: format!("git+{}", source.clone_url),
                integrity: String::new(),
                installed_at: chrono::Utc::now().to_rfc3339(),
                targets: targets.clone(),
            },
        );

        println!(
            "Installed {}@{} ({}) from git",
            pkg.name, version, pkg.package_type
        );
        for target in &targets {
            println!("  -> {}", target);
        }
    }

    lf.save(&project_root)?;
    println!("  Updated mirdan-lock.json");

    // temp_dir drops here, cleaning up the clone
    Ok(())
}

/// Read name and version from YAML frontmatter of a markdown file.
fn read_frontmatter(path: &Path) -> Result<(String, String), RegistryError> {
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
    let yaml: serde_yaml::Value = serde_yaml::from_str(frontmatter)
        .map_err(|e| RegistryError::Validation(format!("Invalid YAML frontmatter: {}", e)))?;

    let name = yaml
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RegistryError::Validation("Missing 'name' in frontmatter".to_string()))?
        .to_string();

    let version = yaml
        .get("metadata")
        .and_then(|m| m.get("version"))
        .and_then(|v| v.as_str())
        .or_else(|| yaml.get("version").and_then(|v| v.as_str()))
        .unwrap_or("0.0.0")
        .to_string();

    Ok((name, version))
}

/// Install a package from the registry.
async fn run_install_registry(
    package_spec: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<(), RegistryError> {
    let (name, version) = parse_package_spec(package_spec);

    let client = RegistryClient::authenticated()?;

    // Resolve version
    let version_detail = if let Some(ref ver) = version {
        println!("Resolving {}@{}...", name, ver);
        client.version_info(&name, ver).await?
    } else {
        println!("Resolving {} (latest)...", name);
        client.latest_version(&name).await?
    };

    let resolved_version = &version_detail.version;
    println!("Installing {}@{}...", name, resolved_version);

    // Download with progress
    let pb = ProgressBar::new(version_detail.size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{bar:40}] {bytes}/{total_bytes}")
            .unwrap()
            .progress_chars("=> "),
    );
    pb.set_message("Downloading");

    let data = client.download(&name, resolved_version).await?;
    pb.set_position(data.len() as u64);
    pb.finish_with_message("Downloaded");

    // Verify integrity
    lockfile::verify_integrity(&data, &version_detail.integrity)
        .map_err(RegistryError::Integrity)?;
    println!("  Integrity verified");

    // Extract to temp dir to detect type
    let temp_dir = tempfile::tempdir()?;
    extract_zip(&data, temp_dir.path())?;

    let pkg_type = package_type::detect_package_type(temp_dir.path()).ok_or_else(|| {
        RegistryError::Validation(
            "Cannot determine package type. Expected SKILL.md, VALIDATOR.md + rules/, TOOL.md, or .claude-plugin/plugin.json".to_string(),
        )
    })?;

    let targets = match pkg_type {
        PackageType::Skill => deploy_skill(&name, temp_dir.path(), agent_filter, global).await?,
        PackageType::Validator => deploy_validator(&name, temp_dir.path(), global)?,
        PackageType::Tool => deploy_tool(&name, temp_dir.path(), agent_filter, global)?,
        PackageType::Plugin => deploy_plugin(&name, temp_dir.path(), agent_filter, global)?,
    };

    // Update lockfile
    let project_root = std::env::current_dir()?;
    let mut lf = Lockfile::load(&project_root)?;
    lf.add_package(
        name.clone(),
        LockedPackage {
            package_type: pkg_type,
            version: resolved_version.clone(),
            resolved: version_detail.download_url.clone(),
            integrity: version_detail.integrity.clone(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            targets: targets.clone(),
        },
    );
    lf.save(&project_root)?;
    println!("  Updated mirdan-lock.json");

    println!("\nInstalled {}@{} ({})", name, resolved_version, pkg_type);
    for target in &targets {
        println!("  -> {}", target);
    }

    Ok(())
}

/// Deploy a skill to the central store, then symlink into each agent's skill directory.
async fn deploy_skill(
    name: &str,
    source_dir: &Path,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<Vec<String>, RegistryError> {
    let config = agents::load_agents_config()?;
    let agents = agents::resolve_target_agents(&config, agent_filter)?;

    if agents.is_empty() {
        return Err(RegistryError::Validation(
            "No agents detected. Run 'mirdan agents' to check.".to_string(),
        ));
    }

    // 1. Copy source into the central store
    let sanitized = sanitize_dir_name(name);
    let store_path = store::skill_store_dir(global).join(&sanitized);

    // Remove existing store entry
    store::remove_if_exists(&store_path)?;

    copy_dir_recursive(source_dir, &store_path)?;
    println!("  Stored in {}", store_path.display());

    // 2. Create symlinks from each agent's skill directory
    let mut targets = Vec::new();

    for agent in &agents {
        let link_name = store::symlink_name(&sanitized, &agent.def.symlink_policy);
        let agent_skill_dir = if global {
            agent_global_skill_dir(&agent.def)
        } else {
            agent_project_skill_dir(&agent.def)
        };
        let link_path = agent_skill_dir.join(&link_name);

        // Remove existing (real dir or stale symlink)
        store::remove_if_exists(&link_path)?;

        store::create_skill_link(&store_path, &link_path)?;
        println!(
            "  Linked {} -> {} ({})",
            link_path.display(),
            store_path.display(),
            agent.def.name
        );
        targets.push(agent.def.id.clone());
    }

    Ok(targets)
}

/// Deploy a validator to .avp/validators/.
fn deploy_validator(
    name: &str,
    source_dir: &Path,
    global: bool,
) -> Result<Vec<String>, RegistryError> {
    let target_dir = validators_dir(global).join(sanitize_dir_name(name));

    if target_dir.exists() {
        std::fs::remove_dir_all(&target_dir)?;
    }

    copy_dir_recursive(source_dir, &target_dir)?;
    let target_path = target_dir.display().to_string();
    println!("  Deployed to {}", target_path);

    Ok(vec![target_path])
}

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
) -> Result<(), RegistryError> {
    let project_root = std::env::current_dir()?;
    let lf = Lockfile::load(&project_root)?;

    // If not a direct package name, check if it's a git source and find matching packages
    if lf.get_package(name).is_none() {
        let matching = find_packages_by_git_source(&lf, name);

        if !matching.is_empty() {
            let mut lf = Lockfile::load(&project_root)?;
            for pkg_name in &matching {
                let pkg = lf.get_package(pkg_name).unwrap();
                let pkg_type = pkg.package_type;
                match pkg_type {
                    PackageType::Skill => uninstall_skill(pkg_name, agent_filter, global)?,
                    PackageType::Validator => uninstall_validator(pkg_name, global)?,
                    PackageType::Tool => uninstall_tool(pkg_name, agent_filter, global)?,
                    PackageType::Plugin => uninstall_plugin(pkg_name, agent_filter, global)?,
                }
                lf.remove_package(pkg_name);
                println!("  Uninstalled {}", pkg_name);
            }
            lf.save(&project_root)?;
            println!("  Updated mirdan-lock.json");
            println!("\nUninstalled {} package(s) from {}", matching.len(), name);
            return Ok(());
        }
    }

    // Direct package name lookup
    let pkg_type = lf
        .get_package(name)
        .map(|p| p.package_type)
        .unwrap_or_else(|| {
            // Try to detect from installed locations
            guess_installed_type(name, global)
        });

    match pkg_type {
        PackageType::Skill => uninstall_skill(name, agent_filter, global)?,
        PackageType::Validator => uninstall_validator(name, global)?,
        PackageType::Tool => uninstall_tool(name, agent_filter, global)?,
        PackageType::Plugin => uninstall_plugin(name, agent_filter, global)?,
    }

    // Update lockfile
    let mut lf = Lockfile::load(&project_root)?;
    lf.remove_package(name);
    lf.save(&project_root)?;
    println!("  Updated mirdan-lock.json");
    println!("\nUninstalled {}", name);

    Ok(())
}

fn uninstall_skill(
    name: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<(), RegistryError> {
    let config = agents::load_agents_config()?;
    let agents = agents::resolve_target_agents(&config, agent_filter)?;

    let sanitized = sanitize_dir_name(name);

    // 1. Remove symlinks from each agent's skill directory
    let mut removed = 0;
    for agent in &agents {
        let link_name = store::symlink_name(&sanitized, &agent.def.symlink_policy);
        let agent_skill_dir = if global {
            agent_global_skill_dir(&agent.def)
        } else {
            agent_project_skill_dir(&agent.def)
        };
        let link_path = agent_skill_dir.join(&link_name);

        // Check if the path exists (symlink or real dir)
        if std::fs::symlink_metadata(&link_path).is_ok() {
            store::remove_if_exists(&link_path)?;
            println!(
                "  Removed from {} ({})",
                link_path.display(),
                agent.def.name
            );
            removed += 1;
        }
    }

    if removed == 0 {
        let scope = if global { "global" } else { "project" };
        return Err(RegistryError::NotFound(format!(
            "Skill '{}' not found in any agent ({} scope)",
            name, scope
        )));
    }

    // 2. Remove store entry if no remaining symlinks reference it
    let store_path = store::skill_store_dir(global).join(&sanitized);
    if store_path.exists() {
        // Collect all agent skill dirs to check for remaining references
        let all_agents = agents::get_detected_agents(&config);
        let all_skill_dirs: Vec<PathBuf> = all_agents
            .iter()
            .map(|a| {
                if global {
                    agent_global_skill_dir(&a.def)
                } else {
                    agent_project_skill_dir(&a.def)
                }
            })
            .collect();

        if !store::store_entry_still_referenced(&store_path, &all_skill_dirs) {
            std::fs::remove_dir_all(&store_path)?;
            println!("  Removed store entry {}", store_path.display());
        }
    }

    Ok(())
}

fn uninstall_validator(name: &str, global: bool) -> Result<(), RegistryError> {
    let target_dir = validators_dir(global).join(sanitize_dir_name(name));

    if !target_dir.exists() {
        let scope = if global { "global" } else { "project" };
        return Err(RegistryError::NotFound(format!(
            "Validator '{}' not found ({} scope)",
            name, scope
        )));
    }

    std::fs::remove_dir_all(&target_dir)?;
    println!("  Removed from {}", target_dir.display());
    Ok(())
}

/// Guess the package type based on what's installed.
fn guess_installed_type(name: &str, global: bool) -> PackageType {
    // Check validator dir first
    if validators_dir(global)
        .join(sanitize_dir_name(name))
        .exists()
    {
        return PackageType::Validator;
    }
    // Check tool store
    if store::tool_store_dir(global)
        .join(sanitize_dir_name(name))
        .exists()
    {
        return PackageType::Tool;
    }
    // Check plugin dirs
    if let Ok(config) = agents::load_agents_config() {
        for agent in &config.agents {
            let plugin_dir = if global {
                agents::agent_global_plugin_dir(agent)
            } else {
                agents::agent_project_plugin_dir(agent)
            };
            if let Some(dir) = plugin_dir {
                if dir.join(sanitize_dir_name(name)).exists() {
                    return PackageType::Plugin;
                }
            }
        }
    }
    // Default to skill
    PackageType::Skill
}

/// Deploy a tool to the central tool store and register in agent MCP configs.
fn deploy_tool(
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
    println!("  Stored in {}", store_path.display());

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
                    &mcp_cfg.servers_key,
                    name,
                    &entry,
                )?;
                println!(
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

    Ok(targets)
}

/// Deploy a plugin to agent plugin directories.
fn deploy_plugin(
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
            println!(
                "  Deployed to {} ({})",
                target.display(),
                agent.def.name
            );
            targets.push(agent.def.id.clone());

            // If plugin contains .mcp.json, also register those MCP servers
            let plugin_mcp = target.join(".mcp.json");
            if plugin_mcp.exists() {
                if let Some(ref mcp_cfg) = agent.def.mcp_config {
                    let config_path = if global {
                        agents::agent_global_mcp_config(&agent.def)
                    } else {
                        agents::agent_project_mcp_config(&agent.def)
                    };

                    if let Some(config_path) = config_path {
                        // Read and register MCP servers from plugin's .mcp.json
                        if let Ok(content) = std::fs::read_to_string(&plugin_mcp) {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(servers) =
                                    json.get(&mcp_cfg.servers_key).and_then(|s| s.as_object())
                                {
                                    for (server_name, server_def) in servers {
                                        if let Ok(entry) =
                                            serde_json::from_value::<mcp_config::McpServerEntry>(
                                                server_def.clone(),
                                            )
                                        {
                                            let _ = mcp_config::register_mcp_server(
                                                &config_path,
                                                &mcp_cfg.servers_key,
                                                server_name,
                                                &entry,
                                            );
                                            println!(
                                                "  Registered MCP server '{}' from plugin",
                                                server_name
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else {
            tracing::debug!(
                "Agent {} has no plugin path, skipping",
                agent.def.id
            );
        }
    }

    if targets.is_empty() {
        return Err(RegistryError::Validation(
            "No agents with plugin support detected. Plugins are currently supported by Claude Code."
                .to_string(),
        ));
    }

    Ok(targets)
}

fn uninstall_tool(
    name: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<(), RegistryError> {
    let config = agents::load_agents_config()?;
    let agents = agents::resolve_target_agents(&config, agent_filter)?;

    let sanitized = sanitize_dir_name(name);

    // 1. Unregister from each agent's MCP config
    let mut removed = 0;
    for agent in &agents {
        if let Some(ref mcp_cfg) = agent.def.mcp_config {
            let config_path = if global {
                agents::agent_global_mcp_config(&agent.def)
            } else {
                agents::agent_project_mcp_config(&agent.def)
            };

            if let Some(config_path) = config_path {
                if mcp_config::unregister_mcp_server(&config_path, &mcp_cfg.servers_key, name)? {
                    println!(
                        "  Unregistered from {} ({})",
                        config_path.display(),
                        agent.def.name
                    );
                    removed += 1;
                }
            }
        }
    }

    // 2. Remove from tool store
    let store_path = store::tool_store_dir(global).join(&sanitized);
    if store_path.exists() {
        std::fs::remove_dir_all(&store_path)?;
        println!("  Removed store entry {}", store_path.display());
        removed += 1;
    }

    if removed == 0 {
        let scope = if global { "global" } else { "project" };
        return Err(RegistryError::NotFound(format!(
            "Tool '{}' not found ({} scope)",
            name, scope
        )));
    }

    Ok(())
}

fn uninstall_plugin(
    name: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<(), RegistryError> {
    let config = agents::load_agents_config()?;
    let agents = agents::resolve_target_agents(&config, agent_filter)?;

    let sanitized = sanitize_dir_name(name);
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
                println!(
                    "  Removed from {} ({})",
                    target.display(),
                    agent.def.name
                );
                removed += 1;
            }
        }
    }

    if removed == 0 {
        let scope = if global { "global" } else { "project" };
        return Err(RegistryError::NotFound(format!(
            "Plugin '{}' not found ({} scope)",
            name, scope
        )));
    }

    Ok(())
}

/// Install a specific package version (used by update command).
pub async fn install_package(
    name: &str,
    version: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<(), RegistryError> {
    let spec = format!("{}@{}", name, version);
    run_install(&spec, agent_filter, global, false, None).await
}

/// Parse a package spec like "name" or "name@version".
pub fn parse_package_spec(spec: &str) -> (String, Option<String>) {
    if let Some((name, version)) = spec.rsplit_once('@') {
        (name.to_string(), Some(version.to_string()))
    } else {
        (spec.to_string(), None)
    }
}

/// Get the validators directory path.
pub fn validators_dir(global: bool) -> PathBuf {
    if global {
        dirs::home_dir()
            .expect("Could not find home directory")
            .join(".avp")
            .join("validators")
    } else {
        PathBuf::from(".avp").join("validators")
    }
}

/// Extract a ZIP archive to a target directory with path traversal protection.
fn extract_zip(data: &[u8], target_dir: &Path) -> Result<(), RegistryError> {
    let cursor = Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| RegistryError::Validation(format!("Invalid ZIP archive: {}", e)))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| RegistryError::Validation(format!("ZIP read error: {}", e)))?;

        let name = file.name().to_string();

        // Path traversal protection
        if name.contains("..") || name.starts_with('/') || name.starts_with('\\') {
            return Err(RegistryError::Validation(format!(
                "Unsafe path in ZIP: {}",
                name
            )));
        }

        // Skip the top-level directory wrapper if present
        let relative_path = if let Some((_prefix, rest)) = name.split_once('/') {
            if rest.is_empty() {
                continue;
            }
            PathBuf::from(rest)
        } else {
            PathBuf::from(&name)
        };

        let target_path = target_dir.join(&relative_path);

        if file.is_dir() {
            std::fs::create_dir_all(&target_path)?;
        } else {
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut outfile = std::fs::File::create(&target_path)?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf).map_err(RegistryError::Io)?;
            std::io::Write::write_all(&mut outfile, &buf)?;
        }
    }

    Ok(())
}

/// Recursively copy a directory.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), RegistryError> {
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_package_spec_name_only() {
        let (name, version) = parse_package_spec("no-secrets");
        assert_eq!(name, "no-secrets");
        assert_eq!(version, None);
    }

    #[test]
    fn test_parse_package_spec_with_version() {
        let (name, version) = parse_package_spec("no-secrets@1.2.3");
        assert_eq!(name, "no-secrets");
        assert_eq!(version, Some("1.2.3".to_string()));
    }

    #[test]
    fn test_validators_dir_local() {
        let dir = validators_dir(false);
        assert_eq!(dir, PathBuf::from(".avp/validators"));
    }

    #[test]
    fn test_validators_dir_global() {
        let dir = validators_dir(true);
        assert!(dir.ends_with(".avp/validators"));
        let home = dirs::home_dir().unwrap();
        assert!(dir.starts_with(home));
    }

    #[test]
    fn test_sanitize_dir_name_url() {
        assert_eq!(
            sanitize_dir_name("https://github.com/anthropics/skills/algorithmic-art"),
            "anthropics/skills/algorithmic-art"
        );
    }

    #[test]
    fn test_sanitize_dir_name_http() {
        assert_eq!(sanitize_dir_name("http://example.com/foo/bar"), "foo/bar");
    }

    #[test]
    fn test_sanitize_dir_name_plain() {
        assert_eq!(sanitize_dir_name("no-secrets"), "no-secrets");
    }

    #[test]
    fn test_sanitize_dir_name_host_only() {
        assert_eq!(sanitize_dir_name("https://github.com"), "github.com");
    }

    #[test]
    fn test_read_frontmatter_skill() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("SKILL.md");
        std::fs::write(
            &md,
            "---\nname: test-skill\nmetadata:\n  version: \"1.2.3\"\n---\n# Test\n",
        )
        .unwrap();

        let (name, version) = read_frontmatter(&md).unwrap();
        assert_eq!(name, "test-skill");
        assert_eq!(version, "1.2.3");
    }

    #[test]
    fn test_read_frontmatter_missing_version_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("SKILL.md");
        std::fs::write(&md, "---\nname: test-skill\n---\n# Test\n").unwrap();

        let (name, version) = read_frontmatter(&md).unwrap();
        assert_eq!(name, "test-skill");
        assert_eq!(version, "0.0.0");
    }

    #[test]
    fn test_read_frontmatter_metadata_version() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("SKILL.md");
        std::fs::write(
            &md,
            "---\nname: test-skill\nmetadata:\n  version: \"2.0.0\"\n---\n# Test\n",
        )
        .unwrap();

        let (name, version) = read_frontmatter(&md).unwrap();
        assert_eq!(name, "test-skill");
        assert_eq!(version, "2.0.0");
    }

    #[test]
    fn test_read_frontmatter_metadata_preferred() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("SKILL.md");
        std::fs::write(
            &md,
            "---\nname: test-skill\nversion: \"1.0.0\"\nmetadata:\n  version: \"2.0.0\"\n---\n# Test\n",
        )
        .unwrap();

        let (name, version) = read_frontmatter(&md).unwrap();
        assert_eq!(name, "test-skill");
        assert_eq!(version, "2.0.0");
    }

    #[test]
    fn test_read_frontmatter_missing_name_errors() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("SKILL.md");
        std::fs::write(&md, "---\nmetadata:\n  version: \"1.0.0\"\n---\n# Test\n").unwrap();

        assert!(read_frontmatter(&md).is_err());
    }

    #[test]
    fn test_read_frontmatter_no_frontmatter_errors() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("SKILL.md");
        std::fs::write(&md, "# Just markdown\nNo frontmatter here.\n").unwrap();

        assert!(read_frontmatter(&md).is_err());
    }

    #[test]
    fn test_copy_dir_recursive() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        let dst_path = dst.path().join("copy");

        std::fs::write(src.path().join("file.txt"), "hello").unwrap();
        std::fs::create_dir(src.path().join("sub")).unwrap();
        std::fs::write(src.path().join("sub/nested.txt"), "world").unwrap();

        copy_dir_recursive(src.path(), &dst_path).unwrap();

        assert!(dst_path.join("file.txt").exists());
        assert!(dst_path.join("sub/nested.txt").exists());
        assert_eq!(
            std::fs::read_to_string(dst_path.join("file.txt")).unwrap(),
            "hello"
        );
    }

    // --- local skill: create, detect, read frontmatter, deploy as validator ---

    /// Helper: create a local skill directory with SKILL.md.
    fn make_local_skill(dir: &Path, name: &str, version: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            format!(
                "---\nname: {}\nmetadata:\n  version: \"{}\"\n---\n# {}\nA test skill.\n",
                name, version, name
            ),
        )
        .unwrap();
    }

    /// Helper: create a local validator directory with VALIDATOR.md + rules/.
    fn make_local_validator(dir: &Path, name: &str, version: &str) {
        std::fs::create_dir_all(dir.join("rules")).unwrap();
        std::fs::write(
            dir.join("VALIDATOR.md"),
            format!(
                "---\nname: {}\nmetadata:\n  version: \"{}\"\n---\n# {}\nA test validator.\n",
                name, version, name
            ),
        )
        .unwrap();
        std::fs::write(
            dir.join("rules/no-secrets.md"),
            "# No Secrets\nDon't commit secrets.\n",
        )
        .unwrap();
    }

    /// Helper: create a local tool directory with a realistic TOOL.md.
    ///
    /// Uses @modelcontextprotocol/server-filesystem as the MCP server —
    /// a real, published npm package that implements the MCP protocol for
    /// filesystem access.
    fn make_local_tool(dir: &Path, name: &str, version: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("TOOL.md"),
            format!(
                r#"---
name: {name}
description: "MCP server for filesystem access"
metadata:
  version: "{version}"
mcp:
  command: npx
  args:
    - "-y"
    - "@modelcontextprotocol/server-filesystem"
    - "/tmp/safe-dir"
  transport: stdio
  env:
    NODE_ENV: production
---

# {name}

An MCP tool that provides filesystem access to AI coding agents
via the Model Context Protocol.

## What This Tool Does

Exposes read/write filesystem operations through MCP so agents can
work with files in a controlled directory.
"#,
                name = name,
                version = version,
            ),
        )
        .unwrap();
        std::fs::write(
            dir.join("README.md"),
            format!("# {}\n\nAn MCP filesystem tool.\n", name),
        )
        .unwrap();
    }

    /// Helper: create a local plugin directory with .claude-plugin/plugin.json.
    ///
    /// Creates a realistic Claude Code plugin with a command and an
    /// optional bundled .mcp.json.
    fn make_local_plugin(dir: &Path, name: &str, with_mcp: bool) {
        let plugin_meta = dir.join(".claude-plugin");
        let commands_dir = dir.join("commands");
        std::fs::create_dir_all(&plugin_meta).unwrap();
        std::fs::create_dir_all(&commands_dir).unwrap();

        std::fs::write(
            plugin_meta.join("plugin.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "name": name,
                "description": "A test plugin for e2e testing",
                "author": { "name": "test" }
            }))
            .unwrap(),
        )
        .unwrap();

        std::fs::write(
            commands_dir.join("greet.md"),
            format!(
                "---\ndescription: \"Say hello from {name}\"\nallowed-tools:\n  - Read\n---\n\n\
                 # Greet\n\nSay hello to the user.\n",
                name = name,
            ),
        )
        .unwrap();

        if with_mcp {
            std::fs::write(
                dir.join(".mcp.json"),
                serde_json::to_string_pretty(&serde_json::json!({
                    "mcpServers": {
                        format!("{}-server", name): {
                            "command": "node",
                            "args": ["./server.js"]
                        }
                    }
                }))
                .unwrap(),
            )
            .unwrap();
        }

        std::fs::write(
            dir.join("README.md"),
            format!("# {}\n\nA test plugin.\n", name),
        )
        .unwrap();
    }

    // --- tool: detection, frontmatter, deploy, uninstall ---

    #[test]
    fn test_local_tool_detection_and_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let tool_dir = dir.path().join("fs-tool");
        make_local_tool(&tool_dir, "fs-tool", "1.0.0");

        // detect_package_type recognises it as a Tool
        let pkg_type = package_type::detect_package_type(&tool_dir);
        assert_eq!(pkg_type, Some(PackageType::Tool));

        // read_frontmatter extracts name + version
        let (name, version) = read_frontmatter(&tool_dir.join("TOOL.md")).unwrap();
        assert_eq!(name, "fs-tool");
        assert_eq!(version, "1.0.0");

        // MCP frontmatter parses correctly
        let yaml = mcp_config::parse_yaml_frontmatter(&tool_dir.join("TOOL.md")).unwrap();
        let mcp_fm = mcp_config::parse_tool_frontmatter(&yaml).unwrap();
        assert_eq!(mcp_fm.command, "npx");
        assert_eq!(
            mcp_fm.args,
            vec!["-y", "@modelcontextprotocol/server-filesystem", "/tmp/safe-dir"]
        );
        assert_eq!(mcp_fm.transport, Some("stdio".to_string()));
        assert_eq!(mcp_fm.env.get("NODE_ENV").unwrap(), "production");
    }

    #[test]
    fn test_deploy_tool_creates_store_and_mcp_json() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        // Create a source tool with a real MCP server reference
        let src = work.path().join("src-tool");
        make_local_tool(&src, "fs-tool", "1.0.0");

        // Deploy it (non-global)
        let targets = deploy_tool("fs-tool", &src, None, false).unwrap();
        // claude-code has mcp_config, so it should be a target
        assert!(
            targets.contains(&"claude-code".to_string()),
            "claude-code should be in targets: {:?}",
            targets
        );

        // 1. Verify tool store: .tools/fs-tool/ has TOOL.md + README.md
        let store = work.path().join(".tools/fs-tool");
        assert!(store.join("TOOL.md").exists(), "TOOL.md should be in store");
        assert!(
            store.join("README.md").exists(),
            "README.md should be in store"
        );

        // Verify the stored TOOL.md is byte-identical to the source
        let src_content = std::fs::read_to_string(src.join("TOOL.md")).unwrap();
        let store_content = std::fs::read_to_string(store.join("TOOL.md")).unwrap();
        assert_eq!(src_content, store_content, "Store copy should match source");

        // 2. Verify .mcp.json was created with the correct MCP server entry
        let mcp_json_path = work.path().join(".mcp.json");
        assert!(mcp_json_path.exists(), ".mcp.json should exist");
        let mcp_content = std::fs::read_to_string(&mcp_json_path).unwrap();
        let mcp_json: serde_json::Value = serde_json::from_str(&mcp_content).unwrap();

        // The entry should be under mcpServers.fs-tool
        let server = &mcp_json["mcpServers"]["fs-tool"];
        assert_eq!(
            server["command"].as_str().unwrap(),
            "npx",
            "command should be npx"
        );
        let args: Vec<&str> = server["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(
            args,
            vec!["-y", "@modelcontextprotocol/server-filesystem", "/tmp/safe-dir"],
            "args should match TOOL.md"
        );
        assert_eq!(
            server["env"]["NODE_ENV"].as_str().unwrap(),
            "production",
            "env should be passed through"
        );

        std::env::set_current_dir(old_dir).unwrap();
    }

    #[test]
    fn test_deploy_and_uninstall_tool() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        // Deploy
        let src = work.path().join("src-tool");
        make_local_tool(&src, "fs-tool", "1.0.0");
        deploy_tool("fs-tool", &src, None, false).unwrap();

        let store = work.path().join(".tools/fs-tool");
        let mcp_json_path = work.path().join(".mcp.json");
        assert!(store.exists());
        assert!(mcp_json_path.exists());

        // Verify server is registered before uninstall
        let mcp: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&mcp_json_path).unwrap()).unwrap();
        assert!(
            mcp["mcpServers"]["fs-tool"].is_object(),
            "Server should be registered"
        );

        // Uninstall
        uninstall_tool("fs-tool", None, false).unwrap();

        // Store entry should be gone
        assert!(!store.exists(), "Tool store entry should be removed");

        // MCP server entry should be gone, but mcpServers key remains
        let mcp: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&mcp_json_path).unwrap()).unwrap();
        assert!(
            mcp["mcpServers"]["fs-tool"].is_null(),
            "Server entry should be removed from .mcp.json"
        );

        std::env::set_current_dir(old_dir).unwrap();
    }

    #[test]
    fn test_deploy_tool_preserves_existing_mcp_servers() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        // Pre-populate .mcp.json with an existing server
        std::fs::write(
            work.path().join(".mcp.json"),
            r#"{
  "mcpServers": {
    "existing-server": {
      "command": "node",
      "args": ["./existing.js"]
    }
  }
}"#,
        )
        .unwrap();

        // Deploy a new tool
        let src = work.path().join("src-tool");
        make_local_tool(&src, "fs-tool", "1.0.0");
        deploy_tool("fs-tool", &src, None, false).unwrap();

        // Both servers should be present
        let mcp: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(work.path().join(".mcp.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            mcp["mcpServers"]["existing-server"]["command"]
                .as_str()
                .unwrap(),
            "node",
            "Existing server should be preserved"
        );
        assert_eq!(
            mcp["mcpServers"]["fs-tool"]["command"].as_str().unwrap(),
            "npx",
            "New tool should be added"
        );

        // Uninstall only the new tool
        uninstall_tool("fs-tool", None, false).unwrap();

        // Existing server should still be there
        let mcp: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(work.path().join(".mcp.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            mcp["mcpServers"]["existing-server"]["command"]
                .as_str()
                .unwrap(),
            "node",
            "Existing server should survive uninstall of other tool"
        );
        assert!(
            mcp["mcpServers"]["fs-tool"].is_null(),
            "Uninstalled tool should be gone"
        );

        std::env::set_current_dir(old_dir).unwrap();
    }

    #[test]
    fn test_uninstall_tool_not_found() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        let result = uninstall_tool("nonexistent-tool", None, false);
        assert!(matches!(result.unwrap_err(), RegistryError::NotFound(_)));

        std::env::set_current_dir(old_dir).unwrap();
    }

    // --- plugin: detection, deploy, uninstall ---

    #[test]
    fn test_local_plugin_detection() {
        let dir = tempfile::tempdir().unwrap();
        let plugin_dir = dir.path().join("my-plugin");
        make_local_plugin(&plugin_dir, "my-plugin", false);

        let pkg_type = package_type::detect_package_type(&plugin_dir);
        assert_eq!(pkg_type, Some(PackageType::Plugin));

        // Read name from plugin.json
        let name = mcp_config::read_plugin_json(
            &plugin_dir.join(".claude-plugin/plugin.json"),
        )
        .unwrap();
        assert_eq!(name, "my-plugin");
    }

    #[test]
    fn test_deploy_plugin_creates_files() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        let src = work.path().join("src-plugin");
        make_local_plugin(&src, "test-plugin", false);

        let targets = deploy_plugin("test-plugin", &src, None, false).unwrap();
        assert!(
            targets.contains(&"claude-code".to_string()),
            "claude-code should be in targets: {:?}",
            targets
        );

        // Verify the plugin was copied to .claude/plugins/test-plugin/
        let deployed = work.path().join(".claude/plugins/test-plugin");
        assert!(deployed.exists(), "Plugin dir should exist");
        assert!(
            deployed.join(".claude-plugin/plugin.json").exists(),
            "plugin.json should be deployed"
        );
        assert!(
            deployed.join("commands/greet.md").exists(),
            "Commands should be deployed"
        );
        assert!(
            deployed.join("README.md").exists(),
            "README should be deployed"
        );

        // Verify plugin.json content is preserved
        let json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(deployed.join(".claude-plugin/plugin.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(json["name"].as_str().unwrap(), "test-plugin");

        std::env::set_current_dir(old_dir).unwrap();
    }

    #[test]
    fn test_deploy_and_uninstall_plugin() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        let src = work.path().join("src-plugin");
        make_local_plugin(&src, "test-plugin", false);
        deploy_plugin("test-plugin", &src, None, false).unwrap();

        let deployed = work.path().join(".claude/plugins/test-plugin");
        assert!(deployed.exists());

        uninstall_plugin("test-plugin", None, false).unwrap();
        assert!(!deployed.exists(), "Plugin dir should be removed");

        std::env::set_current_dir(old_dir).unwrap();
    }

    #[test]
    fn test_deploy_plugin_with_bundled_mcp() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        // Create a plugin that bundles an .mcp.json
        let src = work.path().join("src-plugin");
        make_local_plugin(&src, "mcp-plugin", true);

        deploy_plugin("mcp-plugin", &src, None, false).unwrap();

        // Plugin should be deployed
        let deployed = work.path().join(".claude/plugins/mcp-plugin");
        assert!(deployed.exists());

        // The bundled .mcp.json servers should be registered in the
        // project-level .mcp.json (claude-code's mcp_config.project_path)
        let mcp_json_path = work.path().join(".mcp.json");
        assert!(
            mcp_json_path.exists(),
            ".mcp.json should be created from bundled MCP servers"
        );
        let mcp: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&mcp_json_path).unwrap()).unwrap();
        assert!(
            mcp["mcpServers"]["mcp-plugin-server"].is_object(),
            "Bundled MCP server should be registered"
        );
        assert_eq!(
            mcp["mcpServers"]["mcp-plugin-server"]["command"]
                .as_str()
                .unwrap(),
            "node"
        );

        std::env::set_current_dir(old_dir).unwrap();
    }

    #[test]
    fn test_uninstall_plugin_not_found() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        let result = uninstall_plugin("nonexistent-plugin", None, false);
        assert!(matches!(result.unwrap_err(), RegistryError::NotFound(_)));

        std::env::set_current_dir(old_dir).unwrap();
    }

    // --- e2e: tool install → lockfile → list → uninstall ---

    #[test]
    fn test_e2e_tool_install_list_uninstall() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        // 1. Create and deploy a tool using @modelcontextprotocol/server-filesystem
        let src = work.path().join("src-tool");
        make_local_tool(&src, "fs-tool", "2.1.0");
        let targets = deploy_tool("fs-tool", &src, None, false).unwrap();

        // 2. Write lockfile (mimicking what run_install_local does)
        let mut lf = Lockfile::default();
        lf.add_package(
            "fs-tool".to_string(),
            LockedPackage {
                package_type: PackageType::Tool,
                version: "2.1.0".to_string(),
                resolved: format!("file:{}", src.display()),
                integrity: String::new(),
                installed_at: chrono::Utc::now().to_rfc3339(),
                targets: targets.clone(),
            },
        );
        lf.save(work.path()).unwrap();

        // 3. Verify lockfile round-trip
        let loaded = Lockfile::load(work.path()).unwrap();
        let pkg = loaded.get_package("fs-tool").unwrap();
        assert_eq!(pkg.package_type, PackageType::Tool);
        assert_eq!(pkg.version, "2.1.0");

        // 4. Verify on-disk state
        assert!(work.path().join(".tools/fs-tool/TOOL.md").exists());
        let mcp: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(work.path().join(".mcp.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(mcp["mcpServers"]["fs-tool"]["command"], "npx");

        // 5. Verify list discovers the tool
        let packages = crate::list::discover_packages(false, false, true, false, None);
        let tool_pkgs: Vec<_> = packages.iter().filter(|p| p.name == "fs-tool").collect();
        assert_eq!(tool_pkgs.len(), 1, "list --tools should find fs-tool");
        assert_eq!(tool_pkgs[0].package_type, PackageType::Tool);
        assert_eq!(tool_pkgs[0].version, "2.1.0");

        // 6. Uninstall and verify cleanup
        uninstall_tool("fs-tool", None, false).unwrap();
        assert!(!work.path().join(".tools/fs-tool").exists());
        let mcp: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(work.path().join(".mcp.json")).unwrap(),
        )
        .unwrap();
        assert!(mcp["mcpServers"]["fs-tool"].is_null());

        // 7. Clean lockfile
        let mut lf = Lockfile::load(work.path()).unwrap();
        lf.remove_package("fs-tool");
        lf.save(work.path()).unwrap();
        let lf = Lockfile::load(work.path()).unwrap();
        assert!(lf.packages.is_empty());

        std::env::set_current_dir(old_dir).unwrap();
    }

    #[test]
    fn test_e2e_plugin_install_list_uninstall() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        // 1. Create and deploy a plugin
        let src = work.path().join("src-plugin");
        make_local_plugin(&src, "test-plugin", false);
        let targets = deploy_plugin("test-plugin", &src, None, false).unwrap();

        // 2. Write lockfile
        let mut lf = Lockfile::default();
        lf.add_package(
            "test-plugin".to_string(),
            LockedPackage {
                package_type: PackageType::Plugin,
                version: "0.0.0".to_string(),
                resolved: format!("file:{}", src.display()),
                integrity: String::new(),
                installed_at: chrono::Utc::now().to_rfc3339(),
                targets: targets.clone(),
            },
        );
        lf.save(work.path()).unwrap();

        // 3. Verify lockfile round-trip
        let loaded = Lockfile::load(work.path()).unwrap();
        let pkg = loaded.get_package("test-plugin").unwrap();
        assert_eq!(pkg.package_type, PackageType::Plugin);

        // 4. Verify on-disk state
        let deployed = work.path().join(".claude/plugins/test-plugin");
        assert!(deployed.join(".claude-plugin/plugin.json").exists());
        assert!(deployed.join("commands/greet.md").exists());

        // 5. Verify list discovers the plugin
        let packages = crate::list::discover_packages(false, false, false, true, None);
        let plugin_pkgs: Vec<_> = packages
            .iter()
            .filter(|p| p.name == "test-plugin")
            .collect();
        assert_eq!(
            plugin_pkgs.len(),
            1,
            "list --plugins should find test-plugin"
        );
        assert_eq!(plugin_pkgs[0].package_type, PackageType::Plugin);

        // 6. Uninstall and verify cleanup
        uninstall_plugin("test-plugin", None, false).unwrap();
        assert!(
            !deployed.exists(),
            "Plugin dir should be removed after uninstall"
        );

        // 7. Clean lockfile
        let mut lf = Lockfile::load(work.path()).unwrap();
        lf.remove_package("test-plugin");
        lf.save(work.path()).unwrap();
        let lf = Lockfile::load(work.path()).unwrap();
        assert!(lf.packages.is_empty());

        std::env::set_current_dir(old_dir).unwrap();
    }

    #[test]
    fn test_local_skill_detection_and_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("my-skill");
        make_local_skill(&skill_dir, "my-skill", "1.2.3");

        // detect_package_type recognises it as a Skill
        let pkg_type = package_type::detect_package_type(&skill_dir);
        assert_eq!(pkg_type, Some(PackageType::Skill));

        // read_frontmatter extracts name + version
        let (name, version) = read_frontmatter(&skill_dir.join("SKILL.md")).unwrap();
        assert_eq!(name, "my-skill");
        assert_eq!(version, "1.2.3");
    }

    #[test]
    fn test_local_validator_detection_and_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let val_dir = dir.path().join("my-validator");
        make_local_validator(&val_dir, "my-validator", "0.1.0");

        let pkg_type = package_type::detect_package_type(&val_dir);
        assert_eq!(pkg_type, Some(PackageType::Validator));

        let (name, version) = read_frontmatter(&val_dir.join("VALIDATOR.md")).unwrap();
        assert_eq!(name, "my-validator");
        assert_eq!(version, "0.1.0");
    }

    // --- validator deploy + uninstall (no agents required) ---

    #[test]
    fn test_deploy_validator_creates_files() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        // Create a source validator
        let src = work.path().join("src-val");
        make_local_validator(&src, "test-val", "1.0.0");

        // Deploy it (non-global → .avp/validators/)
        let targets = deploy_validator("test-val", &src, false).unwrap();
        assert_eq!(targets.len(), 1);

        // Verify files exist on disk
        let deployed = work.path().join(".avp/validators/test-val");
        assert!(deployed.join("VALIDATOR.md").exists());
        assert!(deployed.join("rules/no-secrets.md").exists());

        std::env::set_current_dir(old_dir).unwrap();
    }

    #[test]
    fn test_deploy_and_uninstall_validator() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        // Deploy
        let src = work.path().join("src-val");
        make_local_validator(&src, "test-val", "1.0.0");
        deploy_validator("test-val", &src, false).unwrap();

        let deployed = work.path().join(".avp/validators/test-val");
        assert!(deployed.exists());

        // Uninstall
        uninstall_validator("test-val", false).unwrap();
        assert!(!deployed.exists(), "Validator dir should be removed");

        std::env::set_current_dir(old_dir).unwrap();
    }

    #[test]
    fn test_uninstall_validator_not_found() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        let result = uninstall_validator("nonexistent", false);
        assert!(matches!(result.unwrap_err(), RegistryError::NotFound(_)));

        std::env::set_current_dir(old_dir).unwrap();
    }

    // --- lockfile round-trip for git-installed packages ---

    #[test]
    fn test_lockfile_records_git_source() {
        let work = tempfile::tempdir().unwrap();

        let mut lf = Lockfile::default();
        lf.add_package(
            "skill-a".to_string(),
            LockedPackage {
                package_type: PackageType::Skill,
                version: "1.0.0".to_string(),
                resolved: "git+https://github.com/anthropics/skills.git".to_string(),
                integrity: String::new(),
                installed_at: "2026-02-16T00:00:00Z".to_string(),
                targets: vec!["claude-code".to_string()],
            },
        );
        lf.add_package(
            "skill-b".to_string(),
            LockedPackage {
                package_type: PackageType::Skill,
                version: "1.0.0".to_string(),
                resolved: "git+https://github.com/anthropics/skills.git".to_string(),
                integrity: String::new(),
                installed_at: "2026-02-16T00:00:00Z".to_string(),
                targets: vec!["claude-code".to_string()],
            },
        );
        lf.add_package(
            "other-pkg".to_string(),
            LockedPackage {
                package_type: PackageType::Validator,
                version: "0.1.0".to_string(),
                resolved: "https://registry.example.com/other-pkg-0.1.0.zip".to_string(),
                integrity: "sha512-abc".to_string(),
                installed_at: "2026-02-16T00:00:00Z".to_string(),
                targets: vec![".avp/validators/".to_string()],
            },
        );

        lf.save(work.path()).unwrap();
        let loaded = Lockfile::load(work.path()).unwrap();
        assert_eq!(loaded.packages.len(), 3);

        // git packages have empty integrity, git+ resolved prefix
        let a = loaded.get_package("skill-a").unwrap();
        assert!(a.resolved.starts_with("git+"));
        assert!(a.integrity.is_empty());

        // registry package has integrity
        let o = loaded.get_package("other-pkg").unwrap();
        assert!(!o.resolved.starts_with("git+"));
        assert!(!o.integrity.is_empty());
    }

    // --- uninstall-by-URL matching ---

    #[test]
    fn test_find_packages_by_git_url() {
        let mut lf = Lockfile::default();
        lf.add_package(
            "skill-a".to_string(),
            LockedPackage {
                package_type: PackageType::Skill,
                version: "1.0.0".to_string(),
                resolved: "git+https://github.com/anthropics/skills.git".to_string(),
                integrity: String::new(),
                installed_at: String::new(),
                targets: vec![],
            },
        );
        lf.add_package(
            "skill-b".to_string(),
            LockedPackage {
                package_type: PackageType::Skill,
                version: "1.0.0".to_string(),
                resolved: "git+https://github.com/anthropics/skills.git".to_string(),
                integrity: String::new(),
                installed_at: String::new(),
                targets: vec![],
            },
        );
        lf.add_package(
            "other-pkg".to_string(),
            LockedPackage {
                package_type: PackageType::Validator,
                version: "0.1.0".to_string(),
                resolved: "git+https://github.com/other/repo.git".to_string(),
                integrity: String::new(),
                installed_at: String::new(),
                targets: vec![],
            },
        );

        // Full HTTPS URL → matches the two anthropics skills
        let matched = find_packages_by_git_source(&lf, "https://github.com/anthropics/skills");
        assert_eq!(matched.len(), 2);
        assert!(matched.contains(&"skill-a".to_string()));
        assert!(matched.contains(&"skill-b".to_string()));

        // Shorthand → same result
        let matched = find_packages_by_git_source(&lf, "anthropics/skills");
        assert_eq!(matched.len(), 2);

        // Different repo → only one match
        let matched = find_packages_by_git_source(&lf, "other/repo");
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0], "other-pkg");

        // No match
        let matched = find_packages_by_git_source(&lf, "nobody/nothing");
        assert!(matched.is_empty());

        // Plain registry name → not a git source, empty
        let matched = find_packages_by_git_source(&lf, "no-secrets");
        assert!(matched.is_empty());
    }

    #[test]
    fn test_find_packages_by_git_url_with_dot_git_suffix() {
        let mut lf = Lockfile::default();
        lf.add_package(
            "my-skill".to_string(),
            LockedPackage {
                package_type: PackageType::Skill,
                version: "1.0.0".to_string(),
                resolved: "git+https://github.com/owner/repo.git".to_string(),
                integrity: String::new(),
                installed_at: String::new(),
                targets: vec![],
            },
        );

        // URL with .git suffix
        let matched = find_packages_by_git_source(&lf, "https://github.com/owner/repo.git");
        assert_eq!(matched.len(), 1);

        // URL without .git suffix (parse_git_source appends it)
        let matched = find_packages_by_git_source(&lf, "https://github.com/owner/repo");
        assert_eq!(matched.len(), 1);

        // Shorthand
        let matched = find_packages_by_git_source(&lf, "owner/repo");
        assert_eq!(matched.len(), 1);
    }

    // --- end-to-end: clone real repo → deploy validator → lockfile → uninstall ---

    #[test]
    fn test_e2e_deploy_local_validator_and_uninstall_by_name() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        // Create and deploy
        let src = work.path().join("src-val");
        make_local_validator(&src, "e2e-val", "2.0.0");
        let targets = deploy_validator("e2e-val", &src, false).unwrap();
        assert!(!targets.is_empty());

        // Write lockfile
        let mut lf = Lockfile::default();
        lf.add_package(
            "e2e-val".to_string(),
            LockedPackage {
                package_type: PackageType::Validator,
                version: "2.0.0".to_string(),
                resolved: "file:src-val".to_string(),
                integrity: String::new(),
                installed_at: chrono::Utc::now().to_rfc3339(),
                targets: targets.clone(),
            },
        );
        lf.save(work.path()).unwrap();

        // Verify on disk
        let deployed = work.path().join(".avp/validators/e2e-val");
        assert!(deployed.join("VALIDATOR.md").exists());

        // Lockfile has the entry
        let lf = Lockfile::load(work.path()).unwrap();
        assert!(lf.get_package("e2e-val").is_some());

        // Uninstall by name
        uninstall_validator("e2e-val", false).unwrap();
        assert!(!deployed.exists());

        // Update lockfile
        let mut lf = Lockfile::load(work.path()).unwrap();
        lf.remove_package("e2e-val");
        lf.save(work.path()).unwrap();
        let lf = Lockfile::load(work.path()).unwrap();
        assert!(lf.get_package("e2e-val").is_none());

        std::env::set_current_dir(old_dir).unwrap();
    }

    // --- cross-type coexistence and duplicate install tests ---

    #[tokio::test]
    async fn test_e2e_all_four_types_coexist() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        // 1. Install a skill
        let skill_src = work.path().join("src-skill");
        make_local_skill(&skill_src, "test-skill", "1.0.0");
        let skill_targets = deploy_skill("test-skill", &skill_src, None, false).await.unwrap();
        assert!(!skill_targets.is_empty());

        // 2. Install a validator
        let val_src = work.path().join("src-val");
        make_local_validator(&val_src, "test-val", "1.0.0");
        let val_targets = deploy_validator("test-val", &val_src, false).unwrap();
        assert!(!val_targets.is_empty());

        // 3. Install a tool
        let tool_src = work.path().join("src-tool");
        make_local_tool(&tool_src, "test-tool", "1.0.0");
        let tool_targets = deploy_tool("test-tool", &tool_src, None, false).unwrap();
        assert!(!tool_targets.is_empty());

        // 4. Install a plugin
        let plugin_src = work.path().join("src-plugin");
        make_local_plugin(&plugin_src, "test-plugin", false);
        let plugin_targets = deploy_plugin("test-plugin", &plugin_src, None, false).unwrap();
        assert!(!plugin_targets.is_empty());

        // 5. Verify all four are on disk in separate locations
        assert!(work.path().join(".skills/test-skill/SKILL.md").exists());
        assert!(work.path().join(".avp/validators/test-val/VALIDATOR.md").exists());
        assert!(work.path().join(".tools/test-tool/TOOL.md").exists());
        assert!(work.path().join(".claude/plugins/test-plugin/.claude-plugin/plugin.json").exists());

        // 6. Verify list discovers all four
        let all = crate::list::discover_packages(false, false, false, false, None);
        let names: Vec<&str> = all.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"test-skill"), "Should find skill in list: {:?}", names);
        assert!(names.contains(&"test-val"), "Should find validator in list: {:?}", names);
        assert!(names.contains(&"test-tool"), "Should find tool in list: {:?}", names);
        assert!(names.contains(&"test-plugin"), "Should find plugin in list: {:?}", names);

        // 7. Verify type-specific filters work
        let skills_only = crate::list::discover_packages(true, false, false, false, None);
        assert!(skills_only.iter().all(|p| p.package_type == PackageType::Skill));
        assert!(skills_only.iter().any(|p| p.name == "test-skill"));

        let tools_only = crate::list::discover_packages(false, false, true, false, None);
        assert!(tools_only.iter().all(|p| p.package_type == PackageType::Tool));
        assert!(tools_only.iter().any(|p| p.name == "test-tool"));

        let plugins_only = crate::list::discover_packages(false, false, false, true, None);
        assert!(plugins_only.iter().all(|p| p.package_type == PackageType::Plugin));
        assert!(plugins_only.iter().any(|p| p.name == "test-plugin"));

        let vals_only = crate::list::discover_packages(false, true, false, false, None);
        assert!(vals_only.iter().all(|p| p.package_type == PackageType::Validator));
        assert!(vals_only.iter().any(|p| p.name == "test-val"));

        // 8. Uninstall each type independently — others remain
        uninstall_tool("test-tool", None, false).unwrap();
        assert!(!work.path().join(".tools/test-tool").exists());
        assert!(work.path().join(".skills/test-skill/SKILL.md").exists(), "Skill should survive tool uninstall");
        assert!(work.path().join(".avp/validators/test-val").exists(), "Validator should survive tool uninstall");
        assert!(work.path().join(".claude/plugins/test-plugin").exists(), "Plugin should survive tool uninstall");

        uninstall_plugin("test-plugin", None, false).unwrap();
        assert!(!work.path().join(".claude/plugins/test-plugin").exists());
        assert!(work.path().join(".skills/test-skill/SKILL.md").exists(), "Skill should survive plugin uninstall");

        uninstall_validator("test-val", false).unwrap();
        assert!(!work.path().join(".avp/validators/test-val").exists());
        assert!(work.path().join(".skills/test-skill/SKILL.md").exists(), "Skill should survive validator uninstall");

        std::env::set_current_dir(old_dir).unwrap();
    }

    #[test]
    fn test_deploy_tool_twice_overwrites_cleanly() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        // Deploy v1
        let src_v1 = work.path().join("src-v1");
        make_local_tool(&src_v1, "fs-tool", "1.0.0");
        deploy_tool("fs-tool", &src_v1, None, false).unwrap();

        let store = work.path().join(".tools/fs-tool");
        assert!(store.join("TOOL.md").exists());

        // Verify v1 is registered
        let mcp: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(work.path().join(".mcp.json")).unwrap(),
        )
        .unwrap();
        assert!(mcp["mcpServers"]["fs-tool"].is_object());

        // Deploy v2 on top (same name, different version)
        let src_v2 = work.path().join("src-v2");
        make_local_tool(&src_v2, "fs-tool", "2.0.0");
        deploy_tool("fs-tool", &src_v2, None, false).unwrap();

        // Store should have v2 content
        let (_, version) = read_frontmatter(&store.join("TOOL.md")).unwrap();
        assert_eq!(version, "2.0.0", "Version should be updated to 2.0.0");

        // MCP config should still have exactly one entry for fs-tool (not duplicated)
        let mcp: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(work.path().join(".mcp.json")).unwrap(),
        )
        .unwrap();
        let servers = mcp["mcpServers"].as_object().unwrap();
        let fs_entries: Vec<_> = servers.keys().filter(|k| *k == "fs-tool").collect();
        assert_eq!(fs_entries.len(), 1, "Should have exactly one fs-tool entry");

        // Clean uninstall should still work
        uninstall_tool("fs-tool", None, false).unwrap();
        assert!(!store.exists());

        std::env::set_current_dir(old_dir).unwrap();
    }

    #[test]
    fn test_deploy_plugin_twice_overwrites_cleanly() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        // Deploy v1
        let src_v1 = work.path().join("src-v1");
        make_local_plugin(&src_v1, "my-plugin", false);
        deploy_plugin("my-plugin", &src_v1, None, false).unwrap();

        let deployed = work.path().join(".claude/plugins/my-plugin");
        assert!(deployed.join(".claude-plugin/plugin.json").exists());
        assert!(deployed.join("commands/greet.md").exists());

        // Modify v2 source to have a different command file
        let src_v2 = work.path().join("src-v2");
        make_local_plugin(&src_v2, "my-plugin", true); // now with bundled MCP
        // Add an extra file to v2
        std::fs::write(src_v2.join("CHANGELOG.md"), "# Changes\nv2").unwrap();

        deploy_plugin("my-plugin", &src_v2, None, false).unwrap();

        // The deployed dir should have the v2 content
        assert!(
            deployed.join("CHANGELOG.md").exists(),
            "v2 files should be present after re-deploy"
        );
        assert!(
            deployed.join(".mcp.json").exists(),
            "v2 bundled .mcp.json should be present"
        );

        // Uninstall should still work cleanly
        uninstall_plugin("my-plugin", None, false).unwrap();
        assert!(!deployed.exists());

        std::env::set_current_dir(old_dir).unwrap();
    }

    // --- end-to-end: clone real repo → deploy validator → lockfile → uninstall ---

    #[test]
    fn test_e2e_clone_anthropics_deploy_validator_uninstall_by_url() {
        use crate::git_source;

        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        // Clone anthropics/skills
        let source = git_source::parse_git_source("anthropics/skills", None).unwrap();
        let clone_dir = git_source::git_clone(&source).unwrap();

        // Discover packages
        let packages = git_source::discover_packages(clone_dir.path(), None, None).unwrap();
        assert!(!packages.is_empty());

        // Pick first package, deploy it as if it were a validator (create a
        // synthetic validator from its directory to avoid needing agents)
        let pkg = &packages[0];
        let val_src = work.path().join("synthetic-val");
        make_local_validator(&val_src, &pkg.name, "1.0.0");
        deploy_validator(&pkg.name, &val_src, false).unwrap();

        // Write lockfile with git+ resolved
        let mut lf = Lockfile::default();
        lf.add_package(
            pkg.name.clone(),
            LockedPackage {
                package_type: PackageType::Validator,
                version: "1.0.0".to_string(),
                resolved: format!("git+{}", source.clone_url),
                integrity: String::new(),
                installed_at: chrono::Utc::now().to_rfc3339(),
                targets: vec![".avp/validators/".to_string()],
            },
        );
        lf.save(work.path()).unwrap();

        // Verify deploy
        let deployed = work
            .path()
            .join(".avp/validators")
            .join(sanitize_dir_name(&pkg.name));
        assert!(deployed.exists());

        // find_packages_by_git_source matches via URL
        let lf = Lockfile::load(work.path()).unwrap();
        let matched = find_packages_by_git_source(&lf, "https://github.com/anthropics/skills");
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0], pkg.name);

        // Also matches via shorthand
        let matched = find_packages_by_git_source(&lf, "anthropics/skills");
        assert_eq!(matched.len(), 1);

        // Uninstall by name
        uninstall_validator(&pkg.name, false).unwrap();
        assert!(!deployed.exists());

        // Clean lockfile
        let mut lf = Lockfile::load(work.path()).unwrap();
        lf.remove_package(&pkg.name);
        lf.save(work.path()).unwrap();
        let lf = Lockfile::load(work.path()).unwrap();
        assert!(lf.packages.is_empty());

        std::env::set_current_dir(old_dir).unwrap();
    }
}
