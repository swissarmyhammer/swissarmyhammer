//! Mirdan Install/Uninstall - Type-aware package deployment.
//!
//! Skills -> agent skill directories (one copy per detected agent)
//! Validators -> .avp/validators/ (project) or ~/.avp/validators/ (global)
//! Tools -> .tools/ store + agent MCP config files
//! Plugins -> agent plugin directories (e.g. .claude/plugins/)

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use indicatif::{ProgressBar, ProgressStyle};

use crate::agents::{
    self, agent_global_agent_dir, agent_global_skill_dir, agent_project_agent_dir,
    agent_project_skill_dir,
};
use crate::git_source::{self, InstallSource};
use crate::lockfile::{self, LockedPackage, Lockfile};
use crate::mcp_config;
use crate::package_type::{self, PackageType};
use crate::registry::{RegistryClient, RegistryError};
use crate::store;
use crate::{settings, status};

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
) -> Result<Vec<crate::DeployResult>, RegistryError> {
    match git_source::classify_source(package_spec, git) {
        InstallSource::LocalPath(path) => {
            run_install_local(&path, agent_filter, global).await?;
            Ok(vec![crate::DeployResult::message(
                crate::DeployAction::Created,
                format!("Installed from local path: {}", path),
            )])
        }
        InstallSource::GitRepo(source) => {
            run_install_git(&source, agent_filter, global, skill_select).await?;
            Ok(vec![crate::DeployResult::message(
                crate::DeployAction::Created,
                format!("Installed from git: {}", source.display_name),
            )])
        }
        InstallSource::Registry(spec) => {
            match run_install_registry(&spec, agent_filter, global).await {
                Ok(()) => Ok(vec![crate::DeployResult::message(
                    crate::DeployAction::Created,
                    "Installed from registry",
                )]),
                Err(RegistryError::NotFound(_)) => {
                    // Registry miss — try as git source before giving up
                    match git_source::parse_git_source(package_spec, skill_select) {
                        Ok(source) => {
                            tracing::debug!("  Not found in registry, trying as git repository...");
                            run_install_git(&source, agent_filter, global, skill_select).await?;
                            Ok(vec![
                                crate::DeployResult::message(
                                    crate::DeployAction::Warning,
                                    "Not found in registry, trying as git repository...",
                                ),
                                crate::DeployResult::message(
                                    crate::DeployAction::Created,
                                    format!("Installed from git: {}", source.display_name),
                                ),
                            ])
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
        PackageType::Agent => read_frontmatter(&dir.join("AGENT.md"))?,
    };

    tracing::debug!("Installing {} from local path ({})...", name, pkg_type);

    let targets = match pkg_type {
        PackageType::Skill => deploy_skill(&name, &dir, agent_filter, global).await?,
        PackageType::Validator => deploy_validator(&name, &dir, global)?,
        PackageType::Tool => deploy_tool(&name, &dir, agent_filter, global)?,
        PackageType::Plugin => deploy_plugin(&name, &dir, agent_filter, global)?,
        PackageType::Agent => deploy_agent(&name, &dir, agent_filter, global).await?,
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
    tracing::debug!("  Updated mirdan-lock.json");

    tracing::debug!(
        "\nInstalled {}@{} ({}) from local path",
        name,
        version,
        pkg_type
    );
    for target in &targets {
        tracing::debug!("  -> {}", target);
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
    tracing::debug!("Cloning {}...", source.display_name);

    let temp_dir = git_source::git_clone(source)?;

    // Merge select from GitSource and the --skill flag (--skill takes precedence)
    let select = skill_select.or(source.select.as_deref());

    let packages =
        git_source::discover_packages(temp_dir.path(), source.subpath.as_deref(), select)?;

    tracing::debug!(
        "  Found {} package(s) in {}",
        packages.len(),
        source.display_name
    );
    for pkg in &packages {
        tracing::debug!("    - {} ({})", pkg.name, pkg.package_type);
    }

    let project_root = std::env::current_dir()?;
    let mut lf = Lockfile::load(&project_root)?;

    for pkg in &packages {
        tracing::debug!("\nInstalling {} ({})...", pkg.name, pkg.package_type);

        let targets = match pkg.package_type {
            PackageType::Skill => deploy_skill(&pkg.name, &pkg.path, agent_filter, global).await?,
            PackageType::Validator => deploy_validator(&pkg.name, &pkg.path, global)?,
            PackageType::Tool => deploy_tool(&pkg.name, &pkg.path, agent_filter, global)?,
            PackageType::Plugin => deploy_plugin(&pkg.name, &pkg.path, agent_filter, global)?,
            PackageType::Agent => deploy_agent(&pkg.name, &pkg.path, agent_filter, global).await?,
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
            PackageType::Agent => read_frontmatter(&pkg.path.join("AGENT.md"))
                .map(|(_, v)| v)
                .unwrap_or_else(|_| "0.0.0".to_string()),
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

        tracing::debug!(
            "Installed {}@{} ({}) from git",
            pkg.name,
            version,
            pkg.package_type
        );
        for target in &targets {
            tracing::debug!("  -> {}", target);
        }
    }

    lf.save(&project_root)?;
    tracing::debug!("  Updated mirdan-lock.json");

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
    let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(frontmatter)
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

    // Try authenticated client first, fall back to unauthenticated for public packages
    let client = match RegistryClient::authenticated() {
        Ok(c) => c,
        Err(_) => {
            tracing::debug!("No credentials found, using unauthenticated client");
            RegistryClient::new()
        }
    };

    // Resolve version
    let version_detail = if let Some(ref ver) = version {
        tracing::debug!("Resolving {}@{}...", name, ver);
        client.version_info(&name, ver).await?
    } else {
        tracing::debug!("Resolving {} (latest)...", name);
        client.latest_version(&name).await?
    };

    let resolved_version = &version_detail.version;
    tracing::debug!("Installing {}@{}...", name, resolved_version);

    // Try downloading the package artifact
    let download_result = download_package(&client, &version_detail).await;

    match download_result {
        Ok(data) => {
            // Standard path: extract ZIP and deploy
            install_from_archive(&name, &version_detail, &data, agent_filter, global).await
        }
        Err(RegistryError::NotFound(_)) => {
            // No downloadable artifact — try metadata-only install for tools
            install_tool_from_metadata(&name, &version_detail, agent_filter, global).await
        }
        Err(e) => Err(e),
    }
}

/// Download and verify a package artifact from the registry.
async fn download_package(
    client: &RegistryClient,
    version_detail: &crate::registry::types::VersionDetail,
) -> Result<bytes::Bytes, RegistryError> {
    let pb = if let Some(size) = version_detail.size.filter(|&s| s > 0) {
        let pb = ProgressBar::new(size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{msg} [{bar:40}] {bytes}/{total_bytes}")
                .unwrap()
                .progress_chars("=> "),
        );
        pb
    } else {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{msg} {spinner}")
                .unwrap(),
        );
        pb
    };
    pb.set_message("Downloading");

    let data = client
        .download_from_url(&version_detail.download_url)
        .await?;
    pb.set_position(data.len() as u64);
    pb.finish_with_message("Downloaded");

    // Verify integrity (skip if not provided by registry)
    let integrity_hash = version_detail.integrity.as_deref().unwrap_or("");
    if !integrity_hash.is_empty() {
        lockfile::verify_integrity(&data, integrity_hash).map_err(RegistryError::Integrity)?;
        tracing::debug!("  Integrity verified");
    }

    Ok(data)
}

/// Standard install path: extract a downloaded ZIP and deploy based on detected type.
async fn install_from_archive(
    name: &str,
    version_detail: &crate::registry::types::VersionDetail,
    data: &[u8],
    agent_filter: Option<&str>,
    global: bool,
) -> Result<(), RegistryError> {
    let temp_dir = tempfile::tempdir()?;
    extract_zip(data, temp_dir.path())?;

    // Detect package type from contents, with API type hint as fallback
    let pkg_type = package_type::detect_package_type(temp_dir.path())
        .or_else(|| {
            version_detail
                .package_type
                .as_deref()
                .and_then(package_type::parse_package_type)
        })
        .ok_or_else(|| {
            RegistryError::Validation(
                "Cannot determine package type. Expected SKILL.md, VALIDATOR.md + rules/, TOOL.md, or .claude-plugin/plugin.json".to_string(),
            )
        })?;

    let targets = match pkg_type {
        PackageType::Skill => deploy_skill(name, temp_dir.path(), agent_filter, global).await?,
        PackageType::Validator => deploy_validator(name, temp_dir.path(), global)?,
        PackageType::Tool => deploy_tool(name, temp_dir.path(), agent_filter, global)?,
        PackageType::Plugin => deploy_plugin(name, temp_dir.path(), agent_filter, global)?,
        PackageType::Agent => deploy_agent(name, temp_dir.path(), agent_filter, global).await?,
    };

    record_install(name, version_detail, pkg_type, &targets)?;
    Ok(())
}

/// Metadata-only install for tool packages when no downloadable artifact exists.
///
/// Uses MCP config from the API response (mcp field or tool_md content) to register
/// the MCP server directly without needing a ZIP download.
async fn install_tool_from_metadata(
    name: &str,
    version_detail: &crate::registry::types::VersionDetail,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<(), RegistryError> {
    // Verify this is actually a tool
    let is_tool = version_detail
        .package_type
        .as_deref()
        .map(|t| t == "tool")
        .unwrap_or(false);

    if !is_tool {
        return Err(RegistryError::NotFound(format!(
            "Package '{}' has no downloadable artifact and is not a tool",
            name
        )));
    }

    // Try three sources of MCP config, in order:
    // 1. Explicit mcp field from API
    // 2. Parse tool_md content from API
    // 3. Fetch package detail for tool_md/mcp

    if let Some(ref mcp) = version_detail.mcp {
        tracing::debug!("  Installing from registry MCP metadata...");
        return install_tool_from_mcp_config(name, version_detail, mcp, agent_filter, global).await;
    }

    if let Some(ref tool_md) = version_detail.tool_md {
        tracing::debug!("  Installing from registry TOOL.md...");
        return install_tool_from_tool_md_content(
            name,
            version_detail,
            tool_md,
            agent_filter,
            global,
        )
        .await;
    }

    // Try fetching the full package detail which may have mcp/tool_md
    let client = RegistryClient::authenticated().unwrap_or_default();
    let detail = client.package_info(name).await?;

    if let Some(ref mcp) = detail.mcp {
        tracing::debug!("  Installing from registry MCP metadata...");
        let mcp_clone = mcp.clone();
        return install_tool_from_mcp_config(
            name,
            version_detail,
            &mcp_clone,
            agent_filter,
            global,
        )
        .await;
    }

    if let Some(ref tool_md) = detail.tool_md {
        tracing::debug!("  Installing from registry TOOL.md...");
        return install_tool_from_tool_md_content(
            name,
            version_detail,
            tool_md,
            agent_filter,
            global,
        )
        .await;
    }

    Err(RegistryError::Validation(format!(
        "Tool '{}' has no downloadable artifact and no MCP configuration in the registry. \
         The registry entry may be incomplete.",
        name
    )))
}

/// Install a tool using an explicit MCP config from the registry.
async fn install_tool_from_mcp_config(
    name: &str,
    version_detail: &crate::registry::types::VersionDetail,
    mcp: &crate::registry::types::McpConfig,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<(), RegistryError> {
    let config = agents::load_agents_config()?;
    let agents = agents::resolve_target_agents(&config, agent_filter)?;

    let entry = mcp_config::McpServerEntry {
        command: mcp.command.clone(),
        args: mcp.args.clone(),
        env: mcp.env.clone(),
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
                    &mcp_cfg.entry_extras,
                )?;
                tracing::debug!(
                    "  Registered in {} ({})",
                    config_path.display(),
                    agent.def.name
                );
                targets.push(agent.def.id.clone());
            }
        }
    }

    record_install(name, version_detail, PackageType::Tool, &targets)?;
    Ok(())
}

/// Install a tool by parsing TOOL.md content from the registry.
async fn install_tool_from_tool_md_content(
    name: &str,
    version_detail: &crate::registry::types::VersionDetail,
    tool_md_content: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<(), RegistryError> {
    // Write TOOL.md to a temp dir and use the existing deploy_tool path
    let temp_dir = tempfile::tempdir()?;
    std::fs::write(temp_dir.path().join("TOOL.md"), tool_md_content)?;
    let targets = deploy_tool(name, temp_dir.path(), agent_filter, global)?;
    record_install(name, version_detail, PackageType::Tool, &targets)?;
    Ok(())
}

/// Record a successful install in the lockfile.
fn record_install(
    name: &str,
    version_detail: &crate::registry::types::VersionDetail,
    pkg_type: PackageType,
    targets: &[String],
) -> Result<(), RegistryError> {
    let project_root = std::env::current_dir()?;
    let mut lf = Lockfile::load(&project_root)?;
    lf.add_package(
        name.to_string(),
        LockedPackage {
            package_type: pkg_type,
            version: version_detail.version.clone(),
            resolved: version_detail.download_url.clone(),
            integrity: version_detail.integrity.clone().unwrap_or_default(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            targets: targets.to_vec(),
        },
    );
    lf.save(&project_root)?;
    tracing::debug!("  Updated mirdan-lock.json");

    tracing::debug!(
        "\nInstalled {}@{} ({})",
        name,
        version_detail.version,
        pkg_type
    );
    for target in targets {
        tracing::debug!("  -> {}", target);
    }
    Ok(())
}

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
    let config = agents::load_agents_config()?;
    let agents = agents::resolve_target_agents(&config, agent_filter)?;

    if agents.is_empty() {
        return Err(RegistryError::Validation(
            "No agents detected. Run 'mirdan agents' to check.".to_string(),
        ));
    }

    // 1. Copy source into the central store
    let sanitized = sanitize_dir_name(name);
    let store_path = rooted(root, global, store::skill_store_dir(global)).join(&sanitized);

    // Remove existing store entry
    store::remove_if_exists(&store_path)?;

    copy_dir_recursive(source_dir, &store_path)?;
    tracing::debug!("  Stored in {}", store_path.display());

    // 2. Create symlinks from each agent's skill directory
    let mut targets = Vec::new();

    for agent in &agents {
        let link_name = store::symlink_name(&sanitized, &agent.def.symlink_policy);
        let agent_skill_dir = if global {
            agent_global_skill_dir(&agent.def)
        } else {
            rooted(root, global, agent_project_skill_dir(&agent.def))
        };
        let link_path = agent_skill_dir.join(&link_name);

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

/// Resolve a project-scope relative path against an explicit `root`.
///
/// Returns `path` unchanged in global scope (its paths are already absolute) or
/// when no `root` is supplied (CWD-relative behavior). Otherwise joins the
/// relative `path` onto `root`, so deployment never reads `current_dir()`.
fn rooted(root: Option<&Path>, global: bool, path: PathBuf) -> PathBuf {
    match root {
        Some(root) if !global => root.join(path),
        _ => path,
    }
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

    let temp_dir = tempfile::tempdir()
        .map_err(|e| RegistryError::Validation(format!("failed to create temp dir: {e}")))?;
    let skill_dir = temp_dir.path().join(name);
    std::fs::create_dir_all(&skill_dir)
        .map_err(|e| RegistryError::Validation(format!("failed to create temp skill dir: {e}")))?;
    std::fs::write(skill_dir.join("SKILL.md"), skill_content)
        .map_err(|e| RegistryError::Validation(format!("failed to write SKILL.md: {e}")))?;

    deploy_skill_to_agents(name, &skill_dir, None, false)
}

/// Async wrapper around [`deploy_skill_to_agents`] for use in async install paths.
async fn deploy_skill(
    name: &str,
    source_dir: &Path,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<Vec<String>, RegistryError> {
    deploy_skill_to_agents(name, source_dir, agent_filter, global)
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
    let config = agents::load_agents_config()?;
    let agents = agents::resolve_target_agents(&config, agent_filter)?;

    if agents.is_empty() {
        return Err(RegistryError::Validation(
            "No agents detected. Run 'mirdan agents' to check.".to_string(),
        ));
    }

    // 1. Copy source into the central agent store
    let sanitized = sanitize_dir_name(name);
    let store_path = rooted(root, global, store::agent_store_dir(global)).join(&sanitized);

    // Remove existing store entry
    store::remove_if_exists(&store_path)?;

    copy_dir_recursive(source_dir, &store_path)?;
    tracing::debug!("  Stored in {}", store_path.display());

    // 2. Create symlinks from each coding agent's agent directory
    let mut targets = Vec::new();

    for agent in &agents {
        let agent_dir = if global {
            agent_global_agent_dir(&agent.def)
        } else {
            agent_project_agent_dir(&agent.def).map(|d| rooted(root, global, d))
        };

        if let Some(base_dir) = agent_dir {
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
        } else {
            tracing::debug!(
                "Agent {} has no agent directory configured, skipping",
                agent.def.id
            );
        }
    }

    Ok(targets)
}

/// Async wrapper around [`deploy_agent_to_agents`] for use in async install paths.
async fn deploy_agent(
    name: &str,
    source_dir: &Path,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<Vec<String>, RegistryError> {
    deploy_agent_to_agents(name, source_dir, agent_filter, global)
}

// ── Profile installer (the single, data-driven init/deinit) ──────────────────
//
// A `Profile` is the *only* thing that differs between consumers (sah, the tool
// CLIs, the kanban desktop app): a declarative manifest of what to install — an
// MCP server, a selection of builtin skills, a selection of builtin agents, and
// the sah-only statusline/preamble flags. `init_profile`/`deinit_profile` are
// the single code path that interprets that data; there are no per-consumer
// branches. Builtin skills and agents are rendered once through the prompt
// library's Liquid engine (so `{% include "_partials/..." %}` references expand)
// and deployed via the store + symlink mechanism — the same one
// `deploy_skill_to_agents` / `deploy_agent_to_agents` already use.

use swissarmyhammer_agents::AgentResolver;
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_skills::SkillResolver;

/// Selects which builtin items (skills or agents) a profile installs.
///
/// The same shape serves both skills and agents. `Profile` matches against an
/// item's profile-membership tags; builtin agents carry no profile tags, so
/// `Profile` selects nothing for them — use `Named`/`Single`/`All` there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    /// Every builtin item.
    All,
    /// Items tagged with the given init profile (skills only — agents have no
    /// profile tags).
    Profile(String),
    /// The named items, in the given order. Unknown names are skipped.
    Named(Vec<String>),
    /// A single named item.
    Single(String),
}

impl Selector {
    /// Resolve this selector against a `name → membership-tags` view of the
    /// available builtins, returning the selected names (deduplicated, sorted
    /// for `All`/`Profile`, source-ordered for `Named`/`Single`).
    ///
    /// `membership(name)` returns the item's profile tags so `Profile` can match
    /// them; for items without profile tags (agents) it returns an empty slice.
    fn select(&self, available: &std::collections::HashMap<String, Vec<String>>) -> Vec<String> {
        match self {
            Selector::All => {
                let mut names: Vec<String> = available.keys().cloned().collect();
                names.sort();
                names
            }
            Selector::Profile(profile) => {
                let mut names: Vec<String> = available
                    .iter()
                    .filter(|(_, tags)| tags.iter().any(|t| t == profile))
                    .map(|(name, _)| name.clone())
                    .collect();
                names.sort();
                names
            }
            Selector::Named(names) => names
                .iter()
                .filter(|n| available.contains_key(*n))
                .cloned()
                .collect(),
            Selector::Single(name) => {
                if available.contains_key(name) {
                    vec![name.clone()]
                } else {
                    Vec::new()
                }
            }
        }
    }
}

/// The MCP server a profile registers across detected agents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileMcpServer {
    /// The server name (the key under which it is registered, e.g. `sah`).
    pub name: String,
    /// The command to launch the server (e.g. `sah`).
    pub command: String,
    /// Arguments passed to the command (e.g. `["serve"]`).
    pub args: Vec<String>,
}

impl ProfileMcpServer {
    /// A server whose binary name *is* the launch command, started with the
    /// single `serve` argument: `{ name, command: name, args: ["serve"] }`.
    ///
    /// This is the shape every tool CLI and sah register — the binary registers
    /// itself under its own name and runs `<name> serve` — so they declare it
    /// with one value instead of repeating the verbatim triple.
    pub fn serve(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            command: name.clone(),
            name,
            args: vec!["serve".to_string()],
        }
    }
}

/// A declarative manifest of what a CLI or app installs.
///
/// This is the single value that differs per consumer; [`init_profile`] /
/// [`deinit_profile`] interpret it with no per-consumer branching. The sah-only
/// concerns (`statusline`, `preamble`) are plain declarative flags so that sah
/// is "just a bigger profile" rather than a special case.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Profile {
    /// The MCP server to register across detected agents, if any.
    pub mcp_server: Option<ProfileMcpServer>,
    /// Which builtin skills to render and deploy, if any.
    pub skills: Option<Selector>,
    /// Which builtin agents to render and deploy, if any.
    pub agents: Option<Selector>,
    /// Whether to install the Claude Code statusline (`sah statusline`).
    pub statusline: bool,
    /// Whether to ensure the CLAUDE.md preamble is present.
    pub preamble: bool,
}

/// Map an [`InitScope`] to the boolean `global` flag the deploy/store helpers
/// expect: only `User` scope is global; `Project`/`Local` are project-scoped.
fn scope_is_global(scope: InitScope) -> bool {
    matches!(scope, InitScope::User)
}

/// Build the template context used to render builtin skill/agent bodies.
///
/// Exposes `{{version}}` (this crate's package version) so skill/agent metadata
/// and instructions that reference it expand. `PromptLibrary::render_text`
/// additionally injects default variables and environment variables.
fn profile_template_context() -> TemplateContext {
    let mut ctx = TemplateContext::new();
    ctx.set(
        "version".to_string(),
        serde_json::json!(env!("CARGO_PKG_VERSION")),
    );
    ctx
}

/// Render a builtin skill's instructions and metadata through the prompt
/// library's Liquid engine, expanding `{% include "_partials/..." %}` and
/// `{{version}}`.
///
/// Falls back to the raw instructions when rendering fails (logging a warning),
/// matching the per-CLI deploy behavior so a partial-resolution failure degrades
/// instead of aborting the whole install.
fn render_profile_skill(
    library: &PromptLibrary,
    ctx: &TemplateContext,
    skill: &swissarmyhammer_skills::Skill,
) -> (String, std::collections::HashMap<String, String>) {
    // Expose the skill's `agent` frontmatter as a template variable so shared
    // partials (e.g. `_partials/delegate-to-subagent`) can render `{{ agent }}`
    // without each skill hard-coding its delegate name. Cloned per skill because
    // `ctx` is shared across the whole profile.
    let mut skill_ctx = ctx.clone();
    if let Some(agent) = skill.agent.as_deref() {
        skill_ctx.set("agent".to_string(), serde_json::json!(agent));
    }

    let instructions = library
        .render_text(&skill.instructions, &skill_ctx)
        .unwrap_or_else(|err| {
            tracing::warn!(
                skill = skill.name.as_str(),
                error = %err,
                "skill template rendering failed, falling back to raw instructions"
            );
            skill.instructions.clone()
        });

    let mut metadata = skill.metadata.clone();
    for value in metadata.values_mut() {
        if value.contains("{{") || value.contains("{%") {
            if let Ok(rendered) = library.render_text(value, &skill_ctx) {
                *value = rendered;
            }
        }
    }

    (instructions, metadata)
}

/// The known set of init profiles a builtin skill may declare in its `profiles`
/// frontmatter list. This is the single profile registry [`Selector::Profile`]
/// matches against.
///
/// Profile matching is an exact `==` comparison with no normalization, so a typo
/// or case-mismatch (`Kanban`, a trailing space) would silently exclude a skill
/// from every profile rather than fail. [`install_profile_skills`] validates each
/// builtin's `profiles` against this set with a `debug_assert!`, turning that
/// silent drop into a loud development-time failure. Update this set whenever a
/// new profile is introduced.
pub const KNOWN_PROFILES: &[&str] = &["kanban", "code-context"];

/// Resolve, render, and deploy the profile's selected builtin skills.
///
/// Returns the deduplicated list of agent targets the skills were deployed to.
fn install_profile_skills(
    selector: &Selector,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) -> Result<Vec<String>, RegistryError> {
    let resolver = SkillResolver::new();
    let builtins = resolver.resolve_builtins();

    // Catch a mistagged builtin (`Kanban`, trailing space, unknown profile name)
    // loudly during development rather than letting it silently fall out of every
    // `Selector::Profile` filter. Matching is exact `==`, so an out-of-set entry
    // would otherwise just be dropped.
    debug_assert!(
        builtins.values().all(|skill| skill
            .profiles
            .iter()
            .all(|p| KNOWN_PROFILES.contains(&p.as_str()))),
        "a builtin skill declares an unknown profile; known profiles are \
         {KNOWN_PROFILES:?} (exact match, no normalization). Offenders: {:?}",
        builtins
            .values()
            .filter(|skill| skill
                .profiles
                .iter()
                .any(|p| !KNOWN_PROFILES.contains(&p.as_str())))
            .map(|skill| (skill.name.as_str(), &skill.profiles))
            .collect::<Vec<_>>()
    );

    let available: std::collections::HashMap<String, Vec<String>> = builtins
        .iter()
        .map(|(name, skill)| (name.clone(), skill.profiles.clone()))
        .collect();

    let library = PromptLibrary::default();
    let ctx = profile_template_context();

    let selected = selector.select(&available);
    let skill_count = selected.len();
    let mut targets: Vec<String> = Vec::new();
    for name in selected {
        let skill = &builtins[&name];
        let (instructions, metadata) = render_profile_skill(&library, &ctx, skill);
        let content =
            swissarmyhammer_skills::deploy::format_skill_md(skill, &instructions, &metadata);
        let deployed = stage_and_deploy_rendered(&name, &content, "SKILL.md", scope, root, true)?;
        merge_targets(&mut targets, deployed);
    }

    if !targets.is_empty() {
        reporter.emit(&InitEvent::Action {
            verb: "Deployed".to_string(),
            message: format!("{} skill(s) to {}", skill_count, targets.join(", ")),
        });
    }
    Ok(targets)
}

/// Resolve, render, and deploy the profile's selected builtin agents.
///
/// Returns the deduplicated list of agent targets the agents were deployed to.
fn install_profile_agents(
    selector: &Selector,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) -> Result<Vec<String>, RegistryError> {
    let resolver = AgentResolver::new();
    let builtins = resolver.resolve_builtins();
    // Agents carry no profile tags; expose an empty tag list for each.
    let available: std::collections::HashMap<String, Vec<String>> = builtins
        .keys()
        .map(|name| (name.clone(), Vec::new()))
        .collect();

    let library = PromptLibrary::default();
    let ctx = profile_template_context();

    let selected = selector.select(&available);
    let agent_count = selected.len();
    let mut targets: Vec<String> = Vec::new();
    for name in selected {
        let agent = &builtins[&name];
        let rendered_body = library
            .render_text(&agent.instructions, &ctx)
            .unwrap_or_else(|err| {
                tracing::warn!(
                    agent = name.as_str(),
                    error = %err,
                    "agent template rendering failed, falling back to raw instructions"
                );
                agent.instructions.clone()
            });
        let mut rendered_agent = agent.clone();
        for value in rendered_agent.metadata.values_mut() {
            if value.contains("{{") || value.contains("{%") {
                if let Ok(rendered) = library.render_text(value, &ctx) {
                    *value = rendered;
                }
            }
        }
        let content = rendered_agent.to_agent_md(&rendered_body);
        let deployed = stage_and_deploy_rendered(&name, &content, "AGENT.md", scope, root, false)?;
        merge_targets(&mut targets, deployed);
    }

    if !targets.is_empty() {
        reporter.emit(&InitEvent::Action {
            verb: "Deployed".to_string(),
            message: format!("{} agent(s) to {}", agent_count, targets.join(", ")),
        });
    }
    Ok(targets)
}

/// Stage `content` as `<tmp>/<name>/<file_name>` and deploy it via the store +
/// symlink mechanism, rooting at `root` when supplied so deployment never reads
/// `current_dir()`.
///
/// `is_skill` selects the skill store/dirs vs. the agent store/dirs.
fn stage_and_deploy_rendered(
    name: &str,
    content: &str,
    file_name: &str,
    scope: InitScope,
    root: Option<&Path>,
    is_skill: bool,
) -> Result<Vec<String>, RegistryError> {
    if !store::is_safe_name(name) {
        return Err(RegistryError::Validation(format!("unsafe name: {name:?}")));
    }
    let temp_dir = tempfile::tempdir()
        .map_err(|e| RegistryError::Validation(format!("failed to create temp dir: {e}")))?;
    let item_dir = temp_dir.path().join(name);
    std::fs::create_dir_all(&item_dir)
        .map_err(|e| RegistryError::Validation(format!("failed to create temp dir: {e}")))?;
    std::fs::write(item_dir.join(file_name), content)
        .map_err(|e| RegistryError::Validation(format!("failed to write {file_name}: {e}")))?;

    let global = scope_is_global(scope);
    if is_skill {
        deploy_skill_to_agents_at(name, &item_dir, None, global, root)
    } else {
        deploy_agent_to_agents_at(name, &item_dir, None, global, root)
    }
}

/// Append `new` targets to `targets`, skipping duplicates (preserving order).
fn merge_targets(targets: &mut Vec<String>, new: Vec<String>) {
    for target in new {
        if !targets.contains(&target) {
            targets.push(target);
        }
    }
}

/// Top-level key for Claude Code's statusline configuration block.
const STATUSLINE_KEY: &str = "statusLine";

/// The statusline configuration value a profile installs.
///
/// Mirrors the Claude-conventional `{type: "command", command: "sah statusline"}`
/// block the CLI previously wrote by hand, so the `statusline` flag installs
/// exactly the same configuration through the data-driven profile path.
fn desired_statusline_value() -> serde_json::Value {
    serde_json::json!({
        "type": "command",
        "command": "sah statusline"
    })
}

/// Resolve a detected agent's settings/instructions file for `scope`, rooting
/// project-scope relative paths against `root` so the operation is CWD-free.
///
/// `global_resolve` returns the agent's absolute global path (user scope);
/// `project_resolve` returns the agent's project-relative path (project/local
/// scope), which is joined onto `root` via [`rooted`]. Returns `None` when the
/// agent declares no path for the scope.
fn resolve_agent_file(
    agent: &AgentDef,
    global: bool,
    root: Option<&Path>,
    global_resolve: impl Fn(&AgentDef) -> Option<PathBuf>,
    project_resolve: impl Fn(&AgentDef) -> Option<PathBuf>,
) -> Option<PathBuf> {
    if global {
        global_resolve(agent)
    } else {
        project_resolve(agent).map(|relative| rooted(root, global, relative))
    }
}

/// Write the statusline block into every detected agent's settings file, or
/// remove it when `install` is false. Root-aware so project-scope settings
/// files resolve against `root` instead of `current_dir()`.
fn apply_profile_statusline(
    install: bool,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    let verb = if install { "Installed" } else { "Removed" };
    for_each_detected_agent(
        scope,
        reporter,
        |agent, global| {
            let Some(path) = resolve_agent_file(
                agent,
                global,
                root,
                agents::agent_global_settings_file,
                agents::agent_project_settings_file,
            ) else {
                return Ok(None);
            };
            Ok(apply_statusline_at(&path, install)?.then(|| AgentAction {
                verb: verb.to_string(),
                message: format!("statusline for {} ({})", agent.name, path.display()),
            }))
        },
        |changed| {
            InitResult::ok(
                "profile-statusline",
                format!("{verb} statusline for {changed} agent(s)"),
            )
        },
    )
}

/// Set or remove the `statusLine` block in the settings file at `path`.
///
/// On install, missing files are created. On removal, a missing file is a no-op.
/// Returns `Ok(true)` when the file changed, `Ok(false)` when already in the
/// desired state.
fn apply_statusline_at(path: &Path, install: bool) -> Result<bool, RegistryError> {
    if !install && !path.exists() {
        return Ok(false);
    }
    let mut settings = settings::read_json(path)?;
    let changed = if install {
        settings::set_object(&mut settings, STATUSLINE_KEY, desired_statusline_value())
    } else {
        settings::remove_key(&mut settings, STATUSLINE_KEY)
    };
    if changed {
        settings::write_json(path, &settings)?;
    }
    Ok(changed)
}

/// Ensure the CLAUDE.md preamble is present in every detected agent's
/// instructions file, or strip it when `install` is false. Root-aware so
/// project-scope instructions files resolve against `root`.
fn apply_profile_preamble(
    install: bool,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    let verb = if install { "Installed" } else { "Removed" };
    for_each_detected_agent(
        scope,
        reporter,
        |agent, global| {
            let Some(path) = resolve_agent_file(
                agent,
                global,
                root,
                agents::agent_global_instructions_file,
                agents::agent_project_instructions_file,
            ) else {
                return Ok(None);
            };
            let outcome = if install {
                ensure_preamble(&path)?
            } else {
                remove_preamble(&path)?
            };
            Ok(outcome.changed().then(|| AgentAction {
                verb: outcome.verb().to_string(),
                message: format!("preamble for {} ({})", agent.name, path.display()),
            }))
        },
        |changed| {
            InitResult::ok(
                "profile-preamble",
                format!("{verb} preamble for {changed} agent(s)"),
            )
        },
    )
}

/// The outcome of an `ensure_preamble` / `remove_preamble` operation, carrying
/// both a human-readable verb and whether the file actually changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreambleOutcome {
    /// The file was created with the preamble.
    Created,
    /// The preamble was prepended to existing content.
    Prepended,
    /// The preamble was already present; no change.
    AlreadyPresent,
    /// The preamble was stripped from existing content.
    Removed,
    /// The file contained only the preamble and was deleted.
    Deleted,
    /// No file existed, or it had no preamble; no change.
    Absent,
}

impl PreambleOutcome {
    /// A stable verb for the reporter Action event.
    fn verb(self) -> &'static str {
        match self {
            PreambleOutcome::Created => "Created",
            PreambleOutcome::Prepended => "Prepended",
            PreambleOutcome::AlreadyPresent => "Present",
            PreambleOutcome::Removed => "Removed",
            PreambleOutcome::Deleted => "Deleted",
            PreambleOutcome::Absent => "Absent",
        }
    }

    /// Whether the file changed on disk.
    fn changed(self) -> bool {
        matches!(
            self,
            PreambleOutcome::Created
                | PreambleOutcome::Prepended
                | PreambleOutcome::Removed
                | PreambleOutcome::Deleted
        )
    }
}

/// Ensure the instructions file at `path` opens with the required preamble,
/// creating the file (and parents) when missing and prepending the marker when
/// present-but-missing. Delegates the "is it there?" check to
/// [`status::preamble_present_in`] so install and `mirdan status` stay in lockstep.
fn ensure_preamble(path: &Path) -> Result<PreambleOutcome, RegistryError> {
    let marker = status::PREAMBLE_MARKER;
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                RegistryError::Validation(format!("failed to create {}: {e}", parent.display()))
            })?;
        }
        std::fs::write(path, format!("{marker}\n")).map_err(|e| {
            RegistryError::Validation(format!("failed to create {}: {e}", path.display()))
        })?;
        return Ok(PreambleOutcome::Created);
    }
    let content = std::fs::read_to_string(path).map_err(|e| {
        RegistryError::Validation(format!("failed to read {}: {e}", path.display()))
    })?;
    if status::preamble_present_in(&content) {
        return Ok(PreambleOutcome::AlreadyPresent);
    }
    std::fs::write(path, format!("{marker}\n\n{content}")).map_err(|e| {
        RegistryError::Validation(format!("failed to update {}: {e}", path.display()))
    })?;
    Ok(PreambleOutcome::Prepended)
}

/// Strip the preamble (and any blank lines immediately after it) from the
/// instructions file at `path`, deleting the file when only the preamble
/// remained. A missing file or one without the preamble is a no-op.
fn remove_preamble(path: &Path) -> Result<PreambleOutcome, RegistryError> {
    if !path.exists() {
        return Ok(PreambleOutcome::Absent);
    }
    let content = std::fs::read_to_string(path).map_err(|e| {
        RegistryError::Validation(format!("failed to read {}: {e}", path.display()))
    })?;
    if !status::preamble_present_in(&content) {
        return Ok(PreambleOutcome::Absent);
    }
    let mut after_preamble: Vec<&str> = Vec::new();
    let mut found = false;
    for line in content.lines() {
        if !found && line.contains(status::PREAMBLE_MARKER) {
            found = true;
            continue;
        }
        if found {
            after_preamble.push(line);
        }
    }
    while after_preamble.first().is_some_and(|l| l.trim().is_empty()) {
        after_preamble.remove(0);
    }
    if after_preamble.is_empty() {
        std::fs::remove_file(path).map_err(|e| {
            RegistryError::Validation(format!("failed to delete {}: {e}", path.display()))
        })?;
        return Ok(PreambleOutcome::Deleted);
    }
    let new_content = after_preamble.join("\n") + "\n";
    std::fs::write(path, new_content).map_err(|e| {
        RegistryError::Validation(format!("failed to update {}: {e}", path.display()))
    })?;
    Ok(PreambleOutcome::Removed)
}

/// Register the profile's MCP server across detected agents.
///
/// With no explicit `root`, dispatches through the strategy-aware
/// [`register_mcp_server`] applier (handling Claude local scope, generic JSON
/// agents, etc.). With an explicit `root`, registers directly into each agent's
/// root-relative MCP config so the operation never reads `current_dir()`.
fn install_profile_mcp(
    server: &ProfileMcpServer,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    let entry = McpServerEntry {
        command: server.command.clone(),
        args: server.args.clone(),
        env: std::collections::BTreeMap::new(),
    };
    match root {
        None => register_mcp_server(scope, &server.name, &entry, reporter),
        Some(root) => register_mcp_server_at(root, &server.name, &entry, scope, reporter),
    }
}

/// Resolve a detected agent's MCP config and its root-relative config-file path.
///
/// Returns the agent's [`McpConfigDef`] paired with the file to write: the
/// absolute global config under `User` scope (`global`), or the project config
/// joined onto `root` otherwise. `None` when the agent declares no MCP config or
/// no config path for the scope, signalling the caller to skip it.
fn resolve_agent_mcp_config<'a>(
    agent: &'a AgentDef,
    global: bool,
    root: &Path,
) -> Option<(&'a agents::McpConfigDef, PathBuf)> {
    let mcp_cfg = agent.mcp_config.as_ref()?;
    let config_path = if global {
        agents::agent_global_mcp_config(agent)
    } else {
        agents::agent_project_mcp_config(agent).map(|p| root.join(p))
    }?;
    Some((mcp_cfg, config_path))
}

