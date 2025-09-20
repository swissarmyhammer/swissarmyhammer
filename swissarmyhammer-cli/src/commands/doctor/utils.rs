//! Utility functions for the doctor module

use super::types::{DiskSpace, WorkflowCategory, WorkflowDirectory, WorkflowDirectoryInfo};
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

/// Check disk space for a given path and return (available, total) as DiskSpace values
#[cfg(unix)]
pub fn check_disk_space(path: &Path) -> Result<(DiskSpace, DiskSpace)> {
    // Try statvfs system call first (more reliable)
    match check_disk_space_statvfs(path) {
        Ok(result) => Ok(result),
        Err(_) => {
            // Fallback to df command if statvfs fails
            check_disk_space_df(path)
        }
    }
}

/// Check disk space using the POSIX statvfs system call on Unix-like systems.
///
/// This function provides direct access to filesystem statistics through the statvfs
/// system call, which is available on all POSIX-compliant systems including Linux,
/// macOS, FreeBSD, and other Unix variants.
///
/// # Arguments
/// * `path` - Path to any file or directory on the filesystem to query
///
/// # Returns
/// * `Ok((available, total))` - Tuple of available and total disk space in DiskSpace format
/// * `Err` - If path is invalid, system call fails, or path encoding is invalid
///
/// # Implementation Notes
/// - Uses `libc::statvfs` for reliable cross-platform Unix compatibility
/// - Calculates space using `f_bavail` (available to unprivileged users) and `f_blocks` (total)
/// - Uses `f_frsize` (fundamental filesystem block size) for accurate byte calculations
/// - Converts results to megabytes for consistency with other disk space functions
#[cfg(unix)]
fn check_disk_space_statvfs(path: &Path) -> Result<(DiskSpace, DiskSpace)> {
    use libc::{c_char, statvfs};
    use std::ffi::CString;

    let path_cstring = CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| anyhow::anyhow!("Invalid path encoding"))?;

    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let result = unsafe { statvfs(path_cstring.as_ptr() as *const c_char, &mut stat) };

    if result != 0 {
        let errno = unsafe { *libc::__error() };
        anyhow::bail!("statvfs system call failed: errno {}", errno);
    }

    // Calculate available and total space in bytes
    let block_size = stat.f_frsize;
    let total_blocks = stat.f_blocks as u64;
    let available_blocks = stat.f_bavail as u64;

    let total_bytes = total_blocks * block_size;
    let available_bytes = available_blocks * block_size;

    let total_mb = total_bytes / (1024 * 1024);
    let available_mb = available_bytes / (1024 * 1024);

    Ok((
        DiskSpace::from_mb(available_mb),
        DiskSpace::from_mb(total_mb),
    ))
}

/// Check disk space using the `df` command as a fallback on Unix-like systems.
///
/// This function serves as a backup method when the statvfs system call fails
/// or is unavailable. It executes the `df` command with the `-k` flag to get
/// disk usage information in kilobytes, then parses the output.
///
/// # Arguments
/// * `path` - Path to any file or directory on the filesystem to query
///
/// # Returns
/// * `Ok((available, total))` - Tuple of available and total disk space in DiskSpace format
/// * `Err` - If df command fails, output cannot be parsed, or path is invalid
///
/// # Implementation Notes
/// - Executes `df -k` command and parses the second line of output
/// - Expected df output format: `Filesystem 1K-blocks Used Available Use% Mounted`
/// - Converts kilobyte values to megabytes for consistency
/// - Less reliable than statvfs but works on most Unix systems where df is available
/// - Used as fallback when statvfs system call is not available or fails
#[cfg(unix)]
fn check_disk_space_df(path: &Path) -> Result<(DiskSpace, DiskSpace)> {
    use std::process::Command;

    // Use df-like approach to check disk space
    let output = Command::new("df")
        .arg("-k") // Output in KB
        .arg(path)
        .output()?;

    if !output.status.success() {
        anyhow::bail!("df command failed");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse df output to get available space
    // Format: Filesystem 1K-blocks Used Available Use% Mounted
    if let Some(line) = stdout.lines().nth(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            let total_kb = parts[1].parse::<u64>().unwrap_or(0);
            let available_kb = parts[3].parse::<u64>().unwrap_or(0);
            let total_mb = total_kb / 1024;
            let available_mb = available_kb / 1024;
            return Ok((
                DiskSpace::from_mb(available_mb),
                DiskSpace::from_mb(total_mb),
            ));
        }
    }

    anyhow::bail!("Failed to parse df output")
}

/// Check disk space for a given path - Windows implementation
#[cfg(windows)]
pub fn check_disk_space(path: &Path) -> Result<(DiskSpace, DiskSpace)> {
    // Windows-specific implementation using WinAPI
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetDiskFreeSpaceExW(
            lpDirectoryName: *const u16,
            lpFreeBytesAvailable: *mut u64,
            lpTotalNumberOfBytes: *mut u64,
            lpTotalNumberOfFreeBytes: *mut u64,
        ) -> i32;
    }

    let path_str = path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid path encoding"))?;
    let wide: Vec<u16> = OsStr::new(path_str)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut free_bytes_available = 0u64;
    let mut total_bytes = 0u64;
    let mut total_free_bytes = 0u64;

    let result = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_bytes_available,
            &mut total_bytes,
            &mut total_free_bytes,
        )
    };

    if result != 0 {
        let available_mb = free_bytes_available / (1024 * 1024);
        let total_mb = total_bytes / (1024 * 1024);
        Ok((
            DiskSpace::from_mb(available_mb),
            DiskSpace::from_mb(total_mb),
        ))
    } else {
        anyhow::bail!("Failed to get disk space information")
    }
}

/// Check disk space for a given path - Non-Unix, Non-Windows implementation
#[cfg(not(any(unix, windows)))]
pub fn check_disk_space(path: &Path) -> Result<(DiskSpace, DiskSpace)> {
    // Verify path exists first
    std::fs::metadata(path)
        .map_err(|e| anyhow::anyhow!("Failed to access path for disk space check: {}", e))?;

    // For truly unsupported platforms, return an informative error
    anyhow::bail!(
        "Disk space checking is not implemented for this platform. \
         Supported platforms: Unix-like systems (Linux, macOS, BSD), Windows"
    )
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
