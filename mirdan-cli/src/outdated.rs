//! Mirdan Outdated/Update - Check for and apply package updates.
//!
//! Scans installed packages on the filesystem and compares versions
//! against the registry.

use crate::install;
use crate::list;
use crate::registry::{RegistryClient, RegistryError};
use crate::table;

/// Run the outdated command.
///
/// Discovers installed packages from the filesystem and checks the registry
/// for newer versions.
pub async fn run_outdated() -> Result<(), RegistryError> {
    let packages = list::discover_packages(false, false, None);

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
pub async fn run_update(
    name: Option<&str>,
    agent_filter: Option<&str>,
    global: bool,
) -> Result<(), RegistryError> {
    let packages = list::discover_packages(false, false, agent_filter);

    if packages.is_empty() {
        println!("No packages installed.");
        return Ok(());
    }

    let client = RegistryClient::new();

    // If a specific name is given, just update that one
    if let Some(name) = name {
        let pkg = packages.iter().find(|p| p.name == name).ok_or_else(|| {
            RegistryError::NotFound(format!("Package '{}' is not installed", name))
        })?;

        println!("Checking for updates to {}...", name);

        let detail = client.package_info(name).await?;

        if detail.latest == pkg.version {
            println!("{} is already up to date ({})", name, pkg.version);
            return Ok(());
        }

        println!("Updating {}: {} -> {}", name, pkg.version, detail.latest);
        install::install_package(name, &detail.latest, agent_filter, global).await?;
        return Ok(());
    }

    // Update all that have registry updates
    println!("Checking for updates...");

    let mut updated = 0;

    for pkg in &packages {
        match client.package_info(&pkg.name).await {
            Ok(detail) if detail.latest != pkg.version => {
                println!(
                    "  Updating {}: {} -> {}",
                    pkg.name, pkg.version, detail.latest
                );
                install::install_package(&pkg.name, &detail.latest, agent_filter, global).await?;
                updated += 1;
            }
            _ => {}
        }
    }

    if updated > 0 {
        println!("\n{} package(s) updated.", updated);
    } else {
        println!("All packages are up to date.");
    }

    Ok(())
}