/// Root-explicit MCP registration: write the server entry into each detected
/// agent's project MCP config resolved against `root`.
///
/// Only project/local scope is rooted; user scope uses the agent's absolute
/// global MCP config. Agents without an MCP config for the scope are skipped.
fn register_mcp_server_at(
    root: &Path,
    server_name: &str,
    entry: &McpServerEntry,
    scope: InitScope,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    for_each_detected_agent(
        scope,
        reporter,
        |agent, global| {
            let Some((mcp_cfg, config_path)) = resolve_agent_mcp_config(agent, global, root) else {
                return Ok(None);
            };
            mcp_config::register_mcp_server(
                &config_path,
                &mcp_cfg.servers_key,
                server_name,
                entry,
                &mcp_cfg.entry_extras,
            )?;
            Ok(Some(AgentAction {
                verb: "Registered".to_string(),
                message: format!("{server_name} MCP server for {}", agent.name),
            }))
        },
        |changed| {
            InitResult::ok(
                APPLIER_COMPONENT,
                format!("Registered applied to {changed} agent(s)"),
            )
        },
    )
}

/// Root-explicit MCP unregistration mirroring [`register_mcp_server_at`].
fn unregister_mcp_server_at(
    root: &Path,
    server_name: &str,
    scope: InitScope,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    for_each_detected_agent(
        scope,
        reporter,
        |agent, global| {
            let Some((mcp_cfg, config_path)) = resolve_agent_mcp_config(agent, global, root) else {
                return Ok(None);
            };
            Ok(
                mcp_config::unregister_mcp_server(&config_path, &mcp_cfg.servers_key, server_name)?
                    .then(|| AgentAction {
                        verb: "Removed".to_string(),
                        message: format!("{server_name} MCP server from {}", agent.name),
                    }),
            )
        },
        |changed| {
            InitResult::ok(
                APPLIER_COMPONENT,
                format!("Removed applied to {changed} agent(s)"),
            )
        },
    )
}

