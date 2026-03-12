//! Tauri commands exposing mirdan package operations to the frontend.

use serde::Serialize;
use tracing::{error, info};

/// Serializable package info returned to the frontend.
#[derive(Debug, Serialize)]
pub struct PackageInfo {
    pub name: String,
    /// Lockfile key / source URL — use this for uninstall and update, not `name`.
    pub source: String,
    pub package_type: String,
    pub version: String,
    pub targets: Vec<String>,
    pub store_path: Option<String>,
}

/// List all installed packages.
#[tauri::command]
pub fn list_packages() -> Vec<PackageInfo> {
    let packages = mirdan::list::discover_packages(false, false, false, false, None);

    packages
        .into_iter()
        .map(|p| {
            // Try to find the store path for this package
            let store_path = find_store_path(&p.name);
            PackageInfo {
                name: p.name,
                source: p.source,
                package_type: p.package_type.to_string(),
                version: p.version,
                targets: p.targets,
                store_path,
            }
        })
        .collect()
}

/// Uninstall a package by name.
#[tauri::command]
pub async fn uninstall_package(spec: String) -> Result<String, String> {
    info!(spec, "uninstall requested from GUI");

    // Set CWD to HOME (same as deeplink handler — app bundle CWD is read-only)
    if let Some(home) = std::env::var_os("HOME") {
        let _ = std::env::set_current_dir(&home);
    }

    mirdan::install::run_uninstall(&spec, None, true)
        .await
        .map(|()| format!("Uninstalled {spec}"))
        .map_err(|e| {
            error!(spec, "uninstall failed: {e}");
            e.to_string()
        })
}

/// Update a package (or all packages if spec is empty).
#[tauri::command]
pub async fn update_package(spec: String) -> Result<String, String> {
    info!(spec, "update requested from GUI");

    if let Some(home) = std::env::var_os("HOME") {
        let _ = std::env::set_current_dir(&home);
    }

    let name = if spec.is_empty() { None } else { Some(spec.as_str()) };

    mirdan::outdated::run_update(name, None, true)
        .await
        .map_err(|e| {
            error!(spec, "update failed: {e}");
            e.to_string()
        })
}

/// Get the filesystem path for a package (for "Show in Finder").
#[tauri::command]
pub fn get_package_path(name: String) -> Option<String> {
    find_store_path(&name)
}

/// Get the registry URL for a package (for "Open on mirdan.ai").
#[tauri::command]
pub fn get_registry_url(name: String) -> String {
    mirdan::list::registry_url(&name)
}

/// Open a URL or path using the system default handler.
#[tauri::command]
pub fn open_external(target: String) -> Result<(), String> {
    open::that(&target).map_err(|e| format!("Failed to open {target}: {e}"))
}

/// Find the store path for a package by name.
///
/// Checks both global and project stores, walking recursively to find
/// a directory containing SKILL.md whose frontmatter name matches.
fn find_store_path(name: &str) -> Option<String> {
    let global_store = mirdan::store::skill_store_dir(true);
    if let Some(path) = find_in_store(&global_store, name) {
        return Some(path.to_string_lossy().to_string());
    }

    let project_store = mirdan::store::skill_store_dir(false);
    if let Some(path) = find_in_store(&project_store, name) {
        return Some(path.to_string_lossy().to_string());
    }

    None
}

/// Recursively search a store directory for a skill matching the given name.
fn find_in_store(dir: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.join("SKILL.md").exists() {
                // Check if the directory name or frontmatter name matches
                let dir_name = path.file_name()?.to_string_lossy();
                if dir_name == name {
                    return Some(path);
                }
                // Check frontmatter name
                if let Some(fm_name) = read_frontmatter_name(&path.join("SKILL.md")) {
                    if fm_name == name {
                        return Some(path);
                    }
                }
            } else if let Some(found) = find_in_store(&path, name) {
                return Some(found);
            }
        }
    }

    None
}

/// Read the name field from SKILL.md frontmatter.
fn read_frontmatter_name(path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let content = content.trim();
    let rest = content.strip_prefix("---")?;
    let end = rest.find("---")?;
    let frontmatter = &rest[..end];
    let yaml: serde_yaml::Value = serde_yaml::from_str(frontmatter).ok()?;
    yaml.get("name")?.as_str().map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_info_serializes() {
        let info = PackageInfo {
            name: "test-skill".to_string(),
            source: "https://github.com/owner/repo/test-skill".to_string(),
            package_type: "skill".to_string(),
            version: "1.0.0".to_string(),
            targets: vec!["Claude Code".to_string()],
            store_path: Some("/home/user/.skills/test-skill".to_string()),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"name\":\"test-skill\""));
        assert!(json.contains("\"package_type\":\"skill\""));
    }

    #[test]
    fn test_registry_url_delegates_to_mirdan() {
        // Verifies the command calls through to mirdan::list::registry_url
        let url = get_registry_url("no-secrets".to_string());
        assert!(url.starts_with("https://mirdan.ai/package/"));
        assert!(url.contains("no-secrets"));
    }
}
