//! Mirdan Info - Show detailed information about a package.
//!
//! Checks local installations first, then falls back to the registry.

use std::path::Path;

use crate::agents::{self, agent_project_skill_dir};
use crate::frontmatter;
use crate::lockfile::Lockfile;
use crate::package_type::PackageType;
use crate::registry::{RegistryClient, RegistryError};
use crate::store;

/// What the info command prints for a value it cannot determine.
const UNKNOWN: &str = "unknown";

/// Run the info command.
///
/// Checks local lockfile first, then local installations, then the registry.
pub async fn run_info(name: &str, agent_filter: Option<&str>) -> Result<(), RegistryError> {
    // Try lockfile first
    if show_lockfile_info(name) {
        return Ok(());
    }

    // Try local scan
    if show_local_info(name, agent_filter) {
        return Ok(());
    }

    // Fall back to registry
    show_registry_info(name).await
}

/// Show info from the lockfile.
fn show_lockfile_info(name: &str) -> bool {
    let project_root = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(_) => return false,
    };

    let lf = match Lockfile::load(&project_root) {
        Ok(lf) => lf,
        Err(_) => return false,
    };

    let Some(pkg) = lf.get_package(name) else {
        return false;
    };

    println!("{}@{} (installed)\n", name, pkg.version);
    println!("  Type:      {}", pkg.package_type);
    println!("  Integrity: {}", pkg.integrity);
    println!("  Installed: {}", pkg.installed_at);

    if !pkg.targets.is_empty() {
        println!("  Targets:   {}", pkg.targets.join(", "));
    }

    true
}

/// Show info from locally installed packages.
fn show_local_info(name: &str, agent_filter: Option<&str>) -> bool {
    let sanitized = store::sanitize_dir_name(name);

    // Check validator dirs (skip when --agent is set: validators are not agent-scoped)
    if agent_filter.is_none() {
        let local_val = crate::install::validators_dir(false).join(&sanitized);
        if show_package_at(name, &local_val, PackageType::Validator, "local validator") {
            return true;
        }
    }

    // Check skill dirs in target agents (using symlink_name for the lookup)
    if let Ok(config) = agents::load_agents_config() {
        let agents = agents::resolve_target_agents(&config, agent_filter).unwrap_or_default();
        for agent in &agents {
            let link_name = store::symlink_name(&sanitized, &agent.def.symlink_policy);
            let skill_dir = agent_project_skill_dir(&agent.def).join(&link_name);
            let label = format!("local skill, {}", agent.def.name);
            if show_package_at(name, &skill_dir, PackageType::Skill, &label) {
                return true;
            }
        }
    }

    // Also check the central store directly as a fallback
    let store_path = store::skill_store_dir(false).join(&sanitized);
    show_package_at(name, &store_path, PackageType::Skill, "local skill, store")
}

/// Print the package of `package_type` installed at `dir`, and answer whether
/// one was there.
///
/// `label` names where the package was found, and prints in the parentheses
/// after the version.
fn show_package_at(name: &str, dir: &Path, package_type: PackageType, label: &str) -> bool {
    let manifest = dir.join(package_type.manifest_file());
    if !manifest.exists() {
        return false;
    }

    let version = read_frontmatter_field(&manifest, "version");
    let description = read_frontmatter_field(&manifest, "description");

    println!("{}@{} ({})\n", name, version, label);
    println!("  Description: {}", description);
    println!("  Path:        {}", dir.display());

    true
}

/// Show info from the remote registry.
async fn show_registry_info(name: &str) -> Result<(), RegistryError> {
    let client = RegistryClient::new();
    let detail = client.package_info(name).await?;

    let pkg_type = detail.package_type.as_deref().unwrap_or(UNKNOWN);

    println!(
        "{}@{} (registry, {})\n",
        detail.name, detail.latest, pkg_type
    );
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

    println!("\n  Install: mirdan install {}", detail.name);

    Ok(())
}