/// Install everything a [`Profile`] declares, in priority order.
///
/// Steps, each a no-op when the profile does not declare it:
/// 1. register the `mcp_server` across detected agents (strategy-aware, or
///    root-explicit when `root` is `Some`),
/// 2. render the selected builtin skills with Liquid + the partial library and
///    deploy them via store + symlink,
/// 3. render and deploy the selected builtin agents the same way,
/// 4. apply the statusline / preamble when the profile sets those flags.
///
/// `root` makes the whole operation CWD-free: project-scope skill/agent stores,
/// agent directories, and MCP configs are resolved against `root` instead of the
/// process working directory. Pass `None` for the conventional CWD-rooted
/// behavior.
///
/// # Lockfile
///
/// Builtin skills/agents deployed here are deliberately *not* recorded in
/// `mirdan-lock.json`. That lockfile tracks registry-installed packages (download
/// URL + integrity hash) so they can be updated/reinstalled; builtins are shipped
/// in the binary and have no such identity. All builtin lifecycle — `mirdan
/// status`/`list` (filesystem scan of the skill/agent stores) and deinit (profile
/// selector) — is path- and profile-driven, never lockfile-driven, so the absent
/// entries are inert. This is an intentional simplification over the prior
/// per-component installers, which wrote inert `resolved: "builtin"` rows.
pub fn init_profile(
    profile: &Profile,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    let mut results = Vec::new();

    if let Some(ref server) = profile.mcp_server {
        results.extend(install_profile_mcp(server, scope, root, reporter));
    }

    if let Some(ref selector) = profile.skills {
        match install_profile_skills(selector, scope, root, reporter) {
            Ok(targets) if !targets.is_empty() => results.push(InitResult::ok(
                "profile-skills",
                format!("Deployed skills to {}", targets.join(", ")),
            )),
            Ok(_) => {}
            Err(e) => results.push(InitResult::error("profile-skills", e.to_string())),
        }
    }

    if let Some(ref selector) = profile.agents {
        match install_profile_agents(selector, scope, root, reporter) {
            Ok(targets) if !targets.is_empty() => results.push(InitResult::ok(
                "profile-agents",
                format!("Deployed agents to {}", targets.join(", ")),
            )),
            Ok(_) => {}
            Err(e) => results.push(InitResult::error("profile-agents", e.to_string())),
        }
    }

    if profile.statusline {
        results.extend(apply_profile_statusline(true, scope, root, reporter));
    }

    if profile.preamble {
        results.extend(apply_profile_preamble(true, scope, root, reporter));
    }

    results
}

