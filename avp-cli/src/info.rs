//! AVP Info - Show detailed information about a registry package.

use crate::registry::{RegistryClient, RegistryError};

/// Run the info command.
///
/// Displays detailed information about a package from the registry.
pub async fn run_info(name: &str) -> Result<(), RegistryError> {
    let client = RegistryClient::new();
    let detail = client.package_info(name).await?;

    println!("{}@{}\n", detail.name, detail.latest);
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
        // Show first 20 lines of readme
        let excerpt: String = readme.lines().take(20).collect::<Vec<_>>().join("\n");
        println!("\n--- README ---\n{}", excerpt);
        if readme.lines().count() > 20 {
            println!("  ... (truncated)");
        }
    }

    println!("\n  Install: avp install {}", detail.name);

    Ok(())
}
