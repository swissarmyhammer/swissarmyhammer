//! Package installation from the registry, a local path, a git source,
//! or an explicit MCP server command.

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use indicatif::{ProgressBar, ProgressStyle};

use crate::agents;
use crate::frontmatter;
use crate::git_source::{self, InstallSource};
use crate::lockfile::{self, LockedPackage, Lockfile};
use crate::mcp_config::{self, ServersKey, ToolName};
use crate::package_type::{self, PackageType};
use crate::registry::{RegistryClient, RegistryError};

use super::deploy::{
    deploy_agent_to_agents, deploy_plugin, deploy_skill_to_agents, deploy_tool, deploy_validator,
};

/// The version recorded for a package whose manifest names none.
const DEFAULT_VERSION: &str = "0.0.0";

/// How [`run_install`] resolves a package spec to its source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMode {
    /// Detect the source: a local path, then the registry, with git fallback.
    Auto,
    /// Treat the spec as a git source (the `--git` flag).
    ForceGit,
}

/// Run the install command.
///
/// Accepts multiple forms:
/// - `name` or `name@version` — download from registry
/// - `./local-path` — install from a local directory
/// - `owner/repo` or git URL — clone from git (with [`InstallMode::ForceGit`] or as fallback)
///
/// Auto-detects type from contents:
/// - SKILL.md -> deploy to each detected agent's skill directory
/// - VALIDATOR.md + rules/ -> deploy to ./.validators/
///
/// # Errors
///
/// Returns an error when the path does not resolve, the package type cannot
/// be detected, the registry download fails, the git clone fails, deployment
/// fails, or the lockfile cannot be written.
pub async fn run_install(
    package_spec: &str,
    agent_filter: Option<&str>,
    global: bool,
    mode: InstallMode,
    skill_select: Option<&str>,
) -> Result<Vec<crate::DeployResult>, RegistryError> {
    match git_source::classify_source(package_spec, mode == InstallMode::ForceGit) {
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
                                "package '{}' not found in registry",
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
        RegistryError::Validation(format!("cannot resolve path '{}': {}", local_path, e))
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
            "cannot determine package type in '{}'. Expected SKILL.md, VALIDATOR.md + rules/, TOOL.md, or .claude-plugin/plugin.json",
            local_path
        ))
    })?;

    // Read name and version from frontmatter (or plugin.json for plugins)
    let FrontmatterMetadata { name, version } = match pkg_type {
        PackageType::Plugin => FrontmatterMetadata {
            name: mcp_config::read_plugin_json(&dir.join(PackageType::Plugin.manifest_file()))?,
            version: DEFAULT_VERSION.to_string(),
        },
        manifest_type => read_frontmatter(&dir.join(manifest_type.manifest_file()))?,
    };

    tracing::debug!("Installing {} from local path ({})...", name, pkg_type);

    let targets = deploy_by_type(pkg_type, &name, &dir, agent_filter, global)?;

    record_locked_package(
        &name,
        locked_package(
            pkg_type,
            &version,
            format!("file:{}", dir.display()),
            String::new(),
            &targets,
        ),
    )?;

    log_installed(&name, &version, pkg_type, " from local path", &targets);

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

    for pkg in &packages {
        tracing::debug!("\nInstalling {} ({})...", pkg.name, pkg.package_type);

        let targets = deploy_by_type(pkg.package_type, &pkg.name, &pkg.path, agent_filter, global)?;

        // Read version from frontmatter (or plugin.json for plugins)
        let version = match pkg.package_type {
            PackageType::Plugin => DEFAULT_VERSION.to_string(),
            manifest_type => read_frontmatter(&pkg.path.join(manifest_type.manifest_file()))
                .map(|metadata| metadata.version)
                .unwrap_or_else(|_| DEFAULT_VERSION.to_string()),
        };

        record_locked_package(
            &pkg.name,
            locked_package(
                pkg.package_type,
                &version,
                format!("git+{}", source.clone_url),
                String::new(),
                &targets,
            ),
        )?;

        log_installed(&pkg.name, &version, pkg.package_type, " from git", &targets);
    }

    // temp_dir drops here, cleaning up the clone
    Ok(())
}