/// Remove everything a [`Profile`] declares, mirroring [`init_profile`].
///
/// Unregisters the MCP server, removes the selected skills/agents from detected
/// agents, and strips the statusline block / CLAUDE.md preamble when the profile
/// declared them. `root` roots project-scope paths so deinit is CWD-free.
///
/// # Version coupling
///
/// The profile is the single source of truth, so deinit removes exactly the
/// builtin skills/agents the *current* binary's selector resolves to — not a
/// recorded record of what a past install wrote. If the builtin set drifts
/// between the version that installed and the version that deinits (a skill is
/// renamed or dropped), a now-absent name will not be swept, leaving an orphaned
/// symlink + store entry. This is inherent to the data-driven design (there is
/// no per-install manifest to consult); callers that must deinit across such a
/// drift should run deinit with the same binary version that installed.
pub fn deinit_profile(
    profile: &Profile,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    let mut results = Vec::new();
    let global = scope_is_global(scope);

    if let Some(ref server) = profile.mcp_server {
        match root {
            None => results.extend(unregister_mcp_server(scope, &server.name, reporter)),
            Some(root) => results.extend(unregister_mcp_server_at(
                root,
                &server.name,
                scope,
                reporter,
            )),
        }
    }

    if let Some(ref selector) = profile.skills {
        let names = resolved_skill_names(selector);
        for name in &names {
            if let Err(e) = uninstall_skill_at(name, None, global, root) {
                reporter.emit(&InitEvent::Warning {
                    message: format!("Failed to uninstall {name} skill: {e}"),
                });
            }
        }
        if !names.is_empty() {
            results.push(InitResult::ok(
                "profile-skills",
                format!("Removed {} skill(s)", names.len()),
            ));
        }
    }

    if let Some(ref selector) = profile.agents {
        let names = resolved_agent_names(selector);
        for name in &names {
            if let Err(e) = uninstall_agent_at(name, None, global, root) {
                reporter.emit(&InitEvent::Warning {
                    message: format!("Failed to uninstall {name} agent: {e}"),
                });
            }
        }
        if !names.is_empty() {
            results.push(InitResult::ok(
                "profile-agents",
                format!("Removed {} agent(s)", names.len()),
            ));
        }
    }

    if profile.statusline {
        results.extend(apply_profile_statusline(false, scope, root, reporter));
    }

    if profile.preamble {
        results.extend(apply_profile_preamble(false, scope, root, reporter));
    }

    results
}

/// Install a [`Profile`] and then run a registry of genuine tool-lifecycle
/// components, returning the combined results.
///
/// This is the single shared glue every profile consumer needs: the profile
/// installer ([`init_profile`]) followed by the tool's own `Initializable`
/// components (e.g. a `.code-context/` directory, `.kanban/` merge drivers, a
/// `Bash` denial). Each tool CLI and sah build their `Profile`, register their
/// tool-lifecycle components, and call this — instead of re-spelling the
/// "profile then registry, concatenate results" sequence in four places.
///
/// Results are ordered profile-first (matching every prior consumer): the MCP
/// server, skills, agents, statusline/preamble, then the registry components in
/// priority order. `root` is forwarded to [`init_profile`] for CWD-free
/// installs. Compute the exit code from the returned results' `status` fields.
pub fn init_profile_with_registry(
    profile: &Profile,
    registry: &InitRegistry,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    let mut results = init_profile(profile, scope, root, reporter);
    results.extend(registry.run_all_init(&scope, reporter));
    results
}

/// Deinstall a [`Profile`] after deinitializing a registry of tool-lifecycle
/// components, returning the combined results.
///
/// The mirror of [`init_profile_with_registry`]: tool-lifecycle teardown runs
/// first (reverse priority, via [`InitRegistry::run_all_deinit`]), then the
/// profile deinstaller ([`deinit_profile`]) unregisters the MCP server and
/// removes the selected skills/agents. This ordering matches every prior
/// consumer so a tool's directory is removed before its MCP registration.
pub fn deinit_profile_with_registry(
    profile: &Profile,
    registry: &InitRegistry,
    scope: InitScope,
    root: Option<&Path>,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    let mut results = registry.run_all_deinit(&scope, reporter);
    results.extend(deinit_profile(profile, scope, root, reporter));
    results
}

/// Resolve the builtin skill names a selector picks (for deinit).
fn resolved_skill_names(selector: &Selector) -> Vec<String> {
    let resolver = SkillResolver::new();
    let available: std::collections::HashMap<String, Vec<String>> = resolver
        .resolve_builtins()
        .iter()
        .map(|(name, skill)| (name.clone(), skill.profiles.clone()))
        .collect();
    selector.select(&available)
}

/// Resolve the builtin agent names a selector picks (for deinit).
fn resolved_agent_names(selector: &Selector) -> Vec<String> {
    let resolver = AgentResolver::new();
    let available: std::collections::HashMap<String, Vec<String>> = resolver
        .resolve_builtins()
        .keys()
        .map(|name| (name.clone(), Vec::new()))
        .collect();
    selector.select(&available)
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
    tracing::debug!("  Deployed to {}", target_path);

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
) -> Result<Vec<crate::DeployResult>, RegistryError> {
    // Try both CWD and HOME for lockfile (GUI sets CWD to HOME,
    // CLI may be in a project directory).
    let project_root = std::env::current_dir()?;
    let mut lf = Lockfile::load(&project_root)?;

    // Also check HOME if CWD lockfile is empty and differs from HOME
    let home = dirs::home_dir();
    if lf.packages.is_empty() {
        if let Some(ref h) = home {
            if *h != project_root {
                if let Ok(home_lf) = Lockfile::load(h) {
                    if !home_lf.packages.is_empty() {
                        lf = home_lf;
                    }
                }
            }
        }
    }

    // Resolve the lockfile key — try exact match, then display name, then git source
    let lockfile_key = if lf.get_package(name).is_some() {
        Some(name.to_string())
    } else if let Some((key, _)) = lf.find_by_display_name(name) {
        Some(key.to_string())
    } else {
        let matching = find_packages_by_git_source(&lf, name);
        if !matching.is_empty() {
            // Uninstall all packages from this git source
            for pkg_name in &matching {
                let pkg = lf.get_package(pkg_name).unwrap();
                let pkg_type = pkg.package_type;
                match pkg_type {
                    PackageType::Skill => uninstall_skill(pkg_name, agent_filter, global)?,
                    PackageType::Validator => uninstall_validator(pkg_name, global)?,
                    PackageType::Tool => uninstall_tool(pkg_name, agent_filter, global)?,
                    PackageType::Plugin => uninstall_plugin(pkg_name, agent_filter, global)?,
                    PackageType::Agent => uninstall_agent(pkg_name, agent_filter, global)?,
                }
                lf.remove_package(pkg_name);
                tracing::debug!(pkg_name, "uninstalled");
            }
            let save_dir = home.as_deref().unwrap_or(&project_root);
            lf.save(save_dir)?;
            tracing::debug!(
                count = matching.len(),
                source = name,
                "uninstalled packages"
            );
            return Ok(vec![crate::DeployResult::message(
                crate::DeployAction::Removed,
                format!("Uninstalled {} package(s) from {}", matching.len(), name),
            )]);
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

    match pkg_type {
        PackageType::Skill => uninstall_skill(display_name, agent_filter, global)?,
        PackageType::Validator => uninstall_validator(display_name, global)?,
        PackageType::Tool => uninstall_tool(display_name, agent_filter, global)?,
        PackageType::Plugin => uninstall_plugin(display_name, agent_filter, global)?,
        PackageType::Agent => uninstall_agent(display_name, agent_filter, global)?,
    }

    // Update lockfile
    lf.remove_package(key);
    let save_dir = home.as_deref().unwrap_or(&project_root);
    lf.save(save_dir)?;
    tracing::debug!(key, "uninstalled");

    Ok(vec![crate::DeployResult::message(
        crate::DeployAction::Removed,
        format!("Uninstalled {}", key),
    )])
}

pub fn uninstall_skill(
    name: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<(), RegistryError> {
    uninstall_skill_at(name, agent_filter, global, None)
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
            rooted(root, global, agent_project_skill_dir(&agent.def))
        };
        let link_path = agent_skill_dir.join(&link_name);

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

    // 2. Remove all store entries matching this skill name.
    // Skills can exist at both flat paths (e.g. ~/.skills/explain/) and
    // nested paths (e.g. ~/.skills/owner/repo/explain/) depending on
    // how they were installed (git vs registry). Remove all of them.
    let store_root = rooted(root, global, store::skill_store_dir(global));
    let flat_path = store_root.join(&sanitized);
    if flat_path.exists() {
        std::fs::remove_dir_all(&flat_path)?;
        tracing::debug!(path = %flat_path.display(), "removed store entry");
    }
    // Also scan recursively for nested store entries with matching SKILL.md name
    remove_matching_store_entries(&store_root, name)?;

    if removed == 0 {
        tracing::warn!(
            name,
            "no symlinks found in agent dirs (already cleaned up?)"
        );
    }

    Ok(())
}

/// Recursively scan the store for directories containing SKILL.md whose
/// frontmatter name matches, and remove them. Also cleans up empty parent dirs.
fn remove_matching_store_entries(dir: &Path, name: &str) -> Result<(), RegistryError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let skill_md = path.join("SKILL.md");
            if skill_md.exists() {
                // Check if this skill's name matches
                let fm_name = read_skill_frontmatter_name(&skill_md);
                let dir_name = path.file_name().map(|n| n.to_string_lossy().to_string());
                if fm_name.as_deref() == Some(name) || dir_name.as_deref() == Some(name) {
                    std::fs::remove_dir_all(&path)?;
                    tracing::debug!(path = %path.display(), "removed nested store entry");
                    // Clean up empty parent directories up to the store root
                    let mut parent = path.parent();
                    while let Some(p) = parent {
                        if p == dir {
                            break;
                        }
                        if std::fs::read_dir(p)
                            .map(|mut d| d.next().is_none())
                            .unwrap_or(false)
                        {
                            std::fs::remove_dir(p)?;
                        } else {
                            break;
                        }
                        parent = p.parent();
                    }
                }
            } else {
                // Recurse into subdirectories
                remove_matching_store_entries(&path, name)?;
            }
        }
    }

    Ok(())
}

/// Read the name field from a SKILL.md frontmatter.
fn read_skill_frontmatter_name(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let content = content.trim();
    let rest = content.strip_prefix("---")?;
    let end = rest.find("---")?;
    let frontmatter = &rest[..end];
    let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(frontmatter).ok()?;
    yaml.get("name")?.as_str().map(|s| s.to_string())
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
    tracing::debug!("  Removed from {}", target_dir.display());
    Ok(())
}

/// Install an MCP server to all detected (or filtered) agents.
///
/// Registers the MCP server entry in each agent's MCP config file and records
/// it in the lockfile as a Tool package.
pub async fn run_install_mcp(
    name: &str,
    command: &str,
    args: Vec<String>,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<Vec<crate::DeployResult>, RegistryError> {
    let config = agents::load_agents_config()?;
    let target_agents = agents::resolve_target_agents(&config, agent_filter)?;

    let entry = mcp_config::McpServerEntry {
        command: command.to_string(),
        args,
        env: std::collections::BTreeMap::new(),
    };

    let mut installed = Vec::new();

    for agent in &target_agents {
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
                    &mcp_cfg.entry_extras,
                )?;
                tracing::debug!(
                    "  Installed MCP server '{}' for {} ({})",
                    name,
                    agent.def.name,
                    config_path.display()
                );
                installed.push(agent.def.id.clone());
            }
        } else {
            tracing::debug!("  Skipped {} (no MCP support)", agent.def.name);
        }
    }

    if installed.is_empty() {
        tracing::debug!("No agents with MCP support found.");
        return Ok(vec![crate::DeployResult::message(
            crate::DeployAction::Skipped,
            "No agents with MCP support found.",
        )]);
    }

    // Update lockfile
    let project_root = std::env::current_dir()?;
    let mut lf = Lockfile::load(&project_root)?;
    lf.add_package(
        name.to_string(),
        LockedPackage {
            package_type: PackageType::Tool,
            version: "0.0.0".to_string(),
            resolved: format!("mcp:{}", entry.command),
            integrity: String::new(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            targets: installed.clone(),
        },
    );
    lf.save(&project_root)?;
    tracing::debug!("  Updated mirdan-lock.json");

    tracing::debug!(
        "\nInstalled MCP server '{}' for {} agent(s)",
        name,
        installed.len()
    );
    Ok(vec![crate::DeployResult::message(
        crate::DeployAction::Created,
        format!(
            "Installed MCP server '{}' for {} agent(s)",
            name,
            installed.len()
        ),
    )])
}

