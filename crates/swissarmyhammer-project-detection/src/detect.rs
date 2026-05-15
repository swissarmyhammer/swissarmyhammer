//! Project detection implementation

use super::types::*;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Maximum depth to traverse when looking for projects
const MAX_DEPTH: usize = 10;

/// Detect all projects starting from a root directory
pub fn detect_projects(
    root: &Path,
    max_depth: Option<usize>,
) -> Result<Vec<DetectedProject>, String> {
    let max_depth = max_depth.unwrap_or(MAX_DEPTH);
    let mut projects = Vec::new();
    let mut visited_dirs = HashSet::new();

    // Canonicalize the root path to avoid duplicates
    let root = root
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize root path: {}", e))?;

    detect_projects_recursive(&root, 0, max_depth, &mut projects, &mut visited_dirs)?;

    // Sort projects by path for consistent output
    projects.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(projects)
}

/// Recursive directory traversal to find projects
fn detect_projects_recursive(
    current: &Path,
    depth: usize,
    max_depth: usize,
    projects: &mut Vec<DetectedProject>,
    visited_dirs: &mut HashSet<PathBuf>,
) -> Result<(), String> {
    // Stop if we've exceeded max depth
    if depth > max_depth {
        return Ok(());
    }

    // Skip if we've already visited this directory
    if !visited_dirs.insert(current.to_path_buf()) {
        return Ok(());
    }

    // Check if this directory contains any project markers.
    // A single directory can match multiple project types (e.g. Cargo.toml + package.json).
    let detected = detect_project_at_path(current)?;
    let should_stop = detected
        .iter()
        .any(|p| should_stop_after_project(&p.project_type));
    projects.extend(detected);

    if should_stop {
        return Ok(());
    }

    // Read directory contents
    let entries = match fs::read_dir(current) {
        Ok(entries) => entries,
        Err(_) => return Ok(()), // Skip directories we can't read
    };

    // Process subdirectories
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();

        // Only process directories
        if !path.is_dir() {
            continue;
        }

        // Skip excluded directories
        if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
            if should_skip_directory(dir_name) {
                continue;
            }
        }

        // Recurse into subdirectory
        detect_projects_recursive(&path, depth + 1, max_depth, projects, visited_dirs)?;
    }

    Ok(())
}

/// Detect all project types present at a specific path.
///
/// A single directory can contain markers for multiple project types
/// (e.g. both `Cargo.toml` and `package.json`). Returns all matches
/// in priority order.
fn detect_project_at_path(path: &Path) -> Result<Vec<DetectedProject>, String> {
    let project_types = [
        ProjectType::Rust,
        ProjectType::NodeJs,
        ProjectType::Go,
        ProjectType::Python,
        ProjectType::JavaMaven,
        ProjectType::JavaGradle,
        ProjectType::CSharp,
        ProjectType::CMake,
        ProjectType::Makefile,
        ProjectType::Flutter,
        ProjectType::Php,
    ];

    let mut detected = Vec::new();
    for project_type in &project_types {
        if let Some(project) = check_project_type(path, *project_type)? {
            detected.push(project);
        }
    }

    Ok(detected)
}

/// Check if a path contains a specific project type
fn check_project_type(
    path: &Path,
    project_type: ProjectType,
) -> Result<Option<DetectedProject>, String> {
    let marker_files = project_type.marker_files();
    let mut found_markers = Vec::new();

    // Check for marker files
    for marker in marker_files {
        if marker.contains('*') {
            // Handle wildcards (e.g., *.csproj)
            if let Some(pattern_match) = find_wildcard_match(path, marker)? {
                found_markers.push(pattern_match);
            }
        } else {
            // Exact file name
            if path.join(marker).exists() {
                found_markers.push(marker.to_string());
            }
        }
    }

    if found_markers.is_empty() {
        return Ok(None);
    }

    // Detect workspace information
    let workspace_info = detect_workspace_info(path, project_type)?;

    Ok(Some(DetectedProject {
        path: path.to_path_buf(),
        project_type,
        marker_files: found_markers,
        workspace_info,
    }))
}

