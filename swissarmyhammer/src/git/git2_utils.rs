//! Git2 utility functions and helpers
//!
//! This module provides utility functions for working with git2-rs,
//! including error conversion, repository operations, and common patterns.

use crate::{Result, SwissArmyHammerError};
use git2::{Repository, RepositoryOpenFlags};
use std::path::Path;
use tracing::{debug, error, info, trace, warn};

// Import CommitInfo from operations module
use crate::git::operations::CommitInfo;

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

/// Convert git2::Error to enhanced SwissArmyHammerError with context
///
/// This function provides enhanced error conversion with user-friendly messages,
/// recovery suggestions, and detailed context information.
pub fn convert_git2_error_with_context(
    operation: &str,
    context: &str,
    error: git2::Error,
) -> SwissArmyHammerError {
    trace!(
        "Converting git2 error with context for operation '{}' in context '{}': {}",
        operation,
        context,
        error
    );
    SwissArmyHammerError::from_git2_with_context(operation, context, error)
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

/// Collect repository state information for error context
pub fn collect_repository_state(repo: &Repository) -> crate::error::RepositoryState {
    let mut state = crate::error::RepositoryState::default();

    // Get current branch and head information
    if let Ok(head) = repo.head() {
        state.head_detached = !head.is_branch();
        state.current_branch = head.shorthand().map(|s| s.to_string());
        if let Some(oid) = head.target() {
            state.head_commit = Some(oid.to_string());
        }
    }

    // Check if repository is empty
    state.repository_empty = repo.is_empty().unwrap_or(false);

    // Get working directory path
    state.workdir_path = repo.workdir().map(|p| p.to_path_buf());

    // Get repository status
    if let Ok(statuses) = repo.statuses(None) {
        state.working_directory_clean = statuses.is_empty();

        for entry in statuses.iter() {
            if let Some(path) = entry.path() {
                let path_str = path.to_string();
                let status = entry.status();

                if status.is_index_new() || status.is_index_modified() || status.is_index_deleted()
                {
                    state.staged_files.push(path_str.clone());
                }

                if status.is_wt_new() || status.is_wt_modified() || status.is_wt_deleted() {
                    state.modified_files.push(path_str);
                }
            }
        }
    }

    state
}

/// Extract git2 crate version from build environment
fn get_git2_version() -> String {
    // Try to get version from Cargo.toml at build time
    const CARGO_TOML: &str = include_str!("../../Cargo.toml");

    // Parse git2 version from Cargo.toml
    for line in CARGO_TOML.lines() {
        if line.trim().starts_with("git2") && line.contains("=") {
            // Extract version from line like: git2 = "0.19"
            if let Some(version_part) = line.split('=').nth(1) {
                let version = version_part.trim().trim_matches('"').trim_matches('\'');
                return format!("git2 {}", version);
            }
        }
    }

    // Fallback to known version if parsing fails
    "git2 0.19.0".to_string()
}

/// Collect environment information for error context
pub fn collect_environment_info(work_dir: &std::path::Path) -> crate::error::EnvironmentInfo {
    crate::error::EnvironmentInfo {
        git2_version: get_git2_version(), // Use dynamic git2 version detection
        working_directory: work_dir.to_path_buf(),
        user_config: collect_user_config(),
        git_config_locations: find_git_config_files(),
    }
}

/// Collect user git configuration
fn collect_user_config() -> Option<crate::error::UserConfig> {
    git2::Config::open_default().ok().map(|config| {
        let name = config.get_string("user.name").ok();
        let email = config.get_string("user.email").ok();
        crate::error::UserConfig { name, email }
    })
}

/// Find git configuration file locations
fn find_git_config_files() -> Vec<std::path::PathBuf> {
    let mut configs = Vec::new();

    // System config
    if let Ok(path) = git2::Config::find_system() {
        configs.push(path);
    }

    // Global config
    if let Ok(path) = git2::Config::find_global() {
        configs.push(path);
    }

    // XDG config
    if let Ok(path) = git2::Config::find_xdg() {
        configs.push(path);
    }

    configs
}

/// Collect system information for error context
pub fn collect_system_info(repo_path: &std::path::Path) -> crate::error::SystemInfo {
    crate::error::SystemInfo {
        platform: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        filesystem_type: detect_filesystem_type(repo_path),
        permissions: check_repository_permissions(repo_path),
    }
}

/// Detect filesystem type (simplified implementation)
fn detect_filesystem_type(_path: &std::path::Path) -> Option<String> {
    // This is a simplified implementation - could be enhanced with platform-specific code
    None
}

/// Check repository permissions
fn check_repository_permissions(repo_path: &std::path::Path) -> crate::error::PermissionInfo {
    use std::fs;

    let repo_metadata = fs::metadata(repo_path);
    let git_dir = repo_path.join(".git");
    let git_metadata = fs::metadata(&git_dir);

    crate::error::PermissionInfo {
        repo_readable: repo_metadata
            .as_ref()
            .map(|m| !m.permissions().readonly())
            .unwrap_or(false),
        repo_writable: repo_metadata.is_ok(),
        git_dir_accessible: git_metadata.is_ok(),
    }
}

/// Execute git2 operation with automatic error recovery
pub fn execute_with_recovery<T, F>(
    operation_name: &str,
    context: &str,
    repo: &Repository,
    operation: F,
) -> Result<T>
where
    F: Fn(&Repository) -> std::result::Result<T, git2::Error>,
{
    match operation(repo) {
        Ok(result) => {
            debug!("Operation '{}' succeeded", operation_name);
            Ok(result)
        }
        Err(error) => {
            warn!("Operation '{}' failed: {}", operation_name, error);

            // Attempt recovery based on error type
            if let Some(()) = attempt_error_recovery(&error, operation_name, repo)? {
                info!(
                    "Recovered from error {}, retrying {}",
                    error, operation_name
                );
                // Retry the operation after recovery
                operation(repo)
                    .map_err(|e| convert_git2_error_with_context(operation_name, context, e))
            } else {
                // Recovery not possible, return enhanced error
                Err(convert_git2_error_with_context(
                    operation_name,
                    context,
                    error,
                ))
            }
        }
    }
}

/// Attempt to recover from common git2 error conditions
fn attempt_error_recovery(
    error: &git2::Error,
    operation: &str,
    repo: &Repository,
) -> Result<Option<()>> {
    match error.code() {
        git2::ErrorCode::Locked => {
            // Attempt to resolve lock file issue
            resolve_repository_lock(repo)?;
            Ok(Some(()))
        }
        git2::ErrorCode::IndexDirty => {
            if operation.starts_with("merge") || operation.starts_with("checkout") {
                // For merge/checkout operations, refresh index and retry
                refresh_repository_index(repo)?;
                Ok(Some(()))
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    }
}

/// Resolve repository lock files
fn resolve_repository_lock(repo: &Repository) -> Result<()> {
    if let Some(git_dir) = repo.path().parent() {
        let lock_file = git_dir.join(".git").join("index.lock");
        if lock_file.exists() {
            // Check if the lock is stale (older than 10 minutes)
            if let Ok(metadata) = std::fs::metadata(&lock_file) {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(elapsed) = modified.elapsed() {
                        if elapsed.as_secs() > 600 {
                            warn!("Removing stale git lock file: {:?}", lock_file);
                            std::fs::remove_file(&lock_file).map_err(|e| {
                                SwissArmyHammerError::Other(format!(
                                    "Failed to remove stale lock file: {}",
                                    e
                                ))
                            })?;
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    Err(SwissArmyHammerError::Other(
        "Repository is locked and lock cannot be safely removed".to_string(),
    ))
}

/// Refresh repository index
fn refresh_repository_index(repo: &Repository) -> Result<()> {
    let mut index = repo
        .index()
        .map_err(|e| convert_git2_error("get index", e))?;

    index
        .read(true)
        .map_err(|e| convert_git2_error("refresh index", e))?;

    Ok(())
}

/// Generate comprehensive error report
pub fn generate_error_report(
    error: &SwissArmyHammerError,
    operation: &str,
    repo: Option<&Repository>,
    work_dir: &std::path::Path,
) -> crate::error::ErrorReport {
    // Collect error context
    let error_context = if let Some(repo) = repo {
        crate::error::GitErrorContext {
            repository_state: collect_repository_state(repo),
            environment_info: collect_environment_info(work_dir),
            operation_history: vec![operation.to_string()], // Could be enhanced with actual history
            system_info: collect_system_info(work_dir),
        }
    } else {
        crate::error::GitErrorContext {
            repository_state: crate::error::RepositoryState::default(),
            environment_info: collect_environment_info(work_dir),
            operation_history: vec![operation.to_string()],
            system_info: collect_system_info(work_dir),
        }
    };

    crate::error::ErrorReport {
        error_id: ulid::Ulid::new().to_string(),
        timestamp: chrono::Utc::now(),
        operation: operation.to_string(),
        error_type: error.error_type(),
        error_message: error.to_string(),
        recovery_suggestion: error.recovery_suggestion(),
        context: serde_json::to_value(&error_context).unwrap_or(serde_json::Value::Null),
        stack_trace: error.stack_trace(),
        environment: crate::error::collect_environment_variables(),
    }
}

/// Save error report to file for later analysis
pub fn save_error_report(
    report: &crate::error::ErrorReport,
    work_dir: &std::path::Path,
) -> Result<std::path::PathBuf> {
    let reports_dir = work_dir.join(".swissarmyhammer").join("error_reports");
    std::fs::create_dir_all(&reports_dir)?;

    let filename = format!("error_report_{}.json", report.error_id);
    let report_path = reports_dir.join(filename);

    let json = serde_json::to_string_pretty(report).map_err(|e| {
        SwissArmyHammerError::Other(format!("Failed to serialize error report: {}", e))
    })?;

    std::fs::write(&report_path, json)?;

    Ok(report_path)
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

    // Enhanced error handling tests

    #[test]
    fn test_convert_git2_error_with_context() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();

        let git_error = git2::Error::from_str("Test error");
        let app_error =
            convert_git2_error_with_context("test_operation", "test_context", git_error);

        let error_msg = app_error.to_string();
        assert!(error_msg.contains("Git2 operation failed: test_operation"));
    }

    #[test]
    fn test_collect_repository_state() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let repo = open_repository(temp_dir.path()).unwrap();

        let state = collect_repository_state(&repo);

        // Repository should be empty initially
        assert!(state.repository_empty);
        assert!(state.working_directory_clean);
        assert!(state.staged_files.is_empty());
        assert!(state.modified_files.is_empty());
        assert!(state.workdir_path.is_some());
    }

    #[test]
    fn test_collect_environment_info() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();

        let env_info = collect_environment_info(temp_dir.path());

        assert!(!env_info.git2_version.is_empty());
        assert_eq!(env_info.working_directory, temp_dir.path());
        // Note: user_config might be None in test environment
        // git_config_locations might be empty in some test environments
        // assert!(!env_info.git_config_locations.is_empty());
    }

    #[test]
    fn test_collect_system_info() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();

        let sys_info = collect_system_info(temp_dir.path());

        assert!(!sys_info.platform.is_empty());
        assert!(!sys_info.arch.is_empty());
        // Repository should be readable and writable in test
        assert!(sys_info.permissions.repo_readable);
        assert!(sys_info.permissions.repo_writable);
        assert!(sys_info.permissions.git_dir_accessible);
    }

    #[test]
    fn test_execute_with_recovery_success() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let repo = open_repository(temp_dir.path()).unwrap();

        let result = execute_with_recovery("test_operation", "test_context", &repo, |_repo| {
            Ok("success")
        });

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[test]
    fn test_execute_with_recovery_failure() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let repo = open_repository(temp_dir.path()).unwrap();

        let result: Result<&str> =
            execute_with_recovery("test_operation", "test_context", &repo, |_repo| {
                Err(git2::Error::from_str("Test error"))
            });

        assert!(result.is_err());
        // Should return enhanced error with context
        let error = result.unwrap_err();
        let error_msg = error.to_string();
        assert!(error_msg.contains("Git2 operation failed"));
    }

    #[test]
    fn test_generate_error_report() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let repo = open_repository(temp_dir.path()).unwrap();

        let error = SwissArmyHammerError::Git2OperationFailed {
            operation: "test_operation".to_string(),
            source: git2::Error::from_str("Test error"),
        };

        let report = generate_error_report(&error, "test_operation", Some(&repo), temp_dir.path());

        assert!(!report.error_id.is_empty());
        assert_eq!(report.operation, "test_operation");
        assert_eq!(report.error_type, "Git2OperationFailed");
        assert!(!report.error_message.is_empty());
        assert!(!report.stack_trace.is_empty());
        assert!(!report.environment.is_empty());
    }

    #[test]
    fn test_save_error_report() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let repo = open_repository(temp_dir.path()).unwrap();

        let error = SwissArmyHammerError::Git2OperationFailed {
            operation: "test_operation".to_string(),
            source: git2::Error::from_str("Test error"),
        };

        let report = generate_error_report(&error, "test_operation", Some(&repo), temp_dir.path());
        let report_path = save_error_report(&report, temp_dir.path()).unwrap();

        assert!(report_path.exists());
        assert!(report_path.extension().unwrap() == "json");

        // Verify the content is valid JSON
        let content = std::fs::read_to_string(&report_path).unwrap();
        let _parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    }

    #[test]
    fn test_refresh_repository_index() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let repo = open_repository(temp_dir.path()).unwrap();

        // This should not fail on a valid repository
        let result = refresh_repository_index(&repo);
        assert!(result.is_ok());
    }

    #[test]
    fn test_resolve_repository_lock_no_lock() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let repo = open_repository(temp_dir.path()).unwrap();

        // Should return error when no lock file exists
        let result = resolve_repository_lock(&repo);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Repository is locked and lock cannot be safely removed"));
    }
}