/// Uninstall an MCP server from all detected (or filtered) agents.
pub async fn run_uninstall_mcp(
    name: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<Vec<crate::DeployResult>, RegistryError> {
    let config = agents::load_agents_config()?;
    let target_agents = agents::resolve_target_agents(&config, agent_filter)?;

    let mut results = Vec::new();
    let mut removed = 0;

    for agent in &target_agents {
        if let Some(ref mcp_cfg) = agent.def.mcp_config {
            let config_path = if global {
                agents::agent_global_mcp_config(&agent.def)
            } else {
                agents::agent_project_mcp_config(&agent.def)
            };

            if let Some(config_path) = config_path {
                if mcp_config::unregister_mcp_server(&config_path, &mcp_cfg.servers_key, name)? {
                    results.push(crate::DeployResult::removed(
                        &config_path,
                        format!(
                            "Removed MCP server '{}' from {} ({})",
                            name,
                            agent.def.name,
                            config_path.display()
                        ),
                    ));
                    removed += 1;
                }
            }
        }
    }

    // Update lockfile
    let project_root = std::env::current_dir()?;
    let mut lf = Lockfile::load(&project_root)?;
    lf.remove_package(name);
    lf.save(&project_root)?;
    results.push(crate::DeployResult::updated(
        project_root.join("mirdan-lock.json"),
        "Updated mirdan-lock.json".to_string(),
    ));

    results.push(crate::DeployResult::message(
        crate::DeployAction::Removed,
        format!(
            "Uninstalled MCP server '{}' from {} agent(s)",
            name, removed
        ),
    ));
    Ok(results)
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
    // Check agent store
    if store::agent_store_dir(global)
        .join(sanitize_dir_name(name))
        .exists()
    {
        return PackageType::Agent;
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
                    &mcp_cfg.servers_key,
                    name,
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
            tracing::debug!("  Deployed to {} ({})", target.display(), agent.def.name);
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
                        // Read and register MCP servers from plugin's .mcp.json.
                        // Plugin authors edit this file by hand, so accept JSONC.
                        if let Ok(content) = std::fs::read_to_string(&plugin_mcp) {
                            if let Ok(json) = crate::parse_jsonc(&content) {
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
                                                &mcp_cfg.entry_extras,
                                            );
                                            tracing::debug!(
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
            tracing::debug!("Agent {} has no plugin path, skipping", agent.def.id);
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
                    tracing::debug!(
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
        tracing::debug!("  Removed store entry {}", store_path.display());
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
                tracing::debug!("  Removed from {} ({})", target.display(), agent.def.name);
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

fn uninstall_agent(
    name: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<(), RegistryError> {
    let removed = uninstall_agent_at(name, agent_filter, global, None)?;
    if removed == 0 {
        let scope = if global { "global" } else { "project" };
        return Err(RegistryError::NotFound(format!(
            "Agent '{}' not found in any coding agent ({} scope)",
            name, scope
        )));
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
fn uninstall_agent_at(
    name: &str,
    agent_filter: Option<&str>,
    global: bool,
    root: Option<&Path>,
) -> Result<usize, RegistryError> {
    let config = agents::load_agents_config()?;
    let target_agents = agents::resolve_target_agents(&config, agent_filter)?;

    let sanitized = sanitize_dir_name(name);
    let mut removed = 0;

    // 1. Remove symlinks from each coding agent's agent directory
    for agent in &target_agents {
        let agent_dir = if global {
            agent_global_agent_dir(&agent.def)
        } else {
            agent_project_agent_dir(&agent.def).map(|d| rooted(root, global, d))
        };

        if let Some(base_dir) = agent_dir {
            let link_name = store::symlink_name(&sanitized, &agent.def.symlink_policy);
            let link_path = base_dir.join(&link_name);

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
    }

    // 2. Remove store entry if no remaining symlinks reference it
    let store_path = rooted(root, global, store::agent_store_dir(global)).join(&sanitized);
    if store_path.exists() {
        let all_agents = agents::get_detected_agents(&config);
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
            std::fs::remove_dir_all(&store_path)?;
            tracing::debug!("  Removed store entry {}", store_path.display());
        }
    }

    Ok(removed)
}

/// Install a specific package version (used by update command).
pub async fn install_package(
    name: &str,
    version: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<Vec<crate::DeployResult>, RegistryError> {
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

// ── Scope-aware appliers (the single per-agent iteration + reporting site) ──
//
// Tools and CLIs call these to apply a declarative change — register me as an
// MCP server, deny a tool — across every detected agent. Each applier loads
// the detected agents, dispatches to the right [`strategy::AgentConfigStrategy`]
// via [`strategy::strategy_for`], applies the change, and emits reporter
// events. They take a `swissarmyhammer_common` [`InitScope`] + [`InitReporter`]
// so the same implementation serves the shell tool, `sah`, and `shelltool`.

use swissarmyhammer_common::lifecycle::{InitRegistry, InitResult, InitScope};
use swissarmyhammer_common::reporter::{InitEvent, InitReporter};

use crate::agents::AgentDef;
use crate::mcp_config::McpServerEntry;
use crate::strategy::{self, AgentConfigStrategy};

/// Component name used in the `InitResult`s the appliers return.
const APPLIER_COMPONENT: &str = "agent-config";

/// Load detected agents, or return a single error `InitResult` describing the
/// failure (so callers can short-circuit without a panic).
fn detected_agents_or_error() -> Result<Vec<crate::agents::DetectedAgent>, Vec<InitResult>> {
    match agents::load_agents_config() {
        Ok(config) => Ok(agents::get_detected_agents(&config)),
        Err(e) => Err(vec![InitResult::error(
            APPLIER_COMPONENT,
            format!("Failed to load agents config: {e}"),
        )]),
    }
}

/// Apply `action` to every detected agent's strategy, emitting an Action event
/// (with `verb`) for each agent that changed and a Warning for each error, then
/// aggregate into a single `InitResult`.
///
/// `changed_message` / `noop_message` build the human-readable detail for the
/// `InitResult` describing how many agents were affected.
fn for_each_agent_strategy(
    scope: InitScope,
    reporter: &dyn InitReporter,
    verb: &str,
    action: impl Fn(&dyn AgentConfigStrategy, &AgentDef) -> Result<bool, RegistryError>,
    action_message: impl Fn(&AgentDef) -> String,
) -> Vec<InitResult> {
    let agents = match detected_agents_or_error() {
        Ok(a) => a,
        Err(results) => return results,
    };

    let mut changed = 0usize;
    for agent in &agents {
        let strategy = strategy::strategy_for(&agent.def);
        match action(strategy.as_ref(), &agent.def) {
            Ok(true) => {
                reporter.emit(&InitEvent::Action {
                    verb: verb.to_string(),
                    message: action_message(&agent.def),
                });
                changed += 1;
            }
            Ok(false) => {}
            Err(e) => reporter.emit(&InitEvent::Warning {
                message: format!("{} ({}): {e}", agent.def.name, scope_label(scope)),
            }),
        }
    }

    vec![InitResult::ok(
        APPLIER_COMPONENT,
        format!("{verb} applied to {changed} agent(s)"),
    )]
}

/// A per-agent change produced by a [`for_each_detected_agent`] closure: the
/// reporter `verb` and human-readable `message` describing what changed.
struct AgentAction {
    /// Reporter Action verb (e.g. `"Installed"`, `"Removed"`, `"Registered"`).
    verb: String,
    /// Reporter Action message describing the agent and what was applied.
    message: String,
}

/// Drive an applier over every detected agent for the root-explicit init path.
///
/// Owns the structural skeleton shared by the statusline, preamble, and MCP
/// register/unregister appliers: load detected agents (short-circuiting to an
/// error `InitResult` on failure), compute the `global` scope flag, run `apply`
/// per agent, emit an Action event for each `Ok(Some(_))` change, emit a Warning
/// (labelled with `scope`) for each `Err`, count the changes, and aggregate into
/// a single `InitResult` built by `summary` from the change count.
///
/// `apply` receives each [`AgentDef`] plus the resolved `global` flag and returns
/// `Ok(Some(action))` when the agent changed, `Ok(None)` when it was already in
/// the desired state or skipped, or `Err` on failure.
fn for_each_detected_agent(
    scope: InitScope,
    reporter: &dyn InitReporter,
    apply: impl Fn(&AgentDef, bool) -> Result<Option<AgentAction>, RegistryError>,
    summary: impl Fn(usize) -> InitResult,
) -> Vec<InitResult> {
    let agents = match detected_agents_or_error() {
        Ok(a) => a,
        Err(results) => return results,
    };
    let global = scope_is_global(scope);

    let mut changed = 0usize;
    for agent in &agents {
        match apply(&agent.def, global) {
            Ok(Some(action)) => {
                reporter.emit(&InitEvent::Action {
                    verb: action.verb,
                    message: action.message,
                });
                changed += 1;
            }
            Ok(None) => {}
            Err(e) => reporter.emit(&InitEvent::Warning {
                message: format!("{} ({}): {e}", agent.def.name, scope_label(scope)),
            }),
        }
    }

    vec![summary(changed)]
}

/// Short scope label for reporter/warning messages.
fn scope_label(scope: InitScope) -> &'static str {
    match scope {
        InitScope::Project => "project",
        InitScope::Local => "local",
        InitScope::User => "user",
    }
}

/// Register `server_name` → `entry` as an MCP server across every detected
/// agent at `scope`, dispatching to each agent's strategy.
pub fn register_mcp_server(
    scope: InitScope,
    server_name: &str,
    entry: &McpServerEntry,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    for_each_agent_strategy(
        scope,
        reporter,
        "Registered",
        |strategy, agent| strategy.register_mcp(agent, scope, server_name, entry),
        |agent| format!("{server_name} MCP server for {}", agent.name),
    )
}

/// Unregister `server_name` as an MCP server across every detected agent at
/// `scope`, dispatching to each agent's strategy.
pub fn unregister_mcp_server(
    scope: InitScope,
    server_name: &str,
    reporter: &dyn InitReporter,
) -> Vec<InitResult> {
    for_each_agent_strategy(
        scope,
        reporter,
        "Removed",
        |strategy, agent| strategy.unregister_mcp(agent, scope, server_name),
        |agent| format!("{server_name} MCP server from {}", agent.name),
    )
}

/// Deny `tool` across every detected agent at `scope`, dispatching to each
/// agent's strategy. Agents with no permission mechanism are silently skipped.
pub fn deny_tool(scope: InitScope, tool: &str, reporter: &dyn InitReporter) -> Vec<InitResult> {
    for_each_agent_strategy(
        scope,
        reporter,
        "Configured",
        |strategy, agent| strategy.deny_tool(agent, scope, tool),
        |agent| {
            format!(
                "{tool} tool denied for {} — use the shell tool instead",
                agent.name
            )
        },
    )
}

/// Allow `tool` (remove a prior deny) across every detected agent at `scope`,
/// dispatching to each agent's strategy.
pub fn allow_tool(scope: InitScope, tool: &str, reporter: &dyn InitReporter) -> Vec<InitResult> {
    for_each_agent_strategy(
        scope,
        reporter,
        "Removed",
        |strategy, agent| strategy.allow_tool(agent, scope, tool),
        |agent| format!("{tool} deny rule for {}", agent.name),
    )
}

#[cfg(test)]
mod applier_tests {
    use super::*;
    use serial_test::serial;
    use swissarmyhammer_common::reporter::NullReporter;

    use crate::test_support::MirdanConfigGuard;

    /// Write a synthetic single-agent (generic) config whose detect dir is the
    /// project dir (so detection always fires) and whose MCP config is a
    /// relative `.mcp.json`.
    fn write_generic_agents_config(project_dir: &Path) -> PathBuf {
        let agents_yaml = format!(
            r#"agents:
  - id: fake-agent
    name: Fake Agent
    project_path: .fake/skills
    global_path: "~/.fake/skills"
    detect:
      - dir: "{detect}"
    mcp_config:
      project_path: .mcp.json
      servers_key: mcpServers
"#,
            detect = project_dir.display(),
        );
        let config_path = project_dir.join("agents.yaml");
        std::fs::write(&config_path, agents_yaml).unwrap();
        config_path
    }

    fn entry() -> McpServerEntry {
        McpServerEntry {
            command: "sah".to_string(),
            args: vec!["serve".to_string()],
            env: std::collections::BTreeMap::new(),
        }
    }

    #[test]
    #[serial]
    fn register_mcp_server_iterates_detected_agent_and_dispatches() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().canonicalize().unwrap();
        let _cwd = swissarmyhammer_common::test_utils::CurrentDirGuard::new(&project).unwrap();
        let config_path = write_generic_agents_config(&project);
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let reporter = NullReporter;
        let results = register_mcp_server(InitScope::Project, "sah", &entry(), &reporter);
        assert!(results
            .iter()
            .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error));

        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(project.join(".mcp.json")).unwrap())
                .unwrap();
        assert_eq!(json["mcpServers"]["sah"]["command"], "sah");

        let results = unregister_mcp_server(InitScope::Project, "sah", &reporter);
        assert!(results
            .iter()
            .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error));
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(project.join(".mcp.json")).unwrap())
                .unwrap();
        assert!(json["mcpServers"]["sah"].is_null());
    }

    #[test]
    #[serial]
    fn deny_tool_noop_for_agent_without_permission_mechanism() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().canonicalize().unwrap();
        let _cwd = swissarmyhammer_common::test_utils::CurrentDirGuard::new(&project).unwrap();
        let config_path = write_generic_agents_config(&project);
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let reporter = NullReporter;
        // The generic strategy has no deny mechanism: nothing is written and
        // the applier still returns Ok.
        let results = deny_tool(InitScope::Project, "Bash", &reporter);
        assert!(results
            .iter()
            .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

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
    fn test_stage_and_deploy_skill_rejects_traversal() {
        let err = stage_and_deploy_skill("../escape", "# Skill\n").unwrap_err();
        assert!(matches!(err, RegistryError::Validation(_)));
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
            vec![
                "-y",
                "@modelcontextprotocol/server-filesystem",
                "/tmp/safe-dir"
            ]
        );
        assert_eq!(mcp_fm.transport, Some("stdio".to_string()));
        assert_eq!(mcp_fm.env.get("NODE_ENV").unwrap(), "production");
    }

    #[test]
    #[serial]
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
            vec![
                "-y",
                "@modelcontextprotocol/server-filesystem",
                "/tmp/safe-dir"
            ],
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
    #[serial]
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
    #[serial]
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
        let mcp: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(work.path().join(".mcp.json")).unwrap())
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
        let mcp: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(work.path().join(".mcp.json")).unwrap())
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
    #[serial]
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
        let name =
            mcp_config::read_plugin_json(&plugin_dir.join(".claude-plugin/plugin.json")).unwrap();
        assert_eq!(name, "my-plugin");
    }

    #[test]
    #[serial]
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
    #[serial]
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
    #[serial]
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
    #[serial]
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
    #[serial]
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
        let mcp: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(work.path().join(".mcp.json")).unwrap())
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
        let mcp: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(work.path().join(".mcp.json")).unwrap())
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
    #[serial]
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
    #[serial]
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
    #[serial]
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
    #[serial]
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
    #[serial]
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
    #[serial]
    async fn test_e2e_all_four_types_coexist() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        // 1. Install a skill
        let skill_src = work.path().join("src-skill");
        make_local_skill(&skill_src, "test-skill", "1.0.0");
        let skill_targets = deploy_skill("test-skill", &skill_src, None, false)
            .await
            .unwrap();
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
        assert!(work
            .path()
            .join(".avp/validators/test-val/VALIDATOR.md")
            .exists());
        assert!(work.path().join(".tools/test-tool/TOOL.md").exists());
        assert!(work
            .path()
            .join(".claude/plugins/test-plugin/.claude-plugin/plugin.json")
            .exists());

        // 6. Verify list discovers all four
        let all = crate::list::discover_packages(false, false, false, false, None);
        let names: Vec<&str> = all.iter().map(|p| p.name.as_str()).collect();
        assert!(
            names.contains(&"test-skill"),
            "Should find skill in list: {:?}",
            names
        );
        assert!(
            names.contains(&"test-val"),
            "Should find validator in list: {:?}",
            names
        );
        assert!(
            names.contains(&"test-tool"),
            "Should find tool in list: {:?}",
            names
        );
        assert!(
            names.contains(&"test-plugin"),
            "Should find plugin in list: {:?}",
            names
        );

        // 7. Verify type-specific filters work
        let skills_only = crate::list::discover_packages(true, false, false, false, None);
        assert!(skills_only
            .iter()
            .all(|p| p.package_type == PackageType::Skill));
        assert!(skills_only.iter().any(|p| p.name == "test-skill"));

        let tools_only = crate::list::discover_packages(false, false, true, false, None);
        assert!(tools_only
            .iter()
            .all(|p| p.package_type == PackageType::Tool));
        assert!(tools_only.iter().any(|p| p.name == "test-tool"));

        let plugins_only = crate::list::discover_packages(false, false, false, true, None);
        assert!(plugins_only
            .iter()
            .all(|p| p.package_type == PackageType::Plugin));
        assert!(plugins_only.iter().any(|p| p.name == "test-plugin"));

        let vals_only = crate::list::discover_packages(false, true, false, false, None);
        assert!(vals_only
            .iter()
            .all(|p| p.package_type == PackageType::Validator));
        assert!(vals_only.iter().any(|p| p.name == "test-val"));

        // 8. Uninstall each type independently — others remain
        uninstall_tool("test-tool", None, false).unwrap();
        assert!(!work.path().join(".tools/test-tool").exists());
        assert!(
            work.path().join(".skills/test-skill/SKILL.md").exists(),
            "Skill should survive tool uninstall"
        );
        assert!(
            work.path().join(".avp/validators/test-val").exists(),
            "Validator should survive tool uninstall"
        );
        assert!(
            work.path().join(".claude/plugins/test-plugin").exists(),
            "Plugin should survive tool uninstall"
        );

        uninstall_plugin("test-plugin", None, false).unwrap();
        assert!(!work.path().join(".claude/plugins/test-plugin").exists());
        assert!(
            work.path().join(".skills/test-skill/SKILL.md").exists(),
            "Skill should survive plugin uninstall"
        );

        uninstall_validator("test-val", false).unwrap();
        assert!(!work.path().join(".avp/validators/test-val").exists());
        assert!(
            work.path().join(".skills/test-skill/SKILL.md").exists(),
            "Skill should survive validator uninstall"
        );

        std::env::set_current_dir(old_dir).unwrap();
    }

    #[test]
    #[serial]
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
        let mcp: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(work.path().join(".mcp.json")).unwrap())
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
        let mcp: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(work.path().join(".mcp.json")).unwrap())
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
    #[serial]
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
    #[serial]
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

    // --- metadata-only tool install tests ---

    #[tokio::test]
    #[serial]
    async fn test_install_tool_from_mcp_config_registers_server() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        let mcp = crate::registry::types::McpConfig {
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-brave-search".to_string(),
            ],
            env: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("BRAVE_API_KEY".to_string(), "test-key".to_string());
                m
            },
        };

        let version_detail = crate::registry::types::VersionDetail {
            name: "brave-search".to_string(),
            version: "1.0.0".to_string(),
            package_type: Some("tool".to_string()),
            download_url: "https://example.com/download".to_string(),
            integrity: None,
            size: None,
            published_at: "2026-01-01T00:00:00Z".to_string(),
            description: Some("Test tool".to_string()),
            author: None,
            license: None,
            tags: None,
            mcp: Some(mcp.clone()),
            tool_md: None,
        };

        install_tool_from_mcp_config("brave-search", &version_detail, &mcp, None, false)
            .await
            .unwrap();

        // Verify .mcp.json was created with the correct entry
        let mcp_json_path = work.path().join(".mcp.json");
        assert!(mcp_json_path.exists(), ".mcp.json should exist");
        let content = std::fs::read_to_string(&mcp_json_path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        let server = &json["mcpServers"]["brave-search"];
        assert_eq!(server["command"].as_str().unwrap(), "npx");
        assert_eq!(server["args"][0].as_str().unwrap(), "-y");
        assert_eq!(
            server["args"][1].as_str().unwrap(),
            "@modelcontextprotocol/server-brave-search"
        );
        assert_eq!(server["env"]["BRAVE_API_KEY"].as_str().unwrap(), "test-key");

        // Verify lockfile was updated
        let lf = Lockfile::load(work.path()).unwrap();
        let pkg = lf.get_package("brave-search").unwrap();
        assert_eq!(pkg.package_type, PackageType::Tool);
        assert_eq!(pkg.version, "1.0.0");

        std::env::set_current_dir(old_dir).unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_install_tool_from_tool_md_content() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        let tool_md = r#"---
name: test-tool
description: A test tool
metadata:
  version: "2.0.0"
mcp:
  command: uvx
  args:
    - "mcp-server-test"
  transport: stdio
---

# Test Tool
"#;

        let version_detail = crate::registry::types::VersionDetail {
            name: "test-tool".to_string(),
            version: "2.0.0".to_string(),
            package_type: Some("tool".to_string()),
            download_url: "https://example.com/download".to_string(),
            integrity: None,
            size: None,
            published_at: "2026-01-01T00:00:00Z".to_string(),
            description: Some("A test tool".to_string()),
            author: None,
            license: None,
            tags: None,
            mcp: None,
            tool_md: Some(tool_md.to_string()),
        };

        install_tool_from_tool_md_content("test-tool", &version_detail, tool_md, None, false)
            .await
            .unwrap();

        // Verify .mcp.json was created with parsed TOOL.md content
        let mcp_json_path = work.path().join(".mcp.json");
        assert!(mcp_json_path.exists(), ".mcp.json should exist");
        let content = std::fs::read_to_string(&mcp_json_path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        let server = &json["mcpServers"]["test-tool"];
        assert_eq!(server["command"].as_str().unwrap(), "uvx");
        assert_eq!(server["args"][0].as_str().unwrap(), "mcp-server-test");

        // Verify tool was stored
        let store = work.path().join(".tools/test-tool");
        assert!(store.join("TOOL.md").exists(), "TOOL.md should be in store");

        // Verify lockfile
        let lf = Lockfile::load(work.path()).unwrap();
        let pkg = lf.get_package("test-tool").unwrap();
        assert_eq!(pkg.package_type, PackageType::Tool);
        assert_eq!(pkg.version, "2.0.0");

        std::env::set_current_dir(old_dir).unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_install_tool_from_metadata_rejects_non_tool() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        let version_detail = crate::registry::types::VersionDetail {
            name: "some-skill".to_string(),
            version: "1.0.0".to_string(),
            package_type: Some("skill".to_string()),
            download_url: "https://example.com/download".to_string(),
            integrity: None,
            size: None,
            published_at: "2026-01-01T00:00:00Z".to_string(),
            description: None,
            author: None,
            license: None,
            tags: None,
            mcp: None,
            tool_md: None,
        };

        let result = install_tool_from_metadata("some-skill", &version_detail, None, false).await;
        assert!(result.is_err(), "Should reject non-tool packages");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("not a tool"),
            "Error should mention not a tool: {}",
            err
        );

        std::env::set_current_dir(old_dir).unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_install_tool_from_mcp_config_then_uninstall() {
        let work = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(work.path()).unwrap();

        let mcp = crate::registry::types::McpConfig {
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@test/server".to_string()],
            env: std::collections::BTreeMap::new(),
        };

        let version_detail = crate::registry::types::VersionDetail {
            name: "ephemeral-tool".to_string(),
            version: "1.0.0".to_string(),
            package_type: Some("tool".to_string()),
            download_url: "https://example.com/download".to_string(),
            integrity: None,
            size: None,
            published_at: "2026-01-01T00:00:00Z".to_string(),
            description: None,
            author: None,
            license: None,
            tags: None,
            mcp: Some(mcp.clone()),
            tool_md: None,
        };

        // Install
        install_tool_from_mcp_config("ephemeral-tool", &version_detail, &mcp, None, false)
            .await
            .unwrap();

        // Verify it's registered
        let mcp_json_path = work.path().join(".mcp.json");
        let content = std::fs::read_to_string(&mcp_json_path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(json["mcpServers"]["ephemeral-tool"].is_object());

        // Uninstall
        uninstall_tool("ephemeral-tool", None, false).unwrap();

        // Verify it's gone from .mcp.json
        let content = std::fs::read_to_string(&mcp_json_path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(
            json["mcpServers"]["ephemeral-tool"].is_null(),
            "Server should be removed after uninstall"
        );

        std::env::set_current_dir(old_dir).unwrap();
    }

    #[test]
    fn test_parse_package_type_from_string() {
        assert_eq!(
            package_type::parse_package_type("tool"),
            Some(PackageType::Tool)
        );
        assert_eq!(
            package_type::parse_package_type("skill"),
            Some(PackageType::Skill)
        );
        assert_eq!(
            package_type::parse_package_type("validator"),
            Some(PackageType::Validator)
        );
        assert_eq!(
            package_type::parse_package_type("plugin"),
            Some(PackageType::Plugin)
        );
        assert_eq!(package_type::parse_package_type("unknown"), None);
        assert_eq!(package_type::parse_package_type("Tool"), None);
        assert_eq!(package_type::parse_package_type(""), None);
    }

    #[test]
    fn test_version_detail_deserializes_with_mcp() {
        let json = r#"{
            "name": "brave-search",
            "version": "1.0.0",
            "type": "tool",
            "description": "Web search",
            "downloadUrl": "https://example.com/download",
            "publishedAt": "2026-01-01T00:00:00Z",
            "mcp": {
                "command": "npx",
                "args": ["-y", "@mcp/server-brave"],
                "env": {"BRAVE_API_KEY": "test"}
            }
        }"#;

        let detail: crate::registry::types::VersionDetail = serde_json::from_str(json).unwrap();

        assert_eq!(detail.name, "brave-search");
        assert_eq!(detail.package_type.as_deref(), Some("tool"));
        assert!(detail.integrity.is_none());
        assert!(detail.size.is_none());

        let mcp = detail.mcp.unwrap();
        assert_eq!(mcp.command, "npx");
        assert_eq!(mcp.args, vec!["-y", "@mcp/server-brave"]);
        assert_eq!(mcp.env.get("BRAVE_API_KEY").unwrap(), "test");
    }

    #[test]
    fn test_version_detail_deserializes_without_optional_fields() {
        let json = r#"{
            "name": "minimal",
            "version": "0.1.0",
            "downloadUrl": "https://example.com/download",
            "publishedAt": "2026-01-01T00:00:00Z"
        }"#;

        let detail: crate::registry::types::VersionDetail = serde_json::from_str(json).unwrap();

        assert_eq!(detail.name, "minimal");
        assert!(detail.package_type.is_none());
        assert!(detail.integrity.is_none());
        assert!(detail.size.is_none());
        assert!(detail.mcp.is_none());
        assert!(detail.tool_md.is_none());
    }
}

