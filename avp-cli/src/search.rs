//! AVP Search - Search the registry for validator packages.

use comfy_table::{presets::UTF8_FULL, Table};

use crate::registry::{RegistryClient, RegistryError};

/// Run the search command.
///
/// Searches the AVP registry for packages matching the query.
pub async fn run_search(query: &str, tag: Option<&str>, json: bool) -> Result<(), RegistryError> {
    let client = RegistryClient::new();
    let response = client.search(query, tag, None, None).await?;

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
    table.set_header(vec!["Name", "Version", "Description", "Downloads"]);

    for pkg in &response.packages {
        let description = if pkg.description.len() > 60 {
            format!("{}...", &pkg.description[..57])
        } else {
            pkg.description.clone()
        };

        table.add_row(vec![
            pkg.name.clone(),
            pkg.latest.clone(),
            description,
            format_downloads(pkg.downloads),
        ]);
    }

    println!("{table}");
    println!("\nRun 'avp info <name>' for more details.");

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
