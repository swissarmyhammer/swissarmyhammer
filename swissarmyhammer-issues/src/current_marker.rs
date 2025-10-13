//! Current issue marker system
//!
//! This module provides a file-based marker system to track the currently active issue.
//! It replaces the git branch-based detection system with a simple file marker approach.
//!
//! ## Overview
//!
//! The marker system stores the current issue name in a file at `.swissarmyhammer/.current_issue`.
//! This decouples issue tracking from git branch naming conventions, allowing more flexibility
//! in workflow management.
//!
//! ## Usage
//!
//! ```rust
//! use swissarmyhammer_issues::current_marker::{set_current_issue, get_current_issue, clear_current_issue};
//!
//! # fn example() -> Result<(), swissarmyhammer_issues::Error> {
//! // Set the current issue
//! set_current_issue("feature_add_login")?;
//!
//! // Get the current issue
//! let current = get_current_issue()?;
//! assert_eq!(current, Some("feature_add_login".to_string()));
//!
//! // Clear the current issue
//! clear_current_issue()?;
//! assert_eq!(get_current_issue()?, None);
//! # Ok(())
//! # }
//! ```

use crate::error::Result;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::debug;

/// Get the path to the marker file
///
/// Returns the path `.swissarmyhammer/.current_issue` relative to the current directory.
fn marker_file_path() -> PathBuf {
    PathBuf::from(".swissarmyhammer").join(".current_issue")
}

/// Get the path to the marker file in a specific working directory
///
/// Returns the path `.swissarmyhammer/.current_issue` relative to the given directory.
fn marker_file_path_in(work_dir: &Path) -> PathBuf {
    work_dir.join(".swissarmyhammer").join(".current_issue")
}

/// Set the current issue by writing the issue name to the marker file
///
/// This function:
/// - Creates the `.swissarmyhammer/` directory if it doesn't exist
/// - Writes the issue name to `.swissarmyhammer/.current_issue`
/// - Does not add a trailing newline to keep the file clean
///
/// # Arguments
///
/// * `issue_name` - The name of the issue to set as current
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if file operations fail.
///
/// # Examples
///
/// ```rust
/// use swissarmyhammer_issues::current_marker::set_current_issue;
///
/// # fn example() -> Result<(), swissarmyhammer_issues::Error> {
/// set_current_issue("feature_add_authentication")?;
/// # Ok(())
/// # }
/// ```
pub fn set_current_issue(issue_name: &str) -> Result<()> {
    let marker_path = marker_file_path();

    // Create the .swissarmyhammer directory if it doesn't exist
    if let Some(parent) = marker_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Write the issue name to the marker file (no trailing newline)
    fs::write(&marker_path, issue_name)?;

    debug!("Set current issue marker to '{}'", issue_name);
    Ok(())
}

/// Set the current issue in a specific working directory
///
/// This is useful for testing and for operations that need to work with non-current directories.
///
/// # Arguments
///
/// * `issue_name` - The name of the issue to set as current
/// * `work_dir` - The directory in which to set the marker
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if file operations fail.
pub fn set_current_issue_in(issue_name: &str, work_dir: &Path) -> Result<()> {
    let marker_path = marker_file_path_in(work_dir);

    // Create the .swissarmyhammer directory if it doesn't exist
    if let Some(parent) = marker_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Write the issue name to the marker file (no trailing newline)
    fs::write(&marker_path, issue_name)?;

    debug!(
        "Set current issue marker to '{}' in {}",
        issue_name,
        work_dir.display()
    );
    Ok(())
}

