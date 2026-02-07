//! AVP Outdated/Update - Check for and apply package updates.

use comfy_table::{presets::UTF8_FULL, Table};

use avp_common::lockfile::Lockfile;

use crate::package;
use crate::registry::types::InstalledPackage;
use crate::registry::{RegistryClient, RegistryError};

/// Run the outdated command.
///
/// Checks installed packages against the registry for available updates.
pub async fn run_outdated() -> Result<(), RegistryError> {
    let project_root = std::env::current_dir()?;
    let lf = Lockfile::load(&project_root)?;

    let packages = lf.list_packages();
    if packages.is_empty() {
        println!("No packages installed.");
        return Ok(());
    }

    let installed: Vec<InstalledPackage> = packages
        .iter()
        .map(|(name, pkg)| InstalledPackage {
            name: name.to_string(),
            version: pkg.version.clone(),
        })
        .collect();

    let client = RegistryClient::authenticated()?;
    let response = client.check_updates(installed).await?;

    if response.updates.is_empty() {
        println!("All packages are up to date.");
        return Ok(());
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Package", "Current", "Latest", "Type"]);

    for update in &response.updates {
        table.add_row(vec![
            update.name.clone(),
            update.current_version.clone(),
            update.latest_version.clone(),
            update.update_type.clone(),
        ]);
    }

    println!("{table}");
    println!(
        "\n{} package(s) have updates available.",
        response.updates.len()
    );
    println!("Run 'avp update' to update all, or 'avp update <name>' for a specific package.");

    Ok(())
}

/// Run the update command.
///
/// Updates one or all installed packages to their latest versions.
pub async fn run_update(name: Option<&str>, global: bool) -> Result<(), RegistryError> {
    let project_root = std::env::current_dir()?;
    let lf = Lockfile::load(&project_root)?;

    let packages = lf.list_packages();
    if packages.is_empty() {
        println!("No packages installed.");
        return Ok(());
    }

    // If a specific name is given, just update that one
    if let Some(name) = name {
        let pkg = lf.get_package(name).ok_or_else(|| {
            RegistryError::NotFound(format!("Package '{}' is not installed", name))
        })?;
        println!("Checking for updates to {}...", name);

        let installed = vec![InstalledPackage {
            name: name.to_string(),
            version: pkg.version.clone(),
        }];

        let client = RegistryClient::authenticated()?;
        let response = client.check_updates(installed).await?;

        if response.updates.is_empty() {
            println!("{} is already up to date ({})", name, pkg.version);
            return Ok(());
        }

        let update = &response.updates[0];
        println!(
            "Updating {}: {} -> {}",
            name, update.current_version, update.latest_version
        );
        package::install_package(name, &update.latest_version, global).await?;
        return Ok(());
    }

    // Update all
    println!("Checking for updates...");
    let installed: Vec<InstalledPackage> = packages
        .iter()
        .map(|(name, pkg)| InstalledPackage {
            name: name.to_string(),
            version: pkg.version.clone(),
        })
        .collect();

    let client = RegistryClient::authenticated()?;
    let response = client.check_updates(installed).await?;

    if response.updates.is_empty() {
        println!("All packages are up to date.");
        return Ok(());
    }

    println!("Updating {} package(s):\n", response.updates.len());

    for update in &response.updates {
        println!(
            "  {}: {} -> {}",
            update.name, update.current_version, update.latest_version
        );
        package::install_package(&update.name, &update.latest_version, global).await?;
    }

    println!("\nAll packages updated.");
    Ok(())
}
