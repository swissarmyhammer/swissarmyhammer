//! AVP Outdated/Update - Check for and apply package updates.
//!
//! Scans local validators (builtin, user, project) and compares their
//! versions against the registry to find available updates.

use comfy_table::{presets::UTF8_FULL, Table};

use avp_common::builtin::load_builtins;
use avp_common::validator::{ValidatorLoader, ValidatorSource};

use crate::package;
use crate::registry::{RegistryClient, RegistryError};

/// Source label with emoji, matching `avp list` / `avp info` output.
fn source_label(source: &ValidatorSource) -> &'static str {
    match source {
        ValidatorSource::Builtin => "📦 Built-in",
        ValidatorSource::User => "👤 User",
        ValidatorSource::Project => "📁 Project",
    }
}

/// A local validator with its name, version, and source.
struct LocalPackage {
    name: String,
    version: String,
    source: ValidatorSource,
}

/// Collect all local validators from all sources.
fn collect_local_packages() -> Vec<LocalPackage> {
    let mut loader = ValidatorLoader::new();
    load_builtins(&mut loader);
    let _ = loader.load_all();

    let mut packages: Vec<LocalPackage> = loader
        .list_rulesets()
        .into_iter()
        .map(|rs| LocalPackage {
            name: rs.name().to_string(),
            version: rs.manifest.version.clone(),
            source: rs.source.clone(),
        })
        .collect();

    packages.sort_by(|a, b| a.name.cmp(&b.name));
    packages
}

/// Run the outdated command.
///
/// Scans all local validators and checks the registry for newer versions.
/// Shows a table of all validators with local and registry versions.
pub async fn run_outdated() -> Result<(), RegistryError> {
    let packages = collect_local_packages();

    if packages.is_empty() {
        println!("No validators found.");
        return Ok(());
    }

    println!(
        "Checking {} validator(s) against registry...\n",
        packages.len()
    );

    let client = RegistryClient::new();
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Validator", "Local", "Registry", "Status", "Source"]);

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

        table.add_row(vec![
            pkg.name.clone(),
            pkg.version.clone(),
            registry_version,
            status,
            source_label(&pkg.source).to_string(),
        ]);
    }

    println!("{table}");

    if updates_available > 0 {
        println!(
            "\n{} update(s) available. Run 'avp update' to update all, or 'avp update <name>' for a specific package.",
            updates_available
        );
    }

    Ok(())
}

/// Run the update command.
///
/// Updates one or all local validators to their latest registry versions.
pub async fn run_update(name: Option<&str>, global: bool) -> Result<(), RegistryError> {
    let packages = collect_local_packages();

    if packages.is_empty() {
        println!("No validators found.");
        return Ok(());
    }

    let client = RegistryClient::new();

    // If a specific name is given, just update that one
    if let Some(name) = name {
        let pkg = packages.iter().find(|p| p.name == name).ok_or_else(|| {
            RegistryError::NotFound(format!("Validator '{}' is not installed", name))
        })?;

        println!("Checking for updates to {}...", name);

        let detail = client.package_info(name).await?;

        if detail.latest == pkg.version {
            println!("{} is already up to date ({})", name, pkg.version);
            return Ok(());
        }

        println!("Updating {}: {} -> {}", name, pkg.version, detail.latest);
        package::install_package(name, &detail.latest, global).await?;
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
                package::install_package(&pkg.name, &detail.latest, global).await?;
                updated += 1;
            }
            _ => {}
        }
    }

    if updated > 0 {
        println!("\n{} package(s) updated.", updated);
    } else {
        println!("All validators are up to date.");
    }

    Ok(())
}