/// Get the current issue by reading from the marker file
///
/// This function:
/// - Returns `None` if the marker file doesn't exist (not an error)
/// - Returns `Some(issue_name)` if the file exists and can be read
/// - Returns an error if the file exists but cannot be read or contains invalid UTF-8
///
/// # Returns
///
/// Returns `Ok(Some(String))` with the issue name if a current issue is set,
/// `Ok(None)` if no current issue is set, or an error if reading fails.
///
/// # Examples
///
/// ```rust
/// use swissarmyhammer_issues::current_marker::{get_current_issue, set_current_issue};
///
/// # fn example() -> Result<(), swissarmyhammer_issues::Error> {
/// // No current issue initially
/// assert_eq!(get_current_issue()?, None);
///
/// // Set and get
/// set_current_issue("my_issue")?;
/// assert_eq!(get_current_issue()?, Some("my_issue".to_string()));
/// # Ok(())
/// # }
/// ```
pub fn get_current_issue() -> Result<Option<String>> {
    let marker_path = marker_file_path();

    // If the file doesn't exist, return None (not an error)
    if !marker_path.exists() {
        return Ok(None);
    }

    // Read and return the contents
    let contents = fs::read_to_string(&marker_path)?;
    let issue_name = contents.trim().to_string();

    if issue_name.is_empty() {
        Ok(None)
    } else {
        Ok(Some(issue_name))
    }
}

/// Get the current issue in a specific working directory
///
/// This is useful for testing and for operations that need to work with non-current directories.
///
/// # Arguments
///
/// * `work_dir` - The directory from which to read the marker
///
/// # Returns
///
/// Returns `Ok(Some(String))` with the issue name if a current issue is set,
/// `Ok(None)` if no current issue is set, or an error if reading fails.
pub fn get_current_issue_in(work_dir: &Path) -> Result<Option<String>> {
    let marker_path = marker_file_path_in(work_dir);

    // If the file doesn't exist, return None (not an error)
    if !marker_path.exists() {
        return Ok(None);
    }

    // Read and return the contents
    let contents = fs::read_to_string(&marker_path)?;
    let issue_name = contents.trim().to_string();

    if issue_name.is_empty() {
        Ok(None)
    } else {
        Ok(Some(issue_name))
    }
}

/// Clear the current issue by deleting the marker file
///
/// This function:
/// - Deletes the marker file if it exists
/// - Returns success if the file doesn't exist (idempotent operation)
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if file deletion fails.
///
/// # Examples
///
/// ```rust
/// use swissarmyhammer_issues::current_marker::{set_current_issue, clear_current_issue, get_current_issue};
///
/// # fn example() -> Result<(), swissarmyhammer_issues::Error> {
/// set_current_issue("my_issue")?;
/// clear_current_issue()?;
/// assert_eq!(get_current_issue()?, None);
///
/// // Clearing again is safe (idempotent)
/// clear_current_issue()?;
/// # Ok(())
/// # }
/// ```
pub fn clear_current_issue() -> Result<()> {
    let marker_path = marker_file_path();

    // If the file doesn't exist, nothing to do
    if !marker_path.exists() {
        return Ok(());
    }

    // Remove the marker file
    fs::remove_file(&marker_path)?;

    debug!("Cleared current issue marker");
    Ok(())
}

