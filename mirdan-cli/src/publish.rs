//! Mirdan Publish - Publish and unpublish packages.
//!
//! Auto-detects package type from directory contents:
//! - SKILL.md present -> publish as skill
//! - VALIDATOR.md + rules/ present -> publish as validator

use std::io::{Cursor, Read, Write};
use std::path::Path;

use walkdir::WalkDir;

use crate::package_type::{self, PackageType};
use crate::registry::{RegistryClient, RegistryError};

/// Run the publish command.
///
/// Validates the given directory and publishes it to the registry.
pub async fn run_publish(path: &Path, dry_run: bool) -> Result<(), RegistryError> {
    let dir = path.canonicalize().map_err(|e| {
        RegistryError::Validation(format!("Cannot resolve path '{}': {}", path.display(), e))
    })?;

    // Detect package type
    let pkg_type = package_type::detect_package_type(&dir).ok_or_else(|| {
        RegistryError::Validation(
            "Cannot determine package type. Directory must contain SKILL.md (skill) \
             or VALIDATOR.md + rules/ (validator)."
                .to_string(),
        )
    })?;

    println!("Detected package type: {}", pkg_type);

    // Validate structure
    let (name, version) = match pkg_type {
        PackageType::Skill => validate_skill(&dir)?,
        PackageType::Validator => validate_validator(&dir)?,
    };

    println!("  Name:    {}", name);
    println!("  Version: {}", version);

    // Create ZIP archive
    println!("Creating package archive...");
    let archive = create_zip(&dir, &name)?;
    let size = archive.len();
    println!("  Archive size: {} bytes", size);

    if size > 10 * 1024 * 1024 {
        return Err(RegistryError::Validation(format!(
            "Package too large: {} bytes (max 10MB)",
            size
        )));
    }

    if dry_run {
        println!(
            "\n[dry-run] Would publish {}@{} ({}) ({} bytes)",
            name, version, pkg_type, size
        );
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

/// Validate a skill directory and return (name, version).
fn validate_skill(dir: &Path) -> Result<(String, String), RegistryError> {
    let skill_md = dir.join("SKILL.md");
    if !skill_md.exists() {
        return Err(RegistryError::Validation(format!(
            "SKILL.md not found in {}",
            dir.display()
        )));
    }

    let content = std::fs::read_to_string(&skill_md)?;
    let (name, version) = parse_frontmatter(&content, "SKILL.md")?;

    // Validate name per agentskills.io spec
    if !crate::package_type::is_valid_package_name(&name) {
        return Err(RegistryError::Validation(format!(
            "Invalid skill name '{}'. Must be 1-64 chars, lowercase alphanumeric with hyphens.",
            name
        )));
    }

    Ok((name, version))
}

/// Validate a validator directory and return (name, version).
fn validate_validator(dir: &Path) -> Result<(String, String), RegistryError> {
    let validator_md = dir.join("VALIDATOR.md");
    if !validator_md.exists() {
        return Err(RegistryError::Validation(format!(
            "VALIDATOR.md not found in {}",
            dir.display()
        )));
    }

    let content = std::fs::read_to_string(&validator_md)?;
    let (name, version) = parse_frontmatter(&content, "VALIDATOR.md")?;

    let rules_dir = dir.join("rules");
    if !rules_dir.exists() || !rules_dir.is_dir() {
        return Err(RegistryError::Validation(
            "rules/ directory not found".to_string(),
        ));
    }

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

/// Parse name and version from YAML frontmatter.
fn parse_frontmatter(content: &str, filename: &str) -> Result<(String, String), RegistryError> {
    let content = content.trim();
    if !content.starts_with("---") {
        return Err(RegistryError::Validation(format!(
            "{} must start with YAML frontmatter (---)",
            filename
        )));
    }

    let rest = &content[3..];
    let end = rest.find("---").ok_or_else(|| {
        RegistryError::Validation(format!("No closing --- in {} frontmatter", filename))
    })?;

    let frontmatter = &rest[..end];
    let yaml: serde_yaml::Value = serde_yaml::from_str(frontmatter)
        .map_err(|e| RegistryError::Validation(format!("Invalid YAML frontmatter: {}", e)))?;

    let name = yaml
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            RegistryError::Validation(format!("Missing 'name' in {} frontmatter", filename))
        })?
        .to_string();

    let version = yaml
        .get("version")
        .and_then(|v| v.as_str())
        .or_else(|| {
            yaml.get("metadata")
                .and_then(|m| m.get("version"))
                .and_then(|v| v.as_str())
        })
        .ok_or_else(|| {
            RegistryError::Validation(format!("Missing 'version' in {} frontmatter", filename))
        })?
        .to_string();

    Ok((name, version))
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
    fn test_parse_frontmatter_skill() {
        let content = r#"---
name: test-skill
version: "1.0.0"
description: "A test skill"
---
# Body
"#;
        let (name, version) = parse_frontmatter(content, "SKILL.md").unwrap();
        assert_eq!(name, "test-skill");
        assert_eq!(version, "1.0.0");
    }

    #[test]
    fn test_parse_frontmatter_validator() {
        let content = r#"---
name: test-validator
version: "1.0.0"
trigger: PostToolUse
---
# Body
"#;
        let (name, version) = parse_frontmatter(content, "VALIDATOR.md").unwrap();
        assert_eq!(name, "test-validator");
        assert_eq!(version, "1.0.0");
    }

    #[test]
    fn test_parse_frontmatter_metadata_version() {
        let content = r#"---
name: test-skill
metadata:
  version: "2.0.0"
---
# Body
"#;
        let (name, version) = parse_frontmatter(content, "SKILL.md").unwrap();
        assert_eq!(name, "test-skill");
        assert_eq!(version, "2.0.0");
    }

    #[test]
    fn test_parse_frontmatter_missing_name() {
        let content = r#"---
version: "1.0.0"
---
"#;
        assert!(parse_frontmatter(content, "SKILL.md").is_err());
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let content = "# No frontmatter";
        assert!(parse_frontmatter(content, "SKILL.md").is_err());
    }

    #[test]
    fn test_validate_skill() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("SKILL.md"),
            r#"---
name: test-skill
version: "0.1.0"
description: "A test"
---
# Test
"#,
        )
        .unwrap();

        let (name, version) = validate_skill(dir.path()).unwrap();
        assert_eq!(name, "test-skill");
        assert_eq!(version, "0.1.0");
    }

    #[test]
    fn test_validate_validator() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("VALIDATOR.md"),
            r#"---
name: test-val
version: "0.1.0"
trigger: PostToolUse
---
# Test
"#,
        )
        .unwrap();
        let rules_dir = dir.path().join("rules");
        std::fs::create_dir(&rules_dir).unwrap();
        std::fs::write(rules_dir.join("example.md"), "# Rule").unwrap();

        let (name, version) = validate_validator(dir.path()).unwrap();
        assert_eq!(name, "test-val");
        assert_eq!(version, "0.1.0");
    }
}