// ============================================================================
// Git Operations Wrapper Functions
// ============================================================================
//
// This section provides libgit2-based replacements for common shell git commands
// to eliminate subprocess overhead and improve error handling.

/// Git status information for a single file
#[derive(Debug, Clone)]
pub struct GitFileStatus {
    /// Relative path to the file
    pub path: String,
    /// Status flags for the file
    pub status: git2::Status,
    /// Whether the file is staged
    pub is_staged: bool,
    /// Whether the file is modified in working directory
    pub is_modified: bool,
    /// Whether the file is untracked
    pub is_untracked: bool,
    /// Whether the file is deleted
    pub is_deleted: bool,
}

/// Comprehensive git status information
#[derive(Debug, Clone)]
pub struct GitStatus {
    /// List of file statuses
    pub files: Vec<GitFileStatus>,
    /// Whether working directory is clean
    pub is_clean: bool,
    /// Current branch name (None if detached HEAD)
    pub current_branch: Option<String>,
    /// Current HEAD commit hash
    pub head_commit: Option<String>,
    /// Whether repository is empty
    pub is_empty: bool,
}

/// Get comprehensive git status using libgit2
///
/// This replaces `git status` shell commands with native git2 operations.
///
/// # Arguments
///
/// * `repo` - Git repository reference
///
/// # Returns
///
/// * `Ok(GitStatus)` - Complete status information
/// * `Err(SwissArmyHammerError)` - Status check failed
pub fn get_status(repo: &Repository) -> Result<GitStatus> {
    with_git2_logging("get_status", || {
        let statuses = repo
            .statuses(None)
            .map_err(|e| convert_git2_error("get_statuses", e))?;

        let mut files = Vec::new();
        for entry in statuses.iter() {
            if let Some(path) = entry.path() {
                let status = entry.status();
                files.push(GitFileStatus {
                    path: path.to_string(),
                    status,
                    is_staged: status.is_index_new()
                        || status.is_index_modified()
                        || status.is_index_deleted(),
                    is_modified: status.is_wt_modified() || status.is_wt_deleted(),
                    is_untracked: status.is_wt_new(),
                    is_deleted: status.is_wt_deleted() || status.is_index_deleted(),
                });
            }
        }

        let is_clean = statuses.is_empty();

        // Get current branch
        let current_branch = match repo.head() {
            Ok(head) => head.shorthand().map(|s| s.to_string()),
            Err(_) => None,
        };

        // Get HEAD commit
        let head_commit = match repo.head() {
            Ok(head) => head.target().map(|oid| oid.to_string()),
            Err(_) => None,
        };

        let is_empty = repo.is_empty().unwrap_or(false);

        Ok(GitStatus {
            files,
            is_clean,
            current_branch,
            head_commit,
            is_empty,
        })
    })
}

