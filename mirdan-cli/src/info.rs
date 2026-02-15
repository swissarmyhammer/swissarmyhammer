//! Mirdan Info - Show detailed information about a package.
//!
//! Checks local installations first, then falls back to the registry.

use std::path::Path;

use crate::agents::{self, agent_project_skill_dir};
use crate::lockfile::Lockfile;
use crate::registry::{RegistryClient, RegistryError};

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

/// Sanitize a package name for use as a filesystem directory name.
///
/// If the name is a URL, strip the scheme and host.
fn sanitize_dir_name(name: &str) -> String {
    if let Some(rest) = name.strip_prefix("https://") {
        if let Some((_host, path)) = rest.split_once('/') {
            return path.to_string();
        }
        return rest.to_string();
    }
    if let Some(rest) = name.strip_prefix("http://") {
        if let Some((_host, path)) = rest.split_once('/') {
            return path.to_string();
        }
        return rest.to_string();
    }
    name.to_string()
}

/// Show info from locally installed packages.
fn show_local_info(name: &str, agent_filter: Option<&str>) -> bool {
    let dir_name = sanitize_dir_name(name);

    // Check validator dirs (skip when --agent is set: validators are not agent-scoped)
    if agent_filter.is_none() {
        let local_val = Path::new(".avp/validators").join(&dir_name);
        if local_val.exists() && local_val.join("VALIDATOR.md").exists() {
            let version = read_frontmatter_field(&local_val.join("VALIDATOR.md"), "version");
            let description =
                read_frontmatter_field(&local_val.join("VALIDATOR.md"), "description");

            println!("{}@{} (local validator)\n", name, version);
            println!("  Description: {}", description);
            println!("  Path:        {}", local_val.display());
            return true;
        }
    }

    // Check skill dirs in target agents
    if let Ok(config) = agents::load_agents_config() {
        let agents = agents::resolve_target_agents(&config, agent_filter)
            .unwrap_or_default();
        for agent in &agents {
            let skill_dir = agent_project_skill_dir(&agent.def).join(&dir_name);
            if skill_dir.exists() && skill_dir.join("SKILL.md").exists() {
                let version = read_frontmatter_field(&skill_dir.join("SKILL.md"), "version");
                let description =
                    read_frontmatter_field(&skill_dir.join("SKILL.md"), "description");

                println!("{}@{} (local skill, {})\n", name, version, agent.def.name);
                println!("  Description: {}", description);
                println!("  Path:        {}", skill_dir.display());
                return true;
            }
        }
    }

    false
}

/// Show info from the remote registry.
async fn show_registry_info(name: &str) -> Result<(), RegistryError> {
    let client = RegistryClient::new();
    let detail = client.package_info(name).await?;

    let pkg_type = detail
        .package_type
        .as_deref()
        .unwrap_or("unknown");

    println!("{}@{} (registry, {})\n", detail.name, detail.latest, pkg_type);
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
/// Checks top-level fields first, then falls back to `metadata.<field>`.
fn read_frontmatter_field(path: &Path, field: &str) -> String {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return "unknown".to_string(),
    };

    let content = content.trim();
    if !content.starts_with("---") {
        return "unknown".to_string();
    }

    let rest = &content[3..];
    let end = match rest.find("---") {
        Some(pos) => pos,
        None => return "unknown".to_string(),
    };

    let frontmatter = &rest[..end];
    if let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(frontmatter) {
        if let Some(value) = yaml
            .get(field)
            .and_then(|v| v.as_str())
            .or_else(|| {
                yaml.get("metadata")
                    .and_then(|m| m.get(field))
                    .and_then(|v| v.as_str())
            })
        {
            return value.to_string();
        }
    }

    "unknown".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(read_frontmatter_field(&path, "description"), "from metadata");
    }

    #[test]
    fn test_show_local_info_agent_filter_skips_validators() {
        let dir = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        // Create a validator
        let val_dir = dir.path().join(".avp/validators/test-val");
        std::fs::create_dir_all(&val_dir).unwrap();
        std::fs::write(
            val_dir.join("VALIDATOR.md"),
            "---\nname: test-val\nversion: \"1.0.0\"\n---\n# Test\n",
        )
        .unwrap();

        // With agent filter, validator lookup is skipped
        let found = show_local_info("test-val", Some("claude-code"));
        assert!(!found);

        std::env::set_current_dir(old_dir).unwrap();
    }

    #[test]
    fn test_show_local_info_no_filter_finds_validator() {
        let dir = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        // Create a validator
        let val_dir = dir.path().join(".avp/validators/test-val");
        std::fs::create_dir_all(&val_dir).unwrap();
        std::fs::write(
            val_dir.join("VALIDATOR.md"),
            "---\nname: test-val\nversion: \"1.0.0\"\n---\n# Test\n",
        )
        .unwrap();

        // Without agent filter, validator should be found
        let found = show_local_info("test-val", None);
        assert!(found);

        std::env::set_current_dir(old_dir).unwrap();
    }
}
