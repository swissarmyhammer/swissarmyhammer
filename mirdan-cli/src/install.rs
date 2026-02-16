//! Mirdan Install/Uninstall - Type-aware package deployment.
//!
//! Skills -> agent skill directories (one copy per detected agent)
//! Validators -> .avp/validators/ (project) or ~/.avp/validators/ (global)

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use indicatif::{ProgressBar, ProgressStyle};

use crate::agents::{self, agent_global_skill_dir, agent_project_skill_dir};
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

/// Check if a package spec refers to a local path.
fn is_local_path(spec: &str) -> bool {
    spec.starts_with("./") || spec.starts_with("../") || spec.starts_with('/') || Path::new(spec).is_dir()
}

/// Run the install command.
///
/// Accepts three forms:
/// - `name` or `name@version` — download from registry
/// - `./local-path` — install from a local directory
///
/// Auto-detects type from contents:
/// - SKILL.md -> deploy to each detected agent's skill directory
/// - VALIDATOR.md + rules/ -> deploy to .avp/validators/
pub async fn run_install(
    package_spec: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<(), RegistryError> {
    if is_local_path(package_spec) {
        return run_install_local(package_spec, agent_filter, global).await;
    }

    run_install_registry(package_spec, agent_filter, global).await
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

/// Run the uninstall command.
pub async fn run_uninstall(
    name: &str,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<(), RegistryError> {
    let project_root = std::env::current_dir()?;
    let lf = Lockfile::load(&project_root)?;

    // Check lockfile for type info
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
    run_install(&spec, agent_filter, global).await
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
    fn test_is_local_path_relative() {
        assert!(is_local_path("./my-skill"));
        assert!(is_local_path("../other/skill"));
    }

    #[test]
    fn test_is_local_path_absolute() {
        assert!(is_local_path("/tmp/skill"));
    }

    #[test]
    fn test_is_local_path_registry() {
        assert!(!is_local_path("no-secrets"));
        assert!(!is_local_path("my-skill@1.0.0"));
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
}