/// Get the commit hash for a reference (replaces `git rev-parse`)
///
/// # Arguments
///
/// * `repo` - Git repository reference
/// * `reference` - Reference name (branch, tag, or commit hash)
///
/// # Returns
///
/// * `Ok(String)` - Full commit hash
/// * `Err(SwissArmyHammerError)` - Reference resolution failed
pub fn rev_parse(repo: &Repository, reference: &str) -> Result<String> {
    with_git2_logging("rev_parse", || {
        let obj = repo
            .revparse_single(reference)
            .map_err(|e| convert_git2_error("revparse_single", e))?;

        Ok(obj.id().to_string())
    })
}

/// Check if one commit is an ancestor of another (replaces `git merge-base --is-ancestor`)
///
/// # Arguments
///
/// * `repo` - Git repository reference
/// * `ancestor` - Potential ancestor commit reference
/// * `descendant` - Potential descendant commit reference
///
/// # Returns
///
/// * `Ok(true)` - ancestor is an ancestor of descendant
/// * `Ok(false)` - ancestor is not an ancestor of descendant  
/// * `Err(SwissArmyHammerError)` - Operation failed
pub fn is_ancestor(repo: &Repository, ancestor: &str, descendant: &str) -> Result<bool> {
    with_git2_logging("is_ancestor", || {
        let ancestor_oid = repo
            .revparse_single(ancestor)
            .map_err(|e| convert_git2_error("revparse ancestor", e))?
            .id();
        let descendant_oid = repo
            .revparse_single(descendant)
            .map_err(|e| convert_git2_error("revparse descendant", e))?
            .id();

        let merge_base = repo
            .merge_base(ancestor_oid, descendant_oid)
            .map_err(|e| convert_git2_error("merge_base", e))?;

        Ok(merge_base == ancestor_oid)
    })
}