#[cfg(test)]
mod profile_tests {
    use super::*;
    use serial_test::serial;
    use swissarmyhammer_common::reporter::NullReporter;
    use swissarmyhammer_common::test_utils::CurrentDirGuard;

    use crate::test_support::MirdanConfigGuard;

    /// Write a synthetic single-agent config that detects `project_dir` and
    /// declares a relative skill dir (`.fake/skills`), agent dir (`.fake/agents`),
    /// `.mcp.json` MCP config, settings file (`.fake/settings.json`), and
    /// instructions file (`.fake/CLAUDE.md`) — the artifact kinds a profile
    /// installs (skills/agents/mcp + statusline/preamble).
    fn write_profile_agents_config(project_dir: &Path) -> PathBuf {
        let agents_yaml = format!(
            r#"agents:
  - id: fake-agent
    name: Fake Agent
    project_path: .fake/skills
    global_path: "~/.fake/skills"
    agent_path: .fake/agents
    settings_path: .fake/settings.json
    instructions_path: .fake/CLAUDE.md
    detect:
      - dir: "{detect}"
    mcp_config:
      project_path: .mcp.json
      servers_key: mcpServers
"#,
            detect = project_dir.display(),
        );
        let config_path = project_dir.join("agents.yaml");
        std::fs::write(&config_path, agents_yaml).unwrap();
        config_path
    }

    /// A sample profile: register an MCP server, deploy one builtin skill that
    /// uses `{% include %}` partials (`commit`) and one builtin agent
    /// (`reviewer`).
    fn sample_profile() -> Profile {
        Profile {
            mcp_server: Some(ProfileMcpServer::serve("sample")),
            skills: Some(Selector::Single("commit".to_string())),
            agents: Some(Selector::Single("reviewer".to_string())),
            statusline: false,
            preamble: false,
        }
    }