/// Read a specific field from YAML frontmatter.
///
/// Checks `metadata.<field>` first, then falls back to the top-level field.
///
/// The [`frontmatter`] module makes the split and the parse, so the delimiter
/// rule it documents holds here too.
///
/// Returns [`UNKNOWN`] when the file will not read, when it carries no
/// frontmatter block, or when the field is absent.
fn read_frontmatter_field(path: &Path, field: &str) -> String {
    frontmatter::file_metadata_field(path, field).unwrap_or_else(|| UNKNOWN.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontmatter::fixtures::{
        write_skill_md, NO_CLOSING_DELIMITER, OPENING_LINE_OF_FOUR_HYPHENS,
        OPENING_LINE_WITH_TRAILING_TEXT, THREE_HYPHEN_RUN_IN_DESCRIPTION,
    };
    use serial_test::serial;
    use swissarmyhammer_common::test_utils::CurrentDirGuard;

    #[test]
    fn test_read_frontmatter_field_keeps_every_key_past_a_three_hyphen_run() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_skill_md(dir.path(), THREE_HYPHEN_RUN_IN_DESCRIPTION);

        assert_eq!(read_frontmatter_field(&path, "name"), "test-skill");
        assert_eq!(read_frontmatter_field(&path, "version"), "1.2.3");
    }

    #[test]
    fn test_read_frontmatter_field_rejects_an_opening_line_with_trailing_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_skill_md(dir.path(), OPENING_LINE_WITH_TRAILING_TEXT);

        assert_eq!(read_frontmatter_field(&path, "name"), "unknown");
        assert_eq!(read_frontmatter_field(&path, "description"), "unknown");
    }

    #[test]
    fn test_read_frontmatter_field_rejects_an_opening_line_of_four_hyphens() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_skill_md(dir.path(), OPENING_LINE_OF_FOUR_HYPHENS);

        assert_eq!(read_frontmatter_field(&path, "name"), "unknown");
    }

    #[test]
    fn test_read_frontmatter_field_rejects_a_file_with_no_closing_delimiter() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_skill_md(dir.path(), NO_CLOSING_DELIMITER);

        assert_eq!(read_frontmatter_field(&path, "name"), "unknown");
    }

    #[test]
    fn test_read_frontmatter_field_metadata_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SKILL.md");
        std::fs::write(
            &path,
            r#"---
name: test-skill
metadata:
  version: "3.0.0"
  description: "from metadata"
---
# Test
"#,
        )
        .unwrap();

        assert_eq!(read_frontmatter_field(&path, "version"), "3.0.0");
        assert_eq!(
            read_frontmatter_field(&path, "description"),
            "from metadata"
        );
    }

    #[test]
    #[serial]
    fn test_show_local_info_agent_filter_skips_validators() {
        let dir = tempfile::tempdir().unwrap();
        let _cwd = CurrentDirGuard::new(dir.path()).unwrap();

        // Create a validator
        let val_dir = dir.path().join(".validators/test-val");
        std::fs::create_dir_all(&val_dir).unwrap();
        std::fs::write(
            val_dir.join("VALIDATOR.md"),
            "---\nname: test-val\nmetadata:\n  version: \"1.0.0\"\n---\n# Test\n",
        )
        .unwrap();

        // With agent filter, validator lookup is skipped
        let found = show_local_info("test-val", Some("claude-code"));
        assert!(!found);
    }

    #[test]
    #[serial]
    fn test_show_local_info_no_filter_finds_validator() {
        let dir = tempfile::tempdir().unwrap();
        let _cwd = CurrentDirGuard::new(dir.path()).unwrap();

        // Create a validator
        let val_dir = dir.path().join(".validators/test-val");
        std::fs::create_dir_all(&val_dir).unwrap();
        std::fs::write(
            val_dir.join("VALIDATOR.md"),
            "---\nname: test-val\nmetadata:\n  version: \"1.0.0\"\n---\n# Test\n",
        )
        .unwrap();

        // Without agent filter, validator should be found
        let found = show_local_info("test-val", None);
        assert!(found);
    }
}
