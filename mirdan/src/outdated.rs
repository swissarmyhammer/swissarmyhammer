//! Mirdan Outdated/Update - Check for and apply package updates.
//!
//! Scans installed packages on the filesystem and compares versions
//! against the registry.

use tracing::info;

use crate::install;
use crate::list;
use crate::registry::{RegistryClient, RegistryError};
use crate::table;

/// Run the outdated command.
///
/// Discovers installed packages from the filesystem and checks the registry
/// for newer versions.
pub async fn run_outdated() -> Result<(), RegistryError> {
    let packages = list::discover_packages(false, false, false, false, None);

    if packages.is_empty() {
        println!("No packages installed. Run 'mirdan install <package>' to install one.");
        return Ok(());
    }

    println!(
        "Checking {} package(s) against registry...\n",
        packages.len()
    );

    let client = RegistryClient::new();
    let mut tbl = table::new_table();
    tbl.set_header(vec!["Package", "Type", "Local", "Registry", "Status"]);

    let mut updates_available = 0;

    for pkg in &packages {
        let (registry_version, status) = match client.package_info(&pkg.name).await {
            Ok(detail) => {
                if detail.latest == pkg.version {
                    (detail.latest, "up to date".to_string())
                } else {
                    updates_available += 1;
                    (detail.latest, "update available".to_string())
                }
            }
            Err(RegistryError::NotFound(_)) => ("-".to_string(), "local only".to_string()),
            Err(_) => ("-".to_string(), "check failed".to_string()),
        };

        tbl.add_row(vec![
            pkg.name.clone(),
            pkg.package_type.to_string(),
            pkg.version.clone(),
            registry_version,
            status,
        ]);
    }

    println!("{tbl}");

    if updates_available > 0 {
        println!(
            "\n{} update(s) available. Run 'mirdan update' to update all, or 'mirdan update <name>' for a specific package.",
            updates_available
        );
    }

    Ok(())
}

/// Run the update command.
///
/// Updates one or all installed packages to their latest registry versions.
/// Returns a human-readable status message describing what happened.
pub async fn run_update(
    name: Option<&str>,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<String, RegistryError> {
    let packages = list::discover_packages(false, false, false, false, agent_filter);

    if packages.is_empty() {
        let msg = "No packages installed.".to_string();
        info!("{msg}");
        return Ok(msg);
    }

    let client = RegistryClient::new();

    // If a specific name is given, just update that one
    if let Some(name) = name {
        let pkg = packages.iter().find(|p| p.name == name).ok_or_else(|| {
            RegistryError::NotFound(format!("Package '{}' is not installed", name))
        })?;

        info!(name, "checking for updates");

        let detail = match client.package_info(name).await {
            Ok(d) => d,
            Err(RegistryError::NotFound(_)) => {
                let msg = format!("{name} is a local-only package (not in registry)");
                info!("{msg}");
                return Ok(msg);
            }
            Err(RegistryError::Conflict(_)) => {
                let msg = format!("{name} is already up to date ({})", pkg.version);
                info!("{msg}");
                return Ok(msg);
            }
            Err(e) => return Err(e),
        };

        if detail.latest == pkg.version {
            let msg = format!("{name} is already up to date ({})", pkg.version);
            info!("{msg}");
            return Ok(msg);
        }

        info!(name, from = %pkg.version, to = %detail.latest, "updating package");
        install::install_package(name, &detail.latest, agent_filter, global).await?;
        let msg = format!("Updated {name}: {} → {}", pkg.version, detail.latest);
        return Ok(msg);
    }

    // Update all that have registry updates
    info!("checking all packages for updates");

    let mut updated = 0;

    for pkg in &packages {
        match client.package_info(&pkg.name).await {
            Ok(detail) if detail.latest != pkg.version => {
                info!(name = %pkg.name, from = %pkg.version, to = %detail.latest, "updating package");
                install::install_package(&pkg.name, &detail.latest, agent_filter, global).await?;
                updated += 1;
            }
            _ => {}
        }
    }

    let msg = if updated > 0 {
        format!("{updated} package(s) updated.")
    } else {
        "All packages are up to date.".to_string()
    };
    info!("{msg}");

    Ok(msg)
}
