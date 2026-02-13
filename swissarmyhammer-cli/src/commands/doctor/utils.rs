//! Utility functions for the doctor module

use super::types::{WorkflowCategory, WorkflowDirectory, WorkflowDirectoryInfo};
use anyhow::Result;
use std::path::{Path, PathBuf};
use swissarmyhammer_common::SwissarmyhammerDirectory;
use walkdir::WalkDir;

/// Count markdown files in a directory
pub fn count_markdown_files(path: &Path) -> usize {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
        .count()
}

/// Count files with a specific extension in a directory
pub fn count_files_with_extension(path: &Path, extension: &str) -> usize {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some(extension))
        .count()
}

/// Get the Claude add command
pub fn get_claude_add_command() -> String {
    r#"Initialize SwissArmyHammer in your project:

sah init

Or add manually to Claude Code:
claude mcp add --scope user sah sah serve
"#
    .to_string()
}

/// Validate a path doesn't contain directory traversal sequences
pub fn validate_path_no_traversal(path: &Path) -> Result<()> {
    let path_str = path.to_string_lossy();

    // Check for common path traversal patterns
    if path_str.contains("..") || path_str.contains("./") || path_str.contains(".\\") {
        anyhow::bail!("Path contains potential directory traversal: {:?}", path);
    }

    // Check components for any parent directory references
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                anyhow::bail!("Path contains parent directory reference: {:?}", path);
            }
            std::path::Component::RootDir => {
                // Allow absolute paths but log them for review
                // In production, you might want to restrict this based on context
            }
            _ => {} // Normal components are fine
        }
    }

    Ok(())
}

/// Get workflow directories to check
pub fn get_workflow_directories() -> Vec<WorkflowDirectoryInfo> {
    let mut dirs = Vec::new();

    // Add user directory if it exists
    if let Some(home) = dirs::home_dir() {
        let user_workflows_path = home
            .join(SwissarmyhammerDirectory::dir_name())
            .join("workflows");

        // Validate path before adding
        if validate_path_no_traversal(&user_workflows_path).is_ok() {
            dirs.push(WorkflowDirectoryInfo::new(
                WorkflowDirectory::new(user_workflows_path),
                WorkflowCategory::User,
            ));
        }
    }

    // Add local directory
    let local_workflows_path =
        PathBuf::from(SwissarmyhammerDirectory::dir_name()).join("workflows");

    // Validate path before adding
    if validate_path_no_traversal(&local_workflows_path).is_ok() {
        dirs.push(WorkflowDirectoryInfo::new(
            WorkflowDirectory::new(local_workflows_path),
            WorkflowCategory::Local,
        ));
    }

    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_markdown_files() {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create some test files
        std::fs::write(temp_path.join("test1.md"), "# Test 1").unwrap();
        std::fs::write(temp_path.join("test2.md"), "# Test 2").unwrap();
        std::fs::write(temp_path.join("test.txt"), "Not markdown").unwrap();

        let count = count_markdown_files(temp_path);
        assert_eq!(count, 2, "Should find exactly 2 markdown files");
    }

    #[test]
    fn test_count_files_with_extension() {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create some test files
        std::fs::write(temp_path.join("test1.rs"), "fn main() {}").unwrap();
        std::fs::write(temp_path.join("test2.rs"), "fn test() {}").unwrap();
        std::fs::write(temp_path.join("test.txt"), "Not rust").unwrap();

        let count = count_files_with_extension(temp_path, "rs");
        assert_eq!(count, 2, "Should find exactly 2 Rust files");

        let count_txt = count_files_with_extension(temp_path, "txt");
        assert_eq!(count_txt, 1, "Should find exactly 1 text file");
    }

    #[test]
    fn test_validate_path_no_traversal_safe_paths() {
        // Test safe paths
        assert!(validate_path_no_traversal(Path::new("/home/user/documents")).is_ok());
        assert!(validate_path_no_traversal(Path::new("documents/file.txt")).is_ok());
        assert!(validate_path_no_traversal(Path::new("simple_filename.txt")).is_ok());
    }

    #[test]
    fn test_validate_path_no_traversal_unsafe_paths() {
        // Test unsafe paths with traversal sequences
        assert!(validate_path_no_traversal(Path::new("../etc/passwd")).is_err());
        assert!(validate_path_no_traversal(Path::new("documents/../../../etc/passwd")).is_err());
        assert!(validate_path_no_traversal(Path::new("./sensitive/file")).is_err());

        // Test Windows-style traversal
        if cfg!(windows) {
            assert!(validate_path_no_traversal(Path::new("..\\windows\\system32")).is_err());
            assert!(validate_path_no_traversal(Path::new(".\\hidden\\file")).is_err());
        }
    }

    #[test]
    fn test_get_workflow_directories() {
        let dirs = get_workflow_directories();

        // Should return at least one directory (local)
        assert!(
            !dirs.is_empty(),
            "Should return at least one workflow directory"
        );

        // Check that all returned directories have valid categories
        for dir_info in &dirs {
            match dir_info.category {
                WorkflowCategory::User | WorkflowCategory::Local => {
                    // Valid categories
                }
            }

            // Path should contain the SwissArmyHammer directory
            assert!(dir_info
                .path
                .path()
                .to_string_lossy()
                .contains(SwissarmyhammerDirectory::dir_name()));
        }
    }
}