/// Clear the current issue in a specific working directory
///
/// This is useful for testing and for operations that need to work with non-current directories.
///
/// # Arguments
///
/// * `work_dir` - The directory in which to clear the marker
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if file deletion fails.
pub fn clear_current_issue_in(work_dir: &Path) -> Result<()> {
    let marker_path = marker_file_path_in(work_dir);

    // If the file doesn't exist, nothing to do
    if !marker_path.exists() {
        return Ok(());
    }

    // Remove the marker file
    fs::remove_file(&marker_path)?;

    debug!("Cleared current issue marker in {}", work_dir.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_set_and_get_current_issue() {
        let temp_dir = TempDir::new().unwrap();
        let issue_name = "test_issue_123";

        set_current_issue_in(issue_name, temp_dir.path()).unwrap();
        let result = get_current_issue_in(temp_dir.path()).unwrap();

        assert_eq!(result, Some(issue_name.to_string()));
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let temp_dir = TempDir::new().unwrap();

        let result = get_current_issue_in(temp_dir.path()).unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn test_clear_current_issue() {
        let temp_dir = TempDir::new().unwrap();
        let issue_name = "test_issue_456";

        // Set, verify, clear, verify cleared
        set_current_issue_in(issue_name, temp_dir.path()).unwrap();
        assert_eq!(
            get_current_issue_in(temp_dir.path()).unwrap(),
            Some(issue_name.to_string())
        );

        clear_current_issue_in(temp_dir.path()).unwrap();
        assert_eq!(get_current_issue_in(temp_dir.path()).unwrap(), None);
    }

    #[test]
    fn test_clear_nonexistent_is_safe() {
        let temp_dir = TempDir::new().unwrap();

        // Clearing a non-existent marker should succeed (idempotent)
        clear_current_issue_in(temp_dir.path()).unwrap();
    }

    #[test]
    fn test_directory_auto_creation() {
        let temp_dir = TempDir::new().unwrap();
        let issue_name = "test_issue_789";

        // The .swissarmyhammer directory shouldn't exist yet
        let marker_dir = temp_dir.path().join(".swissarmyhammer");
        assert!(!marker_dir.exists());

        // Setting should auto-create the directory
        set_current_issue_in(issue_name, temp_dir.path()).unwrap();

        // Verify directory was created
        assert!(marker_dir.exists());
        assert!(marker_dir.is_dir());

        // Verify marker file was created
        let marker_file = marker_dir.join(".current_issue");
        assert!(marker_file.exists());
        assert!(marker_file.is_file());
    }

    #[test]
    fn test_overwrites_existing_marker() {
        let temp_dir = TempDir::new().unwrap();

        // Set first issue
        set_current_issue_in("issue_1", temp_dir.path()).unwrap();
        assert_eq!(
            get_current_issue_in(temp_dir.path()).unwrap(),
            Some("issue_1".to_string())
        );

        // Set second issue (should overwrite)
        set_current_issue_in("issue_2", temp_dir.path()).unwrap();
        assert_eq!(
            get_current_issue_in(temp_dir.path()).unwrap(),
            Some("issue_2".to_string())
        );
    }

    #[test]
    fn test_handles_issue_names_with_special_chars() {
        let temp_dir = TempDir::new().unwrap();
        let issue_name = "feature_add-authentication_v2";

        set_current_issue_in(issue_name, temp_dir.path()).unwrap();
        let result = get_current_issue_in(temp_dir.path()).unwrap();

        assert_eq!(result, Some(issue_name.to_string()));
    }

    #[test]
    fn test_empty_file_returns_none() {
        let temp_dir = TempDir::new().unwrap();
        let marker_path = marker_file_path_in(temp_dir.path());

        // Create the directory
        fs::create_dir_all(marker_path.parent().unwrap()).unwrap();

        // Create an empty marker file
        fs::write(&marker_path, "").unwrap();

        // Should return None for empty file
        let result = get_current_issue_in(temp_dir.path()).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_whitespace_only_returns_none() {
        let temp_dir = TempDir::new().unwrap();
        let marker_path = marker_file_path_in(temp_dir.path());

        // Create the directory
        fs::create_dir_all(marker_path.parent().unwrap()).unwrap();

        // Create a marker file with only whitespace
        fs::write(&marker_path, "   \n\t  \n").unwrap();

        // Should return None for whitespace-only file
        let result = get_current_issue_in(temp_dir.path()).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_trims_whitespace() {
        let temp_dir = TempDir::new().unwrap();
        let marker_path = marker_file_path_in(temp_dir.path());

        // Create the directory
        fs::create_dir_all(marker_path.parent().unwrap()).unwrap();

        // Write with surrounding whitespace
        fs::write(&marker_path, "  my_issue  \n").unwrap();

        // Should trim the whitespace
        let result = get_current_issue_in(temp_dir.path()).unwrap();
        assert_eq!(result, Some("my_issue".to_string()));
    }
}
