//! AVP Package - Install and uninstall validator packages.

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use indicatif::{ProgressBar, ProgressStyle};

use avp_common::lockfile::{self, LockedPackage, Lockfile};

use crate::registry::{RegistryClient, RegistryError};

/// Run the install command.
///
/// Downloads and installs a validator package from the registry.
pub async fn run_install(package_spec: &str, global: bool) -> Result<(), RegistryError> {
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

    // Extract ZIP
    let target_dir = validators_dir(global);
    let package_dir = target_dir.join(&name);

    // Remove existing if present
    if package_dir.exists() {
        std::fs::remove_dir_all(&package_dir)?;
    }
    std::fs::create_dir_all(&package_dir)?;

    extract_zip(&data, &package_dir)?;
    println!("  Extracted to {}", package_dir.display());

    // Update lockfile
    let project_root = std::env::current_dir()?;
    let mut lf = Lockfile::load(&project_root)?;
    lf.add_package(
        name.clone(),
        LockedPackage {
            version: resolved_version.clone(),
            resolved: version_detail.download_url.clone(),
            integrity: version_detail.integrity.clone(),
            installed_at: chrono::Utc::now().to_rfc3339(),
        },
    );
    lf.save(&project_root)?;
    println!("  Updated avp-lock.json");

    println!("\nInstalled {}@{}", name, resolved_version);
    Ok(())
}

/// Run the uninstall command.
///
/// Removes an installed validator package.
pub async fn run_uninstall(name: &str, global: bool) -> Result<(), RegistryError> {
    let target_dir = validators_dir(global);
    let package_dir = target_dir.join(name);

    if !package_dir.exists() {
        let location = if global { "global" } else { "local" };
        return Err(RegistryError::NotFound(format!(
            "Package '{}' is not installed ({})",
            name, location
        )));
    }

    std::fs::remove_dir_all(&package_dir)?;
    println!("Removed {} from {}", name, target_dir.display());

    // Update lockfile
    let project_root = std::env::current_dir()?;
    let mut lf = Lockfile::load(&project_root)?;
    lf.remove_package(name);
    lf.save(&project_root)?;
    println!("Updated avp-lock.json");

    println!("\nUninstalled {}", name);
    Ok(())
}

/// Install a specific package version (used by update command).
pub async fn install_package(name: &str, version: &str, global: bool) -> Result<(), RegistryError> {
    let spec = format!("{}@{}", name, version);
    run_install(&spec, global).await
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

        // Skip the top-level directory wrapper if present (e.g. "package-name/...")
        // The registry wraps packages in a directory matching the package name
        let relative_path = if let Some((_prefix, rest)) = name.split_once('/') {
            if rest.is_empty() {
                // This is just the directory entry itself, skip
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
            // Ensure parent directory exists
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
    fn test_parse_package_spec_scoped_name() {
        // In case names ever have @ in them (unlikely but safe)
        let (name, version) = parse_package_spec("some-pkg@2.0.0");
        assert_eq!(name, "some-pkg");
        assert_eq!(version, Some("2.0.0".to_string()));
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
        // Should be under home directory
        let home = dirs::home_dir().unwrap();
        assert!(dir.starts_with(home));
    }
}
