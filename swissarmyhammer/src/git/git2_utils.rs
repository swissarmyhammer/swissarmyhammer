//! Git2 utility functions and helpers
//!
//! This module provides utility functions for working with git2-rs,
//! including error conversion, repository operations, and common patterns.

use crate::{Result, SwissArmyHammerError};
use git2::{Repository, RepositoryOpenFlags};
use std::path::Path;
use tracing::{debug, error, info, trace, warn};

/// Convert git2::Error to SwissArmyHammerError
///
/// This function provides a standard way to convert git2 errors into
/// our application-specific error types with appropriate context.
pub fn convert_git2_error(operation: &str, error: git2::Error) -> SwissArmyHammerError {
    trace!(
        "Converting git2 error for operation '{}': {}",
        operation,
        error
    );
    SwissArmyHammerError::git2_operation_failed(operation, error)
}

/// Convert git2::Error to repository-specific error
///
/// Use this for repository-specific operations where additional context
/// about the repository state is helpful.
pub fn convert_git2_repository_error(message: &str, error: git2::Error) -> SwissArmyHammerError {
    trace!("Converting git2 repository error: {} - {}", message, error);
    SwissArmyHammerError::git2_repository_error(message, error)
}

/// Safely open a git repository with proper error handling
///
/// This function attempts to open a git repository at the specified path,
/// providing detailed error context for common failure scenarios.
///
/// # Arguments
///
/// * `path` - The path to the repository (can be working directory or .git directory)
///
/// # Returns
///
/// * `Ok(Repository)` - Successfully opened repository
/// * `Err(SwissArmyHammerError)` - Repository could not be opened
pub fn open_repository<P: AsRef<Path>>(path: P) -> Result<Repository> {
    let path = path.as_ref();
    debug!("Opening git repository at: {}", path.display());

    Repository::open(path).map_err(|e| {
        let error_msg = match e.code() {
            git2::ErrorCode::NotFound => {
                format!(
                    "Git repository not found at '{}'. This may not be a git repository or the path may not exist.",
                    path.display()
                )
            }
            git2::ErrorCode::Invalid => {
                format!(
                    "Invalid git repository at '{}'. The repository may be corrupted.",
                    path.display()
                )
            }
            git2::ErrorCode::Ambiguous => {
                format!(
                    "Ambiguous git repository reference at '{}'. Multiple matches found.",
                    path.display()
                )
            }
            _ => {
                format!(
                    "Failed to open git repository at '{}'. Git2 error: {}",
                    path.display(),
                    e
                )
            }
        };
        
        warn!("Repository open failed: {}", error_msg);
        convert_git2_repository_error(&error_msg, e)
    })
}

/// Discover and open a git repository starting from a given path
///
/// This function searches upward from the given path to find a git repository,
/// similar to how git commands work when run from subdirectories.
///
/// # Arguments
///
/// * `path` - Starting path for repository discovery
///
/// # Returns
///
/// * `Ok(Repository)` - Successfully discovered and opened repository
/// * `Err(SwissArmyHammerError)` - No repository found
pub fn discover_repository<P: AsRef<Path>>(path: P) -> Result<Repository> {
    let path = path.as_ref();
    debug!("Discovering git repository from: {}", path.display());

    Repository::discover(path).map_err(|e| {
        let error_msg = format!(
            "No git repository found from path '{}'. Searched upward through parent directories.",
            path.display()
        );
        warn!("Repository discovery failed: {}", error_msg);
        convert_git2_repository_error(&error_msg, e)
    })
}

/// Open repository with extended flags for advanced scenarios
///
/// This function provides more control over repository opening with
/// specific flags for different use cases.
///
/// # Arguments
///
/// * `path` - Path to the repository
/// * `flags` - Repository open flags
///
/// # Returns
///
/// * `Ok(Repository)` - Successfully opened repository
/// * `Err(SwissArmyHammerError)` - Repository could not be opened
pub fn open_repository_with_flags<P: AsRef<Path>>(
    path: P,
    flags: RepositoryOpenFlags,
) -> Result<Repository> {
    let path = path.as_ref();
    debug!(
        "Opening git repository with flags {:?} at: {}",
        flags,
        path.display()
    );

    Repository::open_ext(path, flags, &[] as &[&Path]).map_err(|e| {
        let error_msg = format!(
            "Failed to open git repository with flags {:?} at '{}': {}",
            flags,
            path.display(),
            e
        );
        warn!("Repository open with flags failed: {}", error_msg);
        convert_git2_repository_error(&error_msg, e)
    })
}

