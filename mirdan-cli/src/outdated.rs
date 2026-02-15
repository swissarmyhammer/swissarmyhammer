//! Mirdan Outdated/Update - Check for and apply package updates.
//!
//! Scans the lockfile for installed packages and compares versions
//! against the registry.

use comfy_table::{presets::UTF8_FULL, Table};

use crate::install;
use crate::lockfile::Lockfile;
use crate::registry::{RegistryClient, RegistryError};

/// Run the outdated command.
///
/// Scans the lockfile and checks the registry for newer versions.
pub async fn run_outdated() -> Result<(), RegistryError> {
    let project_root = std::env::current_dir()?;
    let lf = Lockfile::load(&project_root)?;

    if lf.packages.is_empty() {
        println!("No packages installed. Run 'mirdan install <package>' to install one.");
        return Ok(());
    }

    println!(
        "Checking {} package(s) against registry...\n",
        lf.packages.len()
    );

    let client = RegistryClient::new();
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Package", "Type", "Local", "Registry", "Status"]);

    let mut updates_available = 0;

    for (name, pkg) in &lf.packages {
        let (registry_version, status) = match client.package_info(name).await {
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

        table.add_row(vec![
            name.clone(),
            pkg.package_type.to_string(),
            pkg.version.clone(),
            registry_version,
            status,
        ]);
    }

    println!("{table}");

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
    let project_root = std::env::current_dir()?;
    let lf = Lockfile::load(&project_root)?;

    if lf.packages.is_empty() {
        println!("No packages installed.");
        return Ok(());
    }

    let client = RegistryClient::new();

    // If a specific name is given, just update that one
    if let Some(name) = name {
        let pkg = lf.get_package(name).ok_or_else(|| {
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
    let names: Vec<(String, String)> = lf
        .packages
        .iter()
        .map(|(n, p)| (n.clone(), p.version.clone()))
        .collect();

    for (name, version) in &names {
        match client.package_info(name).await {
            Ok(detail) if detail.latest != *version => {
                println!("  Updating {}: {} -> {}", name, version, detail.latest);
                install::install_package(name, &detail.latest, agent_filter, global).await?;
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