/// Find files matching a wildcard pattern in a directory
fn find_wildcard_match(path: &Path, pattern: &str) -> Result<Option<String>, String> {
    let entries = match fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => return Ok(None),
    };

    let pattern_prefix = pattern.trim_start_matches('*');

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if let Some(file_name) = entry.file_name().to_str() {
            if file_name.ends_with(pattern_prefix) {
                return Ok(Some(file_name.to_string()));
            }
        }
    }

    Ok(None)
}

/// Detect workspace/monorepo information for a project
fn detect_workspace_info(
    path: &Path,
    project_type: ProjectType,
) -> Result<Option<WorkspaceInfo>, String> {
    match project_type {
        ProjectType::Rust => detect_rust_workspace(path),
        ProjectType::NodeJs => detect_npm_workspace(path),
        _ => Ok(None),
    }
}

/// Detect Rust workspace configuration
fn detect_rust_workspace(path: &Path) -> Result<Option<WorkspaceInfo>, String> {
    let cargo_toml_path = path.join("Cargo.toml");
    if !cargo_toml_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&cargo_toml_path)
        .map_err(|e| format!("Failed to read Cargo.toml: {}", e))?;

    // Simple check for [workspace] section
    if content.contains("[workspace]") {
        // Try to extract workspace members
        let members = extract_toml_array(&content, "members");

        return Ok(Some(WorkspaceInfo {
            is_root: true,
            members,
            metadata: None,
        }));
    }

    Ok(None)
}

/// Detect npm workspace configuration
fn detect_npm_workspace(path: &Path) -> Result<Option<WorkspaceInfo>, String> {
    let package_json_path = path.join("package.json");
    if !package_json_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&package_json_path)
        .map_err(|e| format!("Failed to read package.json: {}", e))?;

    // Try to parse as JSON and check for workspaces field
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) {
        if let Some(workspaces) = parsed.get("workspaces") {
            let members = if let Some(arr) = workspaces.as_array() {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            } else if let Some(s) = workspaces.as_str() {
                vec![s.to_string()]
            } else {
                vec![]
            };

            return Ok(Some(WorkspaceInfo {
                is_root: true,
                members,
                metadata: None,
            }));
        }
    }

    Ok(None)
}

/// Extract an array from TOML content (simple parser)
fn extract_toml_array(content: &str, key: &str) -> Vec<String> {
    let mut members = Vec::new();
    let mut in_array = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with(&format!("{} = [", key)) {
            in_array = true;
            // Extract items on same line if any
            if let Some(items) = trimmed.strip_prefix(&format!("{} = [", key)) {
                // Check if the array closes on this same line
                let (items_part, closed) = if let Some(before_close) = items.strip_suffix(']') {
                    (before_close, true)
                } else if items.contains(']') {
                    // closing bracket somewhere in the middle — take everything before it
                    let idx = items.rfind(']').unwrap();
                    (&items[..idx], true)
                } else {
                    (items, false)
                };
                for item in items_part.split(',') {
                    if let Some(cleaned) = clean_toml_string(item) {
                        members.push(cleaned);
                    }
                }
                if closed {
                    in_array = false;
                }
            }
        } else if in_array {
            if trimmed.contains(']') {
                // End of array
                if let Some(items) = trimmed.strip_suffix(']') {
                    for item in items.split(',') {
                        if let Some(cleaned) = clean_toml_string(item) {
                            members.push(cleaned);
                        }
                    }
                }
                break;
            } else {
                // Array item on its own line
                if let Some(cleaned) = clean_toml_string(trimmed) {
                    members.push(cleaned);
                }
            }
        }
    }

    members
}

/// Clean a TOML string value (remove quotes and whitespace)
fn clean_toml_string(s: &str) -> Option<String> {
    let trimmed = s.trim().trim_matches(',').trim();
    if trimmed.is_empty() {
        return None;
    }

    // Remove quotes
    let unquoted = trimmed.trim_matches('"').trim_matches('\'');
    if unquoted.is_empty() {
        None
    } else {
        Some(unquoted.to_string())
    }
}