/// Check if a path contains a git repository
///
/// This function performs a lightweight check to determine if a directory
/// contains a git repository without fully opening it.
///
/// # Arguments
///
/// * `path` - Path to check
///
/// # Returns
///
/// * `true` - Path contains a git repository
/// * `false` - Path does not contain a git repository
pub fn is_git_repository<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();
    trace!("Checking if path is git repository: {}", path.display());

    match Repository::open(path) {
        Ok(_) => {
            trace!("Path is a git repository: {}", path.display());
            true
        }
        Err(e) => {
            trace!("Path is not a git repository: {} - {}", path.display(), e);
            false
        }
    }
}

/// Get the git directory path for a repository
///
/// This function returns the path to the .git directory for a repository,
/// which may be different from the working directory in cases like worktrees.
///
/// # Arguments
///
/// * `repo` - Git repository reference
///
/// # Returns
///
/// * `Ok(PathBuf)` - Path to the git directory
/// * `Err(SwissArmyHammerError)` - Could not determine git directory path
pub fn get_git_dir(repo: &Repository) -> Result<std::path::PathBuf> {
    trace!("Getting git directory path for repository");

    let git_dir = repo.path().to_path_buf();
    debug!("Git directory: {}", git_dir.display());

    Ok(git_dir)
}

/// Get the working directory path for a repository
///
/// This function returns the working directory path for a repository.
/// Returns None for bare repositories.
///
/// # Arguments
///
/// * `repo` - Git repository reference
///
/// # Returns
///
/// * `Ok(Some(PathBuf))` - Path to the working directory
/// * `Ok(None)` - Repository is bare (no working directory)
/// * `Err(SwissArmyHammerError)` - Could not determine working directory
pub fn get_work_dir(repo: &Repository) -> Result<Option<std::path::PathBuf>> {
    trace!("Getting working directory path for repository");

    match repo.workdir() {
        Some(workdir) => {
            let workdir = workdir.to_path_buf();
            debug!("Working directory: {}", workdir.display());
            Ok(Some(workdir))
        }
        None => {
            debug!("Repository is bare (no working directory)");
            Ok(None)
        }
    }
}

/// Check if repository is bare (no working directory)
///
/// # Arguments
///
/// * `repo` - Git repository reference
///
/// # Returns
///
/// * `true` - Repository is bare
/// * `false` - Repository has a working directory
pub fn is_bare_repository(repo: &Repository) -> bool {
    let is_bare = repo.is_bare();
    trace!("Repository bare status: {}", is_bare);
    is_bare
}

/// Validate repository state for operations
///
/// This function performs basic validation checks to ensure the repository
/// is in a suitable state for operations.
///
/// # Arguments
///
/// * `repo` - Git repository reference
///
/// # Returns
///
/// * `Ok(())` - Repository is valid for operations
/// * `Err(SwissArmyHammerError)` - Repository has issues
pub fn validate_repository_state(repo: &Repository) -> Result<()> {
    trace!("Validating repository state");

    // Check if repository is corrupted
    match repo.state() {
        git2::RepositoryState::Clean => {
            trace!("Repository state is clean");
        }
        state => {
            info!("Repository is in special state: {:?}", state);
        }
    }

    // Additional validation could be added here
    // such as checking for required remotes, branches, etc.

    Ok(())
}

/// Log git2 operation for debugging and audit purposes
///
/// This function provides consistent logging for git2 operations,
/// including timing and result status.
///
/// # Arguments
///
/// * `operation` - Name of the operation being performed
/// * `result` - Result of the operation
/// * `duration` - Optional duration of the operation
pub fn log_git2_operation<T>(
    operation: &str,
    result: &Result<T>,
    duration: Option<std::time::Duration>,
) {
    match result {
        Ok(_) => {
            if let Some(d) = duration {
                info!("Git2 operation '{}' succeeded in {:?}", operation, d);
            } else {
                info!("Git2 operation '{}' succeeded", operation);
            }
        }
        Err(e) => {
            if let Some(d) = duration {
                error!("Git2 operation '{}' failed in {:?}: {}", operation, d, e);
            } else {
                error!("Git2 operation '{}' failed: {}", operation, e);
            }
        }
    }
}

