//! Mirdan Install/Uninstall - Type-aware package deployment.
//!
//! Skills -> agent skill directories (one copy per detected agent)
//! Validators -> .avp/validators/ (project) or ~/.avp/validators/ (global)

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use indicatif::{ProgressBar, ProgressStyle};

use crate::agents::{self, agent_global_skill_dir, agent_project_skill_dir};
use crate::git_source::{self, InstallSource};
use crate::lockfile::{self, LockedPackage, Lockfile};
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
        InstallSource::LocalPath(path) => {
            run_install_local(&path, agent_filter, global).await
        }
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
            "Cannot determine package type in '{}'. Expected SKILL.md or VALIDATOR.md + rules/",
            local_path
        ))
    })?;

    // Read name and version from frontmatter
    let (name, version) = match pkg_type {
        PackageType::Skill => read_frontmatter(&dir.join("SKILL.md"))?,
        PackageType::Validator => read_frontmatter(&dir.join("VALIDATOR.md"))?,
    };

    println!(
        "Installing {} from local path ({})...",
        name, pkg_type
    );

    let targets = match pkg_type {
        PackageType::Skill => deploy_skill(&name, &dir, agent_filter, global).await?,
        PackageType::Validator => deploy_validator(&name, &dir, global)?,
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

    println!("\nInstalled {}@{} ({}) from local path", name, version, pkg_type);
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

    let packages = git_source::discover_packages(
        temp_dir.path(),
        source.subpath.as_deref(),
        select,
    )?;

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
            PackageType::Skill => {
                deploy_skill(&pkg.name, &pkg.path, agent_filter, global).await?
            }
            PackageType::Validator => deploy_validator(&pkg.name, &pkg.path, global)?,
        };

        // Read version from frontmatter
        let md_file = match pkg.package_type {
            PackageType::Skill => pkg.path.join("SKILL.md"),
            PackageType::Validator => pkg.path.join("VALIDATOR.md"),
        };
        let version = read_frontmatter(&md_file)
            .map(|(_, v)| v)
            .unwrap_or_else(|_| "0.0.0".to_string());

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

        println!("Installed {}@{} ({}) from git", pkg.name, version, pkg.package_type);
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
        .get("version")
        .and_then(|v| v.as_str())
        .or_else(|| {
            yaml.get("metadata")
                .and_then(|m| m.get("version"))
                .and_then(|v| v.as_str())
        })
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
            "Cannot determine package type. Expected SKILL.md or VALIDATOR.md + rules/".to_string(),
        )
    })?;

    let targets = match pkg_type {
        PackageType::Skill => {
            deploy_skill(&name, temp_dir.path(), agent_filter, global).await?
        }
        PackageType::Validator => deploy_validator(&name, temp_dir.path(), global)?,
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
        println!("  Linked {} -> {} ({})", link_path.display(), store_path.display(), agent.def.name);
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
                }
                lf.remove_package(pkg_name);
                println!("  Uninstalled {}", pkg_name);
            }
            lf.save(&project_root)?;
            println!("  Updated mirdan-lock.json");
            println!(
                "\nUninstalled {} package(s) from {}",
                matching.len(),
                name
            );
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
            println!("  Removed from {} ({})", link_path.display(), agent.def.name);
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
    if validators_dir(global).join(sanitize_dir_name(name)).exists() {
        return PackageType::Validator;
    }
    // Default to skill
    PackageType::Skill
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
        assert_eq!(
            sanitize_dir_name("http://example.com/foo/bar"),
            "foo/bar"
        );
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
            "---\nname: test-skill\nversion: \"1.2.3\"\n---\n# Test\n",
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
    fn test_read_frontmatter_top_level_preferred() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("SKILL.md");
        std::fs::write(
            &md,
            "---\nname: test-skill\nversion: \"1.0.0\"\nmetadata:\n  version: \"2.0.0\"\n---\n# Test\n",
        )
        .unwrap();

        let (name, version) = read_frontmatter(&md).unwrap();
        assert_eq!(name, "test-skill");
        assert_eq!(version, "1.0.0");
    }

    #[test]
    fn test_read_frontmatter_missing_name_errors() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("SKILL.md");
        std::fs::write(&md, "---\nversion: \"1.0.0\"\n---\n# Test\n").unwrap();

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
                "---\nname: {}\nversion: \"{}\"\n---\n# {}\nA test skill.\n",
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
                "---\nname: {}\nversion: \"{}\"\n---\n# {}\nA test validator.\n",
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
        let packages =
            git_source::discover_packages(clone_dir.path(), None, None).unwrap();
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
        let deployed = work.path().join(".avp/validators").join(sanitize_dir_name(&pkg.name));
        assert!(deployed.exists());

        // find_packages_by_git_source matches via URL
        let lf = Lockfile::load(work.path()).unwrap();
        let matched =
            find_packages_by_git_source(&lf, "https://github.com/anthropics/skills");
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
