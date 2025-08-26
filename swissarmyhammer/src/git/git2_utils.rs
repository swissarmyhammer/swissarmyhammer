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
