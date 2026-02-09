//! AVP Info - Show detailed information about a validator.
//!
//! Checks local sources (builtin, user, project) first, then falls back
//! to the remote registry.

use avp_common::builtin::load_builtins;
use avp_common::validator::{ValidatorLoader, ValidatorSource};

use crate::registry::{RegistryClient, RegistryError};

/// Source label with emoji, matching `avp list` output.
fn source_label(source: &ValidatorSource) -> &'static str {
    match source {
        ValidatorSource::Builtin => "📦 Built-in",
        ValidatorSource::User => "👤 User",
        ValidatorSource::Project => "📁 Project",
    }
}

/// Run the info command.
///
/// Checks local validators first (builtin, user, project), then queries
/// the remote registry if not found locally.
pub async fn run_info(name: &str) -> Result<(), RegistryError> {
    // Try local sources first
    if show_local_info(name) {
        return Ok(());
    }

    // Fall back to registry
    show_registry_info(name).await
}

/// Try to show info from locally loaded validators.
///
/// Returns true if the validator was found locally.
fn show_local_info(name: &str) -> bool {
    let mut loader = ValidatorLoader::new();
    load_builtins(&mut loader);
    let _ = loader.load_all();

    let Some(ruleset) = loader.get_ruleset(name) else {
        return false;
    };

    println!(
        "{}@{} ({})\n",
        ruleset.name(),
        ruleset.manifest.version,
        source_label(&ruleset.source)
    );
    println!("  Description: {}", ruleset.description());
    println!("  Trigger:     {}", ruleset.manifest.trigger);
    println!("  Severity:    {}", ruleset.manifest.severity);

    if !ruleset.manifest.tags.is_empty() {
        println!("  Tags:        {}", ruleset.manifest.tags.join(", "));
    }

    println!("  Path:        {}", ruleset.base_path.display());

    if !ruleset.rules.is_empty() {
        println!("\n  Rules:");
        for rule in &ruleset.rules {
            let eff_sev = rule.effective_severity(ruleset);
            if eff_sev != ruleset.manifest.severity {
                println!("    - {} [{}]", rule.name, eff_sev);
            } else {
                println!("    - {}", rule.name);
            }
        }
    }

    true
}

/// Show info from the remote registry.
async fn show_registry_info(name: &str) -> Result<(), RegistryError> {
    let client = RegistryClient::new();
    let detail = client.package_info(name).await?;

    println!("{}@{} (registry)\n", detail.name, detail.latest);
    println!("  Description: {}", detail.description);
    println!("  Author:      {}", detail.author);

    if let Some(license) = &detail.license {
        println!("  License:     {}", license);
    }

    if !detail.tags.is_empty() {
        println!("  Tags:        {}", detail.tags.join(", "));
    }

    println!("  Downloads:   {}", detail.downloads);
    println!("  Created:     {}", detail.created_at);
    println!("  Updated:     {}", detail.updated_at);

    if !detail.versions.is_empty() {
        println!("\n  Versions:    {}", detail.versions.join(", "));
    }

    if let Some(readme) = &detail.readme {
        let excerpt: String = readme.lines().take(20).collect::<Vec<_>>().join("\n");
        println!("\n--- README ---\n{}", excerpt);
        if readme.lines().count() > 20 {
            println!("  ... (truncated)");
        }
    }

    println!("\n  Install: avp install {}", detail.name);

    Ok(())
}