/// Create a new branch (replaces `git checkout -b` or `git branch`)
///
/// # Arguments
///
/// * `repo` - Git repository reference
/// * `branch_name` - Name of the new branch
/// * `start_point` - Optional starting commit (defaults to HEAD)
///
/// # Returns
///
/// * `Ok(())` - Branch created successfully
/// * `Err(SwissArmyHammerError)` - Branch creation failed
pub fn create_branch(
    repo: &Repository,
    branch_name: &str,
    start_point: Option<&str>,
) -> Result<()> {
    with_git2_logging("create_branch", || {
        // Get the starting commit
        let target_commit = if let Some(start) = start_point {
            let oid = repo
                .revparse_single(start)
                .map_err(|e| convert_git2_error("revparse start_point", e))?
                .id();
            repo.find_commit(oid)
                .map_err(|e| convert_git2_error("find_commit", e))?
        } else {
            let head = repo.head().map_err(|e| convert_git2_error("get_head", e))?;
            head.peel_to_commit()
                .map_err(|e| convert_git2_error("peel_to_commit", e))?
        };

        // Create the branch
        repo.branch(branch_name, &target_commit, false)
            .map_err(|e| convert_git2_error("create_branch", e))?;

        Ok(())
    })
}

/// Switch to a branch (replaces `git checkout`)
///
/// # Arguments
///
/// * `repo` - Git repository reference
/// * `branch_name` - Name of the branch to switch to
///
/// # Returns
///
/// * `Ok(())` - Branch switched successfully
/// * `Err(SwissArmyHammerError)` - Branch switch failed
pub fn checkout_branch(repo: &Repository, branch_name: &str) -> Result<()> {
    with_git2_logging("checkout_branch", || {
        // Find the branch
        let branch = repo
            .find_branch(branch_name, git2::BranchType::Local)
            .map_err(|e| convert_git2_error("find_branch", e))?;

        let branch_ref = branch.get();
        let _branch_oid = branch_ref.target().ok_or_else(|| {
            SwissArmyHammerError::Other("Branch has no target commit".to_string())
        })?;

        // Set HEAD to the branch
        repo.set_head(&format!("refs/heads/{}", branch_name))
            .map_err(|e| convert_git2_error("set_head", e))?;

        // Checkout the files
        let mut checkout_opts = git2::build::CheckoutBuilder::new();
        checkout_opts.force();

        repo.checkout_head(Some(&mut checkout_opts))
            .map_err(|e| convert_git2_error("checkout_head", e))?;

        Ok(())
    })
}

