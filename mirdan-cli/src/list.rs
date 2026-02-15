//! Mirdan List - List installed skills and validators.

use std::path::Path;

use comfy_table::{presets::UTF8_FULL, Table};

use crate::agents::{self, agent_project_skill_dir};
use crate::package_type::PackageType;
use crate::registry::RegistryError;

/// An installed package found during scanning.
struct InstalledPackage {
    name: String,
    package_type: PackageType,
    version: String,
    targets: Vec<String>,
}

/// Run the list command.
///
/// Scans agent skill directories and .avp/validators/ for installed packages.
pub fn run_list(
    skills_only: bool,
    validators_only: bool,
    agent_filter: Option<&str>,
    json: bool,
) -> Result<(), RegistryError> {
    let mut packages: Vec<InstalledPackage> = Vec::new();

    // Scan skills from agent directories
    if !validators_only {
        if let Ok(config) = agents::load_agents_config() {
            let agents = agents::resolve_target_agents(&config, agent_filter)
                .unwrap_or_default();

            for agent in &agents {
                let skill_dir = agent_project_skill_dir(&agent.def);
                if skill_dir.exists() {
                    scan_skills(&skill_dir, &agent.def.name, &mut packages);
                }
            }
        }
    }

    // Scan validators from .avp/validators/
    // Skip when --agent is set: validators are not agent-scoped
    if !skills_only && agent_filter.is_none() {
        let local_validators = Path::new(".avp/validators");
        if local_validators.exists() {
            scan_validators(local_validators, ".avp/validators/", &mut packages);
        }

        if let Some(home) = dirs::home_dir() {
            let global_validators = home.join(".avp").join("validators");
            if global_validators.exists() {
                scan_validators(&global_validators, "~/.avp/validators/", &mut packages);
            }
        }
    }

    // Deduplicate by name (merge targets)
    let packages = merge_packages(packages);

    if json {
        let entries: Vec<serde_json::Value> = packages
            .iter()
            .map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "type": p.package_type.to_string(),
                    "version": p.version,
                    "targets": p.targets,
                })
            })
            .collect();
        let output = serde_json::json!({ "packages": entries });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
        return Ok(());
    }

    if packages.is_empty() {
        println!("No packages installed.");
        return Ok(());
    }

    println!("Installed Packages:\n");

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Name", "Type", "Version", "Targets"]);

    for pkg in &packages {
        table.add_row(vec![
            pkg.name.clone(),
            pkg.package_type.to_string(),
            pkg.version.clone(),
            pkg.targets.join(", "),
        ]);
    }

    println!("{table}");
    println!("\n{} package(s) installed.", packages.len());

    Ok(())
}

/// Scan a directory for skill packages (subdirs containing SKILL.md).
fn scan_skills(dir: &Path, agent_name: &str, packages: &mut Vec<InstalledPackage>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join("SKILL.md").exists() {
            let name = entry.file_name().to_string_lossy().to_string();
            let version = read_frontmatter_version(&path.join("SKILL.md"));
            packages.push(InstalledPackage {
                name,
                package_type: PackageType::Skill,
                version,
                targets: vec![agent_name.to_string()],
            });
        }
    }
}

/// Scan a directory for validator packages (subdirs containing VALIDATOR.md + rules/).
fn scan_validators(dir: &Path, location: &str, packages: &mut Vec<InstalledPackage>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join("VALIDATOR.md").exists() && path.join("rules").is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            let version = read_frontmatter_version(&path.join("VALIDATOR.md"));
            packages.push(InstalledPackage {
                name,
                package_type: PackageType::Validator,
                version,
                targets: vec![location.to_string()],
            });
        }
    }
}

/// Read version from YAML frontmatter of SKILL.md or VALIDATOR.md.
fn read_frontmatter_version(path: &Path) -> String {
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
        if let Some(version) = yaml.get("version").and_then(|v| v.as_str()) {
            return version.to_string();
        }
    }

    "unknown".to_string()
}

/// Merge packages with the same name (combining targets).
fn merge_packages(packages: Vec<InstalledPackage>) -> Vec<InstalledPackage> {
    let mut merged: Vec<InstalledPackage> = Vec::new();

    for pkg in packages {
        if let Some(existing) = merged.iter_mut().find(|p| p.name == pkg.name) {
            for target in pkg.targets {
                if !existing.targets.contains(&target) {
                    existing.targets.push(target);
                }
            }
        } else {
            merged.push(pkg);
        }
    }

    merged.sort_by(|a, b| a.name.cmp(&b.name));
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_frontmatter_version_skill() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SKILL.md");
        std::fs::write(
            &path,
            r#"---
name: test-skill
version: "1.2.3"
---
# Test
"#,
        )
        .unwrap();

        assert_eq!(read_frontmatter_version(&path), "1.2.3");
    }

    #[test]
    fn test_read_frontmatter_version_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SKILL.md");
        std::fs::write(&path, "# No frontmatter").unwrap();

        assert_eq!(read_frontmatter_version(&path), "unknown");
    }

    #[test]
    fn test_merge_packages() {
        let packages = vec![
            InstalledPackage {
                name: "skill-a".to_string(),
                package_type: PackageType::Skill,
                version: "1.0.0".to_string(),
                targets: vec!["Claude Code".to_string()],
            },
            InstalledPackage {
                name: "skill-a".to_string(),
                package_type: PackageType::Skill,
                version: "1.0.0".to_string(),
                targets: vec!["Cursor".to_string()],
            },
        ];

        let merged = merge_packages(packages);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].targets.len(), 2);
    }

    #[test]
    fn test_run_list_empty() {
        // Should not panic even with no packages
        let result = run_list(false, false, None, true);
        assert!(result.is_ok());
    }
}
