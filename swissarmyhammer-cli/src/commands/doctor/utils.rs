//! Utility functions for the doctor module

use super::types::{WorkflowCategory, WorkflowDirectory, WorkflowDirectoryInfo};
use anyhow::Result;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Directory name for SwissArmyHammer configuration and data
pub const SWISSARMYHAMMER_DIR: &str = ".swissarmyhammer";

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
    r#"Add swissarmyhammer to Claude Code using this command:

claude mcp add --scope user sah sah serve

Or if swissarmyhammer is not in your PATH, use the full path for sah
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
        let user_workflows_path = home.join(SWISSARMYHAMMER_DIR).join("workflows");

        // Validate path before adding
        if validate_path_no_traversal(&user_workflows_path).is_ok() {
            dirs.push(WorkflowDirectoryInfo::new(
                WorkflowDirectory::new(user_workflows_path),
                WorkflowCategory::User,
            ));
        }
    }

    // Add local directory
    let local_workflows_path = PathBuf::from(SWISSARMYHAMMER_DIR).join("workflows");

    // Validate path before adding
    if validate_path_no_traversal(&local_workflows_path).is_ok() {
        dirs.push(WorkflowDirectoryInfo::new(
            WorkflowDirectory::new(local_workflows_path),
            WorkflowCategory::Local,
        ));
    }

    dirs
}

/// Get the Claude Code configuration file path based on the OS
///
/// Note: This function is kept for backward compatibility but is no longer used.
/// The doctor command now uses `claude mcp list` instead.
///
/// # Returns
///
/// Platform-specific path to claude_desktop_config.json
#[allow(dead_code)]
pub fn get_claude_config_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join("Library")
            .join("Application Support")
            .join("Claude")
            .join("claude_desktop_config.json")
    }

    #[cfg(target_os = "linux")]
    {
        dirs::config_dir()
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("~"))
                    .join(".config")
            })
            .join("Claude")
            .join("claude_desktop_config.json")
    }

    #[cfg(target_os = "windows")]
    {
        dirs::config_dir()
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var("APPDATA").unwrap_or_else(|_| "~".to_string()))
            })
            .join("Claude")
            .join("claude_desktop_config.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_check_disk_space_with_current_directory() {
        // Test with current directory - should work on all platforms
        let current_dir = std::env::current_dir().expect("Failed to get current directory");
        let result = check_disk_space(&current_dir);

        assert!(
            result.is_ok(),
            "Disk space check should succeed for current directory"
        );

        if let Ok((available, total)) = result {
            // Available should be less than or equal to total
            assert!(
                available.as_mb() <= total.as_mb(),
                "Available space {} should be <= total space {}",
                available,
                total
            );

            // Both values should be greater than 0 for a real filesystem
            assert!(total.as_mb() > 0, "Total space should be greater than 0");

            // Available might be 0 on a full disk, but total should always be positive
            assert!(available.as_mb() <= total.as_mb());
        }
    }

    #[test]
    fn test_check_disk_space_with_root() {
        // Test with root directory - should work on all platforms
        let root_dir = if cfg!(windows) {
            PathBuf::from("C:\\")
        } else {
            PathBuf::from("/")
        };

        let result = check_disk_space(&root_dir);
        assert!(
            result.is_ok(),
            "Disk space check should succeed for root directory"
        );

        if let Ok((available, total)) = result {
            assert!(available.as_mb() <= total.as_mb());
            assert!(total.as_mb() > 0);
        }
    }

    #[test]
    fn test_check_disk_space_with_nonexistent_path() {
        // Test with a path that doesn't exist
        let nonexistent_path = PathBuf::from("/path/that/definitely/does/not/exist/anywhere");
        let result = check_disk_space(&nonexistent_path);

        // This should fail on all platforms
        assert!(
            result.is_err(),
            "Disk space check should fail for nonexistent path"
        );
    }

    #[test]
    fn test_disk_space_type_functionality() {
        // Test DiskSpace type behavior
        let space1 = DiskSpace::from_mb(100);
        let space2 = DiskSpace::from_mb(200);

        assert_eq!(space1.as_mb(), 100);
        assert_eq!(space2.as_mb(), 200);

        assert!(space1.is_low(150));
        assert!(!space2.is_low(150));

        assert!(space1 < space2);
        assert!(space2 > space1);
    }

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
                .contains(SWISSARMYHAMMER_DIR));
        }
    }

    // Platform-specific tests
    #[cfg(unix)]
    #[test]
    fn test_unix_specific_disk_space_methods() {
        let current_dir = std::env::current_dir().expect("Failed to get current directory");

        // Test statvfs method directly
        let statvfs_result = check_disk_space_statvfs(&current_dir);

        // statvfs should work on Unix systems
        if let Ok((available, total)) = statvfs_result {
            assert!(available.as_mb() <= total.as_mb());
            assert!(total.as_mb() > 0);
        }

        // Test df fallback method
        let df_result = check_disk_space_df(&current_dir);

        // df should work on most Unix systems (if df command is available)
        if let Ok((available, total)) = df_result {
            assert!(available.as_mb() <= total.as_mb());
            assert!(total.as_mb() > 0);
        }
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_disk_space() {
        let current_dir = std::env::current_dir().expect("Failed to get current directory");
        let result = check_disk_space(&current_dir);

        // Should work on Windows
        assert!(result.is_ok(), "Windows disk space check should succeed");

        if let Ok((available, total)) = result {
            assert!(available.as_mb() <= total.as_mb());
            assert!(total.as_mb() > 0);
        }
    }

    #[cfg(not(any(unix, windows)))]
    #[test]
    fn test_unsupported_platform_handling() {
        let current_dir = std::env::current_dir().expect("Failed to get current directory");
        let result = check_disk_space(&current_dir);

        // Should return an informative error on unsupported platforms
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("not implemented for this platform"));
        assert!(error_msg.contains("Supported platforms"));
    }
}
