//! AVP Publish - Publish and unpublish validator packages.

use std::io::{Cursor, Read, Write};
use std::path::Path;

use walkdir::WalkDir;

use crate::registry::{RegistryClient, RegistryError};

/// Run the publish command.
///
/// Validates the given directory as a RuleSet and publishes it to the registry.
pub async fn run_publish(path: &Path, dry_run: bool) -> Result<(), RegistryError> {
    // Resolve to absolute path (handles "." and relative paths)
    let dir = path.canonicalize().map_err(|e| {
        RegistryError::Validation(format!("Cannot resolve path '{}': {}", path.display(), e))
    })?;

    // Validate RuleSet structure
    println!("Validating RuleSet structure...");
    let (name, version) = validate_ruleset(&dir)?;
    println!("  Name:    {}", name);
    println!("  Version: {}", version);

    // Create ZIP archive
    println!("Creating package archive...");
    let archive = create_zip(&dir, &name)?;
    let size = archive.len();
    println!("  Archive size: {} bytes", size);

    // Check size limit (10MB)
    if size > 10 * 1024 * 1024 {
        return Err(RegistryError::Validation(format!(
            "Package too large: {} bytes (max 10MB)",
            size
        )));
    }

    if dry_run {
        println!(
            "\n[dry-run] Would publish {}@{} ({} bytes)",
            name, version, size
        );
        println!("[dry-run] No changes made.");
        return Ok(());
    }

    // Upload
    println!("Uploading to registry...");
    let client = RegistryClient::authenticated()?;
    let response = client.publish(archive).await?;

    println!("\nPublished {}@{}", response.name, response.version);
    println!("  URL: {}", response.download_url);

    Ok(())
}

/// Run the unpublish command.
///
/// Removes a published version from the registry after confirmation.
pub async fn run_unpublish(name_version: &str) -> Result<(), RegistryError> {
    let (name, version) = parse_name_version(name_version)?;

    // Confirm
    println!(
        "Warning: This will permanently delete version {} of {}",
        version, name
    );

    let confirmed = dialoguer::Confirm::new()
        .with_prompt("Are you sure?")
        .default(false)
        .interact()
        .map_err(|e| RegistryError::Validation(e.to_string()))?;

    if !confirmed {
        println!("Cancelled.");
        return Ok(());
    }

    let client = RegistryClient::authenticated()?;
    client.unpublish(&name, &version).await?;

    println!("Unpublished {}@{}", name, version);
    Ok(())
}

/// Parse "name@version" (both required).
fn parse_name_version(spec: &str) -> Result<(String, String), RegistryError> {
    match spec.rsplit_once('@') {
        Some((name, version)) if !name.is_empty() && !version.is_empty() => {
            Ok((name.to_string(), version.to_string()))
        }
        _ => Err(RegistryError::Validation(format!(
            "Expected format: name@version (got '{}')",
            spec
        ))),
    }
}

/// Validate that a directory contains a valid RuleSet.
///
/// Returns (name, version) from VALIDATOR.md frontmatter.
fn validate_ruleset(dir: &Path) -> Result<(String, String), RegistryError> {
    // Check VALIDATOR.md exists
    let validator_md = dir.join("VALIDATOR.md");
    if !validator_md.exists() {
        return Err(RegistryError::Validation(format!(
            "VALIDATOR.md not found in {}",
            dir.display()
        )));
    }

    // Parse frontmatter to extract name and version
    let content = std::fs::read_to_string(&validator_md)?;
    let (name, version) = parse_validator_frontmatter(&content)?;

    // Check rules directory
    let rules_dir = dir.join("rules");
    if !rules_dir.exists() || !rules_dir.is_dir() {
        return Err(RegistryError::Validation(
            "rules/ directory not found".to_string(),
        ));
    }

    // Check at least one .md file in rules/
    let has_rules = std::fs::read_dir(&rules_dir)?
        .filter_map(|e| e.ok())
        .any(|e| e.path().extension().is_some_and(|ext| ext == "md"));

    if !has_rules {
        return Err(RegistryError::Validation(
            "rules/ directory must contain at least one .md file".to_string(),
        ));
    }

    Ok((name, version))
}

/// Parse name and version from VALIDATOR.md YAML frontmatter.
fn parse_validator_frontmatter(content: &str) -> Result<(String, String), RegistryError> {
    // Simple frontmatter parser: extract content between --- delimiters
    let content = content.trim();
    if !content.starts_with("---") {
        return Err(RegistryError::Validation(
            "VALIDATOR.md must start with YAML frontmatter (---)".to_string(),
        ));
    }

    let rest = &content[3..];
    let end = rest
        .find("---")
        .ok_or_else(|| RegistryError::Validation("No closing --- in frontmatter".to_string()))?;

    let frontmatter = &rest[..end];
    let yaml: serde_yaml::Value = serde_yaml::from_str(frontmatter)
        .map_err(|e| RegistryError::Validation(format!("Invalid YAML frontmatter: {}", e)))?;

    let name = yaml
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RegistryError::Validation("Missing 'name' in frontmatter".to_string()))?
        .to_string();

    let version = yaml
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RegistryError::Validation("Missing 'version' in frontmatter".to_string()))?
        .to_string();

    Ok((name, version))
}

/// Create a ZIP archive of a RuleSet directory.
fn create_zip(dir: &Path, name: &str) -> Result<Vec<u8>, RegistryError> {
    let buffer = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buffer);

    let options =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            // Skip hidden files, target dirs, etc.
            !name.starts_with('.') && name != "target" && name != "node_modules"
        })
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let relative = path
            .strip_prefix(dir)
            .map_err(|e| RegistryError::Validation(e.to_string()))?;

        // Prefix with package name in the ZIP (registry expects this)
        let zip_path = format!("{}/{}", name, relative.display());

        if path.is_dir() {
            zip.add_directory(&zip_path, options)
                .map_err(|e| RegistryError::Validation(format!("ZIP write error: {}", e)))?;
        } else {
            zip.start_file(&zip_path, options)
                .map_err(|e| RegistryError::Validation(format!("ZIP write error: {}", e)))?;
            let mut file = std::fs::File::open(path)?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            zip.write_all(&buf)?;
        }
    }

    let cursor = zip
        .finish()
        .map_err(|e| RegistryError::Validation(format!("ZIP finalize error: {}", e)))?;
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_name_version_valid() {
        let (name, version) = parse_name_version("no-secrets@1.2.3").unwrap();
        assert_eq!(name, "no-secrets");
        assert_eq!(version, "1.2.3");
    }

    #[test]
    fn test_parse_name_version_missing_version() {
        assert!(parse_name_version("no-secrets").is_err());
    }

    #[test]
    fn test_parse_name_version_empty_parts() {
        assert!(parse_name_version("@1.0.0").is_err());
        assert!(parse_name_version("name@").is_err());
    }

    #[test]
    fn test_parse_validator_frontmatter() {
        let content = r#"---
name: test-validator
version: "1.0.0"
trigger: PostToolUse
---
# Body
"#;
        let (name, version) = parse_validator_frontmatter(content).unwrap();
        assert_eq!(name, "test-validator");
        assert_eq!(version, "1.0.0");
    }

    #[test]
    fn test_parse_validator_frontmatter_missing_name() {
        let content = r#"---
version: "1.0.0"
---
"#;
        assert!(parse_validator_frontmatter(content).is_err());
    }

    #[test]
    fn test_parse_validator_frontmatter_no_frontmatter() {
        let content = "# No frontmatter";
        assert!(parse_validator_frontmatter(content).is_err());
    }
}
