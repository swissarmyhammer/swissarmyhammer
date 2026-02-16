//! Mirdan Search - Search the registry for skills and validators.

use comfy_table::{presets::UTF8_FULL, Table};

use crate::registry::{RegistryClient, RegistryError};

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

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Name", "Type", "Version", "Description", "Downloads"]);

    for pkg in &response.packages {
        let description = truncate_str(&pkg.description, 50);

        let pkg_type = pkg
            .package_type
            .as_deref()
            .unwrap_or("unknown");

        table.add_row(vec![
            pkg.name.clone(),
            pkg_type.to_string(),
            pkg.latest.clone(),
            description,
            format_downloads(pkg.downloads),
        ]);
    }

    println!("{table}");
    println!("\nRun 'mirdan info <name>' for more details.");

    Ok(())
}

/// Truncate a string to `max` characters, appending "..." if truncated.
/// Safe for multi-byte (UTF-8) strings.
fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max - 3).collect();
        format!("{}...", truncated)
    }
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