/// Create and switch to a new branch (replaces `git checkout -b`)
///
/// # Arguments
///
/// * `repo` - Git repository reference
/// * `branch_name` - Name of the new branch
/// * `start_point` - Optional starting commit (defaults to HEAD)
///
/// # Returns
///
/// * `Ok(())` - Branch created and switched successfully
/// * `Err(SwissArmyHammerError)` - Operation failed
pub fn checkout_new_branch(
    repo: &Repository,
    branch_name: &str,
    start_point: Option<&str>,
) -> Result<()> {
    create_branch(repo, branch_name, start_point)?;
    checkout_branch(repo, branch_name)
}

/// Delete a branch (replaces `git branch -d` or `git branch -D`)
///
/// # Arguments
///
/// * `repo` - Git repository reference
/// * `branch_name` - Name of the branch to delete
/// * `force` - Whether to force delete (like -D option)
///
/// # Returns
///
/// * `Ok(())` - Branch deleted successfully
/// * `Err(SwissArmyHammerError)` - Branch deletion failed
pub fn delete_branch(repo: &Repository, branch_name: &str, force: bool) -> Result<()> {
    with_git2_logging("delete_branch", || {
        let mut branch = repo
            .find_branch(branch_name, git2::BranchType::Local)
            .map_err(|e| convert_git2_error("find_branch", e))?;

        if !force {
            // Check if branch is merged (simplified check)
            let head = repo.head().map_err(|e| convert_git2_error("get_head", e))?;
            let head_oid = head
                .target()
                .ok_or_else(|| SwissArmyHammerError::Other("HEAD has no target".to_string()))?;

            let branch_ref = branch.get();
            let branch_oid = branch_ref
                .target()
                .ok_or_else(|| SwissArmyHammerError::Other("Branch has no target".to_string()))?;

            // Simple check: if branch points to HEAD, it's safe to delete
            if branch_oid != head_oid {
                // Check if branch is ancestor of HEAD
                if !is_ancestor(repo, &branch_oid.to_string(), &head_oid.to_string())? {
                    return Err(SwissArmyHammerError::Other(format!(
                        "Branch '{}' is not fully merged",
                        branch_name
                    )));
                }
            }
        }

        branch
            .delete()
            .map_err(|e| convert_git2_error("delete_branch", e))?;

        Ok(())
    })
}

/// Add files to the index (replaces `git add`)
///
/// # Arguments
///
/// * `repo` - Git repository reference
/// * `paths` - Paths to add (empty slice adds all files)
///
/// # Returns
///
/// * `Ok(())` - Files added successfully
/// * `Err(SwissArmyHammerError)` - Add operation failed
pub fn add_files(repo: &Repository, paths: &[&str]) -> Result<()> {
    with_git2_logging("add_files", || {
        let mut index = repo
            .index()
            .map_err(|e| convert_git2_error("get_index", e))?;

        if paths.is_empty() {
            // Add all files (equivalent to `git add .`)
            index
                .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
                .map_err(|e| convert_git2_error("add_all", e))?;
        } else {
            // Add specific paths
            for path in paths {
                index
                    .add_path(std::path::Path::new(path))
                    .map_err(|e| convert_git2_error("add_path", e))?;
            }
        }

        index
            .write()
            .map_err(|e| convert_git2_error("write_index", e))?;

        Ok(())
    })
}

