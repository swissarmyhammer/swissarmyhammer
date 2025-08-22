//! Abort file utility functions
//!
//! This module provides shared utility functions for creating abort files throughout
//! the SwissArmyHammer system. This centralizes the abort file creation logic to
//! eliminate duplication between different components.

use crate::{Result, SwissArmyHammerError};
use std::fs;
use std::path::Path;

/// Create an abort file with the specified reason
///
/// This function creates a `.swissarmyhammer/.abort` file containing the abort reason.
/// It ensures the `.swissarmyhammer` directory exists before creating the file.
///
/// # Arguments
/// * `work_dir` - The working directory where the .swissarmyhammer directory should be created
/// * `reason` - The reason for the abort, which will be written to the file
///
/// # Returns
/// * `Ok(())` if the abort file was created successfully
/// * `Err(SwissArmyHammerError)` if there was an error creating the directory or file
///
/// # Examples
/// ```
/// use swissarmyhammer::common::create_abort_file;
/// use std::path::PathBuf;
///
/// let work_dir = PathBuf::from(".");
/// let result = create_abort_file(&work_dir, "Test abort reason");
/// assert!(result.is_ok());
/// ```
pub fn create_abort_file<P: AsRef<Path>>(work_dir: P, reason: &str) -> Result<()> {
    let work_dir = work_dir.as_ref();

    // Ensure .swissarmyhammer directory exists
    let sah_dir = work_dir.join(".swissarmyhammer");
    if !sah_dir.exists() {
        fs::create_dir_all(&sah_dir).map_err(SwissArmyHammerError::Io)?;
    }

    // Create abort file with reason
    let abort_file_path = sah_dir.join(".abort");
    fs::write(&abort_file_path, reason).map_err(SwissArmyHammerError::Io)?;

    tracing::info!("Created abort file: {}", abort_file_path.display());
    Ok(())
}

/// Create an abort file in the current working directory
///
/// This is a convenience function that calls `create_abort_file` with the current
/// working directory. This function will panic if it cannot create the abort file,
/// as failure to create an abort file in an error condition is a catastrophic system failure.
///
/// # Arguments
/// * `reason` - The reason for the abort, which will be written to the file
///
/// # Panics
/// * Panics if unable to get current working directory or create the abort file
pub fn create_abort_file_current_dir(reason: &str) {
    let current_dir = std::env::current_dir()
        .expect("Failed to get current working directory for abort file creation");
    create_abort_file(&current_dir, reason)
        .expect("Failed to create abort file - this is a catastrophic system failure");
}

/// Check if an abort file exists in the specified directory
///
/// # Arguments
/// * `work_dir` - The working directory to check for the abort file
///
/// # Returns
/// * `true` if the abort file exists, `false` otherwise
pub fn abort_file_exists<P: AsRef<Path>>(work_dir: P) -> bool {
    let abort_file_path = work_dir.as_ref().join(".swissarmyhammer").join(".abort");
    abort_file_path.exists()
}

/// Read the abort file contents if it exists
///
/// # Arguments
/// * `work_dir` - The working directory to check for the abort file
///
/// # Returns
/// * `Ok(Some(contents))` if the abort file exists and can be read
/// * `Ok(None)` if the abort file does not exist
/// * `Err(SwissArmyHammerError)` if there was an error reading the file
pub fn read_abort_file<P: AsRef<Path>>(work_dir: P) -> Result<Option<String>> {
    let abort_file_path = work_dir.as_ref().join(".swissarmyhammer").join(".abort");

    if !abort_file_path.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(&abort_file_path).map_err(SwissArmyHammerError::Io)?;
    Ok(Some(contents))
}