/// Determine if we should stop traversing after finding this project type
fn should_stop_after_project(_project_type: &ProjectType) -> bool {
    // Don't stop for any project type - we want to find all nested projects
    // This allows us to detect monorepos with multiple project types
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_rust_project() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create Cargo.toml
        fs::write(project_dir.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

        let projects = detect_projects(project_dir, Some(1)).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].project_type, ProjectType::Rust);
        assert!(projects[0].marker_files.contains(&"Cargo.toml".to_string()));
    }

    #[test]
    fn test_detect_nodejs_project() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create package.json
        fs::write(project_dir.join("package.json"), r#"{"name": "test"}"#).unwrap();

        let projects = detect_projects(project_dir, Some(1)).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].project_type, ProjectType::NodeJs);
    }

    #[test]
    fn test_detect_monorepo() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create root Rust workspace
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"backend\"]",
        )
        .unwrap();

        // Create backend subdirectory with Rust project
        let backend = root.join("backend");
        fs::create_dir(&backend).unwrap();
        fs::write(backend.join("Cargo.toml"), "[package]\nname = \"backend\"").unwrap();

        // Create frontend subdirectory with Node.js project
        let frontend = root.join("frontend");
        fs::create_dir(&frontend).unwrap();
        fs::write(frontend.join("package.json"), r#"{"name": "frontend"}"#).unwrap();

        let projects = detect_projects(root, Some(3)).unwrap();

        // Should find: root workspace, backend, frontend
        assert!(
            projects.len() >= 2,
            "Expected at least 2 projects, found {}",
            projects.len()
        );

        // Check we found both Rust and Node.js
        let has_rust = projects.iter().any(|p| p.project_type == ProjectType::Rust);
        let has_nodejs = projects
            .iter()
            .any(|p| p.project_type == ProjectType::NodeJs);
        assert!(has_rust, "Should detect Rust project");
        assert!(has_nodejs, "Should detect Node.js project");
    }

    #[test]
    fn test_detect_npm_workspace_array_form() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create package.json with workspaces as an array
        fs::write(
            project_dir.join("package.json"),
            r#"{"name": "root", "workspaces": ["packages/*", "apps/*"]}"#,
        )
        .unwrap();

        let result = detect_npm_workspace(project_dir).unwrap();
        let info = result.expect("should detect workspace");
        assert!(info.is_root);
        assert_eq!(info.members, vec!["packages/*", "apps/*"]);
    }

    #[test]
    fn test_detect_npm_workspace_string_form() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create package.json with workspaces as a single string
        fs::write(
            project_dir.join("package.json"),
            r#"{"name": "root", "workspaces": "packages/*"}"#,
        )
        .unwrap();

        let result = detect_npm_workspace(project_dir).unwrap();
        let info = result.expect("should detect workspace");
        assert!(info.is_root);
        assert_eq!(info.members, vec!["packages/*"]);
    }

    #[test]
    fn test_detect_npm_workspace_absent_key() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create package.json without workspaces key
        fs::write(
            project_dir.join("package.json"),
            r#"{"name": "root", "version": "1.0.0"}"#,
        )
        .unwrap();

        let result = detect_npm_workspace(project_dir).unwrap();
        assert!(
            result.is_none(),
            "should return None when workspaces absent"
        );
    }

    #[test]
    fn test_detect_npm_workspace_no_package_json() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // No package.json present
        let result = detect_npm_workspace(project_dir).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_toml_array_multiline() {
        let content = r#"
[workspace]
members = [
    "crate-a",
    "crate-b",
    "crate-c",
]
"#;
        let members = extract_toml_array(content, "members");
        assert_eq!(members, vec!["crate-a", "crate-b", "crate-c"]);
    }

    #[test]
    fn test_extract_toml_array_inline() {
        // All items on the same line as the opening bracket
        let content = r#"members = ["crate-a", "crate-b"]"#;
        let members = extract_toml_array(content, "members");
        assert_eq!(members, vec!["crate-a", "crate-b"]);
    }

    #[test]
    fn test_extract_toml_array_items_with_closing_bracket() {
        // Items followed by closing bracket on the same line
        let content = r#"
members = [
    "crate-a", "crate-b"]
"#;
        let members = extract_toml_array(content, "members");
        assert!(
            members.contains(&"crate-a".to_string()),
            "should contain crate-a"
        );
        assert!(
            members.contains(&"crate-b".to_string()),
            "should contain crate-b"
        );
    }

    #[test]
    fn test_extract_toml_array_missing_key() {
        let content = "[package]\nname = \"foo\"";
        let members = extract_toml_array(content, "members");
        assert!(members.is_empty());
    }

    #[test]
    fn test_clean_toml_string_double_quotes() {
        let result = clean_toml_string(r#""crate-a""#);
        assert_eq!(result, Some("crate-a".to_string()));
    }

    #[test]
    fn test_clean_toml_string_single_quotes() {
        let result = clean_toml_string("'crate-a'");
        assert_eq!(result, Some("crate-a".to_string()));
    }

    #[test]
    fn test_clean_toml_string_with_comma() {
        let result = clean_toml_string(r#""crate-a","#);
        assert_eq!(result, Some("crate-a".to_string()));
    }

    #[test]
    fn test_clean_toml_string_empty() {
        let result = clean_toml_string("   ");
        assert!(result.is_none());
    }

    #[test]
    fn test_clean_toml_string_empty_after_cleaning() {
        // A string that is just quotes
        let result = clean_toml_string(r#""""#);
        assert!(result.is_none());
    }

    #[test]
    fn test_skip_node_modules() {
        let temp_dir = TempDir::new().unwrap();
        // On macOS, /var is a symlink to /private/var, so TempDir may return
        // /var/folders/... while canonicalize() resolves to /private/var/folders/...
        // We canonicalize the root path upfront to ensure consistent comparison.
        let root = temp_dir.path().canonicalize().unwrap();

        // Create root package.json
        fs::write(root.join("package.json"), r#"{"name": "root"}"#).unwrap();

        // Create node_modules with a package (should be skipped)
        let node_modules = root.join("node_modules");
        fs::create_dir(&node_modules).unwrap();
        let nested_pkg = node_modules.join("some-package");
        fs::create_dir(&nested_pkg).unwrap();
        fs::write(nested_pkg.join("package.json"), r#"{"name": "nested"}"#).unwrap();

        let projects = detect_projects(&root, Some(3)).unwrap();

        // Should only find root, not the one in node_modules
        assert_eq!(projects.len(), 1);
        // Both paths are now canonicalized, so they should match exactly
        assert_eq!(projects[0].path, root);
    }

    #[test]
    fn test_detect_projects_default_max_depth() {
        // Exercise the `None` path for max_depth (uses MAX_DEPTH constant)
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().canonicalize().unwrap();

        fs::write(root.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

        let projects = detect_projects(&root, None).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].project_type, ProjectType::Rust);
    }

    #[test]
    fn test_detect_projects_max_depth_zero_stops_recursion() {
        // With max_depth=0, only the root directory itself is checked.
        // Subdirectories should not be visited.
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().canonicalize().unwrap();

        // Root has no project marker
        // But a subdirectory does
        let sub = root.join("child");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("Cargo.toml"), "[package]\nname = \"child\"").unwrap();

        let projects = detect_projects(&root, Some(0)).unwrap();
        // depth 0 processes root only; recursion into child is at depth 1 which exceeds max_depth=0
        assert!(
            projects.is_empty(),
            "max_depth=0 should not recurse into subdirectories, found {:?}",
            projects
        );
    }

    #[test]
    fn test_detect_projects_canonicalize_error() {
        // Passing a nonexistent path should trigger the canonicalize error branch
        let result = detect_projects(Path::new("/nonexistent/path/that/does/not/exist"), Some(1));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Failed to canonicalize root path"));
    }

    #[test]
    fn test_detect_csharp_project_wildcard() {
        // Exercise the wildcard matching branch in check_project_type
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().canonicalize().unwrap();

        fs::write(root.join("MyApp.csproj"), "<Project></Project>").unwrap();

        let projects = detect_projects(&root, Some(1)).unwrap();
        assert!(
            projects
                .iter()
                .any(|p| p.project_type == ProjectType::CSharp),
            "Should detect C# project via *.csproj wildcard"
        );
        let csharp = projects
            .iter()
            .find(|p| p.project_type == ProjectType::CSharp)
            .unwrap();
        assert!(
            csharp.marker_files.iter().any(|f| f.ends_with(".csproj")),
            "marker_files should contain the .csproj file"
        );
    }

    #[test]
    fn test_detect_npm_workspace_object_form() {
        // workspaces value that is neither array nor string (e.g. an object)
        // should produce an empty members vec
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        fs::write(
            project_dir.join("package.json"),
            r#"{"name": "root", "workspaces": {"packages": ["a", "b"]}}"#,
        )
        .unwrap();

        let result = detect_npm_workspace(project_dir).unwrap();
        let info = result.expect("should detect workspace even with object form");
        assert!(info.is_root);
        assert!(
            info.members.is_empty(),
            "object-form workspaces should yield empty members, got {:?}",
            info.members
        );
    }

    #[test]
    fn test_extract_toml_array_bracket_in_middle_of_line() {
        // The closing bracket is in the middle of the line (not a suffix),
        // exercising the rfind(']') branch
        let content = "members = [\"a\", \"b\"] # some comment\n";
        let members = extract_toml_array(content, "members");
        assert_eq!(members, vec!["a", "b"]);
    }

    #[test]
    fn test_extract_toml_array_multiline_closing_bracket_suffix() {
        // Multiline array where the closing line ends exactly with `]`
        // and has items on the same line — exercises the strip_suffix(']') branch
        let content = r#"
[workspace]
members = [
    "crate-a",
    "crate-b", "crate-c"]
"#;
        let members = extract_toml_array(content, "members");
        assert!(
            members.contains(&"crate-a".to_string()),
            "should contain crate-a"
        );
        assert!(
            members.contains(&"crate-b".to_string()),
            "should contain crate-b"
        );
        assert!(
            members.contains(&"crate-c".to_string()),
            "should contain crate-c"
        );
    }

    #[test]
    fn test_extract_toml_array_multiline_bracket_not_suffix() {
        // When the closing `]` is present but NOT the last char on the line,
        // strip_suffix fails and the parser breaks out without extracting
        // items on that line. This tests the current behavior.
        let content = r#"
[workspace]
members = [
    "crate-a",
    "crate-b"] # trailing comment
"#;
        let members = extract_toml_array(content, "members");
        // crate-a is on its own line and gets extracted
        assert!(
            members.contains(&"crate-a".to_string()),
            "should contain crate-a"
        );
        // crate-b is on the line with ']' but not as suffix — not extracted
        assert_eq!(members.len(), 1, "only crate-a should be extracted");
    }

    #[cfg(unix)]
    #[test]
    fn test_detect_projects_visited_dirs_skips_symlink_loop() {
        // Exercise the already-visited early return by creating a symlink cycle
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().canonicalize().unwrap();

        fs::write(root.join("Cargo.toml"), "[package]\nname = \"root\"").unwrap();

        // Create a subdirectory with a symlink back to root
        let sub = root.join("subdir");
        fs::create_dir(&sub).unwrap();
        symlink(&root, sub.join("loop_back")).unwrap();

        // Should not infinite loop; the visited set prevents re-visiting root
        let projects = detect_projects(&root, Some(5)).unwrap();
        // Should find at least the root project without getting stuck
        assert!(
            !projects.is_empty(),
            "should detect at least the root project"
        );
    }

    #[test]
    fn test_find_wildcard_match_no_match() {
        // A directory with no matching files should return None
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        fs::write(root.join("readme.md"), "hello").unwrap();

        let result = find_wildcard_match(root, "*.csproj").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_find_wildcard_match_unreadable_dir() {
        // A nonexistent directory should return Ok(None)
        let result = find_wildcard_match(Path::new("/nonexistent/dir"), "*.csproj").unwrap();
        assert!(result.is_none());
    }
}