/// Create a commit (replaces `git commit`)
///
/// # Arguments
///
/// * `repo` - Git repository reference
/// * `message` - Commit message
/// * `author_name` - Author name (uses config if None)
/// * `author_email` - Author email (uses config if None)
///
/// # Returns
///
/// * `Ok(String)` - Commit hash
/// * `Err(SwissArmyHammerError)` - Commit creation failed
pub fn create_commit(
    repo: &Repository,
    message: &str,
    author_name: Option<&str>,
    author_email: Option<&str>,
) -> Result<String> {
    with_git2_logging("create_commit", || {
        // Get signature (author and committer)
        let signature = if let (Some(name), Some(email)) = (author_name, author_email) {
            git2::Signature::now(name, email)
                .map_err(|e| convert_git2_error("create_signature", e))?
        } else {
            // Use repository config
            repo.signature()
                .map_err(|e| convert_git2_error("get_signature", e))?
        };

        // Get the index and write tree
        let mut index = repo
            .index()
            .map_err(|e| convert_git2_error("get_index", e))?;
        let tree_oid = index
            .write_tree()
            .map_err(|e| convert_git2_error("write_tree", e))?;
        let tree = repo
            .find_tree(tree_oid)
            .map_err(|e| convert_git2_error("find_tree", e))?;

        // Get parent commit(s)
        let parents: Vec<git2::Commit> = match repo.head() {
            Ok(head) => {
                let commit = head
                    .peel_to_commit()
                    .map_err(|e| convert_git2_error("peel_to_commit", e))?;
                vec![commit]
            }
            Err(_) => Vec::new(), // Initial commit
        };

        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

        // Create the commit
        let commit_oid = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parent_refs,
            )
            .map_err(|e| convert_git2_error("create_commit", e))?;

        Ok(commit_oid.to_string())
    })
}

/// Get git log entries (replaces `git log`)
///
/// # Arguments
///
/// * `repo` - Git repository reference
/// * `max_count` - Maximum number of entries to return (None for all)
/// * `start_ref` - Starting reference (None for HEAD)
///
/// # Returns
///
/// * `Ok(Vec<CommitInfo>)` - List of commit information
/// * `Err(SwissArmyHammerError)` - Log operation failed
pub fn get_log(
    repo: &Repository,
    max_count: Option<usize>,
    start_ref: Option<&str>,
) -> Result<Vec<CommitInfo>> {
    with_git2_logging("get_log", || {
        let mut revwalk = repo
            .revwalk()
            .map_err(|e| convert_git2_error("create_revwalk", e))?;

        // Set starting point
        if let Some(start) = start_ref {
            let oid = repo
                .revparse_single(start)
                .map_err(|e| convert_git2_error("revparse_start", e))?
                .id();
            revwalk
                .push(oid)
                .map_err(|e| convert_git2_error("revwalk_push", e))?;
        } else {
            revwalk
                .push_head()
                .map_err(|e| convert_git2_error("revwalk_push_head", e))?;
        }

        let mut commits = Vec::new();

        for (count, oid) in revwalk.enumerate() {
            if let Some(max) = max_count {
                if count >= max {
                    break;
                }
            }

            let oid = oid.map_err(|e| convert_git2_error("revwalk_next", e))?;
            let commit = repo
                .find_commit(oid)
                .map_err(|e| convert_git2_error("find_commit", e))?;

            let commit_info = CommitInfo {
                hash: oid.to_string(),
                short_hash: oid.to_string()[0..7].to_string(),
                message: commit.message().unwrap_or("").to_string(),
                summary: commit.summary().unwrap_or("").to_string(),
                author_name: commit.author().name().unwrap_or("").to_string(),
                author_email: commit.author().email().unwrap_or("").to_string(),
                committer_name: commit.committer().name().unwrap_or("").to_string(),
                committer_email: commit.committer().email().unwrap_or("").to_string(),
                timestamp: commit.time().seconds(),
                parent_count: commit.parent_count(),
            };

            commits.push(commit_info);
        }

        Ok(commits)
    })
}

