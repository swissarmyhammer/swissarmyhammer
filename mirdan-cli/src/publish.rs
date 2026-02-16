//! Mirdan Publish - Publish and unpublish packages.
//!
//! Two modes:
//! - **URL source**: Registers a marketplace via `POST /api/marketplaces` and
//!   triggers a sync. The registry clones and discovers skills server-side.
//! - **Local path**: Zips the directory and uploads via `POST /api/packages`.

use std::io::{Cursor, Read, Write};
use std::path::Path;

use walkdir::WalkDir;

use crate::registry::{RegistryClient, RegistryError};

/// Check if a source string looks like a git URL or owner/repo shorthand.
fn is_remote_source(source: &str) -> bool {
    source.starts_with("https://")
        || source.starts_with("http://")
        || source.starts_with("git@")
        || is_owner_repo(source)
}

/// Check if a source looks like `owner/repo` shorthand (no spaces, exactly one slash,
/// no leading dot or slash).
fn is_owner_repo(source: &str) -> bool {
    let parts: Vec<&str> = source.split('/').collect();
    parts.len() == 2
        && !parts[0].is_empty()
        && !parts[1].is_empty()
        && !source.starts_with('.')
        && !source.starts_with('/')
        && !source.contains(' ')
}

/// Run the publish command.
///
/// - For URLs or `owner/repo` shorthand: registers as a marketplace and triggers sync.
/// - For local paths: zips and uploads to the package registry.
pub async fn run_publish(source: &str, dry_run: bool) -> Result<(), RegistryError> {
    if is_remote_source(source) {
        run_publish_marketplace(source, dry_run).await
    } else {
        run_publish_local(source, dry_run).await
    }
}

/// Register a marketplace and trigger sync.
async fn run_publish_marketplace(source: &str, dry_run: bool) -> Result<(), RegistryError> {
    println!("Registering marketplace: {}", source);

    if dry_run {
        println!("\n[dry-run] Would register marketplace: {}", source);
        println!("[dry-run] No changes made.");
        return Ok(());
    }

    let client = RegistryClient::authenticated()?;
    let marketplace = client.register_marketplace(source).await?;

    println!("  ID:       {}", marketplace.id);
    println!("  URL:      {}", marketplace.url);
    println!("  Provider: {}", marketplace.provider);

    // Trigger sync to discover skills
    println!("Syncing marketplace...");
    let sync = client.sync_marketplace(&marketplace.id).await?;

    println!(
        "\nDiscovered {} skill(s) via {}",
        sync.skill_count, sync.discovery_mode
    );
    for skill in &sync.skills {
        println!("  - {}", skill.qualified_name);
    }

    Ok(())
}

/// Zip a local directory and upload to the package registry.
async fn run_publish_local(source: &str, dry_run: bool) -> Result<(), RegistryError> {
    let path = Path::new(source);
    let dir = path.canonicalize().map_err(|e| {
        RegistryError::Validation(format!("Cannot resolve path '{}': {}", path.display(), e))
    })?;

    let dir_name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "package".to_string());

    println!("Creating package archive from {}...", source);
    let archive = create_zip(&dir, &dir_name)?;
    let size = archive.len();
    println!("  Archive size: {} bytes", size);

    if dry_run {
        println!("\n[dry-run] Would upload archive ({} bytes)", size);
        println!("[dry-run] No changes made.");
        return Ok(());
    }

    println!("Uploading to registry...");
    let client = RegistryClient::authenticated()?;
    let response = client.publish(archive).await?;

    println!("\nPublished {}@{}", response.name, response.version);
    println!("  URL: {}", response.download_url);

    Ok(())
}

/// Run the unpublish command.
pub async fn run_unpublish(name_version: &str) -> Result<(), RegistryError> {
    let (name, version) = parse_name_version(name_version)?;

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

/// Create a ZIP archive of a package directory.
fn create_zip(dir: &Path, name: &str) -> Result<Vec<u8>, RegistryError> {
    let buffer = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buffer);

    let options =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && name != "target" && name != "node_modules"
        })
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let relative = path
            .strip_prefix(dir)
            .map_err(|e| RegistryError::Validation(e.to_string()))?;

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
    fn test_is_remote_source_https() {
        assert!(is_remote_source("https://github.com/obra/superpowers"));
    }

    #[test]
    fn test_is_remote_source_http() {
        assert!(is_remote_source("http://github.com/obra/superpowers"));
    }

    #[test]
    fn test_is_remote_source_ssh() {
        assert!(is_remote_source("git@github.com:obra/superpowers.git"));
    }

    #[test]
    fn test_is_remote_source_owner_repo() {
        assert!(is_remote_source("obra/superpowers"));
        assert!(is_remote_source("anthropics/skills"));
    }

    #[test]
    fn test_is_remote_source_local_path() {
        assert!(!is_remote_source("."));
        assert!(!is_remote_source("./my-skill"));
        assert!(!is_remote_source("/tmp/my-skill"));
        assert!(!is_remote_source("../other"));
    }

    #[test]
    fn test_is_owner_repo() {
        assert!(is_owner_repo("obra/superpowers"));
        assert!(is_owner_repo("anthropics/skills"));
        assert!(!is_owner_repo("just-a-name"));
        assert!(!is_owner_repo("a/b/c"));
        assert!(!is_owner_repo("./relative/path"));
        assert!(!is_owner_repo("/absolute/path"));
        assert!(!is_owner_repo("has space/repo"));
        assert!(!is_owner_repo("/foo"));
        assert!(!is_owner_repo("foo/"));
    }
}