    /// `init_profile` with `root: None` installs skills (symlinked + rendered),
    /// the MCP server, and agents into CWD-relative locations; `deinit_profile`
    /// removes them.
    #[test]
    #[serial]
    fn init_profile_installs_and_deinit_removes_cwd_rooted() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().canonicalize().unwrap();
        let _cwd = CurrentDirGuard::new(&project).unwrap();
        let config_path = write_profile_agents_config(&project);
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let profile = sample_profile();
        let reporter = NullReporter;
        let results = init_profile(&profile, InitScope::Project, None, &reporter);
        assert!(
            results
                .iter()
                .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error),
            "init_profile must not error: {results:?}"
        );

        // Skill: stored centrally and symlinked into the agent's skill dir.
        let skill_store = project.join(".skills/commit/SKILL.md");
        assert!(
            skill_store.exists(),
            "skill should be stored: {skill_store:?}"
        );
        let skill_link = project.join(".fake/skills/commit");
        assert!(
            std::fs::symlink_metadata(&skill_link).is_ok(),
            "skill should be symlinked into agent dir: {skill_link:?}"
        );

        // The deployed SKILL.md was rendered with Liquid + the partial library,
        // so no `{% include %}` references survive.
        let rendered = std::fs::read_to_string(&skill_store).unwrap();
        assert!(
            !rendered.contains("{% include"),
            "partials must be expanded in the deployed SKILL.md"
        );

        // Agent: stored and symlinked into the agent's agent dir.
        assert!(
            project.join(".agents/reviewer/AGENT.md").exists(),
            "agent should be stored"
        );
        assert!(
            std::fs::symlink_metadata(project.join(".fake/agents/reviewer")).is_ok(),
            "agent should be symlinked into agent dir"
        );

        // MCP server registered in the agent's .mcp.json.
        let mcp: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(project.join(".mcp.json")).unwrap())
                .unwrap();
        assert_eq!(mcp["mcpServers"]["sample"]["command"], "sample");

        // Deinit removes everything.
        let results = deinit_profile(&profile, InitScope::Project, None, &reporter);
        assert!(results
            .iter()
            .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error));
        assert!(
            std::fs::symlink_metadata(&skill_link).is_err(),
            "skill symlink should be removed on deinit"
        );
        assert!(
            std::fs::symlink_metadata(project.join(".fake/agents/reviewer")).is_err(),
            "agent symlink should be removed on deinit"
        );
        let mcp: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(project.join(".mcp.json")).unwrap())
                .unwrap();
        assert!(
            mcp["mcpServers"]["sample"].is_null(),
            "MCP server should be unregistered on deinit"
        );
    }

    /// `init_profile` with an explicit `root` targets that root for every
    /// project-scope artifact and never reads the process working directory.
    #[test]
    #[serial]
    fn init_profile_explicit_root_targets_given_root() {
        // The install root is a temp dir distinct from the (arbitrary) CWD.
        let root_dir = tempfile::tempdir().unwrap();
        let root = root_dir.path().canonicalize().unwrap();

        // CWD points somewhere else entirely; nothing must land here.
        let cwd_dir = tempfile::tempdir().unwrap();
        let cwd = cwd_dir.path().canonicalize().unwrap();
        let _cwd = CurrentDirGuard::new(&cwd).unwrap();

        // Detection keys off the absolute root dir, independent of CWD.
        let config_path = write_profile_agents_config(&root);
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let profile = sample_profile();
        let reporter = NullReporter;
        let results = init_profile(&profile, InitScope::Project, Some(&root), &reporter);
        assert!(
            results
                .iter()
                .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error),
            "explicit-root init must not error: {results:?}"
        );

        // Artifacts land under the explicit root.
        assert!(root.join(".skills/commit/SKILL.md").exists());
        assert!(std::fs::symlink_metadata(root.join(".fake/skills/commit")).is_ok());
        assert!(root.join(".agents/reviewer/AGENT.md").exists());
        let mcp: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(root.join(".mcp.json")).unwrap())
                .unwrap();
        assert_eq!(mcp["mcpServers"]["sample"]["command"], "sample");

        // Nothing was written to the CWD.
        assert!(
            !cwd.join(".skills").exists(),
            "explicit-root install must not touch CWD"
        );
        assert!(!cwd.join(".mcp.json").exists());

        // Explicit-root deinit cleans the root.
        deinit_profile(&profile, InitScope::Project, Some(&root), &reporter);
        assert!(std::fs::symlink_metadata(root.join(".fake/skills/commit")).is_err());
    }

    /// A profile that declares `statusline`/`preamble` writes the `statusLine`
    /// block and the CLAUDE.md preamble into the detected agent's files, and
    /// `deinit_profile` removes both. Exercised with an explicit `root` to prove
    /// step 4 is CWD-free.
    #[test]
    #[serial]
    fn init_profile_statusline_and_preamble_install_and_deinit() {
        let root_dir = tempfile::tempdir().unwrap();
        let root = root_dir.path().canonicalize().unwrap();

        // CWD elsewhere: nothing must land in it.
        let cwd_dir = tempfile::tempdir().unwrap();
        let cwd = cwd_dir.path().canonicalize().unwrap();
        let _cwd = CurrentDirGuard::new(&cwd).unwrap();

        let config_path = write_profile_agents_config(&root);
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let profile = Profile {
            statusline: true,
            preamble: true,
            ..Profile::default()
        };
        let reporter = NullReporter;
        let results = init_profile(&profile, InitScope::Project, Some(&root), &reporter);
        assert!(
            results
                .iter()
                .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error),
            "statusline/preamble init must not error: {results:?}"
        );

        // Statusline block written to the agent's settings file under root.
        let settings_path = root.join(".fake/settings.json");
        let settings: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(settings["statusLine"]["type"], "command");
        assert_eq!(settings["statusLine"]["command"], "sah statusline");

        // Preamble prepended to the agent's instructions file under root.
        let claude_md = root.join(".fake/CLAUDE.md");
        let body = std::fs::read_to_string(&claude_md).unwrap();
        assert!(
            status::preamble_present_in(&body),
            "preamble must be present: {body:?}"
        );

        // Nothing leaked into the CWD.
        assert!(!cwd.join(".fake").exists(), "step 4 must not touch CWD");

        // Deinit strips both.
        let results = deinit_profile(&profile, InitScope::Project, Some(&root), &reporter);
        assert!(results
            .iter()
            .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error));
        let settings: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert!(
            settings.get("statusLine").is_none(),
            "statusLine must be removed on deinit"
        );
        // The instructions file held only the preamble, so deinit deletes it.
        assert!(
            !claude_md.exists(),
            "preamble-only instructions file should be deleted on deinit"
        );
    }

    /// The skill `Selector::Profile` variant matches the builtin profile tags.
    #[test]
    fn selector_profile_matches_tagged_skills() {
        let names = resolved_skill_names(&Selector::Profile("kanban".to_string()));
        assert!(
            names.contains(&"kanban".to_string()) && names.contains(&"implement".to_string()),
            "kanban-profile selector should pick the tagged skills, got {names:?}"
        );
        assert!(
            !names.contains(&"commit".to_string()),
            "untagged 'commit' must not be selected by the kanban profile"
        );
    }

    /// `Selector::Named` resolves in source order, skipping unknown names.
    #[test]
    fn selector_named_resolves_known_and_skips_unknown() {
        let available: std::collections::HashMap<String, Vec<String>> =
            [("a", vec![]), ("b", vec![]), ("c", vec![])]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect();
        let got = Selector::Named(vec![
            "b".to_string(),
            "missing".to_string(),
            "a".to_string(),
        ])
        .select(&available);
        assert_eq!(got, vec!["b".to_string(), "a".to_string()]);
    }

    /// `Selector::Single` for an unknown name selects nothing.
    #[test]
    fn selector_single_unknown_is_empty() {
        let available: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        assert!(Selector::Single("nope".to_string())
            .select(&available)
            .is_empty());
    }

    /// `ProfileMcpServer::serve` builds the canonical self-launching triple:
    /// the binary registers under its own name and runs `<name> serve`.
    #[test]
    fn profile_mcp_server_serve_builds_self_launching_triple() {
        let server = ProfileMcpServer::serve("shelltool");
        assert_eq!(server.name, "shelltool");
        assert_eq!(server.command, "shelltool");
        assert_eq!(server.args, vec!["serve".to_string()]);
    }

    /// A test `Initializable` that records the lifecycle method invoked, so the
    /// `*_with_registry` helpers can be checked for both result aggregation and
    /// the profile-vs-registry ordering.
    struct RecordingComponent;

    impl swissarmyhammer_common::lifecycle::Initializable for RecordingComponent {
        fn name(&self) -> &str {
            "recording-component"
        }
        fn category(&self) -> &str {
            "test"
        }
        fn init(&self, _scope: &InitScope, _reporter: &dyn InitReporter) -> Vec<InitResult> {
            vec![InitResult::ok("recording-component", "init ran")]
        }
        fn deinit(&self, _scope: &InitScope, _reporter: &dyn InitReporter) -> Vec<InitResult> {
            vec![InitResult::ok("recording-component", "deinit ran")]
        }
    }

    /// `init_profile_with_registry` returns the profile install results followed
    /// by the registry components' init results (profile-first ordering), and
    /// `deinit_profile_with_registry` runs the registry teardown first.
    #[test]
    #[serial]
    fn with_registry_helpers_aggregate_profile_then_registry() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().canonicalize().unwrap();
        let _cwd = CurrentDirGuard::new(&project).unwrap();
        let config_path = write_profile_agents_config(&project);
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let profile = sample_profile();
        let reporter = NullReporter;

        let mut registry = InitRegistry::new();
        registry.register(RecordingComponent);
        let init_results =
            init_profile_with_registry(&profile, &registry, InitScope::Project, None, &reporter);
        // The registry's init result is present and trails the profile results.
        let recorded = init_results
            .iter()
            .position(|r| r.name == "recording-component" && r.message == "init ran")
            .expect("registry init result present");
        let last_profile = init_results
            .iter()
            .rposition(|r| r.name != "recording-component")
            .expect("profile results present");
        assert!(
            recorded > last_profile,
            "registry init must run after profile install: {init_results:?}"
        );

        let deinit_results =
            deinit_profile_with_registry(&profile, &registry, InitScope::Project, None, &reporter);
        // The registry's deinit result leads the profile teardown.
        let recorded = deinit_results
            .iter()
            .position(|r| r.name == "recording-component" && r.message == "deinit ran")
            .expect("registry deinit result present");
        let first_profile = deinit_results
            .iter()
            .position(|r| r.name != "recording-component")
            .expect("profile teardown results present");
        assert!(
            recorded < first_profile,
            "registry deinit must run before profile teardown: {deinit_results:?}"
        );
    }

    /// A profile with no skills/agents/mcp_server is a clean no-op.
    #[test]
    #[serial]
    fn empty_profile_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().canonicalize().unwrap();
        let _cwd = CurrentDirGuard::new(&project).unwrap();
        let config_path = write_profile_agents_config(&project);
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let reporter = NullReporter;
        let results = init_profile(&Profile::default(), InitScope::Project, None, &reporter);
        assert!(results.is_empty(), "empty profile should do nothing");
        assert!(!project.join(".skills").exists());
    }
}

