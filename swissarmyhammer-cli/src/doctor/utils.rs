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

/// Check disk space for a given path - Windows/non-Unix implementation
#[cfg(not(unix))]
pub fn check_disk_space(path: &Path) -> Result<(DiskSpace, DiskSpace)> {
    #[cfg(windows)]
    {
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

    #[cfg(not(windows))]
    {
        // For other non-Unix systems, try using `statvfs` crate if available
        // Otherwise, return a reasonable estimate with a note about limitations
        match fs::metadata(path) {
            Ok(_) => {
                // Path exists - return conservative estimates that indicate
                // we cannot determine actual disk space
                // Using 0 to indicate unknown rather than misleading values
                Err(anyhow::anyhow!(
                    "Disk space checking not implemented for this platform"
                ))
            }
            Err(e) => {
                anyhow::bail!("Failed to access path for disk space check: {}", e)
            }
        }
    }
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
