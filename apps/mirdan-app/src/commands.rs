//! Tauri commands exposing mirdan package operations to the frontend.

use serde::Serialize;
use tracing::{debug, error, info};

use mirdan::registry::RegistryClient;

/// Serializable package info returned to the frontend.
#[derive(Debug, Serialize)]
pub struct PackageInfo {
    pub name: String,
    /// Lockfile key / source URL — use this for uninstall and update, not `name`.
    pub source: String,
    pub description: String,
    pub package_type: String,
    pub version: String,
    pub targets: Vec<String>,
    pub store_path: Option<String>,
}

/// A registry search result returned to the frontend.
#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub name: String,
    /// Qualified name for install routing (e.g. "owner/repo/skill").
    pub qualified_name: String,
    pub description: String,
    pub author: String,
    pub package_type: String,
    pub downloads: u64,
}

/// List all installed packages.
#[tauri::command]
pub fn list_packages() -> Vec<PackageInfo> {
    let packages = mirdan::list::discover_packages(false, false, false, false, None);

    packages
        .into_iter()
        .map(|p| {
            let store_path = find_store_path(&p.name);
            PackageInfo {
                name: p.name,
                source: p.source,
                description: p.description,
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

    mirdan::install::run_uninstall(&spec, None, true)
        .await
        .map(|_results| format!("Uninstalled {spec}"))
        .map_err(|e| {
            error!(spec, "uninstall failed: {e}");
            e.to_string()
        })
}

/// Update a package (or all packages if spec is empty).
#[tauri::command]
pub async fn update_package(spec: String) -> Result<String, String> {
    info!(spec, "update requested from GUI");

    let name = if spec.is_empty() {
        None
    } else {
        Some(spec.as_str())
    };

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

/// Search the registry for packages.
#[tauri::command]
pub async fn search_registry(query: String) -> Result<Vec<SearchResult>, String> {
    info!(query = %query, "search_registry called from GUI");
    let client = RegistryClient::authenticated().unwrap_or_else(|e| {
        debug!("registry auth failed, falling back to unauthenticated: {e}");
        RegistryClient::default()
    });
    let response = client.fuzzy_search(&query, Some(20)).await.map_err(|e| {
        error!(query = %query, error = %e, "search_registry failed");
        e.to_string()
    })?;

    info!(
        query = %query,
        total = response.total,
        count = response.results.len(),
        "search_registry returned results"
    );

    Ok(response
        .results
        .into_iter()
        .map(|r| {
            let qualified = r.qualified_name.clone().unwrap_or_else(|| r.name.clone());
            SearchResult {
                name: r.name,
                qualified_name: qualified,
                description: r.description,
                author: r.author,
                package_type: r.package_type.unwrap_or_default(),
                downloads: r.downloads,
            }
        })
        .collect())
}

/// Install a package from the registry by name.
#[tauri::command]
pub async fn install_package(spec: String) -> Result<String, String> {
    info!(spec, "install requested from GUI");

    mirdan::install::run_install(&spec, None, true, false, None)
        .await
        .map(|_results| format!("Installed {spec}"))
        .map_err(|e| {
            error!(spec, "install failed: {e}");
            e.to_string()
        })
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
    if let Some(path) = find_in_store(&global_store, name, 5) {
        return Some(path.to_string_lossy().to_string());
    }

    let project_store = mirdan::store::skill_store_dir(false);
    if let Some(path) = find_in_store(&project_store, name, 5) {
        return Some(path.to_string_lossy().to_string());
    }

    None
}

/// Recursively search a store directory for a skill matching the given name.
///
/// `max_depth` guards against symlink cycles or unexpectedly deep nesting.
/// The store structure is normally `~/.skills/owner/repo/skill/SKILL.md` (3 levels).
fn find_in_store(dir: &std::path::Path, name: &str, max_depth: u32) -> Option<std::path::PathBuf> {
    if max_depth == 0 {
        return None;
    }
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
                if let Some(fm_name) = mirdan::list::read_frontmatter_name(&path.join("SKILL.md")) {
                    if fm_name == name {
                        return Some(path);
                    }
                }
            } else if let Some(found) = find_in_store(&path, name, max_depth - 1) {
                return Some(found);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_info_serializes() {
        let info = PackageInfo {
            name: "test-skill".to_string(),
            source: "https://github.com/owner/repo/test-skill".to_string(),
            description: "A test skill".to_string(),
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