/// Production-path consistency tests for the four real CLI profiles.
///
/// Card "Real-path tests: every profile init/deinit is consistent and
/// round-trips". Where [`profile_tests`] exercises the installer with a
/// synthetic `sample_profile`, this module reconstructs the *actual* profiles
/// declared by the four consumers — sah ([`apps/swissarmyhammer-cli`]),
/// shelltool, kanban-cli, and code-context — from the same public mirdan
/// primitives (`ProfileMcpServer::serve`, `Selector::*`) those CLIs use, then
/// drives them all through the single [`init_profile`] / [`deinit_profile`]
/// path. The point is the "one mechanism, no drift" guarantee: every profile
/// installs the same way (store + symlink, never copied files), registers its
/// MCP server in the right place, and round-trips clean — and a regression that
/// reintroduced a per-app installer or a copy-vs-symlink fork would fail here.
///
/// Scope note: these reconstructions cover the install *mechanism* across the
/// four profile *shapes*; they deliberately do **not** enumerate each profile's
/// real skill set. That authority — "which skills does this CLI actually
/// deploy" — lives in each app's own `commands::registry` test, which drives the
/// real `profile(scope)` through [`init_profile`] via the shared
/// [`crate::test_support`] asserters and so can never silently mirror a bug in
/// the real profile. mirdan cannot import the app crates, so the mechanism
/// coverage lives here while the skill-set authority lives there.
///
/// Tests are HOME/tempdir-isolated (mirroring the `MIRDAN_AGENTS_CONFIG`
/// isolation in [`profile_tests`]) and `#[serial]` because they mutate the
/// process CWD and shared env; nothing leaks into the repo. They reuse the
/// public [`crate::test_support`] scaffolding (`write_single_agent_config`,
/// `assert_no_init_error`, `read_json`) so the in-crate and app-crate tests
/// share one config writer and one set of asserters.
#[cfg(test)]
mod profile_consistency_tests {
    use super::*;
    use crate::test_support::{
        assert_no_init_error, read_json, write_single_agent_config, MirdanConfigGuard,
    };
    use serial_test::serial;
    use swissarmyhammer_common::reporter::NullReporter;
    use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};

    /// One CLI consumer's real install profile, reconstructed from the same
    /// public mirdan primitives the consumer's `registry.rs`/`profile.rs` uses.
    ///
    /// These reconstructions exist only to exercise the shared install
    /// *mechanism* (store + symlink, MCP registration, round-trip, scope matrix)
    /// across the four profile *shapes* — the coverage that legitimately must
    /// live in mirdan, which cannot import the app crates. They deliberately do
    /// **not** enumerate each profile's real skill set: the authoritative,
    /// drift-proof check of "which skills does this CLI actually deploy" lives in
    /// each app's own `commands::registry` test, which drives the real
    /// `profile(scope)` through [`init_profile`] (see
    /// `apps/*/src/commands/registry.rs`). Enumerating the set here too would
    /// only re-introduce the mirror-the-bug drift this card set out to remove.
    struct CliProfile {
        /// Consumer label, for assertion messages.
        label: &'static str,
        /// The registered MCP server name (`<name> serve`).
        server: &'static str,
        /// A single representative skill the profile is known to select, used to
        /// probe the store+symlink deploy mechanism. The mechanism is identical
        /// regardless of which builtin we probe, so one name suffices — this is
        /// not an assertion about the profile's full skill set (owned by the
        /// per-CLI registry tests).
        probe_skill: &'static str,
        /// Build the profile for `scope`, mirroring the consumer's `profile(scope)`.
        build: fn(InitScope) -> Profile,
    }

    /// sah's profile — the "bigger profile": all builtin skills + all builtin
    /// agents + statusline + preamble (`apps/swissarmyhammer-cli/.../profile.rs`).
    fn sah_profile(_scope: InitScope) -> Profile {
        Profile {
            mcp_server: Some(ProfileMcpServer::serve("sah")),
            skills: Some(Selector::All),
            agents: Some(Selector::All),
            statusline: true,
            preamble: true,
        }
    }

    /// shelltool's profile — `shelltool serve` + the single `shell` skill,
    /// deployed at every scope (`apps/shelltool-cli/.../registry.rs`).
    fn shelltool_profile(_scope: InitScope) -> Profile {
        Profile {
            mcp_server: Some(ProfileMcpServer::serve("shelltool")),
            skills: Some(Selector::Single("shell".to_string())),
            ..Default::default()
        }
    }

    /// kanban-cli's profile — `kanban serve` + the `kanban`-profile skill
    /// cluster, deployed at every scope (`apps/kanban-cli/.../registry.rs`).
    fn kanban_profile(_scope: InitScope) -> Profile {
        Profile {
            mcp_server: Some(ProfileMcpServer::serve("kanban")),
            skills: Some(Selector::Profile("kanban".to_string())),
            ..Default::default()
        }
    }

    /// code-context's profile — `code-context serve` + the named
    /// `code-context` + `explore` + `lsp` + `detected-projects` skills, deployed
    /// at every scope (`apps/code-context-cli/.../registry.rs`).
    fn code_context_profile(_scope: InitScope) -> Profile {
        Profile {
            mcp_server: Some(ProfileMcpServer::serve("code-context")),
            skills: Some(Selector::Named(vec![
                "code-context".to_string(),
                "explore".to_string(),
                "lsp".to_string(),
                "detected-projects".to_string(),
            ])),
            ..Default::default()
        }
    }

    /// The four real CLI profiles, in the order their cards migrated them. Each
    /// carries a single `probe_skill` to exercise the deploy mechanism — not the
    /// full skill set, which the per-CLI registry tests own (see [`CliProfile`]).
    fn cli_profiles() -> [CliProfile; 4] {
        [
            CliProfile {
                label: "sah",
                server: "sah",
                probe_skill: "commit",
                build: sah_profile,
            },
            CliProfile {
                label: "shelltool",
                server: "shelltool",
                probe_skill: "shell",
                build: shelltool_profile,
            },
            CliProfile {
                label: "kanban",
                server: "kanban",
                probe_skill: "kanban",
                build: kanban_profile,
            },
            CliProfile {
                label: "code-context",
                server: "code-context",
                probe_skill: "code-context",
                build: code_context_profile,
            },
        ]
    }

    /// Assert no result is an error, with a profile-labelled message. Wraps the
    /// shared [`assert_no_init_error`] with a combined `<label> <phase>` label.
    fn assert_no_error(label: &str, phase: &str, results: &[InitResult]) {
        assert_no_init_error(&format!("{label} {phase}"), results);
    }

    /// Case 1 + 2 + 5: every real CLI profile installs through the *identical*
    /// mechanism — each selected skill is a central store entry **symlinked**
    /// (never copied) into the agent's skill dir, the MCP server lands in the
    /// agent's `.mcp.json`, and `deinit_profile` round-trips every artifact away.
    /// Driven with an explicit root so the four runs are independent tempdirs.
    ///
    /// Round-trip here is asserted at `Project` scope only; `Local`-scope
    /// round-trip is exercised by the code-context regression test
    /// ([`code_context_local_scope_registers_in_claude_json_projects_map`]), and
    /// per-scope landing is covered by [`scope_matrix_lands_artifacts_in_the_right_place`].
    #[test]
    #[serial]
    fn every_cli_profile_installs_by_store_symlink_and_round_trips() {
        // CWD points somewhere neutral; explicit roots isolate each profile.
        let cwd_dir = tempfile::tempdir().unwrap();
        let _cwd = CurrentDirGuard::new(cwd_dir.path().canonicalize().unwrap()).unwrap();
        let reporter = NullReporter;

        for cli in cli_profiles() {
            let root_dir = tempfile::tempdir().unwrap();
            let root = root_dir.path().canonicalize().unwrap();
            // Root and home coincide so the generic agent's project and global
            // dirs both live under this tempdir — `Project` scope only touches
            // the project dirs anyway.
            let config_path = write_single_agent_config(&root, &root);
            let _mirdan = MirdanConfigGuard::set(&config_path);

            let profile = (cli.build)(InitScope::Project);
            let results = init_profile(&profile, InitScope::Project, Some(&root), &reporter);
            assert_no_error(cli.label, "init", &results);

            // Probe skill: central store entry + a *symlink* (not a copy) in the
            // agent dir — the single deploy mechanism, identical for all four.
            // (The full per-profile skill set is asserted by the per-CLI registry
            // tests; here we only prove the mechanism.)
            let skill = cli.probe_skill;
            let store = root.join(".skills").join(skill).join("SKILL.md");
            assert!(
                store.is_file(),
                "{}: skill `{skill}` must be in the .skills store: {store:?}",
                cli.label
            );
            let link = root.join(".fake/skills").join(skill);
            let meta = std::fs::symlink_metadata(&link).unwrap_or_else(|e| {
                panic!(
                    "{}: skill `{skill}` link must exist ({link:?}): {e}",
                    cli.label
                )
            });
            assert!(
                meta.file_type().is_symlink(),
                "{}: skill `{skill}` must be a SYMLINK, not a copied dir — store+symlink is the one mechanism",
                cli.label
            );

            // MCP server registered in the agent's project `.mcp.json`, launched
            // via `<name> serve`.
            let mcp = read_json(&root.join(".mcp.json"));
            assert_eq!(
                mcp["mcpServers"][cli.server]["command"], cli.server,
                "{}: MCP server `{}` must be registered",
                cli.label, cli.server
            );
            assert_eq!(mcp["mcpServers"][cli.server]["args"][0], "serve");

            // Round-trip: deinit removes the symlink and unregisters the MCP
            // server, leaving the agent config clean.
            let results = deinit_profile(&profile, InitScope::Project, Some(&root), &reporter);
            assert_no_error(cli.label, "deinit", &results);
            let link = root.join(".fake/skills").join(skill);
            assert!(
                std::fs::symlink_metadata(&link).is_err(),
                "{}: skill `{skill}` symlink must be removed on deinit",
                cli.label
            );
            let mcp = read_json(&root.join(".mcp.json"));
            assert!(
                mcp["mcpServers"][cli.server].is_null(),
                "{}: MCP server must be unregistered on deinit",
                cli.label
            );
        }
    }

    /// Case 3: an explicit-root install targets exactly that root and never
    /// reads or writes the process working directory — for every CLI profile.
    /// This is the property the kanban-app's long-running process relies on.
    #[test]
    #[serial]
    fn explicit_root_install_never_touches_cwd_for_any_profile() {
        let cwd_dir = tempfile::tempdir().unwrap();
        let cwd = cwd_dir.path().canonicalize().unwrap();
        let _cwd = CurrentDirGuard::new(&cwd).unwrap();
        let reporter = NullReporter;

        for cli in cli_profiles() {
            let root_dir = tempfile::tempdir().unwrap();
            let root = root_dir.path().canonicalize().unwrap();
            let config_path = write_single_agent_config(&root, &root);
            let _mirdan = MirdanConfigGuard::set(&config_path);

            let profile = (cli.build)(InitScope::Project);
            let results = init_profile(&profile, InitScope::Project, Some(&root), &reporter);
            assert_no_error(cli.label, "explicit-root init", &results);

            // Artifacts land under the explicit root.
            assert!(
                root.join(".skills").is_dir(),
                "{}: .skills store must be under the explicit root",
                cli.label
            );
            // Nothing was written into the CWD.
            assert!(
                !cwd.join(".skills").exists() && !cwd.join(".mcp.json").exists(),
                "{}: explicit-root install must not touch CWD",
                cli.label
            );
        }
    }

    /// Case 4: the scope matrix. For a representative profile (shelltool), each
    /// scope lands in the correct location: `Project`/`Local` deploy skills under
    /// the project store; `User` deploys skills into the *global* store and
    /// registers the MCP server in the agent's global config. The MCP target file
    /// differs by scope (project `.mcp.json` vs the agent's global config).
    #[test]
    #[serial]
    fn scope_matrix_lands_artifacts_in_the_right_place() {
        let reporter = NullReporter;

        // Project scope: skills + project `.mcp.json`.
        {
            let root_dir = tempfile::tempdir().unwrap();
            let root = root_dir.path().canonicalize().unwrap();
            let _cwd = CurrentDirGuard::new(&root).unwrap();
            let config_path = write_single_agent_config(&root, &root);
            let _mirdan = MirdanConfigGuard::set(&config_path);

            let results = init_profile(
                &shelltool_profile(InitScope::Project),
                InitScope::Project,
                None,
                &reporter,
            );
            assert_no_error("shelltool", "project init", &results);
            assert!(
                root.join(".skills/shell/SKILL.md").is_file(),
                "project scope must deploy the shell skill"
            );
            assert!(
                read_json(&root.join(".mcp.json"))["mcpServers"]["shelltool"]["command"]
                    == "shelltool",
                "project scope must register MCP in project .mcp.json"
            );
        }

        // Local scope: skills still deploy; MCP target is scope-specific (the
        // generic agent still uses its project `.mcp.json` here — the Claude
        // local-scope special case is covered separately).
        {
            let root_dir = tempfile::tempdir().unwrap();
            let root = root_dir.path().canonicalize().unwrap();
            let _cwd = CurrentDirGuard::new(&root).unwrap();
            let config_path = write_single_agent_config(&root, &root);
            let _mirdan = MirdanConfigGuard::set(&config_path);

            let results = init_profile(
                &shelltool_profile(InitScope::Local),
                InitScope::Local,
                None,
                &reporter,
            );
            assert_no_error("shelltool", "local init", &results);
            assert!(
                root.join(".skills/shell/SKILL.md").is_file(),
                "local scope must deploy the shell skill"
            );
        }

        // User scope: skills deploy into the *global* store (`~/.skills` + the
        // agent's global skill dir), and the MCP server registers in the agent's
        // *global* config file, not a project `.mcp.json`.
        {
            let env = IsolatedTestEnvironment::new().unwrap();
            let work = env.temp_dir().canonicalize().unwrap();
            let _cwd = CurrentDirGuard::new(&work).unwrap();
            let config_path = write_single_agent_config(&work, &work);
            let _mirdan = MirdanConfigGuard::set(&config_path);

            let results = init_profile(
                &shelltool_profile(InitScope::User),
                InitScope::User,
                None,
                &reporter,
            );
            assert_no_error("shelltool", "user init", &results);
            // The global store is `~/.skills` (HOME-rooted, via `dirs`); the
            // agent's global skill dir is its config's `global_path`
            // (`<work>/.fake/skills`, since `write_single_agent_config` roots
            // globals at its `home` argument, here `work`), which holds the symlink.
            assert!(
                env.home_path().join(".skills/shell/SKILL.md").is_file(),
                "user scope must deploy the shell skill into the global ~/.skills store"
            );
            let link = work.join(".fake/skills/shell");
            assert!(
                std::fs::symlink_metadata(&link).is_ok_and(|m| m.file_type().is_symlink()),
                "user scope must symlink the shell skill into the agent's global skill dir"
            );
            assert!(
                !work.join(".skills").exists(),
                "user scope must NOT write a project .skills store"
            );
            // The agent's global mcp config (`<work>/.fake/mcp.json`) holds the
            // registration; the project `.mcp.json` must be untouched.
            let global_mcp = work.join(".fake/mcp.json");
            assert!(
                global_mcp.is_file()
                    && read_json(&global_mcp)["mcpServers"]["shelltool"]["command"] == "shelltool",
                "user scope must register MCP in the agent's global config"
            );
            assert!(
                !work.join(".mcp.json").exists(),
                "user scope must not write a project .mcp.json"
            );
        }
    }

    /// Case 6: the code-context local-scope MCP regression. Routing MCP
    /// registration through the profile's strategy-aware applier means a real
    /// `claude-code` agent at `Local` scope registers in `~/.claude.json` under
    /// `projects.<root>.mcpServers` — the location the old hand-rolled
    /// code-context loop silently dropped. HOME is isolated so `~/.claude.json`
    /// is the tempdir's.
    #[test]
    #[serial]
    fn code_context_local_scope_registers_in_claude_json_projects_map() {
        let env = IsolatedTestEnvironment::new().unwrap();
        // CWD is the project root; `project_key()` falls back to it (the tempdir
        // is not inside a git repo), giving a deterministic projects-map key.
        let work = env.temp_dir().canonicalize().unwrap();
        let _cwd = CurrentDirGuard::new(&work).unwrap();

        // A real `claude-code` agent so `strategy_for` selects ClaudeCodeStrategy
        // (its Local scope writes `~/.claude.json`, not a project `.mcp.json`).
        let agents_yaml = format!(
            r#"agents:
  - id: claude-code
    name: Claude Code
    project_path: .claude/skills
    global_path: "{home}/.claude/skills"
    settings_path: .claude/settings.json
    global_settings_path: "{home}/.claude/settings.json"
    detect:
      - dir: "{detect}"
    mcp_config:
      project_path: .mcp.json
      global_path: "{home}/.claude.json"
      servers_key: mcpServers
"#,
            detect = work.display(),
            home = env.home_path().display(),
        );
        let config_path = work.join("agents.yaml");
        std::fs::write(&config_path, &agents_yaml).unwrap();
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let reporter = NullReporter;
        // root: None so registration flows through the strategy-aware applier
        // (the explicit-root path bypasses the Claude local special case).
        let profile = code_context_profile(InitScope::Local);
        let results = init_profile(&profile, InitScope::Local, None, &reporter);
        assert_no_error("code-context", "local init", &results);

        // The MCP server lands in `~/.claude.json` under the project entry —
        // NOT in a project `.mcp.json`. This is the regression the migration fixed.
        let claude_json = env.home_path().join(".claude.json");
        assert!(
            claude_json.is_file(),
            "Claude local scope must write ~/.claude.json"
        );
        let json = read_json(&claude_json);
        let key = work.to_string_lossy().to_string();
        assert_eq!(
            json["projects"][&key]["mcpServers"]["code-context"]["command"], "code-context",
            "code-context MCP must register in ~/.claude.json projects.<root>.mcpServers (local scope), got: {json}"
        );
        assert!(
            !work.join(".mcp.json").exists(),
            "Claude local scope must NOT write a project .mcp.json"
        );

        // Round-trip: deinit prunes the local-scope registration.
        let results = deinit_profile(&profile, InitScope::Local, None, &reporter);
        assert_no_error("code-context", "local deinit", &results);
        let json = read_json(&claude_json);
        assert!(
            json["projects"][&key]["mcpServers"]
                .get("code-context")
                .is_none(),
            "deinit must remove the local-scope MCP registration"
        );
    }
}