/// Deploy one package of `pkg_type` from `source_dir` to its targets.
///
/// The single package-type dispatch behind [`run_install_local`],
/// [`run_install_git`], and [`install_from_archive`].
fn deploy_by_type(
    pkg_type: PackageType,
    name: &str,
    source_dir: &Path,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<Vec<String>, RegistryError> {
    match pkg_type {
        PackageType::Skill => deploy_skill_to_agents(name, source_dir, agent_filter, global),
        PackageType::Validator => deploy_validator(name, source_dir, global),
        PackageType::Tool => deploy_tool(name, source_dir, agent_filter, global),
        PackageType::Plugin => deploy_plugin(name, source_dir, agent_filter, global),
        PackageType::Agent => deploy_agent_to_agents(name, source_dir, agent_filter, global),
    }
}

/// Record one installed package in the CWD lockfile and save it.
///
/// The single lockfile-recording step behind every install path.
fn record_locked_package(name: &str, package: LockedPackage) -> Result<(), RegistryError> {
    let project_root = std::env::current_dir()?;
    let mut lf = Lockfile::load(&project_root)?;
    lf.add_package(name.to_string(), package);
    lf.save(&project_root)?;
    tracing::debug!("  Updated mirdan-lock.json");
    Ok(())
}

/// Build the lockfile entry for a freshly installed package.
///
/// The single [`LockedPackage`] construction behind every install path; only
/// the source-specific `resolved` and `integrity` values vary.
fn locked_package(
    pkg_type: PackageType,
    version: &str,
    resolved: String,
    integrity: String,
    targets: &[String],
) -> LockedPackage {
    LockedPackage {
        package_type: pkg_type,
        version: version.to_string(),
        resolved,
        integrity,
        installed_at: chrono::Utc::now().to_rfc3339(),
        targets: targets.to_vec(),
    }
}

/// Log one installed package and its deploy targets at debug level.
fn log_installed(
    name: &str,
    version: &str,
    pkg_type: PackageType,
    source: &str,
    targets: &[String],
) {
    tracing::debug!("\nInstalled {}@{} ({}){}", name, version, pkg_type, source);
    for target in targets {
        tracing::debug!("  -> {}", target);
    }
}

/// What a package manifest states about itself in its frontmatter.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct FrontmatterMetadata {
    /// The package name, from the `name` key.
    pub(crate) name: String,
    /// The package version, from `metadata.version` or from a top-level
    /// `version`. A manifest that names neither gets [`DEFAULT_VERSION`].
    pub(crate) version: String,
}

/// Read name and version from YAML frontmatter of a markdown file.
///
/// The [`frontmatter`] module makes the split and the parse. A manifest that
/// names no version gets [`DEFAULT_VERSION`].
///
/// # Errors
///
/// Returns an error when the file cannot be read, the frontmatter is missing
/// or unterminated, the YAML does not parse, or the `name` key is absent.
pub(crate) fn read_frontmatter(path: &Path) -> Result<FrontmatterMetadata, RegistryError> {
    let yaml = frontmatter::read_file(path)?;

    let name = frontmatter::field(&yaml, "name")
        .ok_or_else(|| RegistryError::Validation("missing 'name' in frontmatter".to_string()))?
        .to_string();

    let version = frontmatter::metadata_field(&yaml, "version")
        .unwrap_or(DEFAULT_VERSION)
        .to_string();

    Ok(FrontmatterMetadata { name, version })
}