/// Simple merge operation (replaces `git merge`)
///
/// This performs a basic merge operation. For complex scenarios,
/// additional merge strategies and conflict resolution may be needed.
///
/// # Arguments
///
/// * `repo` - Git repository reference
/// * `branch_name` - Branch to merge into current branch
/// * `message` - Merge commit message (None for default)
///
/// # Returns
///
/// * `Ok(String)` - Merge commit hash (if merge commit created)
/// * `Err(SwissArmyHammerError)` - Merge operation failed
pub fn merge_branch(repo: &Repository, branch_name: &str, message: Option<&str>) -> Result<String> {
    with_git2_logging("merge_branch", || {
        // Find the branch to merge
        let branch = repo
            .find_branch(branch_name, git2::BranchType::Local)
            .map_err(|e| convert_git2_error("find_branch", e))?;

        let branch_ref = branch.get();
        let branch_oid = branch_ref.target().ok_or_else(|| {
            SwissArmyHammerError::Other("Branch has no target commit".to_string())
        })?;

        // Get the branch commit
        let branch_commit = repo
            .find_commit(branch_oid)
            .map_err(|e| convert_git2_error("find_commit", e))?;

        // Get HEAD commit
        let head = repo.head().map_err(|e| convert_git2_error("get_head", e))?;
        let head_oid = head
            .target()
            .ok_or_else(|| SwissArmyHammerError::Other("HEAD has no target".to_string()))?;
        let head_commit = repo
            .find_commit(head_oid)
            .map_err(|e| convert_git2_error("find_head_commit", e))?;

        // Create annotated commit for merge analysis
        let annotated_commit = repo
            .find_annotated_commit(branch_oid)
            .map_err(|e| convert_git2_error("find_annotated_commit", e))?;

        // Analyze merge
        let analysis = repo
            .merge_analysis(&[&annotated_commit])
            .map_err(|e| convert_git2_error("merge_analysis", e))?;

        if analysis.0.is_fast_forward() {
            // Fast-forward merge
            let mut checkout_opts = git2::build::CheckoutBuilder::new();
            checkout_opts.force();

            repo.checkout_tree(branch_commit.as_object(), Some(&mut checkout_opts))
                .map_err(|e| convert_git2_error("checkout_tree", e))?;

            repo.set_head(&format!(
                "refs/heads/{}",
                head.shorthand().unwrap_or("HEAD")
            ))
            .map_err(|e| convert_git2_error("set_head", e))?;

            Ok(branch_oid.to_string())
        } else if analysis.0.is_normal() {
            // Normal merge - create merge commit
            let signature = repo
                .signature()
                .map_err(|e| convert_git2_error("get_signature", e))?;

            // Merge trees
            let head_tree = head_commit
                .tree()
                .map_err(|e| convert_git2_error("get_head_tree", e))?;
            let branch_tree = branch_commit
                .tree()
                .map_err(|e| convert_git2_error("get_branch_tree", e))?;

            let merge_base = repo
                .merge_base(head_oid, branch_oid)
                .map_err(|e| convert_git2_error("merge_base", e))?;
            let base_commit = repo
                .find_commit(merge_base)
                .map_err(|e| convert_git2_error("find_base_commit", e))?;
            let base_tree = base_commit
                .tree()
                .map_err(|e| convert_git2_error("get_base_tree", e))?;

            let mut index = repo
                .merge_trees(&base_tree, &head_tree, &branch_tree, None)
                .map_err(|e| convert_git2_error("merge_trees", e))?;

            if index.has_conflicts() {
                return Err(SwissArmyHammerError::Other(
                    "Merge conflicts detected - manual resolution required".to_string(),
                ));
            }

            let tree_oid = index
                .write_tree_to(repo)
                .map_err(|e| convert_git2_error("write_tree", e))?;
            let tree = repo
                .find_tree(tree_oid)
                .map_err(|e| convert_git2_error("find_tree", e))?;

            let default_message = format!("Merge branch '{}'", branch_name);
            let merge_message = message.unwrap_or(&default_message);

            let commit_oid = repo
                .commit(
                    Some("HEAD"),
                    &signature,
                    &signature,
                    merge_message,
                    &tree,
                    &[&head_commit, &branch_commit],
                )
                .map_err(|e| convert_git2_error("create_merge_commit", e))?;

            Ok(commit_oid.to_string())
        } else {
            Err(SwissArmyHammerError::Other(
                "Cannot merge - branches are up to date or unrelated".to_string(),
            ))
        }
    })
}

/// Get merge base between two commits (replaces `git merge-base`)
///
/// # Arguments
///
/// * `repo` - Git repository reference
/// * `commit1` - First commit reference
/// * `commit2` - Second commit reference
///
/// # Returns
///
/// * `Ok(String)` - Merge base commit hash
/// * `Err(SwissArmyHammerError)` - Merge base operation failed
pub fn get_merge_base(repo: &Repository, commit1: &str, commit2: &str) -> Result<String> {
    with_git2_logging("get_merge_base", || {
        let oid1 = repo
            .revparse_single(commit1)
            .map_err(|e| convert_git2_error("revparse commit1", e))?
            .id();
        let oid2 = repo
            .revparse_single(commit2)
            .map_err(|e| convert_git2_error("revparse commit2", e))?
            .id();

        let merge_base = repo
            .merge_base(oid1, oid2)
            .map_err(|e| convert_git2_error("merge_base", e))?;

        Ok(merge_base.to_string())
    })
}

/// List branches (replaces `git branch`)
///
/// # Arguments
///
/// * `repo` - Git repository reference
/// * `branch_type` - Type of branches to list (local, remote, or all)
///
/// # Returns
///
/// * `Ok(Vec<String>)` - List of branch names
/// * `Err(SwissArmyHammerError)` - List operation failed
pub fn list_branches(repo: &Repository, branch_type: git2::BranchType) -> Result<Vec<String>> {
    with_git2_logging("list_branches", || {
        let branches = repo
            .branches(Some(branch_type))
            .map_err(|e| convert_git2_error("get_branches", e))?;

        let mut branch_names = Vec::new();
        for branch in branches {
            let (branch, _type) = branch.map_err(|e| convert_git2_error("iterate_branch", e))?;

            if let Some(name) = branch.name().unwrap_or(None) {
                branch_names.push(name.to_string());
            }
        }

        Ok(branch_names)
    })
}

/// Get current branch name (part of status operations)
///
/// # Arguments
///
/// * `repo` - Git repository reference
///
/// # Returns
///
/// * `Ok(Some(String))` - Current branch name
/// * `Ok(None)` - Detached HEAD state
/// * `Err(SwissArmyHammerError)` - Operation failed
pub fn get_current_branch(repo: &Repository) -> Result<Option<String>> {
    with_git2_logging("get_current_branch", || {
        match repo.head() {
            Ok(head) => Ok(head.shorthand().map(|s| s.to_string())),
            Err(e) => {
                if e.code() == git2::ErrorCode::UnbornBranch {
                    // Repository exists but has no commits yet
                    Ok(Some("main".to_string())) // Default branch name
                } else {
                    Err(convert_git2_error("get_head", e))
                }
            }
        }
    })
}

