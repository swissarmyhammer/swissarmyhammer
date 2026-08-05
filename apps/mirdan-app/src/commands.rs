//! Tauri commands exposing mirdan package operations to the frontend.

use serde::Serialize;
use tracing::{debug, error, info};

use mirdan::registry::RegistryClient;

/// Maximum number of registry search results returned to the frontend.
const MAX_REGISTRY_SEARCH_RESULTS: usize = 20;

/// Maximum recursion depth for store directory traversal. The store structure
/// is normally `~/.skills/owner/repo/skill/SKILL.md` (3 levels); the margin
/// guards against symlink cycles or unexpectedly deep nesting.
const MAX_STORE_SEARCH_DEPTH: u32 = 5;

/// Format the success message for a completed package action
/// (e.g. `"Installed"`, `"Uninstalled"`).
fn format_action_result(spec: &str, verb: &str) -> String {
    format!("{verb} {spec}")
}

/// Log a failed package action and convert the error to the string the
/// frontend displays. `action` names the operation (e.g. `"install"`).
fn log_and_stringify_error(spec: &str, action: &str, e: impl std::fmt::Display) -> String {
    error!(spec, "{action} failed: {e}");
    e.to_string()
}

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
        .map(|_results| format_action_result(&spec, "Uninstalled"))
        .map_err(|e| log_and_stringify_error(&spec, "uninstall", e))
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
        .map_err(|e| log_and_stringify_error(&spec, "update", e))
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
    let response = client
        .fuzzy_search(&query, Some(MAX_REGISTRY_SEARCH_RESULTS))
        .await
        .map_err(|e| {
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

    mirdan::install::run_install(&spec, None, true, mirdan::install::InstallMode::Auto, None)
        .await
        .map(|_results| format_action_result(&spec, "Installed"))
        .map_err(|e| log_and_stringify_error(&spec, "install", e))
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
    search_store(name, true).or_else(|| search_store(name, false))
}

/// Search one skill store (global or project scope) for `name`, returning
/// the matching directory as a display string.
fn search_store(name: &str, global: bool) -> Option<String> {
    let store = mirdan::store::skill_store_dir(global);
    find_in_store(&store, name, MAX_STORE_SEARCH_DEPTH).map(|p| p.to_string_lossy().to_string())
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
        if !path.is_dir() {
            continue;
        }
        if !path.join("SKILL.md").exists() {
            if let Some(found) = find_in_store(&path, name, max_depth - 1) {
                return Some(found);
            }
            continue;
        }
        if skill_metadata_matches(&path, name) {
            return Some(path);
        }
    }

    None
}

/// Check whether the skill directory at `path` matches `name` — by its
/// directory name or by the `name` field in its SKILL.md frontmatter.
fn skill_metadata_matches(path: &std::path::Path, name: &str) -> bool {
    if path
        .file_name()
        .is_some_and(|dir_name| dir_name.to_string_lossy() == name)
    {
        return true;
    }
    mirdan::list::read_frontmatter_name(&path.join("SKILL.md"))
        .is_some_and(|fm_name| fm_name == name)
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