/// Install a package from the registry.
async fn run_install_registry(
    package_spec: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<(), RegistryError> {
    let PackageSpec { name, version } = parse_package_spec(package_spec);

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

/// Character width of the download progress bar.
const PROGRESS_BAR_WIDTH: usize = 40;

/// Download and verify a package artifact from the registry.
async fn download_package(
    client: &RegistryClient,
    version_detail: &crate::registry::types::VersionDetail,
) -> Result<bytes::Bytes, RegistryError> {
    let pb = if let Some(size) = version_detail.size.filter(|&s| s > 0) {
        let pb = ProgressBar::new(size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(&format!(
                    "{{msg}} [{{bar:{PROGRESS_BAR_WIDTH}}}] {{bytes}}/{{total_bytes}}"
                ))
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
                "cannot determine package type. Expected SKILL.md, VALIDATOR.md + rules/, TOOL.md, or .claude-plugin/plugin.json".to_string(),
            )
        })?;

    let targets = deploy_by_type(pkg_type, name, temp_dir.path(), agent_filter, global)?;

    record_install(name, version_detail, pkg_type, &targets)?;
    Ok(())
}

/// Metadata-only install for tool packages when no downloadable artifact exists.
///
/// Uses MCP config from the API response (mcp field or tool_md content) to register
/// the MCP server directly without needing a ZIP download.
///
/// # Errors
///
/// Returns an error when the package is not a tool, the registry lookup
/// fails, or no MCP configuration exists in the registry entry.
pub(crate) async fn install_tool_from_metadata(
    name: &str,
    version_detail: &crate::registry::types::VersionDetail,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<(), RegistryError> {
    // Verify this is actually a tool. The registry may return the type in any
    // casing, so compare case-insensitively.
    let is_tool = version_detail
        .package_type
        .as_deref()
        .map(|t| t.eq_ignore_ascii_case("tool"))
        .unwrap_or(false);

    if !is_tool {
        return Err(RegistryError::Validation(format!(
            "package '{}' has no downloadable artifact and is not a tool",
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
        "tool '{}' has no downloadable artifact and no MCP configuration in the registry. \
         The registry entry may be incomplete",
        name
    )))
}

/// Install a tool using an explicit MCP config from the registry.
///
/// # Errors
///
/// Returns an error when the agents config cannot be loaded, an agent MCP
/// config cannot be written, or the lockfile cannot be updated.
pub(crate) async fn install_tool_from_mcp_config(
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

    let targets = register_mcp_server_across_agents(&agents, name, &entry, global)?;

    record_install(name, version_detail, PackageType::Tool, &targets)?;
    Ok(())
}

/// Register `entry` as MCP server `name` in each agent's MCP config file,
/// returning the ids of the agents whose config was written.
///
/// The single registration loop behind [`install_tool_from_mcp_config`] and
/// [`run_install_mcp`]. Agents without MCP support, or without a config path
/// for the scope, are skipped.
fn register_mcp_server_across_agents(
    agents: &[crate::agents::DetectedAgent],
    name: &str,
    entry: &mcp_config::McpServerEntry,
    global: bool,
) -> Result<Vec<String>, RegistryError> {
    let mut targets = Vec::new();
    for agent in agents {
        let Some(ref mcp_cfg) = agent.def.mcp_config else {
            tracing::debug!("  Skipped {} (no MCP support)", agent.def.name);
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
        mcp_config::register_mcp_server(
            &config_path,
            &ServersKey::new(&mcp_cfg.servers_key),
            &ToolName::new(name),
            entry,
            &mcp_cfg.entry_extras,
        )?;
        tracing::debug!(
            "  Registered in {} ({})",
            config_path.display(),
            agent.def.name
        );
        targets.push(agent.def.id.clone());
    }
    Ok(targets)
}

/// Install a tool by parsing TOOL.md content from the registry.
///
/// # Errors
///
/// Returns an error when staging the TOOL.md fails, tool deployment fails,
/// or the lockfile cannot be updated.
pub(crate) async fn install_tool_from_tool_md_content(
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

/// Record a successful registry install in the lockfile.
fn record_install(
    name: &str,
    version_detail: &crate::registry::types::VersionDetail,
    pkg_type: PackageType,
    targets: &[String],
) -> Result<(), RegistryError> {
    record_locked_package(
        name,
        LockedPackage {
            package_type: pkg_type,
            version: version_detail.version.clone(),
            resolved: version_detail.download_url.clone(),
            integrity: version_detail.integrity.clone().unwrap_or_default(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            targets: targets.to_vec(),
        },
    )?;

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

/// Install an MCP server to all detected (or filtered) agents.
///
/// Registers the MCP server entry in each agent's MCP config file and records
/// it in the lockfile as a Tool package.
///
/// # Errors
///
/// Returns an error when the agents config cannot be loaded, an agent MCP
/// config cannot be written, or the lockfile cannot be updated.
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

    let installed = register_mcp_server_across_agents(&target_agents, name, &entry, global)?;

    if installed.is_empty() {
        tracing::debug!("No agents with MCP support found.");
        return Ok(vec![crate::DeployResult::message(
            crate::DeployAction::Skipped,
            "No agents with MCP support found.",
        )]);
    }

    record_locked_package(
        name,
        LockedPackage {
            package_type: PackageType::Tool,
            version: DEFAULT_VERSION.to_string(),
            resolved: format!("mcp:{}", entry.command),
            integrity: String::new(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            targets: installed.clone(),
        },
    )?;

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

/// Install a specific package version (used by update command).
///
/// # Errors
///
/// Returns any error [`run_install`] returns for the versioned spec.
pub async fn install_package(
    name: &str,
    version: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<Vec<crate::DeployResult>, RegistryError> {
    let spec = format!("{}@{}", name, version);
    run_install(&spec, agent_filter, global, InstallMode::Auto, None).await
}

/// A package spec split into the package it names and the version it pins.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageSpec {
    /// The package name.
    pub name: String,
    /// The version the spec pins, or `None` when it pins none.
    pub version: Option<String>,
}

/// Parse a package spec like "name" or "name@version".
pub fn parse_package_spec(spec: &str) -> PackageSpec {
    match spec.rsplit_once('@') {
        Some((name, version)) => PackageSpec {
            name: name.to_string(),
            version: Some(version.to_string()),
        },
        None => PackageSpec {
            name: spec.to_string(),
            version: None,
        },
    }
}

/// Extract a ZIP archive to a target directory with path traversal protection.
fn extract_zip(data: &[u8], target_dir: &Path) -> Result<(), RegistryError> {
    let cursor = Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| RegistryError::Validation(format!("invalid ZIP archive: {}", e)))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| RegistryError::Validation(format!("ZIP read error: {}", e)))?;

        let name = file.name().to_string();
        let Some(relative_path) = safe_zip_entry_path(&name)? else {
            continue;
        };
        let target_path = target_dir.join(&relative_path);

        if file.is_dir() {
            std::fs::create_dir_all(&target_path)?;
        } else {
            write_zip_entry(&mut file, &target_path)?;
        }
    }

    Ok(())
}

/// Validate a ZIP entry name and map it to a relative extraction path.
///
/// Rejects path traversal (`..`, absolute paths). Strips the top-level
/// directory wrapper archives commonly carry. Returns `Ok(None)` for the
/// bare wrapper entry itself, which has nothing to extract.
fn safe_zip_entry_path(name: &str) -> Result<Option<PathBuf>, RegistryError> {
    if name.contains("..") || name.starts_with('/') || name.starts_with('\\') {
        return Err(RegistryError::Validation(format!(
            "unsafe path in ZIP: {}",
            name
        )));
    }
    let relative_path = if let Some((_prefix, rest)) = name.split_once('/') {
        if rest.is_empty() {
            return Ok(None);
        }
        PathBuf::from(rest)
    } else {
        PathBuf::from(name)
    };
    Ok(Some(relative_path))
}

/// Write one ZIP file entry to `target_path`, creating parent directories.
fn write_zip_entry(
    file: &mut zip::read::ZipFile<'_>,
    target_path: &Path,
) -> Result<(), RegistryError> {
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut outfile = std::fs::File::create(target_path)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).map_err(RegistryError::Io)?;
    std::io::Write::write_all(&mut outfile, &buf)?;
    Ok(())
}