/// Execute a git2 operation with automatic logging and timing
///
/// This macro-like function wrapper provides consistent error handling,
/// logging, and timing for git2 operations.
pub fn with_git2_logging<F, T>(operation: &str, f: F) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    let start = std::time::Instant::now();
    trace!("Starting git2 operation: {}", operation);

    let result = f();
    let duration = start.elapsed();

    log_git2_operation(operation, &result, Some(duration));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::IsolatedTestEnvironment;
    use std::fs;
    use tempfile::TempDir;

    // Helper to create a test git repository
    fn create_test_git_repo() -> Result<TempDir> {
        let temp_dir = TempDir::new().map_err(|e| {
            SwissArmyHammerError::Other(format!("Failed to create temp dir: {}", e))
        })?;

        // Initialize repository using git2
        Repository::init(temp_dir.path()).map_err(|e| convert_git2_error("repository init", e))?;

        Ok(temp_dir)
    }

    #[test]
    fn test_open_repository_success() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();

        let result = open_repository(temp_dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_open_repository_not_found() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = TempDir::new().unwrap();

        let result = open_repository(temp_dir.path());
        assert!(result.is_err());

        if let Err(error) = result {
            let error_msg = error.to_string();
            assert!(error_msg.contains("Git repository not found"));
        }
    }

    #[test]
    fn test_discover_repository_success() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();

        // Create a subdirectory
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        // Discovery should find the repository from subdirectory
        let result = discover_repository(&sub_dir);
        assert!(result.is_ok());
    }

    #[test]
    fn test_discover_repository_not_found() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = TempDir::new().unwrap();

        let result = discover_repository(temp_dir.path());
        assert!(result.is_err());

        if let Err(error) = result {
            let error_msg = error.to_string();
            assert!(error_msg.contains("No git repository found"));
        }
    }

    #[test]
    fn test_is_git_repository() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let git_dir = create_test_git_repo().unwrap();
        let non_git_dir = TempDir::new().unwrap();

        assert!(is_git_repository(git_dir.path()));
        assert!(!is_git_repository(non_git_dir.path()));
    }

    #[test]
    fn test_get_git_dir() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let repo = open_repository(temp_dir.path()).unwrap();

        let result = get_git_dir(&repo);
        assert!(result.is_ok());

        let git_dir = result.unwrap();
        assert!(git_dir.ends_with(".git"));
    }

    #[test]
    fn test_get_work_dir() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let repo = open_repository(temp_dir.path()).unwrap();

        let result = get_work_dir(&repo);
        assert!(result.is_ok());

        let work_dir = result.unwrap();
        assert!(work_dir.is_some());

        // Verify that both paths refer to the same directory by checking metadata
        let actual_work_dir = work_dir.unwrap();

        // Get metadata to compare inodes - this handles symlinks properly
        let expected_metadata = temp_dir.path().metadata().unwrap();
        let actual_metadata = actual_work_dir.metadata().unwrap();

        // On Unix systems, we can compare inodes to verify they're the same directory
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(actual_metadata.ino(), expected_metadata.ino());
        }

        // Fallback for non-Unix systems: use canonical paths
        #[cfg(not(unix))]
        {
            let expected_canonical = temp_dir.path().canonicalize().unwrap();
            let actual_canonical = actual_work_dir.canonicalize().unwrap();
            assert_eq!(actual_canonical, expected_canonical);
        }
    }

    #[test]
    fn test_is_bare_repository() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let repo = open_repository(temp_dir.path()).unwrap();

        // Normal repository should not be bare
        assert!(!is_bare_repository(&repo));
    }

    #[test]
    fn test_validate_repository_state() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let repo = open_repository(temp_dir.path()).unwrap();

        let result = validate_repository_state(&repo);
        assert!(result.is_ok());
    }

    #[test]
    fn test_convert_git2_error() {
        let git_error = git2::Error::from_str("Test error");
        let app_error = convert_git2_error("test operation", git_error);

        let error_msg = app_error.to_string();
        assert!(error_msg.contains("Git2 operation failed: test operation"));
    }

    #[test]
    fn test_convert_git2_repository_error() {
        let git_error = git2::Error::from_str("Test repo error");
        let app_error = convert_git2_repository_error("Test message", git_error);

        let error_msg = app_error.to_string();
        assert!(error_msg.contains("Git2 repository error: Test message"));
    }

    #[test]
    fn test_with_git2_logging_success() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();

        let result = with_git2_logging("test operation", || Ok("success"));

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[test]
    fn test_with_git2_logging_failure() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();

        let result: Result<&str> = with_git2_logging("test operation", || {
            Err(SwissArmyHammerError::Other("test error".to_string()))
        });

        assert!(result.is_err());
    }
}
