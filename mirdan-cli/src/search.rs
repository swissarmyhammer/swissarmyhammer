//! Mirdan Search - Search the registry for skills and validators.

use crate::registry::{RegistryClient, RegistryError};
use crate::table;

/// Run the search command.
///
/// Searches the registry for packages matching the query.
pub async fn run_search(query: &str, json: bool) -> Result<(), RegistryError> {
    let client = RegistryClient::new();
    let response = client.search(query, None, None).await?;

    if json {
        let output = serde_json::to_string_pretty(&response)?;
        println!("{}", output);
        return Ok(());
    }

    if response.packages.is_empty() {
        println!("No packages found matching \"{}\".", query);
        return Ok(());
    }

    println!(
        "Found {} package(s) matching \"{}\":\n",
        response.total, query
    );

    let mut tbl = table::new_table();
    tbl.set_header(vec!["Name", "Type", "Version", "Description", "Downloads"]);

    for pkg in &response.packages {
        let name = table::short_name(&pkg.name);
        let description = table::truncate_str(&pkg.description, 50);
        let pkg_type = pkg.package_type.as_deref().unwrap_or("unknown");

        tbl.add_row(vec![
            name,
            pkg_type.to_string(),
            pkg.latest.clone(),
            description,
            format_downloads(pkg.downloads),
        ]);
    }

    println!("{tbl}");
    println!("\nRun 'mirdan info <name>' for more details.");

    Ok(())
}

/// Format download count for display (e.g. 1234 -> "1.2k").
fn format_downloads(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}k", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_downloads() {
        assert_eq!(format_downloads(0), "0");
        assert_eq!(format_downloads(500), "500");
        assert_eq!(format_downloads(1234), "1.2k");
        assert_eq!(format_downloads(12345), "12.3k");
        assert_eq!(format_downloads(1_234_567), "1.2M");
    }
}