/// Remove the abort file if it exists
///
/// # Arguments
/// * `work_dir` - The working directory where the abort file should be removed
///
/// # Returns
/// * `Ok(true)` if the abort file existed and was removed
/// * `Ok(false)` if the abort file did not exist
/// * `Err(SwissArmyHammerError)` if there was an error removing the file
pub fn remove_abort_file<P: AsRef<Path>>(work_dir: P) -> Result<bool> {
    let abort_file_path = work_dir.as_ref().join(".swissarmyhammer").join(".abort");

    if !abort_file_path.exists() {
        return Ok(false);
    }

    fs::remove_file(&abort_file_path).map_err(SwissArmyHammerError::Io)?;
    tracing::info!("Removed abort file: {}", abort_file_path.display());
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_abort_file_success() {
        let temp_dir = crate::test_utils::create_temp_dir_with_retry();
        let work_dir = temp_dir.path();
        let reason = "Test abort reason";

        let result = create_abort_file(work_dir, reason);
        assert!(result.is_ok());

        let abort_file = work_dir.join(".swissarmyhammer").join(".abort");
        assert!(abort_file.exists());

        let content = std::fs::read_to_string(&abort_file).unwrap();
        assert_eq!(content, reason);
    }

    #[test]
    fn test_create_abort_file_creates_directory() {
        let temp_dir = crate::test_utils::create_temp_dir_with_retry();
        let work_dir = temp_dir.path();

        // Directory shouldn't exist initially
        let sah_dir = work_dir.join(".swissarmyhammer");
        assert!(!sah_dir.exists());

        let result = create_abort_file(work_dir, "test");
        assert!(result.is_ok());

        // Directory should be created
        assert!(sah_dir.exists());
        assert!(sah_dir.is_dir());
    }

    #[test]
    fn test_create_abort_file_existing_directory() {
        let temp_dir = crate::test_utils::create_temp_dir_with_retry();
        let work_dir = temp_dir.path();

        // Pre-create the directory
        let sah_dir = work_dir.join(".swissarmyhammer");
        std::fs::create_dir(&sah_dir).unwrap();

        let result = create_abort_file(work_dir, "test");
        assert!(result.is_ok());

        let abort_file = sah_dir.join(".abort");
        assert!(abort_file.exists());
    }

    #[test]
    fn test_abort_file_exists() {
        let temp_dir = crate::test_utils::create_temp_dir_with_retry();
        let work_dir = temp_dir.path();

        // Initially should not exist
        assert!(!abort_file_exists(work_dir));

        // Create abort file
        create_abort_file(work_dir, "test").unwrap();

        // Should now exist
        assert!(abort_file_exists(work_dir));
    }

    #[test]
    fn test_read_abort_file() {
        let temp_dir = crate::test_utils::create_temp_dir_with_retry();
        let work_dir = temp_dir.path();
        let reason = "Test abort reason";

        // Initially should return None
        let result = read_abort_file(work_dir).unwrap();
        assert!(result.is_none());

        // Create abort file
        create_abort_file(work_dir, reason).unwrap();

        // Should now return the content
        let result = read_abort_file(work_dir).unwrap();
        assert_eq!(result, Some(reason.to_string()));
    }

    #[test]
    fn test_remove_abort_file() {
        let temp_dir = crate::test_utils::create_temp_dir_with_retry();
        let work_dir = temp_dir.path();

        // Initially should return false (file doesn't exist)
        let result = remove_abort_file(work_dir).unwrap();
        assert!(!result);

        // Create abort file
        create_abort_file(work_dir, "test").unwrap();
        assert!(abort_file_exists(work_dir));

        // Remove should return true
        let result = remove_abort_file(work_dir).unwrap();
        assert!(result);
        assert!(!abort_file_exists(work_dir));

        // Second remove should return false
        let result = remove_abort_file(work_dir).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_create_abort_file_unicode() {
        let temp_dir = crate::test_utils::create_temp_dir_with_retry();
        let work_dir = temp_dir.path();
        let reason = "Test abort with émojis 🚫 and ñoñ-ASCII characters 中文";

        let result = create_abort_file(work_dir, reason);
        assert!(result.is_ok());

        let content = read_abort_file(work_dir).unwrap().unwrap();
        assert_eq!(content, reason);
    }

    #[test]
    fn test_create_abort_file_overwrite() {
        let temp_dir = crate::test_utils::create_temp_dir_with_retry();
        let work_dir = temp_dir.path();

        // Create first abort file
        let first_reason = "First reason";
        create_abort_file(work_dir, first_reason).unwrap();

        let content = read_abort_file(work_dir).unwrap().unwrap();
        assert_eq!(content, first_reason);

        // Create second abort file (should overwrite)
        let second_reason = "Second reason";
        create_abort_file(work_dir, second_reason).unwrap();

        let content = read_abort_file(work_dir).unwrap().unwrap();
        assert_eq!(content, second_reason);
    }
}