/// Simple push operation (replaces `git push`)
///
/// Note: This is a basic implementation. Complex authentication and
/// remote configurations may require additional setup.
///
/// # Arguments
///
/// * `repo` - Git repository reference
/// * `remote_name` - Name of remote (e.g., "origin")
/// * `refspec` - Refspec to push (e.g., "refs/heads/main:refs/heads/main")
///
/// # Returns
///
/// * `Ok(())` - Push completed successfully
/// * `Err(SwissArmyHammerError)` - Push operation failed
pub fn push_to_remote(repo: &Repository, remote_name: &str, refspec: &str) -> Result<()> {
    with_git2_logging("push_to_remote", || {
        let mut remote = repo
            .find_remote(remote_name)
            .map_err(|e| convert_git2_error("find_remote", e))?;

        let mut callbacks = git2::RemoteCallbacks::new();

        // Basic authentication callback - in real usage, this would need
        // proper credential handling
        callbacks.credentials(|_url, username_from_url, _allowed_types| {
            if let Some(username) = username_from_url {
                git2::Cred::ssh_key_from_agent(username)
            } else {
                git2::Cred::ssh_key_from_agent("git")
            }
        });

        let mut push_options = git2::PushOptions::new();
        push_options.remote_callbacks(callbacks);

        remote
            .push(&[refspec], Some(&mut push_options))
            .map_err(|e| convert_git2_error("push", e))?;

        Ok(())
    })
}

/// Simple fetch operation (replaces `git fetch`)
///
/// # Arguments
///
/// * `repo` - Git repository reference
/// * `remote_name` - Name of remote (e.g., "origin")
/// * `refspecs` - Refspecs to fetch (empty for all)
///
/// # Returns
///
/// * `Ok(())` - Fetch completed successfully
/// * `Err(SwissArmyHammerError)` - Fetch operation failed
pub fn fetch_from_remote(repo: &Repository, remote_name: &str, refspecs: &[&str]) -> Result<()> {
    with_git2_logging("fetch_from_remote", || {
        let mut remote = repo
            .find_remote(remote_name)
            .map_err(|e| convert_git2_error("find_remote", e))?;

        let mut callbacks = git2::RemoteCallbacks::new();

        // Basic authentication callback
        callbacks.credentials(|_url, username_from_url, _allowed_types| {
            if let Some(username) = username_from_url {
                git2::Cred::ssh_key_from_agent(username)
            } else {
                git2::Cred::ssh_key_from_agent("git")
            }
        });

        let mut fetch_options = git2::FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        let refspecs_to_fetch: Vec<String> = if refspecs.is_empty() {
            // Fetch all refspecs configured for this remote
            let fetch_refspecs = remote
                .fetch_refspecs()
                .map_err(|e| convert_git2_error("get_fetch_refspecs", e))?;

            // Convert StringArray to Vec<String>
            let mut specs = Vec::new();
            for i in 0..fetch_refspecs.len() {
                if let Some(spec) = fetch_refspecs.get(i) {
                    specs.push(spec.to_string());
                }
            }
            specs
        } else {
            refspecs.iter().map(|s| s.to_string()).collect()
        };

        // Convert to string references for the fetch call
        let refspec_refs: Vec<&str> = refspecs_to_fetch.iter().map(|s| s.as_str()).collect();
        remote
            .fetch(&refspec_refs, Some(&mut fetch_options), None)
            .map_err(|e| convert_git2_error("fetch", e))?;

        Ok(())
    })
}

/// Check if working directory is clean (no uncommitted changes)
///
/// # Arguments
///
/// * `repo` - Git repository reference
///
/// # Returns
///
/// * `Ok(true)` - Working directory is clean
/// * `Ok(false)` - Working directory has changes
/// * `Err(SwissArmyHammerError)` - Status check failed
pub fn is_working_directory_clean(repo: &Repository) -> Result<bool> {
    let status = get_status(repo)?;
    Ok(status.is_clean)
}

/// Get short commit hash (first 7 characters)
///
/// # Arguments
///
/// * `full_hash` - Full commit hash
///
/// # Returns
///
/// * `String` - Short commit hash
pub fn get_short_hash(full_hash: &str) -> String {
    if full_hash.len() >= 7 {
        full_hash[0..7].to_string()
    } else {
        full_hash.to_string()
    }
}

/// Format commit for display (similar to git log --oneline)
///
/// # Arguments
///
/// * `commit_info` - Commit information
///
/// # Returns
///
/// * `String` - Formatted commit string
pub fn format_commit_oneline(commit_info: &CommitInfo) -> String {
    format!("{} {}", commit_info.short_hash, commit_info.summary)
}
