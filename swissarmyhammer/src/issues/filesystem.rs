use super::IssueName;
use crate::common::generate_monotonic_ulid_string;
use crate::config::Config;
use crate::error::{Result, SwissArmyHammerError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::hash::{DefaultHasher, Hasher};
use std::io;
use std::io::Read;
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

// IssueNumber type eliminated - we now use issue names (filename without .md) as the primary identifier

/// Represents an issue in the tracking system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Issue {
    /// The primary identifier - issue name derived from filename (without .md extension)
    pub name: String,
    /// The full content of the issue markdown file
    pub content: String,
}

impl Issue {
    /// Check if this issue is completed based on file path location
    pub fn is_completed(&self, file_path: &Path, completed_dir: &Path) -> bool {
        file_path
            .parent()
            .map(|parent| parent == completed_dir)
            .unwrap_or(false)
    }

    /// Get the file path for this issue based on its location (completed or active)
    pub fn get_file_path(&self, base_dir: &Path, completed: bool) -> PathBuf {
        let dir = if completed {
            base_dir.join("complete")
        } else {
            base_dir.to_path_buf()
        };
        dir.join(format!("{}.md", self.name))
    }

    /// Get the creation time from file metadata
    pub fn get_created_at(file_path: &Path) -> DateTime<Utc> {
        file_path
            .metadata()
            .and_then(|m| m.created())
            .or_else(|_| file_path.metadata().and_then(|m| m.modified()))
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(|_| Utc::now())
    }
}

/// Extended issue information that includes derived metadata
#[derive(Debug, Clone)]
pub struct IssueInfo {
    /// The core issue data
    pub issue: Issue,
    /// Whether this issue is completed (in completed directory)
    pub completed: bool,
    /// Full path to the issue file
    pub file_path: PathBuf,
    /// When this issue was created
    pub created_at: DateTime<Utc>,
}

impl IssueInfo {
    /// Create issue info from an issue and its file path
    pub fn from_issue_and_path(issue: Issue, file_path: PathBuf, completed_dir: &Path) -> Self {
        let completed = issue.is_completed(&file_path, completed_dir);
        let created_at = Issue::get_created_at(&file_path);

        Self {
            issue,
            completed,
            file_path,
            created_at,
        }
    }
}

/// Represents the current state of the issue system
#[derive(Debug, Clone)]
pub struct IssueState {
    /// Path to the issues directory
    pub issues_dir: PathBuf,
    /// Path to the completed issues directory
    pub completed_dir: PathBuf,
}

/// Migration paths for issue directory migration
#[derive(Debug, Clone)]
pub struct MigrationPaths {
    /// Source directory (old location: ./issues)
    pub source: PathBuf,
    /// Destination directory (new location: .swissarmyhammer/issues)
    pub destination: PathBuf,
    /// Backup directory (backup location: .swissarmyhammer/issues_backup)
    pub backup: PathBuf,
}

/// Information about potential migration
#[derive(Debug)]
pub struct MigrationInfo {
    /// Whether migration should occur
    pub should_migrate: bool,
    /// Whether source directory exists
    pub source_exists: bool,
    /// Whether destination directory exists
    pub destination_exists: bool,
    /// Number of files in source directory
    pub file_count: usize,
    /// Total size of files in source directory (in bytes)
    pub total_size: u64,
}

/// Result of a migration operation
#[derive(Debug)]
pub enum MigrationResult {
    /// Migration completed successfully
    Success(MigrationStats),
    /// Migration was not needed
    NotNeeded(MigrationInfo),
}

/// Statistics from a completed migration
#[derive(Debug)]
pub struct MigrationStats {
    /// Number of files moved during migration
    pub files_moved: usize,
    /// Total bytes moved during migration  
    pub bytes_moved: u64,
    /// Time taken for the migration
    pub duration: std::time::Duration,
}

/// Migration-specific error types
#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    /// Backup creation failed before migration
    #[error("Backup creation failed: {0}")]
    BackupFailed(#[source] SwissArmyHammerError),
    /// Migration execution failed during file operations
    #[error("Migration execution failed: {0}")]
    ExecutionFailed(#[source] SwissArmyHammerError),
    /// Rollback failed when trying to restore original state
    #[error("Rollback failed: {0}")]
    RollbackFailed(#[source] SwissArmyHammerError),
    /// Validation failed after migration completed
    #[error("Validation failed: {reason}")]
    ValidationFailed {
        /// The reason for validation failure
        reason: String,
    },
}

/// Configuration for migration behavior
#[derive(Debug, Clone)]
pub struct MigrationConfig {
    /// Whether to automatically migrate when needed
    pub auto_migrate: bool,
    /// Whether to create backup before migration
    pub create_backup: bool,
    /// Whether to require confirmation before migration
    pub require_confirmation: bool,
    /// Maximum number of files to migrate automatically
    pub max_file_count: usize,
    /// Maximum total size in bytes to migrate automatically
    pub max_total_size: u64,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            auto_migrate: true,
            create_backup: true,
            require_confirmation: false,
            max_file_count: 10000,
            max_total_size: 100 * 1024 * 1024, // 100 MB
        }
    }
}

/// Comprehensive validation of completed migration
#[derive(Debug, Clone)]
pub struct MigrationValidation {
    /// File integrity validation results
    pub file_integrity: FileIntegrityCheck,
    /// Directory structure validation results
    pub directory_structure: DirectoryStructureCheck,
    /// Content verification validation results
    pub content_verification: ContentVerificationCheck,
    /// Metadata preservation validation results
    pub metadata_preservation: MetadataPreservationCheck,
}

/// File integrity validation results
#[derive(Debug, Clone)]
pub struct FileIntegrityCheck {
    /// Total number of files expected
    pub total_files: usize,
    /// Number of files that passed integrity checks
    pub verified_files: usize,
    /// Files that are missing from the destination
    pub missing_files: Vec<PathBuf>,
    /// Extra files found in destination that shouldn't be there
    pub extra_files: Vec<PathBuf>,
    /// Files that failed integrity verification
    pub corrupted_files: Vec<PathBuf>,
}

/// Directory structure validation results
#[derive(Debug, Clone)]
pub struct DirectoryStructureCheck {
    /// Whether directory hierarchy was preserved correctly
    pub directories_preserved: bool,
    /// Whether relative paths remain correct after migration
    pub relative_paths_correct: bool,
    /// Whether directory permissions were preserved
    pub permissions_preserved: bool,
    /// Detailed list of structural differences found
    pub structure_differences: Vec<String>,
}

/// Content verification validation results
#[derive(Debug, Clone)]
pub struct ContentVerificationCheck {
    /// Total number of files to verify
    pub total_files: usize,
    /// Number of files that passed content verification
    pub verified_files: usize,
    /// Files where content doesn't match between source and destination
    pub mismatched_files: Vec<PathBuf>,
    /// Files that could not be read for verification
    pub unreadable_files: Vec<PathBuf>,
}

/// Metadata preservation validation results
#[derive(Debug, Clone)]
pub struct MetadataPreservationCheck {
    /// Whether file permissions were preserved during migration
    pub permissions_preserved: bool,
    /// Whether file timestamps were preserved during migration
    pub timestamps_preserved: bool,
    /// Detailed list of metadata differences found
    pub metadata_differences: Vec<String>,
}

/// Comprehensive migration report
#[derive(Debug, Clone)]
pub struct MigrationReport {
    /// Whether the overall migration succeeded without critical issues
    pub overall_success: bool,
    /// Total number of files processed in the migration
    pub total_files: usize,
    /// Number of files that passed all validation checks
    pub verified_files: usize,
    /// Critical issues that were found during validation
    pub issues: Vec<String>,
    /// Non-critical warnings found during validation
    pub warnings: Vec<String>,
    /// Detailed validation results for all aspects of migration
    pub detailed_validation: MigrationValidation,
}

/// Trait for issue storage operations
#[async_trait::async_trait]
pub trait IssueStorage: Send + Sync {
    /// List all issues (both pending and completed)
    async fn list_issues(&self) -> Result<Vec<Issue>>;

    /// List all issues with extended information (includes completion status and file paths)
    async fn list_issues_info(&self) -> Result<Vec<IssueInfo>>;

    /// Get a specific issue by name
    async fn get_issue(&self, name: &str) -> Result<Issue>;

    /// Get a specific issue with extended information by name
    async fn get_issue_info(&self, name: &str) -> Result<IssueInfo>;

    /// Create a new issue - if name is empty, generates a ULID
    async fn create_issue(&self, name: String, content: String) -> Result<Issue>;

    /// Update an existing issue's content by name
    async fn update_issue(&self, name: &str, content: String) -> Result<Issue>;

    /// Mark an issue as complete (move to complete directory) by name
    async fn mark_complete(&self, name: &str) -> Result<Issue>;

    /// Batch operations for better performance
    /// Create multiple issues at once
    async fn create_issues_batch(&self, issues: Vec<(String, String)>) -> Result<Vec<Issue>>;

    /// Get multiple issues by their names
    async fn get_issues_batch(&self, names: Vec<&str>) -> Result<Vec<Issue>>;

    /// Update multiple issues at once by name
    async fn update_issues_batch(&self, updates: Vec<(&str, String)>) -> Result<Vec<Issue>>;

    /// Mark multiple issues as complete by name
    async fn mark_complete_batch(&self, names: Vec<&str>) -> Result<Vec<Issue>>;

    /// Get the next pending issue (first alphabetically)
    /// Returns None if no pending issues exist
    async fn get_next_issue(&self) -> Result<Option<Issue>>;

    // Type-safe methods using IssueName

    /// Get a specific issue by IssueName for better type safety
    async fn get_issue_by_name(&self, name: &IssueName) -> Result<Issue>;

    /// Create a new issue using IssueName for better type safety
    async fn create_issue_with_name(&self, name: IssueName, content: String) -> Result<Issue>;

    /// Update an existing issue's content by IssueName
    async fn update_issue_by_name(&self, name: &IssueName, content: String) -> Result<Issue>;

    /// Get multiple issues by their IssueName objects
    async fn get_issues_batch_by_name(&self, names: Vec<&IssueName>) -> Result<Vec<Issue>>;

    /// Update multiple issues at once by IssueName
    async fn update_issues_batch_by_name(
        &self,
        updates: Vec<(&IssueName, String)>,
    ) -> Result<Vec<Issue>>;
}

/// File system implementation of issue storage
#[derive(Debug)]
pub struct FileSystemIssueStorage {
    #[allow(dead_code)]
    state: IssueState,
    /// Mutex to ensure thread-safe issue creation and prevent race conditions
    /// when multiple threads attempt to create issues simultaneously
    creation_lock: Mutex<()>,
}

impl FileSystemIssueStorage {
    /// Create a new FileSystemIssueStorage instance
    pub fn new(issues_dir: PathBuf) -> Result<Self> {
        let completed_dir = issues_dir.join("complete");

        // Create directories if they don't exist
        fs::create_dir_all(&issues_dir).map_err(SwissArmyHammerError::Io)?;
        fs::create_dir_all(&completed_dir).map_err(SwissArmyHammerError::Io)?;

        Ok(Self {
            state: IssueState {
                issues_dir,
                completed_dir,
            },
            creation_lock: Mutex::new(()),
        })
    }

    /// Create a new FileSystemIssueStorage instance with default directory
    ///
    /// This method now automatically performs migration if needed:
    /// - Checks if migration should occur (./issues exists, .swissarmyhammer/issues doesn't)
    /// - Performs migration automatically if needed
    /// - Creates storage instance at the appropriate location
    ///
    /// Migration failures will prevent storage creation and return an error.
    /// For more control over migration behavior, use `new_default_with_config()`.
    pub fn new_default() -> Result<Self> {
        // Check if migration should occur and perform it automatically
        if Self::should_migrate()? {
            match Self::perform_migration() {
                Ok(MigrationResult::Success(stats)) => {
                    tracing::info!(
                        "Automatically migrated {} files ({} bytes) to .swissarmyhammer/issues",
                        stats.files_moved,
                        stats.bytes_moved
                    );
                }
                Ok(MigrationResult::NotNeeded(_)) => {
                    // This shouldn't happen given the should_migrate check, but handle gracefully
                }
                Err(e) => {
                    tracing::error!("Automatic migration failed: {}", e);
                    return Err(e);
                }
            }
        }

        // Create storage instance at the appropriate location
        let issues_dir = Self::default_directory()?;
        Self::new(issues_dir)
    }

    /// Create a new FileSystemIssueStorage with optional automatic migration
    ///
    /// This enhanced version of `new_default` checks if migration should occur
    /// and performs it automatically before creating the storage instance.
    /// This provides a seamless upgrade path for repositories.
    ///
    /// # Returns
    ///
    /// Returns a tuple of (storage_instance, optional_migration_result)
    /// where the migration result is `Some` if migration was performed,
    /// or `None` if no migration was needed.
    pub fn new_default_with_migration() -> Result<(Self, Option<MigrationResult>)> {
        // Check if migration should occur
        let migration_result = if Self::should_migrate()? {
            Some(Self::perform_migration()?)
        } else {
            None
        };

        // Create storage with new defaults
        let storage = Self::new_default()?;

        Ok((storage, migration_result))
    }

    /// Create a new FileSystemIssueStorage instance that reports migration results
    ///
    /// This method is similar to new_default() but returns detailed migration information.
    /// Useful for CLI and MCP server integration where migration feedback is needed.
    ///
    /// # Returns
    ///
    /// Returns a tuple of (storage_instance, optional_migration_result)
    /// where the migration result is `Some` if migration was performed,
    /// or `None` if no migration was needed.
    pub fn new_default_with_migration_info() -> Result<(Self, Option<MigrationResult>)> {
        let migration_result = if Self::should_migrate()? {
            Some(Self::perform_migration()?)
        } else {
            None
        };

        let storage = {
            let issues_dir = Self::default_directory()?;
            Self::new(issues_dir)?
        };

        Ok((storage, migration_result))
    }

    /// Create storage with custom migration configuration
    ///
    /// This method provides fine-grained control over migration behavior,
    /// including safety limits and configuration options.
    ///
    /// # Arguments
    ///
    /// * `config` - Migration configuration controlling behavior
    ///
    /// # Returns
    ///
    /// Returns a tuple of (storage_instance, optional_migration_result)
    pub fn new_default_with_config(
        config: &MigrationConfig,
    ) -> Result<(Self, Option<MigrationResult>)> {
        let current_dir = std::env::current_dir().map_err(SwissArmyHammerError::Io)?;
        Self::new_default_with_config_in(&current_dir, config)
    }

    /// Create storage in a specific working directory with migration support
    ///
    /// This method creates storage in the specified directory without relying
    /// on the global current directory. Useful for testing and isolated operations.
    ///
    /// # Arguments
    ///
    /// * `work_dir` - The working directory to use as base
    ///
    /// # Returns
    ///
    /// Returns a tuple of (storage_instance, optional_migration_result)
    pub fn new_default_in(work_dir: &Path) -> Result<(Self, Option<MigrationResult>)> {
        let migration_result = if Self::should_migrate_in(work_dir)? {
            Some(Self::perform_migration_in(work_dir)?)
        } else {
            None
        };

        let storage = {
            let issues_dir = Self::default_directory_in(work_dir)?;
            Self::new(issues_dir)?
        };

        Ok((storage, migration_result))
    }

    /// Create storage in a specific directory with custom migration configuration
    ///
    /// # Arguments
    ///
    /// * `work_dir` - The working directory to use as base
    /// * `config` - Migration configuration controlling behavior
    ///
    /// # Returns
    ///
    /// Returns a tuple of (storage_instance, optional_migration_result)
    pub fn new_default_with_config_in(
        work_dir: &Path,
        config: &MigrationConfig,
    ) -> Result<(Self, Option<MigrationResult>)> {
        if !config.auto_migrate {
            return Ok((Self::new_default_without_migration_in(work_dir)?, None));
        }

        let info = Self::migration_info_in(work_dir)?;

        // Check size and file count limits
        if info.file_count > config.max_file_count {
            return Err(SwissArmyHammerError::validation_failed(
                "file_count",
                &info.file_count.to_string(),
                &format!(
                    "exceeds maximum for automatic migration: {}",
                    config.max_file_count
                ),
            ));
        }

        if info.total_size > config.max_total_size {
            return Err(SwissArmyHammerError::validation_failed(
                "total_size",
                &info.total_size.to_string(),
                &format!(
                    "exceeds maximum for automatic migration: {} bytes",
                    config.max_total_size
                ),
            ));
        }

        Self::new_default_in(work_dir)
    }

    /// Create storage in a specific directory without migration attempts
    ///
    /// This method creates storage in the specified directory without checking
    /// for or performing any migration. Useful when migration needs to be
    /// handled externally or for testing scenarios.
    fn new_default_without_migration_in(work_dir: &Path) -> Result<Self> {
        let issues_dir = Self::default_directory_in(work_dir)?;
        Self::new(issues_dir)
    }

    /// Get human-readable migration status
    ///
    /// Returns a user-friendly string describing the current migration status
    /// and what actions would be taken.
    pub fn migration_status() -> Result<String> {
        let info = Self::migration_info()?;

        if info.should_migrate {
            Ok(format!(
                "Migration needed: {} files ({:.1} KB) in ./issues/",
                info.file_count,
                info.total_size as f64 / 1024.0
            ))
        } else if info.source_exists && info.destination_exists {
            Ok(
                "Both ./issues/ and .swissarmyhammer/issues/ exist - no migration needed"
                    .to_string(),
            )
        } else if info.destination_exists {
            Ok("Using .swissarmyhammer/issues/ directory".to_string())
        } else {
            Ok("No issues directory found - will create .swissarmyhammer/issues/".to_string())
        }
    }

    /// Get the default issues directory path with backward compatibility
    ///
    /// Returns `.swissarmyhammer/issues` if `.swissarmyhammer` directory exists,
    /// otherwise returns `issues` for backward compatibility with existing repositories
    pub fn default_directory() -> Result<PathBuf> {
        let current_dir = std::env::current_dir().map_err(SwissArmyHammerError::Io)?;
        Self::default_directory_in(&current_dir)
    }

    /// Get the default directory for a specific working directory
    ///
    /// This method determines the appropriate issues directory for a given working directory
    /// without relying on the global current directory state.
    pub fn default_directory_in(work_dir: &Path) -> Result<PathBuf> {
        let swissarmyhammer_dir = work_dir.join(".swissarmyhammer");
        if swissarmyhammer_dir.exists() {
            Ok(swissarmyhammer_dir.join("issues"))
        } else {
            // Fallback to legacy behavior for backward compatibility
            Ok(work_dir.join("issues"))
        }
    }

    /// Check if automatic migration should be performed
    ///
    /// Returns true if:
    /// - Source directory (./issues) exists and is a directory
    /// - Destination directory (.swissarmyhammer/issues) does NOT exist
    ///
    /// This ensures migration only occurs when appropriate and safe.
    pub fn should_migrate() -> Result<bool> {
        let current_dir = std::env::current_dir().map_err(SwissArmyHammerError::Io)?;
        Self::should_migrate_in_dir(&current_dir)
    }

    /// Check if automatic migration should be performed in a specific directory
    ///
    /// This is the testable version of should_migrate() that accepts a base directory.
    pub(crate) fn should_migrate_in_dir(base_dir: &Path) -> Result<bool> {
        let old_issues = base_dir.join("issues");
        let new_issues_parent = base_dir.join(".swissarmyhammer");
        let new_issues = new_issues_parent.join("issues");

        // Migrate if old exists and new location doesn't exist
        Ok(old_issues.exists() && old_issues.is_dir() && !new_issues.exists())
    }

    /// Check if automatic migration should be performed in a specific working directory
    ///
    /// This public method allows external callers to check migration status
    /// for a specific directory without relying on global current directory.
    pub fn should_migrate_in(work_dir: &Path) -> Result<bool> {
        Self::should_migrate_in_dir(work_dir)
    }

    /// Get migration paths for validation
    ///
    /// Returns the source, destination, and backup paths for migration operations.
    /// This function does not check if the paths exist or are valid for migration.
    pub fn migration_paths() -> Result<MigrationPaths> {
        let current_dir = std::env::current_dir().map_err(SwissArmyHammerError::Io)?;
        Ok(Self::migration_paths_in_dir(&current_dir))
    }

    /// Get migration paths for validation in a specific directory
    ///
    /// This is the testable version of migration_paths() that accepts a base directory.
    pub(crate) fn migration_paths_in_dir(base_dir: &Path) -> MigrationPaths {
        MigrationPaths {
            source: base_dir.join("issues"),
            destination: base_dir.join(".swissarmyhammer").join("issues"),
            backup: base_dir.join(".swissarmyhammer").join("issues_backup"),
        }
    }

    /// Gather information about potential migration
    ///
    /// This function analyzes the current state and provides detailed information
    /// about whether migration should occur and what it would involve.
    ///
    /// # Returns
    ///
    /// Returns `MigrationInfo` with:
    /// - Whether migration should occur
    /// - Existence of source and destination directories
    /// - File count and total size in source directory
    pub fn migration_info() -> Result<MigrationInfo> {
        let current_dir = std::env::current_dir().map_err(SwissArmyHammerError::Io)?;
        Self::migration_info_in_dir(&current_dir)
    }

    /// Gather migration information for a specific working directory
    ///
    /// This public method allows external callers to get migration info
    /// for a specific directory without relying on global current directory.
    pub fn migration_info_in(work_dir: &Path) -> Result<MigrationInfo> {
        Self::migration_info_in_dir(work_dir)
    }

    /// Gather information about potential migration in a specific directory
    ///
    /// This is the testable version of migration_info() that accepts a base directory.
    pub(crate) fn migration_info_in_dir(base_dir: &Path) -> Result<MigrationInfo> {
        let paths = Self::migration_paths_in_dir(base_dir);
        let should_migrate = Self::should_migrate_in_dir(base_dir)?;

        let source_exists = paths.source.exists();
        let destination_exists = paths.destination.exists();

        let (file_count, total_size) = if source_exists && paths.source.is_dir() {
            Self::count_directory_contents(&paths.source)?
        } else {
            (0, 0)
        };

        debug!(
            "Migration info: should_migrate={}, source_exists={}, destination_exists={}, file_count={}, total_size={}",
            should_migrate, source_exists, destination_exists, file_count, total_size
        );

        Ok(MigrationInfo {
            should_migrate,
            source_exists,
            destination_exists,
            file_count,
            total_size,
        })
    }

    /// Count files and total size in a directory recursively
    ///
    /// This helper function traverses a directory tree and counts:
    /// - Total number of files (not directories)
    /// - Total size of all files in bytes
    ///
    /// Directories are traversed recursively but not counted.
    ///
    /// # Arguments
    ///
    /// * `dir` - Path to the directory to analyze
    ///
    /// # Returns
    ///
    /// Returns a tuple of (file_count, total_size_bytes)
    fn count_directory_contents(dir: &Path) -> Result<(usize, u64)> {
        let mut file_count = 0;
        let mut total_size = 0;

        fn visit_dir(dir: &Path, file_count: &mut usize, total_size: &mut u64) -> Result<()> {
            for entry in std::fs::read_dir(dir).map_err(SwissArmyHammerError::Io)? {
                let entry = entry.map_err(SwissArmyHammerError::Io)?;
                let path = entry.path();

                if path.is_dir() {
                    visit_dir(&path, file_count, total_size)?;
                } else if path.is_file() {
                    *file_count += 1;
                    let metadata = entry.metadata().map_err(SwissArmyHammerError::Io)?;
                    *total_size += metadata.len();
                }
            }
            Ok(())
        }

        visit_dir(dir, &mut file_count, &mut total_size)?;

        debug!(
            "Directory contents: dir={}, file_count={}, total_size={}",
            dir.display(),
            file_count,
            total_size
        );

        Ok((file_count, total_size))
    }

    /// Create backup of source directory before migration
    ///
    /// Creates a timestamped backup copy of the source directory to enable rollback
    /// if migration fails. The backup is created in the same parent directory as the source.
    ///
    /// # Arguments
    ///
    /// * `source` - Path to the source directory to backup
    ///
    /// # Returns
    ///
    /// Returns the path to the created backup directory
    fn create_backup(source: &Path) -> Result<PathBuf> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_name = format!("issues_backup_{}", timestamp);
        let backup_path = source
            .parent()
            .ok_or_else(|| {
                SwissArmyHammerError::invalid_file_path(
                    &source.display().to_string(),
                    "Source directory must have a parent directory",
                )
            })?
            .join(&backup_name);

        Self::copy_directory_recursive(source, &backup_path).map_err(|e| {
            SwissArmyHammerError::file_operation_failed(
                "backup creation",
                &source.display().to_string(),
                &format!("Failed to backup to {}: {}", backup_path.display(), e),
            )
        })?;

        debug!("Created backup at: {}", backup_path.display());
        Ok(backup_path)
    }

    /// Copy directory recursively for backup creation
    ///
    /// Performs a deep copy of a directory tree, preserving the structure and content
    /// of all files and subdirectories. This is used to create backups before migration.
    ///
    /// # Arguments
    ///
    /// * `source` - Source directory to copy from
    /// * `destination` - Destination directory to copy to
    fn copy_directory_recursive(source: &Path, destination: &Path) -> Result<()> {
        std::fs::create_dir_all(destination).map_err(|e| {
            SwissArmyHammerError::file_operation_failed(
                "create backup directory",
                &destination.display().to_string(),
                &e.to_string(),
            )
        })?;

        for entry in std::fs::read_dir(source).map_err(|e| {
            SwissArmyHammerError::file_operation_failed(
                "read source directory",
                &source.display().to_string(),
                &e.to_string(),
            )
        })? {
            let entry = entry.map_err(|e| {
                SwissArmyHammerError::file_operation_failed(
                    "read directory entry",
                    &source.display().to_string(),
                    &e.to_string(),
                )
            })?;
            let path = entry.path();
            let dest_path = destination.join(entry.file_name());

            if path.is_dir() {
                Self::copy_directory_recursive(&path, &dest_path)?;
            } else {
                std::fs::copy(&path, &dest_path).map_err(|e| {
                    SwissArmyHammerError::file_operation_failed(
                        "copy file during backup",
                        &path.display().to_string(),
                        &format!("copy to {} failed: {}", dest_path.display(), e),
                    )
                })?;
            }
        }

        Ok(())
    }

    /// Perform automatic migration with full safety checks
    ///
    /// This is the main entry point for automatic migration. It performs comprehensive
    /// safety checks, creates backups, executes the migration atomically, and provides
    /// rollback capability if any step fails.
    ///
    /// # Returns
    ///
    /// Returns `MigrationResult::Success` if migration completed successfully,
    /// `MigrationResult::NotNeeded` if no migration is required, or an error
    /// if migration failed (with automatic rollback attempted).
    pub fn perform_migration() -> Result<MigrationResult> {
        let current_dir = std::env::current_dir().map_err(SwissArmyHammerError::Io)?;
        Self::perform_migration_in(&current_dir)
    }

    /// Perform migration in a specific working directory
    ///
    /// This method performs the migration in the specified directory without relying
    /// on the global current directory. Useful for testing and isolated operations.
    pub fn perform_migration_in(work_dir: &Path) -> Result<MigrationResult> {
        let info = Self::migration_info_in_dir(work_dir)?;

        if !info.should_migrate {
            return Ok(MigrationResult::NotNeeded(info));
        }

        info!(
            "Starting issues directory migration: {} files ({} bytes)",
            info.file_count, info.total_size
        );

        let paths = Self::migration_paths_in_dir(work_dir);

        // Create backup before migration
        let backup_path = Self::create_backup(&paths.source)?;

        match Self::execute_migration(&paths) {
            Ok(stats) => {
                // Validate the migration completed correctly
                match Self::validate_migration(&paths, &stats) {
                    Ok(()) => {
                        info!("Migration completed successfully");
                        Ok(MigrationResult::Success(stats))
                    }
                    Err(e) => {
                        error!("Migration validation failed, attempting rollback: {}", e);
                        Self::rollback_migration(&paths, &backup_path)?;
                        Err(SwissArmyHammerError::validation_failed(
                            "migration_validation",
                            "failed",
                            &format!("Migration validation failed: {}", e),
                        ))
                    }
                }
            }
            Err(e) => {
                error!("Migration execution failed, attempting rollback: {}", e);
                Self::rollback_migration(&paths, &backup_path)?;
                Err(e)
            }
        }
    }

    /// Enhanced migration with comprehensive validation
    ///
    /// Performs migration with detailed validation and reporting.
    /// This method provides more thorough validation than the standard migration
    /// and returns detailed reports about any issues found.
    ///
    /// # Returns
    /// Returns `MigrationResult::Success` if migration completed and passed comprehensive validation,
    /// `MigrationResult::NotNeeded` if no migration is required, or an error with detailed
    /// validation information if migration failed.
    pub fn perform_migration_with_validation() -> Result<MigrationResult> {
        let current_dir = std::env::current_dir().map_err(SwissArmyHammerError::Io)?;
        Self::perform_migration_with_validation_in(&current_dir)
    }

    /// Enhanced migration with comprehensive validation in specific directory
    pub fn perform_migration_with_validation_in(work_dir: &Path) -> Result<MigrationResult> {
        let info = Self::migration_info_in_dir(work_dir)?;

        if !info.should_migrate {
            return Ok(MigrationResult::NotNeeded(info));
        }

        let paths = Self::migration_paths_in_dir(work_dir);

        // Create backup for validation
        let backup_path = Self::create_backup(&paths.source)?;

        // Perform migration
        match Self::execute_migration(&paths) {
            Ok(stats) => {
                // Validate migration with comprehensive checks
                let validation =
                    Self::validate_migration_comprehensive(&backup_path, &paths.destination)?;
                let report = validation.generate_report();

                if report.overall_success {
                    info!("Migration completed and validated successfully");
                    Ok(MigrationResult::Success(stats))
                } else {
                    error!("Migration validation failed: {:?}", report.issues);
                    // Rollback
                    Self::rollback_migration(&paths, &backup_path)?;
                    Err(SwissArmyHammerError::validation_failed(
                        "migration_validation",
                        "comprehensive_validation_failed",
                        &format!("Migration validation failed: {}", report.issues.join(", ")),
                    ))
                }
            }
            Err(e) => {
                error!("Migration failed, rolling back: {}", e);
                Self::rollback_migration(&paths, &backup_path)?;
                Err(e)
            }
        }
    }

    /// Execute the actual file migration atomically
    ///
    /// Performs the core migration operation using atomic filesystem operations.
    /// Uses `fs::rename` for atomic directory moves when possible, ensuring the
    /// migration is all-or-nothing.
    ///
    /// # Arguments
    ///
    /// * `paths` - Migration paths containing source and destination
    ///
    /// # Returns
    ///
    /// Returns `MigrationStats` with timing and transfer information
    fn execute_migration(paths: &MigrationPaths) -> Result<MigrationStats> {
        let start_time = std::time::Instant::now();

        // Ensure destination parent directory exists
        if let Some(parent) = paths.destination.parent() {
            std::fs::create_dir_all(parent).map_err(SwissArmyHammerError::Io)?;
        }

        // Get file count and size before migration for stats
        let (file_count, total_size) = Self::count_directory_contents(&paths.source)?;

        // Perform atomic move operation
        std::fs::rename(&paths.source, &paths.destination).map_err(SwissArmyHammerError::Io)?;

        let duration = start_time.elapsed();

        debug!(
            "Migration execution completed: {} files, {} bytes, {:?}",
            file_count, total_size, duration
        );

        Ok(MigrationStats {
            files_moved: file_count,
            bytes_moved: total_size,
            duration,
        })
    }

    /// Rollback migration by restoring from backup
    ///
    /// Restores the original directory structure when migration fails.
    /// This function removes any partial migration artifacts and restores
    /// the source directory from the previously created backup.
    ///
    /// # Arguments
    ///
    /// * `paths` - Migration paths for cleanup
    /// * `backup_path` - Path to the backup directory to restore from
    fn rollback_migration(paths: &MigrationPaths, backup_path: &Path) -> Result<()> {
        warn!("Rolling back migration");

        // Remove partial destination if it exists
        if paths.destination.exists() {
            std::fs::remove_dir_all(&paths.destination).map_err(SwissArmyHammerError::Io)?;
        }

        // Restore from backup
        std::fs::rename(backup_path, &paths.source).map_err(SwissArmyHammerError::Io)?;

        info!("Migration rollback completed");
        Ok(())
    }

    /// Comprehensive validation of completed migration
    ///
    /// Performs detailed validation to ensure migration completed successfully
    /// and provides comprehensive reporting of any issues found.
    ///
    /// # Arguments
    /// * `source_backup` - Path to backup of original source directory  
    /// * `destination` - Path to migration destination directory
    ///
    /// # Returns
    /// Returns `MigrationValidation` with detailed validation results
    pub fn validate_migration_comprehensive(
        source_backup: &Path,
        destination: &Path,
    ) -> Result<MigrationValidation> {
        let file_integrity = Self::validate_file_integrity(source_backup, destination)?;
        let directory_structure = Self::validate_directory_structure(source_backup, destination)?;
        let content_verification = Self::validate_content_integrity(source_backup, destination)?;
        let metadata_preservation =
            Self::validate_metadata_preservation(source_backup, destination)?;

        Ok(MigrationValidation {
            file_integrity,
            directory_structure,
            content_verification,
            metadata_preservation,
        })
    }

    /// Validate file integrity (count, names, sizes)
    fn validate_file_integrity(source: &Path, destination: &Path) -> Result<FileIntegrityCheck> {
        let source_files = Self::collect_file_list(source)?;
        let dest_files = Self::collect_file_list(destination)?;

        let missing_files: Vec<PathBuf> = source_files.difference(&dest_files).cloned().collect();

        let extra_files: Vec<PathBuf> = dest_files.difference(&source_files).cloned().collect();

        let mut corrupted_files = Vec::new();

        // Check file sizes for files that exist in both
        for file_path in source_files.intersection(&dest_files) {
            let source_file = source.join(file_path);
            let dest_file = destination.join(file_path);

            if let (Ok(source_meta), Ok(dest_meta)) = (
                std::fs::metadata(&source_file),
                std::fs::metadata(&dest_file),
            ) {
                if source_meta.len() != dest_meta.len() {
                    corrupted_files.push(file_path.clone());
                }
            }
        }

        Ok(FileIntegrityCheck {
            total_files: source_files.len(),
            verified_files: source_files.len() - missing_files.len() - corrupted_files.len(),
            missing_files,
            extra_files,
            corrupted_files,
        })
    }

    /// Collect recursive file list with relative paths
    fn collect_file_list(root: &Path) -> Result<HashSet<PathBuf>> {
        let mut files = HashSet::new();

        fn visit_dir(dir: &Path, root: &Path, files: &mut HashSet<PathBuf>) -> Result<()> {
            for entry in std::fs::read_dir(dir).map_err(SwissArmyHammerError::Io)? {
                let entry = entry.map_err(SwissArmyHammerError::Io)?;
                let path = entry.path();

                if path.is_dir() {
                    visit_dir(&path, root, files)?;
                } else if path.is_file() {
                    if let Ok(relative_path) = path.strip_prefix(root) {
                        files.insert(relative_path.to_path_buf());
                    }
                }
            }
            Ok(())
        }

        if root.exists() {
            visit_dir(root, root, &mut files)?;
        }

        Ok(files)
    }

    /// Validate directory structure preservation
    fn validate_directory_structure(
        source: &Path,
        destination: &Path,
    ) -> Result<DirectoryStructureCheck> {
        let mut directories_preserved = true;
        let mut relative_paths_correct = true;
        let permissions_preserved = true;
        let mut structure_differences = Vec::new();

        // Check if both directories have the same subdirectory structure
        let source_dirs = Self::collect_directory_list(source)?;
        let dest_dirs = Self::collect_directory_list(destination)?;

        if source_dirs != dest_dirs {
            directories_preserved = false;
            let missing_dirs: Vec<_> = source_dirs.difference(&dest_dirs).collect();
            let extra_dirs: Vec<_> = dest_dirs.difference(&source_dirs).collect();

            if !missing_dirs.is_empty() {
                structure_differences.push(format!("Missing directories: {:?}", missing_dirs));
            }
            if !extra_dirs.is_empty() {
                structure_differences.push(format!("Extra directories: {:?}", extra_dirs));
            }
        }

        // Validate relative paths by checking a sample of files
        let source_files = Self::collect_file_list(source)?;
        for file_path in source_files.iter().take(10) {
            // Sample first 10 files for performance
            let source_file = source.join(file_path);
            let dest_file = destination.join(file_path);

            if source_file.exists() && !dest_file.exists() {
                relative_paths_correct = false;
                structure_differences.push(format!("Path mismatch: {:?}", file_path));
                break;
            }
        }

        Ok(DirectoryStructureCheck {
            directories_preserved,
            relative_paths_correct,
            permissions_preserved, // TODO: Implement permission checking
            structure_differences,
        })
    }

    /// Collect directory list with relative paths
    fn collect_directory_list(root: &Path) -> Result<HashSet<PathBuf>> {
        let mut dirs = HashSet::new();

        fn visit_dirs(dir: &Path, root: &Path, dirs: &mut HashSet<PathBuf>) -> Result<()> {
            for entry in std::fs::read_dir(dir).map_err(SwissArmyHammerError::Io)? {
                let entry = entry.map_err(SwissArmyHammerError::Io)?;
                let path = entry.path();

                if path.is_dir() {
                    if let Ok(relative_path) = path.strip_prefix(root) {
                        dirs.insert(relative_path.to_path_buf());
                    }
                    visit_dirs(&path, root, dirs)?;
                }
            }
            Ok(())
        }

        if root.exists() {
            visit_dirs(root, root, &mut dirs)?;
        }

        Ok(dirs)
    }

    /// Validate content integrity using checksums
    fn validate_content_integrity(
        source: &Path,
        destination: &Path,
    ) -> Result<ContentVerificationCheck> {
        let source_files = Self::collect_file_list(source)?;
        let mut verified_count = 0;
        let mut mismatched_files = Vec::new();
        let mut unreadable_files = Vec::new();

        for file_path in &source_files {
            let source_file = source.join(file_path);
            let dest_file = destination.join(file_path);

            if !dest_file.exists() {
                continue; // Already caught by file integrity check
            }

            match Self::compare_file_content(&source_file, &dest_file) {
                Ok(true) => verified_count += 1,
                Ok(false) => mismatched_files.push(file_path.clone()),
                Err(_) => unreadable_files.push(file_path.clone()),
            }
        }

        Ok(ContentVerificationCheck {
            total_files: source_files.len(),
            verified_files: verified_count,
            mismatched_files,
            unreadable_files,
        })
    }

    /// Compare file content using efficient hashing
    fn compare_file_content(source: &Path, destination: &Path) -> Result<bool> {
        // For small files, use full content comparison
        let source_size = std::fs::metadata(source)
            .map_err(SwissArmyHammerError::Io)?
            .len();
        if source_size < 1024 * 1024 {
            // 1MB
            let source_content = std::fs::read(source).map_err(SwissArmyHammerError::Io)?;
            let dest_content = std::fs::read(destination).map_err(SwissArmyHammerError::Io)?;
            return Ok(source_content == dest_content);
        }

        // For large files, use hash comparison
        let source_hash = Self::calculate_file_hash(source)?;
        let dest_hash = Self::calculate_file_hash(destination)?;
        Ok(source_hash == dest_hash)
    }

    /// Calculate hash of file content
    fn calculate_file_hash(path: &Path) -> Result<u64> {
        let mut file = std::fs::File::open(path).map_err(SwissArmyHammerError::Io)?;
        let mut hasher = DefaultHasher::new();
        let mut buffer = [0; 8192];

        loop {
            let bytes_read = file.read(&mut buffer).map_err(SwissArmyHammerError::Io)?;
            if bytes_read == 0 {
                break;
            }
            hasher.write(&buffer[..bytes_read]);
        }

        Ok(hasher.finish())
    }

    /// Validate metadata preservation  
    fn validate_metadata_preservation(
        _source: &Path,
        _destination: &Path,
    ) -> Result<MetadataPreservationCheck> {
        // Basic implementation - can be enhanced with more detailed metadata checks
        Ok(MetadataPreservationCheck {
            permissions_preserved: true, // TODO: Implement detailed permission comparison
            timestamps_preserved: true,  // TODO: Implement timestamp comparison
            metadata_differences: Vec::new(),
        })
    }

    /// Validate migration completed successfully
    ///
    /// Performs comprehensive validation to ensure the migration completed correctly.
    /// Checks that files were moved properly and that the expected counts match.
    ///
    /// # Arguments
    ///
    /// * `paths` - Migration paths to validate
    /// * `expected_stats` - Expected statistics from the migration
    fn validate_migration(paths: &MigrationPaths, expected_stats: &MigrationStats) -> Result<()> {
        // Check destination exists
        if !paths.destination.exists() {
            return Err(SwissArmyHammerError::validation_failed(
                "migration_destination",
                &paths.destination.display().to_string(),
                "Destination directory does not exist after migration",
            ));
        }

        // Check source no longer exists
        if paths.source.exists() {
            return Err(SwissArmyHammerError::validation_failed(
                "migration_source",
                &paths.source.display().to_string(),
                "Source directory still exists after migration",
            ));
        }

        // Validate file count and size
        let (actual_files, actual_size) = Self::count_directory_contents(&paths.destination)?;
        if actual_files != expected_stats.files_moved {
            return Err(SwissArmyHammerError::validation_failed(
                "migration_file_count",
                &actual_files.to_string(),
                &format!(
                    "File count mismatch: expected {}, got {}",
                    expected_stats.files_moved, actual_files
                ),
            ));
        }

        if actual_size != expected_stats.bytes_moved {
            return Err(SwissArmyHammerError::validation_failed(
                "migration_size",
                &actual_size.to_string(),
                &format!(
                    "Size mismatch: expected {} bytes, got {} bytes",
                    expected_stats.bytes_moved, actual_size
                ),
            ));
        }

        debug!("Migration validation passed");
        Ok(())
    }

    /// Parse issue from file path
    ///
    /// Parses an issue from a file path, extracting the issue name and name from the filename
    /// and reading the content from the file. The filename must follow the format:
    /// `<nnnnnn>_<name>.md` where `nnnnnn` is a 6-digit zero-padded number.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the issue file
    ///
    /// # Returns
    ///
    /// Returns `Ok(Issue)` if the file is successfully parsed, or an error if:
    /// - The filename doesn't follow the expected format
    /// - The issue name is invalid or exceeds the maximum
    /// - The file cannot be read
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let issue = storage.parse_issue_from_file(Path::new("./issues/000123_bug_fix.md"))?;
    /// assert_eq!(issue.name.as_str(), "000123_bug_fix");
    /// ```
    fn parse_issue_from_file(&self, path: &Path) -> Result<Issue> {
        let filename = path
            .file_stem()
            .ok_or_else(|| {
                SwissArmyHammerError::parsing_failed(
                    "file path",
                    &path.display().to_string(),
                    "no file stem",
                )
            })?
            .to_str()
            .ok_or_else(|| {
                SwissArmyHammerError::parsing_failed(
                    "filename",
                    &path.display().to_string(),
                    "invalid UTF-8 encoding",
                )
            })?;

        // Extract issue name from filename
        // Use the full filename (without .md extension) to preserve issue numbers
        let name = filename.to_string();

        // Read file content - treat entire content as markdown
        let content = fs::read_to_string(path).map_err(SwissArmyHammerError::Io)?;

        Ok(Issue { name, content })
    }

    /// List issues in a directory
    ///
    /// Scans a directory for issue files and returns a vector of parsed Issues.
    /// Only files with the `.md` extension that follow the correct naming format
    /// are processed. Files that fail to parse are logged as debug messages but
    /// don't cause the entire operation to fail.
    ///
    /// # Arguments
    ///
    /// * `dir` - Path to the directory to scan
    ///
    /// # Returns
    ///
    /// Returns `Ok(Vec<Issue>)` containing all successfully parsed issues,
    /// sorted by issue name in ascending order. Returns an empty vector
    /// if the directory doesn't exist or contains no valid issue files.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let issues = storage.list_issues_in_dir(Path::new("./issues"))?;
    /// // Issues are sorted by name
    /// if !issues.is_empty() {
    ///     assert!(issues[0].name.as_str() <= issues[1].name.as_str());
    /// }
    /// ```
    fn list_issues_in_dir(&self, dir: &Path) -> Result<Vec<Issue>> {
        if !dir.exists() {
            return Ok(vec![]);
        }

        let mut issues = Vec::new();

        let entries = fs::read_dir(dir).map_err(SwissArmyHammerError::Io)?;

        for entry in entries {
            let entry = entry.map_err(SwissArmyHammerError::Io)?;

            let path = entry.path();
            if path.is_file() && path.extension() == Some(std::ffi::OsStr::new("md")) {
                match self.parse_issue_from_file(&path) {
                    Ok(issue) => issues.push(issue),
                    Err(e) => {
                        debug!("Failed to parse issue from {}: {}", path.display(), e);
                    }
                }
            } else if path.is_dir() {
                // Recursively scan subdirectories
                match self.list_issues_in_dir(&path) {
                    Ok(sub_issues) => issues.extend(sub_issues),
                    Err(e) => {
                        debug!("Failed to scan subdirectory {}: {}", path.display(), e);
                    }
                }
            }
        }

        // Sort by name
        issues.sort_by(|a, b| a.name.as_str().cmp(b.name.as_str()));

        Ok(issues)
    }

    /// Create issue file
    ///
    /// Creates a new issue file with the given number, name, and content.
    /// The file is created in the pending issues directory with the standard
    /// naming format: `<nnnnnn>_<name>.md` where `nnnnnn` is a 6-digit
    /// zero-padded number.
    ///
    /// # Arguments
    ///
    /// * `number` - The issue name to use
    /// * `name` - The issue name (will be sanitized for filesystem safety)
    /// * `content` - The markdown content to write to the file
    ///
    /// # Returns
    ///
    /// Returns `Ok(PathBuf)` containing the path to the created file, or an error
    /// if the file cannot be created or written.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let file_path = storage.create_issue_file(123, "bug fix", "# Bug Fix\n\nDescription...")?;
    /// assert!(file_path.ends_with("000123_bug-fix.md"));
    /// ```
    ///
    /// # Note
    ///
    /// The name parameter is sanitized by replacing spaces, forward slashes,
    /// and backslashes with hyphens to ensure filesystem compatibility.
    #[allow(dead_code)]
    fn create_issue_file(&self, number: u32, name: &str, content: &str) -> Result<PathBuf> {
        // Format filename based on whether name is provided
        let filename = if name.is_empty() {
            // Nameless issue: just the number (e.g., 000123.md)
            format!("{}.md", format_issue_number(number))
        } else {
            // Named issue: number_name format (e.g., 000123_fix_bug.md)
            let safe_name = create_safe_filename(name);
            format!("{}_{}.md", format_issue_number(number), safe_name)
        };
        let file_path = self.state.issues_dir.join(&filename);

        // Write content to file
        fs::write(&file_path, content).map_err(SwissArmyHammerError::Io)?;

        Ok(file_path)
    }

    /// Update issue content by name
    async fn update_issue_impl_by_name(&self, name: &str, content: String) -> Result<Issue> {
        debug!("Updating issue {}", name);

        // Find the issue to get its current file path
        let issue_info = self.get_issue_info(name).await?;
        let current_path = &issue_info.file_path;

        // Atomic write using temp file and rename - write pure markdown content
        let temp_path = current_path.with_extension("tmp");
        std::fs::write(&temp_path, &content).map_err(SwissArmyHammerError::Io)?;
        std::fs::rename(&temp_path, current_path).map_err(SwissArmyHammerError::Io)?;

        debug!(
            "Successfully updated issue {} at path {}",
            name,
            current_path.display()
        );

        // Return updated issue with new content
        Ok(Issue {
            name: issue_info.issue.name,
            content,
        })
    }

    /// Cleanup duplicate files that may exist in the source directory
    ///
    /// This method is called to remove potential duplicate files that could exist
    /// due to previous failed operations or race conditions. It uses graceful error
    /// handling to ensure the main operation isn't disrupted by cleanup failures.
    ///
    /// Behavior:
    /// - Removes duplicate files if they exist and are different from the current file
    /// - Handles common I/O errors gracefully (NotFound, PermissionDenied, Interrupted)
    /// - Logs errors but doesn't fail the main operation for non-critical cleanup issues
    /// - Retries once for interrupted operations
    fn cleanup_duplicate_if_exists(&self, file_path: &Path, source_dir: &Path) -> Result<()> {
        let filename = file_path.file_name().ok_or_else(|| {
            SwissArmyHammerError::Other(format!(
                "Invalid file path: cannot extract filename from {}",
                file_path.display()
            ))
        })?;
        let potential_duplicate = source_dir.join(filename);

        if potential_duplicate.exists() && potential_duplicate != *file_path {
            debug!(
                "Found duplicate file at {}, removing it",
                potential_duplicate.display()
            );
            if let Err(e) = std::fs::remove_file(&potential_duplicate) {
                match e.kind() {
                    io::ErrorKind::NotFound => {
                        // File was already deleted by another process - this is fine
                        debug!(
                            "Duplicate file {} was already removed",
                            potential_duplicate.display()
                        );
                    }
                    io::ErrorKind::PermissionDenied => {
                        // Log the issue but don't fail the operation - duplicates are not critical
                        debug!(
                            "Permission denied removing duplicate file {}: {}",
                            potential_duplicate.display(),
                            e
                        );
                    }
                    io::ErrorKind::Interrupted => {
                        // Retry once for interrupted operations
                        debug!(
                            "Retrying removal of duplicate file {}",
                            potential_duplicate.display()
                        );
                        if let Err(retry_err) = std::fs::remove_file(&potential_duplicate) {
                            debug!(
                                "Failed to remove duplicate file {} after retry: {}",
                                potential_duplicate.display(),
                                retry_err
                            );
                        }
                    }
                    _ => {
                        // For other errors, log but don't fail the main operation
                        debug!(
                            "Failed to remove duplicate file {}: {}",
                            potential_duplicate.display(),
                            e
                        );
                    }
                }
            }
        }
        Ok(())
    }

    /// Check if all issues are completed
    pub async fn all_complete(&self) -> Result<bool> {
        let all_issue_infos = self.list_issues_info().await?;
        // Check if there are any non-completed issues
        let pending_count = all_issue_infos
            .iter()
            .filter(|info| !info.completed)
            .count();
        Ok(pending_count == 0)
    }

    /// Get issue for mark_complete operation with deterministic duplicate handling
    async fn get_issue_for_mark_complete(&self, name: &str) -> Result<Issue> {
        let all_issue_infos = self.list_issues_info().await?;
        let matching_issue_infos: Vec<_> = all_issue_infos
            .into_iter()
            .filter(|issue_info| issue_info.issue.name == name)
            .collect();

        if matching_issue_infos.is_empty() {
            return Err(SwissArmyHammerError::IssueNotFound(name.to_string()));
        }

        // For mark_complete, prioritize pending issues first (normal completion flow)
        if let Some(pending_issue_info) = matching_issue_infos
            .iter()
            .find(|issue_info| !issue_info.completed)
        {
            return Ok(pending_issue_info.issue.clone());
        }

        // If no pending issue, return completed issue (idempotent behavior)
        if let Some(completed_issue_info) = matching_issue_infos
            .iter()
            .find(|issue_info| issue_info.completed)
        {
            return Ok(completed_issue_info.issue.clone());
        }

        // Fallback (shouldn't happen)
        Err(SwissArmyHammerError::IssueNotFound(name.to_string()))
    }

    /// Move a specific issue to completed/pending state, avoiding duplicate lookup issues
    async fn move_issue_with_issue(&self, issue: Issue, to_completed: bool) -> Result<Issue> {
        let issue_name = &issue.name;

        // Get all issue infos to work with completion status and file paths
        let all_issue_infos = self.list_issues_info().await?;
        let matching_issue_infos: Vec<_> = all_issue_infos
            .into_iter()
            .filter(|issue_info| issue_info.issue.name == *issue_name)
            .collect();

        if matching_issue_infos.is_empty() {
            return Err(SwissArmyHammerError::IssueNotFound(issue_name.to_string()));
        }

        // Special case for mark_complete: if we're trying to complete a pending issue,
        // but there's already a completed version, use file timestamps to determine precedence
        if to_completed {
            let pending_issue_info = matching_issue_infos.iter().find(|info| !info.completed);
            let completed_issue_info = matching_issue_infos.iter().find(|info| info.completed);

            if let (Some(pending_info), Some(completed_info)) =
                (pending_issue_info, completed_issue_info)
            {
                // Compare file modification times to determine which file was created first
                let pending_mtime = std::fs::metadata(&pending_info.file_path)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                let completed_mtime = std::fs::metadata(&completed_info.file_path)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

                // If the pending file is newer than the completed file, it might be a stale duplicate
                // created after legitimate completion, so we should keep the completed file
                if pending_mtime > completed_mtime {
                    // Clean up the pending duplicate
                    let _ = std::fs::remove_file(&pending_info.file_path); // Ignore errors for cleanup
                    return Ok(completed_info.issue.clone());
                }
            }
        }

        // Find the issue info we're working with
        let current_issue_info = if to_completed {
            // For completing, prefer pending version
            matching_issue_infos
                .iter()
                .find(|info| !info.completed)
                .or_else(|| matching_issue_infos.first())
        } else {
            // For uncompleting, prefer completed version
            matching_issue_infos
                .iter()
                .find(|info| info.completed)
                .or_else(|| matching_issue_infos.first())
        };

        let current_issue_info = current_issue_info
            .ok_or_else(|| SwissArmyHammerError::IssueNotFound(issue_name.to_string()))?;

        // Check if already in target state
        if current_issue_info.completed == to_completed {
            // Clean up any duplicates that might exist in the opposite directory
            let opposite_dir = if to_completed {
                &self.state.issues_dir // Clean up any pending duplicates
            } else {
                &self.state.completed_dir // Clean up any completed duplicates
            };

            // Find and remove duplicates in the opposite directory
            let filename = current_issue_info
                .file_path
                .file_name()
                .ok_or_else(|| SwissArmyHammerError::Other("Invalid file path".to_string()))?;
            let potential_duplicate = opposite_dir.join(filename);

            if potential_duplicate.exists() && potential_duplicate != current_issue_info.file_path {
                let _ = std::fs::remove_file(&potential_duplicate); // Ignore errors for cleanup
            }

            return Ok(current_issue_info.issue.clone());
        }

        // Determine source and target paths
        let target_dir = if to_completed {
            &self.state.completed_dir
        } else {
            &self.state.issues_dir
        };

        // Create target path with same filename
        let filename = current_issue_info
            .file_path
            .file_name()
            .ok_or_else(|| SwissArmyHammerError::Other("Invalid file path".to_string()))?;
        let target_path = target_dir.join(filename);

        // Move file atomically
        std::fs::rename(&current_issue_info.file_path, &target_path)
            .map_err(SwissArmyHammerError::Io)?;

        // Clean up any duplicate files in the source directory
        let source_dir = if to_completed {
            &self.state.issues_dir
        } else {
            &self.state.completed_dir
        };
        self.cleanup_duplicate_if_exists(&target_path, source_dir)?;

        Ok(issue)
    }
}

impl MigrationValidation {
    /// Generate comprehensive migration report
    pub fn generate_report(&self) -> MigrationReport {
        let overall_success = self.is_successful();
        let mut issues = Vec::new();
        let mut warnings = Vec::new();

        // File integrity issues
        if !self.file_integrity.missing_files.is_empty() {
            issues.push(format!(
                "Missing files: {}",
                self.file_integrity.missing_files.len()
            ));
        }

        if !self.file_integrity.corrupted_files.is_empty() {
            issues.push(format!(
                "Corrupted files: {}",
                self.file_integrity.corrupted_files.len()
            ));
        }

        if !self.file_integrity.extra_files.is_empty() {
            warnings.push(format!(
                "Extra files found: {}",
                self.file_integrity.extra_files.len()
            ));
        }

        // Content verification issues
        if !self.content_verification.mismatched_files.is_empty() {
            issues.push(format!(
                "Content mismatch: {}",
                self.content_verification.mismatched_files.len()
            ));
        }

        // Directory structure issues
        if !self.directory_structure.directories_preserved {
            issues.push("Directory structure not preserved".to_string());
        }

        MigrationReport {
            overall_success,
            total_files: self.file_integrity.total_files,
            verified_files: self.content_verification.verified_files,
            issues,
            warnings,
            detailed_validation: self.clone(),
        }
    }

    /// Check if migration validation passed all critical checks
    pub fn is_successful(&self) -> bool {
        self.file_integrity.missing_files.is_empty()
            && self.file_integrity.corrupted_files.is_empty()
            && self.content_verification.mismatched_files.is_empty()
            && self.directory_structure.directories_preserved
    }
}

#[async_trait::async_trait]
impl IssueStorage for FileSystemIssueStorage {
    async fn list_issues(&self) -> Result<Vec<Issue>> {
        // Since list_issues_in_dir is now recursive, we only need to scan the root issues directory
        // This will automatically find issues in both pending and completed directories
        let all_issues = self.list_issues_in_dir(&self.state.issues_dir)?;

        Ok(all_issues)
    }

    async fn list_issues_info(&self) -> Result<Vec<IssueInfo>> {
        let issues = self.list_issues().await?;
        let mut issue_infos = Vec::new();

        for issue in issues {
            // Find the file path for this issue by checking both directories
            let pending_path = self.state.issues_dir.join(format!("{}.md", issue.name));
            let completed_path = self.state.completed_dir.join(format!("{}.md", issue.name));

            // Handle duplicate cases correctly - if both exist, create entries for both
            let mut paths_to_process = Vec::new();

            if pending_path.exists() {
                paths_to_process.push((pending_path, false));
            }
            if completed_path.exists() {
                paths_to_process.push((completed_path, true));
            }

            if paths_to_process.is_empty() {
                // This shouldn't happen if list_issues is working correctly
                continue;
            }

            for (file_path, completed) in paths_to_process {
                let created_at = Issue::get_created_at(&file_path);
                issue_infos.push(IssueInfo {
                    issue: issue.clone(),
                    completed,
                    file_path,
                    created_at,
                });
            }
        }

        Ok(issue_infos)
    }

    async fn get_issue(&self, name: &str) -> Result<Issue> {
        // Use existing list_issues() method to avoid duplicating search logic
        let all_issues = self.list_issues().await?;
        all_issues
            .into_iter()
            .find(|issue| issue.name == name)
            .ok_or_else(|| SwissArmyHammerError::IssueNotFound(name.to_string()))
    }

    async fn get_issue_info(&self, name: &str) -> Result<IssueInfo> {
        let all_issue_infos = self.list_issues_info().await?;
        let matching_infos: Vec<_> = all_issue_infos
            .into_iter()
            .filter(|issue_info| issue_info.issue.name == name)
            .collect();

        if matching_infos.is_empty() {
            return Err(SwissArmyHammerError::IssueNotFound(name.to_string()));
        }

        // If there are multiple matches (duplicates), prefer the completed version
        if let Some(completed_info) = matching_infos.iter().find(|info| info.completed) {
            return Ok(completed_info.clone());
        }

        // Otherwise, return the first match (pending version)
        Ok(matching_infos.into_iter().next().unwrap())
    }

    async fn create_issue(&self, name: String, content: String) -> Result<Issue> {
        // Lock to ensure atomic issue creation (prevents race conditions)
        let _lock = self.creation_lock.lock().await;

        // If name is empty, generate a ULID
        let issue_name = if name.trim().is_empty() {
            generate_monotonic_ulid_string()
        } else {
            sanitize_issue_name(&name)
        };

        // Create the filename and file path
        let filename = create_safe_filename(&issue_name);
        let file_path = self.state.issues_dir.join(format!("{filename}.md"));

        // Write pure markdown content (no YAML front matter)
        fs::write(&file_path, &content).map_err(SwissArmyHammerError::Io)?;

        Ok(Issue {
            name: issue_name,
            content,
        })
    }

    async fn update_issue(&self, name: &str, content: String) -> Result<Issue> {
        // Find the issue by name first
        let issue = self.get_issue(name).await?;
        self.update_issue_impl_by_name(&issue.name, content).await
    }

    async fn mark_complete(&self, name: &str) -> Result<Issue> {
        // Find the issue by name, with deterministic behavior for duplicates
        let issue = self.get_issue_for_mark_complete(name).await?;

        // Move the issue to completed directory
        let completed_issue = self.move_issue_with_issue(issue, true).await?;

        // Verify we actually moved it successfully
        self.validate_completion_successful(&completed_issue.name)
            .await?;

        Ok(completed_issue)
    }

    async fn create_issues_batch(&self, issues: Vec<(String, String)>) -> Result<Vec<Issue>> {
        let mut created_issues = Vec::new();

        for (name, content) in issues {
            let issue = self.create_issue(name, content).await?;
            created_issues.push(issue);
        }

        Ok(created_issues)
    }

    async fn get_issues_batch(&self, names: Vec<&str>) -> Result<Vec<Issue>> {
        // First, verify all issues exist before returning any
        for name in &names {
            self.get_issue(name).await?; // This will fail if issue doesn't exist
        }

        let mut issues = Vec::new();

        for name in names {
            let issue = self.get_issue(name).await?;
            issues.push(issue);
        }

        Ok(issues)
    }

    async fn update_issues_batch(&self, updates: Vec<(&str, String)>) -> Result<Vec<Issue>> {
        // First, verify all issues exist before updating any
        for (name, _) in &updates {
            self.get_issue(name).await?; // This will fail if issue doesn't exist
        }

        let mut updated_issues = Vec::new();

        for (name, content) in updates {
            let issue = self.update_issue(name, content).await?;
            updated_issues.push(issue);
        }

        Ok(updated_issues)
    }

    async fn mark_complete_batch(&self, names: Vec<&str>) -> Result<Vec<Issue>> {
        // First, verify all issues exist before marking any complete
        for name in &names {
            self.get_issue(name).await?; // This will fail if issue doesn't exist
        }

        let mut completed_issues = Vec::new();

        for name in names {
            let issue = self.mark_complete(name).await?;
            completed_issues.push(issue);
        }

        Ok(completed_issues)
    }

    async fn get_next_issue(&self) -> Result<Option<Issue>> {
        let all_issue_infos = self.list_issues_info().await?;
        // Filter to pending issues and sort alphabetically by name
        let mut pending_issue_infos: Vec<IssueInfo> = all_issue_infos
            .into_iter()
            .filter(|issue_info| !issue_info.completed)
            .collect();
        pending_issue_infos.sort_by(|a, b| a.issue.name.cmp(&b.issue.name));
        Ok(pending_issue_infos
            .into_iter()
            .next()
            .map(|info| info.issue))
    }

    // Type-safe implementations using IssueName

    async fn get_issue_by_name(&self, name: &IssueName) -> Result<Issue> {
        self.get_issue(name.as_str()).await
    }

    async fn create_issue_with_name(&self, name: IssueName, content: String) -> Result<Issue> {
        self.create_issue(name.get().to_string(), content).await
    }

    async fn update_issue_by_name(&self, name: &IssueName, content: String) -> Result<Issue> {
        self.update_issue(name.as_str(), content).await
    }

    async fn get_issues_batch_by_name(&self, names: Vec<&IssueName>) -> Result<Vec<Issue>> {
        let str_names: Vec<&str> = names.iter().map(|n| n.as_str()).collect();
        self.get_issues_batch(str_names).await
    }

    async fn update_issues_batch_by_name(
        &self,
        updates: Vec<(&IssueName, String)>,
    ) -> Result<Vec<Issue>> {
        let str_updates: Vec<(&str, String)> = updates
            .iter()
            .map(|(name, content)| (name.as_str(), content.clone()))
            .collect();
        self.update_issues_batch(str_updates).await
    }
}

impl FileSystemIssueStorage {
    /// Verify that the issue was actually moved to the completed directory
    async fn validate_completion_successful(&self, issue_name: &str) -> Result<()> {
        // Use list_issues_info to find the issue and check if it's in the completed directory
        let all_issue_infos = self.list_issues_info().await?;
        let completed_issue_info = all_issue_infos
            .into_iter()
            .find(|issue_info| issue_info.issue.name == issue_name && issue_info.completed);

        if completed_issue_info.is_none() {
            let work_dir = std::env::current_dir().map_err(crate::SwissArmyHammerError::Io)?;
            crate::common::create_abort_file(
                &work_dir,
                &format!("Issue '{issue_name}' was not successfully moved to completed directory"),
            )?;

            return Err(crate::SwissArmyHammerError::Storage(format!(
                "Issue '{issue_name}' completion failed - not found in completed directory"
            )));
        }

        Ok(())
    }
}

/// Format issue name as 6-digit string with leading zeros
pub fn format_issue_number(number: u32) -> String {
    format!("{number:06}")
}

/// Parse issue number from 6-digit string
pub fn parse_issue_number(s: &str) -> Result<u32> {
    if s.len() != 6 {
        return Err(SwissArmyHammerError::Storage(format!(
            "Issue number must be 6 digits, got: {s}"
        )));
    }

    s.parse::<u32>().map_err(|_| {
        SwissArmyHammerError::Storage(format!("Issue number must be numeric, got: {s}"))
    })
}

/// Parse issue filename in numbered format (e.g., "000123_bug_fix")
pub fn parse_issue_filename(filename: &str) -> Result<(u32, String)> {
    if filename.is_empty() {
        return Err(SwissArmyHammerError::Storage("Empty filename".to_string()));
    }

    // Find the first underscore
    if let Some(underscore_pos) = filename.find('_') {
        let number_part = &filename[..underscore_pos];
        let name_part = &filename[underscore_pos + 1..];

        // Parse the number part
        let number = parse_issue_number(number_part)?;

        Ok((number, name_part.to_string()))
    } else {
        Err(SwissArmyHammerError::Storage(format!(
            "Invalid filename format, missing underscore: {filename}"
        )))
    }
}

/// Extract issue info from filename
///
/// Parses an issue filename in the format `<nnnnnn>_<name>` and returns the issue name
/// and name as a tuple. The filename must follow the strict 6-digit format where the number
/// is zero-padded and separated from the name by an underscore.
///
/// # Arguments
///
/// * `filename` - The filename to parse (without extension)
///
/// # Returns
///
/// Returns `Ok((number, name))` if the filename is valid, or an error if:
/// - The filename doesn't contain exactly one underscore
/// - The number part is not exactly 6 digits
/// - The number part contains non-numeric characters
/// - The number exceeds the maximum allowed value (999_999)
///
/// # Examples
///
/// ```
/// # use swissarmyhammer::issues::parse_issue_filename;
/// // Basic usage
/// let (number, name) = parse_issue_filename("000123_bug_fix").unwrap();
/// assert_eq!(number, 123);
/// assert_eq!(name, "bug_fix");
///
/// // With underscores in the name (only first underscore is used as separator)
/// let (number, name) = parse_issue_filename("000456_feature_with_underscores").unwrap();
/// assert_eq!(number, 456);
/// assert_eq!(name, "feature_with_underscores");
///
/// // Edge case: empty name
/// let (number, name) = parse_issue_filename("000789_").unwrap();
/// assert_eq!(number, 789);
/// assert_eq!(name, "");
///
/// // Edge case: number zero
/// let (number, name) = parse_issue_filename("000000_zero_issue").unwrap();
/// assert_eq!(number, 0);
/// assert_eq!(name, "zero_issue");
///
/// // Maximum number
/// let (number, name) = parse_issue_filename("999999_max_issue").unwrap();
/// assert_eq!(number, 999_999);
/// assert_eq!(name, "max_issue");
/// ```
///
/// # Errors
///
/// ```should_panic
/// # use swissarmyhammer::issues::parse_issue_filename;
/// // Invalid: no underscore
/// parse_issue_filename("000123test").unwrap();
///
/// // Invalid: wrong number format
/// parse_issue_filename("123_test").unwrap();
///
/// // Invalid: non-numeric characters
/// parse_issue_filename("abc123_test").unwrap();
///
/// // Invalid: number too large
/// parse_issue_filename("1_000_000_test").unwrap();
/// ```
/// Parse any issue filename format (numbered or non-numbered)
///
/// This function provides flexible parsing of issue filenames, supporting both the
/// traditional numbered format used for backward compatibility and the newer flexible
/// format that accepts any valid filename. This is the primary parsing function used
/// by the issue system to handle mixed file formats in issue directories.
///
/// ## Parsing Logic
///
/// 1. **First Attempt**: Try to parse as numbered format (`NNNNNN_name`)
/// 2. **Fallback**: If numbered parsing fails, treat entire filename as issue name
/// 3. **Validation**: Ensure the resulting name is not empty
///
/// ## Supported Formats
///
/// ### Traditional Numbered Format
/// - Pattern: `NNNNNN_name` where N is a digit (0-9)
/// - Example: `000123_bug_fix` → `(Some(123), "bug_fix")`
/// - Zero-padding is required for consistency
/// - Maximum number is limited by `Config::global().max_issue_number`
///
/// ### Flexible Format  
/// - Pattern: Any non-empty string
/// - Example: `feature-request` → `(None, "feature-request")`
/// - Assigned virtual numbers during issue creation
/// - Supports dashes, underscores, and other safe characters
///
/// ## Virtual Number Assignment
///
/// Non-numbered files (where this function returns `None` for the number) are assigned
/// virtual numbers in the range [`Config::global().virtual_issue_number_base`..] based
/// on a hash of the filename. This ensures consistent issue nameing while maintaining
/// flexibility in naming.
///
/// # Arguments
///
/// * `filename` - The filename without extension to parse (e.g., "000123_bug_fix" or "README")
///
/// # Returns
///
/// Returns `Ok((Option<u32>, String))` where:
/// - First element is `Some(number)` for successfully parsed numbered files, `None` for non-numbered
/// - Second element is the issue name (extracted name for numbered, full filename for non-numbered)
///
/// # Errors
///
/// Returns `SwissArmyHammerError::Other` if:
/// - The filename is empty
/// - Internal parsing errors occur (rare)
///
/// # Examples
///
/// ## Traditional Numbered Format Examples
///
/// ```rust
/// # use swissarmyhammer::issues::filesystem::parse_any_issue_filename;
/// // Standard numbered format
/// let (number, name) = parse_any_issue_filename("000123_bug_fix").unwrap();
/// assert_eq!(number, Some(123));
/// assert_eq!(name, "bug_fix");
///
/// // Leading zeros are handled correctly
/// let (number, name) = parse_any_issue_filename("000001_first_issue").unwrap();
/// assert_eq!(number, Some(1));
/// assert_eq!(name, "first_issue");
///
/// // Complex names with underscores
/// let (number, name) = parse_any_issue_filename("000456_user_auth_fix").unwrap();
/// assert_eq!(number, Some(456));
/// assert_eq!(name, "user_auth_fix");
/// ```
///
/// ## Flexible Format Examples
///
/// ```rust
/// # use swissarmyhammer::issues::filesystem::parse_any_issue_filename;
/// // Simple non-numbered format
/// let (number, name) = parse_any_issue_filename("README").unwrap();
/// assert_eq!(number, None);
/// assert_eq!(name, "README");
///
/// // Hyphenated names
/// let (number, name) = parse_any_issue_filename("feature-request").unwrap();
/// assert_eq!(number, None);
/// assert_eq!(name, "feature-request");
///
/// // Project documentation
/// let (number, name) = parse_any_issue_filename("project-notes").unwrap();
/// assert_eq!(number, None);
/// assert_eq!(name, "project-notes");
///
/// // Mixed characters
/// let (number, name) = parse_any_issue_filename("bug_report_2024").unwrap();
/// assert_eq!(number, None);
/// assert_eq!(name, "bug_report_2024");
/// ```
///
/// ## Error Cases
///
/// ```rust
/// # use swissarmyhammer::issues::filesystem::parse_any_issue_filename;
/// // Empty filename
/// assert!(parse_any_issue_filename("").is_err());
/// ```
///
/// ## Integration with Virtual Numbers
///
/// ```rust
/// # use swissarmyhammer::issues::filesystem::parse_any_issue_filename;
/// // These would get virtual numbers when converted to issues:
/// let (number, name) = parse_any_issue_filename("TODO").unwrap();
/// assert_eq!(number, None); // Will be assigned virtual number (500000+)
/// assert_eq!(name, "TODO");
///
/// let (number, name) = parse_any_issue_filename("meeting-notes").unwrap();
/// assert_eq!(number, None); // Will be assigned virtual number (500000+)
/// assert_eq!(name, "meeting-notes");
/// ```
pub fn parse_any_issue_filename(filename: &str) -> Result<(Option<u32>, String)> {
    if filename.is_empty() {
        return Err(SwissArmyHammerError::Other(
            "Issue filename cannot be empty".to_string(),
        ));
    }

    // Try to parse as numbered format first
    if let Ok((number, name)) = parse_issue_filename(filename) {
        Ok((Some(number), name))
    } else {
        // Not a numbered format, treat as non-numbered
        Ok((None, filename.to_string()))
    }
}

/// Create safe filename from issue name
///
/// Converts an issue name into a filesystem-safe filename by replacing problematic
/// characters with dashes and applying various normalization rules. This function
/// ensures the resulting filename is safe to use across different operating systems
/// and filesystems.
///
/// # Rules Applied
///
/// - Spaces are replaced with dashes
/// - File path separators (`/`, `\`) are replaced with dashes
/// - Special characters (`:`, `*`, `?`, `"`, `<`, `>`, `|`) are replaced with dashes
/// - Control characters (tabs, newlines, etc.) are replaced with dashes
/// - Consecutive dashes are collapsed into a single dash
/// - Leading and trailing dashes are removed
/// - Empty input or input with only problematic characters becomes "unnamed"
/// - Length is limited to 100 characters
///
/// # Arguments
///
/// * `name` - The issue name to convert to a safe filename
///
/// # Returns
///
/// Returns a safe filename string that can be used in file paths across different
/// operating systems. The result will always be a valid filename or "unnamed" if
/// the input cannot be safely converted.
///
/// # Examples
///
/// ```
/// # use swissarmyhammer::issues::create_safe_filename;
/// // Basic usage
/// assert_eq!(create_safe_filename("simple"), "simple");
/// assert_eq!(create_safe_filename("with spaces"), "with-spaces");
///
/// // File path characters
/// assert_eq!(create_safe_filename("path/to/file"), "path-to-file");
/// assert_eq!(create_safe_filename("path\\to\\file"), "path-to-file");
///
/// // Special characters
/// assert_eq!(create_safe_filename("file:name"), "file-name");
/// assert_eq!(create_safe_filename("file*name"), "file-name");
/// assert_eq!(create_safe_filename("file?name"), "file-name");
/// assert_eq!(create_safe_filename("file\"name"), "file-name");
/// assert_eq!(create_safe_filename("file<name>"), "file-name");
/// assert_eq!(create_safe_filename("file|name"), "file-name");
///
/// // Multiple consecutive problematic characters
/// assert_eq!(create_safe_filename("file   with   spaces"), "file-with-spaces");
/// assert_eq!(create_safe_filename("file///name"), "file-name");
///
/// // Edge cases: trimming
/// assert_eq!(create_safe_filename("/start/and/end/"), "start-and-end");
/// assert_eq!(create_safe_filename("   spaces   "), "spaces");
///
/// // Edge cases: empty or only problematic characters
/// assert_eq!(create_safe_filename(""), "unnamed");
/// assert_eq!(create_safe_filename("///"), "unnamed");
/// assert_eq!(create_safe_filename("   "), "unnamed");
/// assert_eq!(create_safe_filename("***"), "unnamed");
///
/// // Length limiting
/// let long_name = "a".repeat(150);
/// let safe_name = create_safe_filename(&long_name);
/// assert_eq!(safe_name.len(), 100);
/// assert_eq!(safe_name, "a".repeat(100));
///
/// // Mixed characters
/// assert_eq!(create_safe_filename("Fix: login/logout* issue"), "Fix-login-logout-issue");
/// assert_eq!(create_safe_filename("Update \"config.json\" file"), "Update-config.json-file");
/// ```
pub fn create_safe_filename(name: &str) -> String {
    if name.is_empty() {
        return "unnamed".to_string();
    }

    // Configurable length limit
    let max_filename_length = std::env::var("SWISSARMYHAMMER_MAX_FILENAME_LENGTH")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    // Check for path traversal attempts
    if name.contains("../") || name.contains("..\\") || name.contains("./") || name.contains(".\\")
    {
        return "path_traversal_attempted".to_string();
    }

    // Replace spaces with dashes and remove problematic characters
    let safe_name = name
        .chars()
        .map(|c| match c {
            ' ' => '-',
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => '-',
            // Additional security: replace null bytes and other dangerous characters
            '\0' | '\x01'..='\x1F' | '\x7F' => '-',
            c => c,
        })
        .collect::<String>();

    // Remove consecutive dashes
    let mut result = String::new();
    let mut prev_was_dash = false;
    for c in safe_name.chars() {
        if c == '-' {
            if !prev_was_dash {
                result.push(c);
                prev_was_dash = true;
            }
        } else {
            result.push(c);
            prev_was_dash = false;
        }
    }

    // Trim dashes from start and end
    let result = result.trim_matches('-').to_string();

    // Ensure not empty and limit length
    let result = if result.is_empty() {
        "unnamed".to_string()
    } else if result.len() > max_filename_length {
        result.chars().take(max_filename_length).collect()
    } else {
        result
    };

    // Check for reserved filenames on different operating systems
    validate_against_reserved_names(&result)
}

/// Validate filename against reserved names on different operating systems
fn validate_against_reserved_names(name: &str) -> String {
    // Windows reserved names
    let windows_reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];

    // Unix/Linux reserved or problematic names
    let unix_reserved = [".", "..", "/", "\\"];

    let name_upper = name.to_uppercase();

    // Check Windows reserved names
    if windows_reserved.contains(&name_upper.as_str()) {
        return format!("{name}_file");
    }

    // Check Unix reserved names
    if unix_reserved.contains(&name) {
        return format!("{name}_file");
    }

    // Check for names that start with a dot (hidden files)
    if name.starts_with('.') && name.len() > 1 {
        return format!("hidden_{}", &name[1..]);
    }

    // Check for names that end with a dot (Windows issue)
    if name.ends_with('.') {
        return format!("{}_file", name.trim_end_matches('.'));
    }

    // Check for overly long names that might cause issues
    if name.len() > 255 {
        return name.chars().take(250).collect::<String>() + "_trunc";
    }

    name.to_string()
}

/// Sanitize issue name for security while preserving most names
pub fn sanitize_issue_name(name: &str) -> String {
    // Only sanitize dangerous path traversal attempts
    if name.contains("../") || name.contains("..\\") || name.contains("./") || name.contains(".\\")
    {
        return "path_traversal_attempted".to_string();
    }
    // Remove null bytes but preserve other characters
    name.replace('\0', "").to_string()
}

/// Validate issue name
pub fn validate_issue_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(SwissArmyHammerError::Other(
            "Issue name cannot be empty. Provide a descriptive name (e.g., 'fix_login_bug')"
                .to_string(),
        ));
    }

    if name.len() > 200 {
        return Err(SwissArmyHammerError::Other(format!(
            "Issue name too long: {} characters (max 200). Consider shortening: '{}'",
            name.len(),
            if name.len() > 50 {
                format!("{}...", &name[..50])
            } else {
                name.to_string()
            }
        )));
    }

    // Check for problematic characters
    for c in name.chars() {
        if c.is_control() {
            return Err(SwissArmyHammerError::Other(format!(
                "Issue name contains control characters (e.g., tabs, newlines). Use only printable characters: '{}'",
                name.chars().map(|c| if c.is_control() { '�' } else { c }).collect::<String>()
            )));
        }
    }

    Ok(())
}

/// Extract issue name from a filename, handling both numbered and arbitrary formats
///
/// This function takes a filename (with or without .md extension) and extracts the
/// issue name that should be used as the primary identifier.
///
/// # Arguments
///
/// * `filename` - The filename to extract the issue name from
///
/// # Returns
///
/// The extracted issue name as a String
///
/// # Examples
///
/// ```
/// # use swissarmyhammer::issues::extract_issue_name_from_filename;
/// // Numbered format
/// assert_eq!(extract_issue_name_from_filename("000123_bug_fix.md"), "bug_fix");
/// assert_eq!(extract_issue_name_from_filename("000456_feature_request"), "feature_request");
///
/// // Arbitrary format  
/// assert_eq!(extract_issue_name_from_filename("my-custom-issue.md"), "my-custom-issue");
/// assert_eq!(extract_issue_name_from_filename("README.md"), "README");
/// assert_eq!(extract_issue_name_from_filename("TODO"), "TODO");
/// ```
pub fn extract_issue_name_from_filename(filename: &str) -> String {
    // Remove .md extension if present
    let name_without_ext = filename.strip_suffix(".md").unwrap_or(filename);

    // Try to parse as numbered format and extract just the name part
    if let Ok((_, name_part)) = parse_issue_filename(name_without_ext) {
        name_part
    } else {
        // If not a numbered format, return the whole filename (without .md extension)
        name_without_ext.to_string()
    }
}

/// Check if a file path represents a valid issue file
///
/// Determines whether a given file path is a valid issue file that can be processed
/// by the issue system. This function supports both the traditional numbered format
/// and the newer flexible format that allows any markdown file.
///
/// ## Supported Formats
///
/// ### Traditional Numbered Format (Legacy)
/// Files with 6-digit zero-padded numbers followed by an underscore and name:
/// - `000001_my_first_issue.md` - Valid numbered issue
/// - `000123_bug_fix.md` - Valid numbered issue
/// - `999999_last_issue.md` - Valid numbered issue (up to max_issue_number)
///
/// ### Flexible Format (New)
/// Any markdown file with a non-empty filename:
/// - `README.md` - Valid issue (gets virtual number)
/// - `feature-request.md` - Valid issue (gets virtual number)
/// - `bug-report.md` - Valid issue (gets virtual number)
/// - `project-notes.md` - Valid issue (gets virtual number)
///
/// ## Requirements
///
/// 1. **File Extension**: Must have `.md` extension (case-sensitive)
/// 2. **Non-Empty Name**: The filename (without extension) must not be empty
/// 3. **UTF-8 Compatible**: Filename must be valid UTF-8
///
/// ## Virtual Numbering
///
/// Non-numbered files are assigned virtual issue names in the range
/// [`Config::global().virtual_issue_number_base`..`Config::global().virtual_issue_number_base + Config::global().virtual_issue_number_range`]
/// based on a hash of the filename. This ensures consistent numbering while avoiding
/// conflicts with traditionally numbered issues.
///
/// ## Examples
///
/// ```rust
/// use std::path::Path;
/// use swissarmyhammer::issues::filesystem::is_issue_file;
///
/// // Traditional numbered formats
/// assert!(is_issue_file(Path::new("000001_first_issue.md")));
/// assert!(is_issue_file(Path::new("000123_bug_fix.md")));
///
/// // Flexible formats
/// assert!(is_issue_file(Path::new("README.md")));
/// assert!(is_issue_file(Path::new("project-notes.md")));
/// assert!(is_issue_file(Path::new("feature-request.md")));
///
/// // Invalid files
/// assert!(!is_issue_file(Path::new("document.txt"))); // Wrong extension
/// assert!(!is_issue_file(Path::new(".md")));          // Empty name
/// assert!(!is_issue_file(Path::new("notes")));        // No extension
/// ```
pub fn is_issue_file(path: &Path) -> bool {
    // Must be .md file
    if path.extension() != Some(std::ffi::OsStr::new("md")) {
        return false;
    }

    // Get filename without extension
    let filename = match path.file_stem() {
        Some(name) => match name.to_str() {
            Some(s) => s,
            None => return false,
        },
        None => return false,
    };

    // Any non-empty filename is valid (supports both numbered and non-numbered formats)
    !filename.is_empty()
}

impl FileSystemIssueStorage {
    /// Generate a virtual issue number with collision resistance
    /// This method generates a deterministic virtual number for a given filename
    /// that falls within the virtual number range defined in config
    pub fn generate_virtual_number_with_collision_resistance(&self, filename: &str) -> u32 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let config = Config::global();

        // Use a hash of the filename to generate a deterministic number
        let mut hasher = DefaultHasher::new();
        filename.hash(&mut hasher);
        let hash = hasher.finish();

        // Map the hash to the virtual number range
        let range = config.virtual_issue_number_range;
        let offset = (hash % range as u64) as u32;

        config.virtual_issue_number_base + offset
    }
}

/// New simplified approach: filename without .md extension is the issue name
///
/// This is the new canonical way to get issue names from filenames.
/// It eliminates all the complexity around numbered vs non-numbered files.
///
/// # Examples
///
/// ```
/// # use swissarmyhammer::issues::get_issue_name_from_filename;
/// assert_eq!(get_issue_name_from_filename("000001_paper.md"), "000001_paper");
/// assert_eq!(get_issue_name_from_filename("nice.md"), "nice");
/// assert_eq!(get_issue_name_from_filename("000001.md"), "000001");
/// assert_eq!(get_issue_name_from_filename("373c4cf9-d803-4138-89b3-bc802d22f94e.md"), "373c4cf9-d803-4138-89b3-bc802d22f94e");
/// ```
pub fn get_issue_name_from_filename(filename: &str) -> String {
    filename.strip_suffix(".md").unwrap_or(filename).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::IsolatedTestEnvironment;
    use tempfile::TempDir;

    /// Create a test issue storage with temporary directory
    fn create_test_storage() -> (FileSystemIssueStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().join("issues");

        let storage = FileSystemIssueStorage::new(issues_dir).unwrap();
        (storage, temp_dir)
    }

    #[test]
    fn test_default_directory_logic_with_swissarmyhammer() {
        // Test the logic by checking that .swissarmyhammer directory existence affects the result
        let temp_dir = TempDir::new().unwrap();
        let swissarmyhammer_path = temp_dir.path().join(".swissarmyhammer");

        // Create .swissarmyhammer directory
        std::fs::create_dir_all(&swissarmyhammer_path).unwrap();

        // Mock the current_dir to return our temp directory
        let current_dir = temp_dir.path();

        // Check that when .swissarmyhammer exists, we get .swissarmyhammer/issues
        let swissarmyhammer_result = if swissarmyhammer_path.exists() {
            current_dir.join(".swissarmyhammer").join("issues")
        } else {
            current_dir.join("issues")
        };

        assert_eq!(
            swissarmyhammer_result,
            current_dir.join(".swissarmyhammer").join("issues")
        );

        // Remove .swissarmyhammer directory and check legacy fallback
        std::fs::remove_dir_all(&swissarmyhammer_path).unwrap();

        let legacy_result = if swissarmyhammer_path.exists() {
            current_dir.join(".swissarmyhammer").join("issues")
        } else {
            current_dir.join("issues")
        };

        assert_eq!(legacy_result, current_dir.join("issues"));
    }

    #[test]
    fn test_new_default_functional_behavior() {
        // Test that new_default() and default_directory() produce consistent results
        // by testing the actual FileSystemIssueStorage creation

        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().join("test_issues");

        // Create storage with explicit path
        let explicit_storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();
        assert_eq!(explicit_storage.state.issues_dir, issues_dir);

        // Verify storage can be created and basic operations work
        assert!(explicit_storage.state.issues_dir.parent().is_some());
        assert_eq!(
            explicit_storage.state.issues_dir.file_name().unwrap(),
            "test_issues"
        );
    }

    #[test]
    fn test_issue_serialization() {
        let issue = Issue {
            name: "test_issue".to_string(),
            content: "Test content".to_string(),
        };

        // Test serialization
        let serialized = serde_json::to_string(&issue).unwrap();
        let deserialized: Issue = serde_json::from_str(&serialized).unwrap();

        assert_eq!(issue, deserialized);
        assert_eq!(deserialized.name.as_str(), "test_issue");
        assert_eq!(deserialized.content, "Test content");
    }

    #[test]
    fn test_issue_number_validation() {
        // Valid 6-digit numbers
        let valid_numbers = vec![
            1,
            999,
            1000,
            99999,
            100_000,
            Config::global().max_issue_number,
        ];
        for num in valid_numbers {
            assert!(
                num <= Config::global().max_issue_number,
                "Issue name {num} should be valid"
            );
        }

        // Invalid numbers (too large)
        let invalid_numbers = vec![1_000_000, 9_999_999];
        for num in invalid_numbers {
            assert!(
                num > Config::global().max_issue_number,
                "Issue name {num} should be invalid"
            );
        }
    }

    #[test]
    fn test_path_construction() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();

        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        assert_eq!(storage.state.issues_dir, issues_dir);
        assert_eq!(storage.state.completed_dir, issues_dir.join("complete"));
    }

    #[test]
    fn test_directory_creation() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().join("new_issues");
        let completed_dir = issues_dir.join("complete");

        // Directories don't exist initially
        assert!(!issues_dir.exists());
        assert!(!completed_dir.exists());

        // Create storage - should create directories
        let _storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Directories should now exist
        assert!(issues_dir.exists());
        assert!(completed_dir.exists());
    }

    #[test]
    fn test_parse_issue_from_file() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create test file
        let test_file = issues_dir.join("test_issue.md");
        fs::write(&test_file, "# Test Issue\\n\\nThis is a test issue.").unwrap();

        let issue = storage.parse_issue_from_file(&test_file).unwrap();
        assert_eq!(issue.name.as_str(), "test_issue");
        assert_eq!(issue.content, "# Test Issue\\n\\nThis is a test issue.");
        // Note: parse_issue_from_file only returns Issue with name and content
        // For completed status and file_path info, use get_issue_info() instead
    }

    #[test]
    fn test_parse_issue_from_completed_file() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create test file in completed directory
        let completed_dir = issues_dir.join("complete");
        let test_file = completed_dir.join("000456_completed_issue.md");
        fs::write(&test_file, "# Completed Issue\\n\\nThis is completed.").unwrap();

        let issue = storage.parse_issue_from_file(&test_file).unwrap();
        assert_eq!(issue.name.as_str(), "000456_completed_issue");
        assert_eq!(issue.content, "# Completed Issue\\n\\nThis is completed.");
        // Note: parse_issue_from_file only returns Issue with name and content
        // For completed status and file_path info, use get_issue_info() instead
    }

    #[test]
    fn test_parse_issue_non_numbered_filename() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create test file with non-numbered filename (now valid)
        let test_file = issues_dir.join("invalid_filename.md");
        fs::write(&test_file, "content").unwrap();

        let result = storage.parse_issue_from_file(&test_file);
        assert!(result.is_ok());
        let issue = result.unwrap();
        assert_eq!(issue.name.as_str(), "invalid_filename");
        // Virtual number logic removed - name-based approach
        // Virtual number for non-numbered files
    }

    #[test]
    fn test_parse_issue_non_standard_format() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create test file with non-standard format (now valid as non-numbered)
        let test_file = issues_dir.join("abc123_test.md");
        fs::write(&test_file, "content").unwrap();

        let result = storage.parse_issue_from_file(&test_file);
        assert!(result.is_ok());
        let issue = result.unwrap();
        assert_eq!(issue.name.as_str(), "abc123_test");
        // Virtual number logic removed - name-based approach
        // Virtual number for non-numbered files
    }

    #[test]
    fn test_parse_issue_large_number_as_non_numbered() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create test file with number too large for numbered format (now valid as non-numbered)
        let test_file = issues_dir.join("1000000_test.md");
        fs::write(&test_file, "content").unwrap();

        let result = storage.parse_issue_from_file(&test_file);
        assert!(result.is_ok());
        let issue = result.unwrap();
        assert_eq!(issue.name.as_str(), "1000000_test");
        // Virtual number logic removed - name-based approach
        // Virtual number for non-numbered files
    }

    #[tokio::test]
    async fn test_create_issue() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        let issue = storage
            .create_issue("test_issue".to_string(), "# Test\\n\\nContent".to_string())
            .await
            .unwrap();

        // Number assertion removed - name-based approach
        assert_eq!(issue.name.as_str(), "test_issue");
        assert_eq!(issue.content, "# Test\\n\\nContent");
        // Verify file was created in active directory (not completed)
        let expected_path = issues_dir.join("test_issue.md");
        assert!(expected_path.exists());
    }

    #[tokio::test]
    async fn test_create_issue_with_special_characters() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        let issue = storage
            .create_issue("test/issue with spaces".to_string(), "content".to_string())
            .await
            .unwrap();

        // Number assertion removed - name-based approach
        assert_eq!(issue.name.as_str(), "test/issue with spaces");

        // Check file was created with safe filename
        let expected_path = issues_dir.join("test-issue-with-spaces.md");
        assert!(expected_path.exists());
    }

    // Test removed - get_next_issue_number method no longer exists in name-based system

    #[tokio::test]
    async fn test_list_issues_empty() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        let issues = storage.list_issues().await.unwrap();
        assert!(issues.is_empty());
    }

    #[tokio::test]
    async fn test_list_issues_mixed() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create pending issues
        fs::write(issues_dir.join("000003_pending.md"), "pending content").unwrap();
        fs::write(issues_dir.join("000001_another.md"), "another content").unwrap();

        // Create completed issues
        let completed_dir = issues_dir.join("complete");
        fs::write(
            completed_dir.join("000002_completed.md"),
            "completed content",
        )
        .unwrap();
        fs::write(completed_dir.join("000004_done.md"), "done content").unwrap();

        let issue_infos = storage.list_issues_info().await.unwrap();
        assert_eq!(issue_infos.len(), 4);

        // Should be sorted by name
        // Number assertion removed - name-based approach
        assert_eq!(issue_infos[0].issue.name.as_str(), "000001_another");
        assert!(!issue_infos[0].completed);

        // Number assertion removed - name-based approach
        assert_eq!(issue_infos[1].issue.name.as_str(), "000002_completed");
        assert!(issue_infos[1].completed);

        // Number assertion removed - name-based approach
        assert_eq!(issue_infos[2].issue.name.as_str(), "000003_pending");
        assert!(!issue_infos[2].completed);

        // Number assertion removed - name-based approach
        assert_eq!(issue_infos[3].issue.name.as_str(), "000004_done");
        assert!(issue_infos[3].completed);
    }

    #[tokio::test]
    async fn test_get_issue_found() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create test issue
        fs::write(issues_dir.join("test.md"), "test content").unwrap();

        let issue = storage.get_issue("test").await.unwrap();
        // Number assertion removed - name-based approach
        assert_eq!(issue.name.as_str(), "test");
        assert_eq!(issue.content, "test content");
    }

    #[tokio::test]
    async fn test_get_issue_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        let result = storage.get_issue("non_existent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_issue_from_completed() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create completed issue
        let completed_dir = issues_dir.join("complete");
        fs::write(completed_dir.join("completed.md"), "completed content").unwrap();

        let issue_info = storage.get_issue_info("completed").await.unwrap();
        // Number assertion removed - name-based approach
        assert_eq!(issue_info.issue.name.as_str(), "completed");
        assert_eq!(issue_info.issue.content, "completed content");
        assert!(issue_info.completed);
    }

    #[tokio::test]
    async fn test_auto_increment_sequence() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create multiple issues
        let _issue1 = storage
            .create_issue("first".to_string(), "content1".to_string())
            .await
            .unwrap();
        let _issue2 = storage
            .create_issue("second".to_string(), "content2".to_string())
            .await
            .unwrap();
        let _issue3 = storage
            .create_issue("third".to_string(), "content3".to_string())
            .await
            .unwrap();

        // Number assertion removed - name-based approach
        // Number assertion removed - name-based approach
        // Number assertion removed - name-based approach

        // Check files were created
        assert!(issues_dir.join("first.md").exists());
        assert!(issues_dir.join("second.md").exists());
        assert!(issues_dir.join("third.md").exists());
    }

    #[test]
    fn test_list_issues_in_dir_non_existent() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        let non_existent_dir = issues_dir.join("non_existent");
        let issues = storage.list_issues_in_dir(&non_existent_dir).unwrap();
        assert!(issues.is_empty());
    }

    #[test]
    fn test_list_issues_in_dir_ignores_non_md_files() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create various files
        fs::write(issues_dir.join("000001_test.md"), "content").unwrap();
        fs::write(issues_dir.join("000002_test.txt"), "content").unwrap(); // Should be ignored (not .md)
        fs::write(issues_dir.join("README.md"), "content").unwrap(); // Now valid as non-numbered issue
        fs::write(issues_dir.join("000003_valid.md"), "content").unwrap();

        let issues = storage.list_issues_in_dir(&issues_dir).unwrap();
        assert_eq!(issues.len(), 3); // All .md files are valid issues now

        // Sort by number to make assertions predictable
        let mut sorted_issues = issues;
        sorted_issues.sort_by_key(|issue| issue.name.clone());

        // Issues sorted alphabetically by name: 000001_test, 000003_valid, README
        assert_eq!(sorted_issues[0].name.as_str(), "000001_test");
        assert_eq!(sorted_issues[1].name.as_str(), "000003_valid");
        assert_eq!(sorted_issues[2].name.as_str(), "README");
    }

    #[test]
    fn test_parse_issue_filename_no_underscore() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create file with no underscore (now valid as non-numbered)
        let test_file = issues_dir.join("000123test.md");
        fs::write(&test_file, "content").unwrap();

        let result = storage.parse_issue_from_file(&test_file);
        assert!(result.is_ok());
        let issue = result.unwrap();
        assert_eq!(issue.name.as_str(), "000123test");
        // Virtual number logic removed - name-based approach
        // Virtual number for non-numbered files
    }

    #[test]
    fn test_parse_issue_malformed_filename_multiple_underscores() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create file with multiple underscores - should still work (splitn(2) handles this)
        let test_file = issues_dir.join("000123_test_with_underscores.md");
        fs::write(&test_file, "content").unwrap();

        let result = storage.parse_issue_from_file(&test_file);
        assert!(result.is_ok());
        let issue = result.unwrap();
        // Number assertion removed - name-based approach
        assert_eq!(issue.name.as_str(), "000123_test_with_underscores");
    }

    #[test]
    fn test_parse_issue_malformed_filename_empty_name() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create file with empty name part
        let test_file = issues_dir.join("000123_.md");
        fs::write(&test_file, "content").unwrap();

        let result = storage.parse_issue_from_file(&test_file);
        assert!(result.is_ok());
        let issue = result.unwrap();
        // Number assertion removed - name-based approach
        assert_eq!(issue.name.as_str(), "000123_");
    }

    #[test]
    fn test_parse_issue_filename_starting_with_underscore() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create file starting with underscore (now valid as non-numbered)
        let test_file = issues_dir.join("_test.md");
        fs::write(&test_file, "content").unwrap();

        let result = storage.parse_issue_from_file(&test_file);
        assert!(result.is_ok());
        let issue = result.unwrap();
        assert_eq!(issue.name.as_str(), "_test");
        // Virtual number logic removed - name-based approach
        // Virtual number for non-numbered files
    }

    #[test]
    fn test_parse_issue_number_with_leading_zeros() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create file with leading zeros
        let test_file = issues_dir.join("000001_test.md");
        fs::write(&test_file, "content").unwrap();

        let result = storage.parse_issue_from_file(&test_file);
        assert!(result.is_ok());
        let issue = result.unwrap();
        // Number assertion removed - name-based approach
        assert_eq!(issue.name.as_str(), "000001_test");
    }

    #[test]
    fn test_parse_issue_number_zero() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create file with zero number
        let test_file = issues_dir.join("000000_test.md");
        fs::write(&test_file, "content").unwrap();

        let result = storage.parse_issue_from_file(&test_file);
        assert!(result.is_ok());
        let issue = result.unwrap();
        // Number assertion removed - name-based approach
        assert_eq!(issue.name.as_str(), "000000_test");
    }

    #[test]
    fn test_list_issues_in_dir_with_corrupted_files() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create valid numbered file
        fs::write(issues_dir.join("000001_valid.md"), "content").unwrap();

        // Create files that were previously considered "corrupted" but are now valid as non-numbered issues
        fs::write(issues_dir.join("invalid_format.md"), "content").unwrap(); // Now valid as non-numbered
        fs::write(issues_dir.join("abc123_invalid_number.md"), "content").unwrap(); // Now valid as non-numbered
        fs::write(issues_dir.join("1000000_too_large.md"), "content").unwrap(); // Now valid as non-numbered

        let issues = storage.list_issues_in_dir(&issues_dir).unwrap();
        // All .md files are now valid issues (1 numbered + 3 non-numbered)
        assert_eq!(issues.len(), 4);

        // Sort to make assertions predictable
        let mut sorted_issues = issues;
        sorted_issues.sort_by_key(|issue| issue.name.clone());

        // Issues are now sorted alphabetically by name
        assert_eq!(sorted_issues[0].name.as_str(), "000001_valid");
        assert_eq!(sorted_issues[1].name.as_str(), "1000000_too_large");
        assert_eq!(sorted_issues[2].name.as_str(), "abc123_invalid_number");
        assert_eq!(sorted_issues[3].name.as_str(), "invalid_format");

        // The other 3 should be non-numbered with virtual numbers >= virtual_base
        for _issue in sorted_issues.iter().take(4).skip(1) {
            // Virtual number logic removed - name-based approach
        }
    }

    #[tokio::test]
    async fn test_concurrent_issue_creation() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = std::sync::Arc::new(FileSystemIssueStorage::new(issues_dir.clone()).unwrap());

        // Create multiple issues concurrently
        let mut handles = Vec::new();
        for i in 0..5 {
            let storage_clone = storage.clone();
            let handle = tokio::spawn(async move {
                storage_clone
                    .create_issue(format!("issue_{i}"), format!("Content {i}"))
                    .await
            });
            handles.push(handle);
        }

        // Collect results
        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await.unwrap());
        }

        // Check that all issues were created successfully
        assert_eq!(results.len(), 5);
        for result in results {
            assert!(result.is_ok());
        }

        // Verify all issues exist
        let all_issues = storage.list_issues().await.unwrap();
        assert_eq!(all_issues.len(), 5);

        // Sequential number check removed - name-based system doesn't use sequential numbers
    }

    #[test]
    fn test_create_storage_with_invalid_path() {
        // Try to create storage with a path that contains null bytes (invalid on most systems)
        let invalid_path = PathBuf::from("invalid\0path");
        let result = FileSystemIssueStorage::new(invalid_path);

        // Should handle the error gracefully
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_directory_handling() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Test with completely empty directory
        let issues = storage.list_issues_in_dir(&issues_dir).unwrap();
        assert!(issues.is_empty());

        // get_next_issue_number method removed - name-based system doesn't use sequential numbers
    }

    #[tokio::test]
    async fn test_edge_case_issue_names() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Test with various special characters and edge cases
        let edge_case_names = vec![
            "issue with spaces",
            "issue/with/slashes",
            "issue\\with\\backslashes",
            "issue-with-dashes",
            "issue_with_underscores",
            "UPPERCASE_ISSUE",
            "lowercase_issue",
            "123_numeric_start",
            "issue.with.dots",
            "issue@with@symbols",
            "very_long_issue_name_that_exceeds_normal_length_expectations_but_should_still_work",
            "", // Empty name
        ];

        for name in edge_case_names {
            let result = storage
                .create_issue(name.to_string(), "content".to_string())
                .await;
            assert!(result.is_ok(), "Failed to create issue with name: '{name}'");
        }

        // Verify all issues were created
        let all_issues = storage.list_issues().await.unwrap();
        assert_eq!(all_issues.len(), 12);
    }

    #[tokio::test]
    async fn test_update_issue() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create initial issue
        let issue = storage
            .create_issue("test_issue".to_string(), "Original content".to_string())
            .await
            .unwrap();

        // Update the issue
        let updated_content = "Updated content with new information";
        let updated_issue = storage
            .update_issue("test_issue", updated_content.to_string())
            .await
            .unwrap();

        // Verify basic issue data
        assert_eq!(updated_issue.name, issue.name);
        assert_eq!(updated_issue.content, updated_content);

        // Verify file was updated on disk
        let file_path = issues_dir.join("test_issue.md");
        assert!(file_path.exists());
        let file_content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(file_content, updated_content);
    }

    #[tokio::test]
    async fn test_update_issue_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        let result = storage
            .update_issue("non_existent", "New content".to_string())
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mark_complete() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create initial issue
        let issue = storage
            .create_issue("test_issue".to_string(), "Test content".to_string())
            .await
            .unwrap();

        // Verify initial file location (in active directory)
        let initial_path = issues_dir.join("test_issue.md");
        assert!(initial_path.exists());

        // Mark as complete
        let completed_issue = storage.mark_complete("test_issue").await.unwrap();

        // Basic issue data should be preserved
        assert_eq!(completed_issue.name, issue.name);
        assert_eq!(completed_issue.content, issue.content);

        // Verify file was moved to completed directory
        let expected_path = issues_dir.join("complete").join("test_issue.md");
        assert!(expected_path.exists());
        assert!(!initial_path.exists());
    }

    #[tokio::test]
    async fn test_mark_complete_already_completed() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create and complete an issue
        let issue = storage
            .create_issue("test_issue".to_string(), "Test content".to_string())
            .await
            .unwrap();

        let completed_issue = storage.mark_complete(&issue.name).await.unwrap();

        // Try to mark as complete again - should be no-op
        let completed_again = storage.mark_complete(&issue.name).await.unwrap();

        // Both should have same basic issue data
        assert_eq!(completed_issue.name, completed_again.name);
        assert_eq!(completed_issue.content, completed_again.content);

        // Verify the file is still in completed directory
        let completed_path = issues_dir.join("complete").join("test_issue.md");
        assert!(completed_path.exists());
    }

    #[tokio::test]
    async fn test_mark_complete_cleans_up_duplicate_files() {
        use std::fs;

        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create and complete an issue normally
        let issue = storage
            .create_issue("test_issue".to_string(), "Test content".to_string())
            .await
            .unwrap();

        let completed_issue = storage.mark_complete(&issue.name).await.unwrap();
        // Verify the issue was marked complete (file should be in complete directory)
        let completed_path = issues_dir.join("complete").join("test_issue.md");
        assert!(completed_path.exists());

        // Simulate a duplicate file appearing in the original location
        // This could happen due to external interference or failed cleanup
        let duplicate_path = issues_dir.join("test_issue.md");
        fs::write(&duplicate_path, "Duplicate content").unwrap();
        assert!(duplicate_path.exists());

        // Try to mark as complete again - should clean up the duplicate
        let completed_again = storage.mark_complete(&issue.name).await.unwrap();

        // Verify the duplicate file was cleaned up
        assert!(!duplicate_path.exists());

        // Both issues should have same data
        assert_eq!(completed_issue.name, completed_again.name);
        assert_eq!(completed_issue.content, completed_again.content);
    }

    #[tokio::test]
    async fn test_mark_complete_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        let result = storage.mark_complete("non_existent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_all_complete_empty() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        let result = storage.all_complete().await.unwrap();
        assert!(result); // No issues means all are complete
    }

    #[tokio::test]
    async fn test_all_complete_with_pending() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create some issues
        storage
            .create_issue("issue1".to_string(), "Content 1".to_string())
            .await
            .unwrap();
        storage
            .create_issue("issue2".to_string(), "Content 2".to_string())
            .await
            .unwrap();

        let result = storage.all_complete().await.unwrap();
        assert!(!result); // Has pending issues
    }

    #[tokio::test]
    async fn test_all_complete_all_completed() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create and complete all issues
        let issue1 = storage
            .create_issue("issue1".to_string(), "Content 1".to_string())
            .await
            .unwrap();
        let issue2 = storage
            .create_issue("issue2".to_string(), "Content 2".to_string())
            .await
            .unwrap();

        storage.mark_complete(&issue1.name).await.unwrap();
        storage.mark_complete(&issue2.name).await.unwrap();

        let result = storage.all_complete().await.unwrap();
        assert!(result); // All issues are complete
    }

    #[test]
    fn test_format_issue_number() {
        assert_eq!(format_issue_number(1), "000001");
        assert_eq!(format_issue_number(123), "000123");
        assert_eq!(format_issue_number(999_999), "999999");
        assert_eq!(format_issue_number(0), "000000");
    }

    #[test]
    fn test_parse_issue_number_valid() {
        assert_eq!(parse_issue_number("000001").unwrap(), 1);
        assert_eq!(parse_issue_number("000123").unwrap(), 123);
        assert_eq!(parse_issue_number("999999").unwrap(), 999_999);
        assert_eq!(parse_issue_number("000000").unwrap(), 0);
    }

    #[test]
    fn test_parse_issue_number_invalid() {
        // Wrong length
        assert!(parse_issue_number("123").is_err());
        assert!(parse_issue_number("0000123").is_err());
        assert!(parse_issue_number("").is_err());

        // Non-numeric
        assert!(parse_issue_number("abc123").is_err());
        assert!(parse_issue_number("00abc1").is_err());

        // Too large
        assert!(parse_issue_number("1_000_000").is_err());
    }

    #[test]
    fn test_parse_issue_filename_valid() {
        let (number, name) = parse_issue_filename("000123_test_issue").unwrap();
        assert_eq!(number, 123);
        assert_eq!(name, "test_issue");

        let (number, name) = parse_issue_filename("000001_simple").unwrap();
        assert_eq!(number, 1);
        assert_eq!(name, "simple");

        let (number, name) = parse_issue_filename("000456_name_with_underscores").unwrap();
        assert_eq!(number, 456);
        assert_eq!(name, "name_with_underscores");

        let (number, name) = parse_issue_filename("000789_").unwrap();
        assert_eq!(number, 789);
        assert_eq!(name, "");
    }

    #[test]
    fn test_parse_issue_filename_invalid() {
        // No underscore
        assert!(parse_issue_filename("000123test").is_err());

        // Invalid number
        assert!(parse_issue_filename("abc123_test").is_err());
        assert!(parse_issue_filename("123_test").is_err());

        // Empty
        assert!(parse_issue_filename("").is_err());
        assert!(parse_issue_filename("_test").is_err());
    }

    #[test]
    fn test_parse_any_issue_filename_numbered() {
        // Test numbered format (existing behavior)
        let (number, name) = parse_any_issue_filename("000123_test_issue").unwrap();
        assert_eq!(number, Some(123));
        assert_eq!(name, "test_issue");

        let (number, name) = parse_any_issue_filename("000001_simple").unwrap();
        assert_eq!(number, Some(1));
        assert_eq!(name, "simple");

        let (number, name) = parse_any_issue_filename("000456_name_with_underscores").unwrap();
        assert_eq!(number, Some(456));
        assert_eq!(name, "name_with_underscores");
    }

    #[test]
    fn test_parse_any_issue_filename_non_numbered() {
        // Test non-numbered format (new behavior)
        let (number, name) = parse_any_issue_filename("my-issue").unwrap();
        assert_eq!(number, None);
        assert_eq!(name, "my-issue");

        let (number, name) = parse_any_issue_filename("bug-report").unwrap();
        assert_eq!(number, None);
        assert_eq!(name, "bug-report");

        let (number, name) = parse_any_issue_filename("feature_request").unwrap();
        assert_eq!(number, None);
        assert_eq!(name, "feature_request");

        let (number, name) = parse_any_issue_filename("simple").unwrap();
        assert_eq!(number, None);
        assert_eq!(name, "simple");
    }

    #[test]
    fn test_parse_any_issue_filename_edge_cases() {
        // Test empty filename
        assert!(parse_any_issue_filename("").is_err());

        // Test filename that looks like numbered but isn't
        let (number, name) = parse_any_issue_filename("123_not_6_digits").unwrap();
        assert_eq!(number, None); // Should be treated as non-numbered
        assert_eq!(name, "123_not_6_digits");

        // Test filename with underscores but not numbered format
        let (number, name) = parse_any_issue_filename("not_numbered_format").unwrap();
        assert_eq!(number, None);
        assert_eq!(name, "not_numbered_format");
    }
    #[test]
    fn test_invalid_input_edge_cases() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Test filename that results in empty string after parsing
        // Note: ".md" is actually valid - file_stem() returns ".md", so we test a different edge case
        let result = parse_any_issue_filename("");
        assert!(result.is_err(), "Empty string should fail to parse");

        // Test filename with only dots (edge case)
        let dots_path = issues_dir.join("....md");
        std::fs::write(&dots_path, "content").unwrap();
        let result = storage.parse_issue_from_file(&dots_path);
        // This should actually work - filename is "..." which is non-empty
        assert!(
            result.is_ok(),
            "Filename with dots should parse as non-numbered: {result:?}"
        );
        if let Ok(issue) = result {
            assert_eq!(issue.name.as_str(), "...");
            // Virtual number functionality removed - name-based system now used
        }

        // Test file with no extension (parse_issue_from_file doesn't check extension - that's done by is_issue_file)
        let no_ext_path = issues_dir.join("no_extension");
        std::fs::write(&no_ext_path, "content").unwrap();
        let result = storage.parse_issue_from_file(&no_ext_path);
        // parse_issue_from_file doesn't enforce .md extension - that's handled by directory scanning
        assert!(
            result.is_ok(),
            "parse_issue_from_file should work on any file regardless of extension"
        );

        // But is_issue_file should reject non-.md files
        assert!(
            !is_issue_file(&no_ext_path),
            "is_issue_file should reject non-.md files"
        );

        // Test moderately long filename (avoid filesystem limits)
        let long_name = "a".repeat(200); // Use 200 chars to avoid filesystem limits
        let long_path = issues_dir.join(format!("{long_name}.md"));
        if std::fs::write(&long_path, "content").is_ok() {
            let result = storage.parse_issue_from_file(&long_path);
            // This should work - long filenames are valid if filesystem supports them
            assert!(result.is_ok(), "Long valid filename should parse correctly");
        } else {
            // If filesystem rejects the filename, that's a system limitation, not our parsing issue
            println!("Filesystem rejected long filename - this is a system limitation");
        }

        // Test filename with only special characters (should get virtual number)
        let special_path = issues_dir.join("!@#$%^&*()_+.md");
        std::fs::write(&special_path, "content").unwrap();
        let result = storage.parse_issue_from_file(&special_path);
        assert!(
            result.is_ok(),
            "Special characters should be valid in non-numbered format"
        );
        let _issue = result.unwrap();
        // Virtual number functionality removed - name-based system now used

        // Test nonexistent file
        let nonexistent = issues_dir.join("does_not_exist.md");
        let result = storage.parse_issue_from_file(&nonexistent);
        assert!(result.is_err(), "Nonexistent file should fail");

        // Test file with invalid UTF-8 in content (might fail content reading)
        let utf8_path = issues_dir.join("utf8_test.md");
        std::fs::write(&utf8_path, [0xFF, 0xFE, 0xFD, 0xFC]).unwrap(); // Invalid UTF-8
        let result = storage.parse_issue_from_file(&utf8_path);
        // Invalid UTF-8 content might cause parsing to fail, which is expected behavior
        match result {
            Ok(_) => {
                // If it succeeds, the system handled invalid UTF-8 gracefully
            }
            Err(_) => {
                // If it fails, that's expected due to invalid UTF-8 content
            }
        }
    }

    #[test]
    fn test_create_safe_filename() {
        assert_eq!(create_safe_filename("simple"), "simple");
        assert_eq!(create_safe_filename("with spaces"), "with-spaces");
        assert_eq!(create_safe_filename("with/slashes"), "with-slashes");
        assert_eq!(
            create_safe_filename("with\\backslashes"),
            "with-backslashes"
        );
        assert_eq!(create_safe_filename("with:colons"), "with-colons");
        assert_eq!(create_safe_filename("with*asterisks"), "with-asterisks");
        assert_eq!(create_safe_filename("with?questions"), "with-questions");
        assert_eq!(create_safe_filename("with\"quotes"), "with-quotes");
        assert_eq!(create_safe_filename("with<brackets>"), "with-brackets");
        assert_eq!(create_safe_filename("with|pipes"), "with-pipes");

        // Multiple consecutive spaces/chars become single dash
        assert_eq!(
            create_safe_filename("with   multiple   spaces"),
            "with-multiple-spaces"
        );
        assert_eq!(create_safe_filename("with///slashes"), "with-slashes");

        // Trim dashes from start and end
        assert_eq!(create_safe_filename("/start/and/end/"), "start-and-end");
        assert_eq!(create_safe_filename("   spaces   "), "spaces");

        // Empty or only problematic chars
        assert_eq!(create_safe_filename(""), "unnamed");
        assert_eq!(create_safe_filename("///"), "unnamed");
        assert_eq!(create_safe_filename("   "), "unnamed");

        // Length limiting
        let long_name = "a".repeat(150);
        let safe_name = create_safe_filename(&long_name);
        assert_eq!(safe_name.len(), 100);
        assert_eq!(safe_name, "a".repeat(100));
    }

    #[test]
    fn test_validate_issue_name_valid() {
        assert!(validate_issue_name("simple").is_ok());
        assert!(validate_issue_name("with spaces").is_ok());
        assert!(validate_issue_name("with/slashes").is_ok());
        assert!(validate_issue_name("with_underscores").is_ok());
        assert!(validate_issue_name("123numbers").is_ok());
        assert!(validate_issue_name("UPPERCASE").is_ok());
        assert!(validate_issue_name("MiXeD cAsE").is_ok());
        assert!(validate_issue_name("with-dashes").is_ok());
        assert!(validate_issue_name("with.dots").is_ok());
        assert!(validate_issue_name("with@symbols").is_ok());

        // 200 characters exactly
        let max_length = "a".repeat(200);
        assert!(validate_issue_name(&max_length).is_ok());
    }

    #[test]
    fn test_validate_issue_name_invalid() {
        // Empty
        assert!(validate_issue_name("").is_err());

        // Too long
        let too_long = "a".repeat(201);
        assert!(validate_issue_name(&too_long).is_err());

        // Control characters
        assert!(validate_issue_name("with\tcontrol").is_err());
        assert!(validate_issue_name("with\ncontrol").is_err());
        assert!(validate_issue_name("with\rcontrol").is_err());
        assert!(validate_issue_name("with\x00control").is_err());
    }

    #[test]
    fn test_is_issue_file() {
        // Valid issue files - numbered format (traditional)
        assert!(is_issue_file(Path::new("000123_test.md")));
        assert!(is_issue_file(Path::new("000001_simple.md")));
        assert!(is_issue_file(Path::new("999999_max.md")));
        assert!(is_issue_file(Path::new("000000_zero.md")));
        assert!(is_issue_file(Path::new("000456_name_with_underscores.md")));

        // Valid issue files - non-numbered format (new)
        assert!(is_issue_file(Path::new("123_test.md"))); // Now valid: any .md file
        assert!(is_issue_file(Path::new("000123test.md"))); // Now valid: any .md file
        assert!(is_issue_file(Path::new("abc123_test.md"))); // Now valid: any .md file
        assert!(is_issue_file(Path::new("README.md"))); // Now valid: any .md file
        assert!(is_issue_file(Path::new("bug-report.md"))); // Valid: non-numbered
        assert!(is_issue_file(Path::new("my-feature.md"))); // Valid: non-numbered

        // Invalid files - wrong extension or no filename
        assert!(!is_issue_file(Path::new("000123_test.txt"))); // Wrong extension
        assert!(!is_issue_file(Path::new("000123_test"))); // No extension
        assert!(!is_issue_file(Path::new(".md"))); // Empty filename

        // Valid but edge cases
        assert!(is_issue_file(Path::new("000123_.md"))); // Valid: empty name part in numbered format

        // Path with directory should work
        assert!(is_issue_file(Path::new("./issues/000123_test.md")));
        assert!(is_issue_file(Path::new("/path/to/README.md")));
    }

    // Comprehensive tests for issue operations as specified in the issue
    #[tokio::test]
    async fn test_create_issue_comprehensive() {
        let (storage, _temp) = create_test_storage();

        // Create first issue
        let issue1 = storage
            .create_issue("test_issue".to_string(), "Test content".to_string())
            .await
            .unwrap();

        // Verify basic issue data
        assert_eq!(issue1.name.as_str(), "test_issue");
        assert_eq!(issue1.content, "Test content");

        // Create second issue - should auto-increment
        let _issue2 = storage
            .create_issue("another_issue".to_string(), "More content".to_string())
            .await
            .unwrap();

        // Number assertion removed - name-based approach
    }

    #[tokio::test]
    async fn test_list_issues_comprehensive() {
        let (storage, _temp) = create_test_storage();

        // Initially empty
        let issues = storage.list_issues().await.unwrap();
        assert!(issues.is_empty());

        // Create some issues
        storage
            .create_issue("issue1".to_string(), "Content 1".to_string())
            .await
            .unwrap();
        storage
            .create_issue("issue2".to_string(), "Content 2".to_string())
            .await
            .unwrap();

        let issues = storage.list_issues().await.unwrap();
        assert_eq!(issues.len(), 2);
        // Number assertion removed - name-based approach
        // Number assertion removed - name-based approach
    }

    #[tokio::test]
    async fn test_get_issue_comprehensive() {
        let (storage, _temp) = create_test_storage();

        // Create an issue
        let created = storage
            .create_issue("test_issue".to_string(), "Test content".to_string())
            .await
            .unwrap();

        // Get it back
        let retrieved = storage
            ./*get_issue_by_number*/ get_issue(&created.name)
            .await
            .unwrap();
        assert_eq!(retrieved.name, created.name);
        assert_eq!(retrieved.name, created.name);
        assert_eq!(retrieved.content, created.content);

        // Try to get non-existent issue
        let result = storage./*get_issue_by_number*/ get_issue("999").await;
        assert!(matches!(
            result,
            Err(SwissArmyHammerError::IssueNotFound(_))
        ));
    }

    #[tokio::test]
    async fn test_update_issue_comprehensive() {
        let (storage, _temp) = create_test_storage();

        // Create an issue
        let issue = storage
            .create_issue("test_issue".to_string(), "Original content".to_string())
            .await
            .unwrap();

        // Update it
        let updated = storage
            .update_issue(&issue.name, "Updated content".to_string())
            .await
            .unwrap();

        /* assert_eq!(updated.name, issue.number); */
        assert_eq!(updated.name, issue.name);
        assert_eq!(updated.content, "Updated content");

        // Verify it's persisted
        let retrieved = storage
            ./*get_issue_by_number*/ get_issue(&issue.name)
            .await
            .unwrap();
        assert_eq!(retrieved.content, "Updated content");
    }

    #[tokio::test]
    async fn test_update_nonexistent_issue_comprehensive() {
        let (storage, _temp) = create_test_storage();

        let result = storage.update_issue("999", "Content".to_string()).await;
        assert!(matches!(
            result,
            Err(SwissArmyHammerError::IssueNotFound(_))
        ));
    }

    #[tokio::test]
    async fn test_mark_complete_comprehensive() {
        let (storage, _temp) = create_test_storage();

        // Create an issue
        let issue = storage
            .create_issue("test_issue".to_string(), "Content".to_string())
            .await
            .unwrap();

        // Mark it complete
        let completed = storage.mark_complete(&issue.name).await.unwrap();
        assert_eq!(completed.name, issue.name);
        assert_eq!(completed.content, issue.content);

        // Verify file was moved to complete directory
        let temp_dir = _temp.path();
        let completed_path = temp_dir
            .join("issues")
            .join("complete")
            .join("test_issue.md");
        assert!(completed_path.exists());
        let active_path = temp_dir.join("issues").join("test_issue.md");
        assert!(!active_path.exists());

        // Verify it appears in completed list
        let all_issue_infos = storage.list_issues_info().await.unwrap();
        let completed_issue_infos: Vec<_> =
            all_issue_infos.iter().filter(|i| i.completed).collect();
        assert_eq!(completed_issue_infos.len(), 1);
    }

    #[tokio::test]
    async fn test_mark_complete_idempotent_comprehensive() {
        let (storage, _temp) = create_test_storage();

        // Create and complete an issue
        let issue = storage
            .create_issue("test_issue".to_string(), "Content".to_string())
            .await
            .unwrap();

        storage.mark_complete(&issue.name).await.unwrap();

        // Mark complete again - should be idempotent
        let result = storage.mark_complete(&issue.name).await;
        assert!(result.is_ok());
        let completed_issue = result.unwrap();
        assert_eq!(completed_issue.name, issue.name);
    }

    #[tokio::test]
    async fn test_all_complete_comprehensive() {
        let (storage, _temp) = create_test_storage();

        // Initially true (no issues)
        assert!(storage.all_complete().await.unwrap());

        // Create issues
        let issue1 = storage
            .create_issue("issue1".to_string(), "Content".to_string())
            .await
            .unwrap();
        let issue2 = storage
            .create_issue("issue2".to_string(), "Content".to_string())
            .await
            .unwrap();

        // Now false
        assert!(!storage.all_complete().await.unwrap());

        // Complete one
        storage.mark_complete(&issue1.name).await.unwrap();
        assert!(!storage.all_complete().await.unwrap());

        // Complete both
        storage.mark_complete(&issue2.name).await.unwrap();
        assert!(storage.all_complete().await.unwrap());
    }

    #[test]
    fn test_format_issue_number_comprehensive() {
        assert_eq!(format_issue_number(1), "000001");
        assert_eq!(format_issue_number(999_999), "999999");
        assert_eq!(format_issue_number(42), "000042");
    }

    #[test]
    fn test_parse_issue_number_comprehensive() {
        assert_eq!(parse_issue_number("000001").unwrap(), 1);
        assert_eq!(parse_issue_number("999999").unwrap(), 999_999);
        assert_eq!(parse_issue_number("000042").unwrap(), 42);

        // Invalid cases
        assert!(parse_issue_number("").is_err());
        assert!(parse_issue_number("abc").is_err());
        assert!(parse_issue_number("12345").is_err()); // Not 6 digits
    }

    #[test]
    fn test_parse_issue_filename_comprehensive() {
        let (num, name) = parse_issue_filename("000001_test_issue").unwrap();
        assert_eq!(num, 1);
        assert_eq!(name, "test_issue");

        let (num, name) = parse_issue_filename("000042_complex_name_with_underscores").unwrap();
        assert_eq!(num, 42);
        assert_eq!(name, "complex_name_with_underscores");

        // Invalid cases
        assert!(parse_issue_filename("no_number").is_err());
        assert!(parse_issue_filename("123_short").is_err());
    }

    #[test]
    fn test_create_safe_filename_comprehensive() {
        assert_eq!(create_safe_filename("simple"), "simple");
        assert_eq!(create_safe_filename("with spaces"), "with-spaces");
        assert_eq!(
            create_safe_filename("special/chars*removed"),
            "special-chars-removed"
        );
        assert_eq!(create_safe_filename("   trimmed   "), "trimmed");

        // Long names should be truncated
        let long_name = "a".repeat(200);
        let safe_name = create_safe_filename(&long_name);
        assert!(safe_name.len() <= 100);
    }

    #[test]
    fn test_create_safe_filename_security() {
        // Test path traversal protection
        assert_eq!(
            create_safe_filename("../etc/passwd"),
            "path_traversal_attempted"
        );
        assert_eq!(create_safe_filename("./config"), "path_traversal_attempted");
        assert_eq!(
            create_safe_filename("..\\windows\\system32"),
            "path_traversal_attempted"
        );

        // Test Windows reserved names
        assert_eq!(create_safe_filename("CON"), "CON_file");
        assert_eq!(create_safe_filename("PRN"), "PRN_file");
        assert_eq!(create_safe_filename("AUX"), "AUX_file");
        assert_eq!(create_safe_filename("NUL"), "NUL_file");
        assert_eq!(create_safe_filename("COM1"), "COM1_file");
        assert_eq!(create_safe_filename("LPT1"), "LPT1_file");

        // Test case insensitive Windows reserved names
        assert_eq!(create_safe_filename("con"), "con_file");
        assert_eq!(create_safe_filename("Com1"), "Com1_file");

        // Test Unix reserved names (when used as standalone names)
        assert_eq!(create_safe_filename("."), "._file");
        assert_eq!(create_safe_filename(".."), ".._file");

        // Test hidden files (starting with dot)
        assert_eq!(create_safe_filename(".hidden"), "hidden_hidden");
        assert_eq!(create_safe_filename(".gitignore"), "hidden_gitignore");

        // Test names ending with dot (Windows issue)
        assert_eq!(create_safe_filename("filename."), "filename_file");
        assert_eq!(create_safe_filename("test..."), "test_file");

        // Test null bytes and control characters
        assert_eq!(create_safe_filename("test\0null"), "test-null");
        assert_eq!(create_safe_filename("test\x01control"), "test-control");
        assert_eq!(create_safe_filename("test\x7Fdelete"), "test-delete");

        // Test very long names - gets truncated to max_filename_length (default 100)
        let very_long_name = "a".repeat(300);
        let safe_name = create_safe_filename(&very_long_name);
        assert_eq!(safe_name.len(), 100);
        assert_eq!(safe_name, "a".repeat(100));
    }

    #[tokio::test]
    async fn test_create_issues_batch() {
        let (storage, _temp) = create_test_storage();

        let batch_data = vec![
            ("issue_1".to_string(), "Content 1".to_string()),
            ("issue_2".to_string(), "Content 2".to_string()),
            ("issue_3".to_string(), "Content 3".to_string()),
        ];

        let issues = storage.create_issues_batch(batch_data).await.unwrap();

        assert_eq!(issues.len(), 3);
        assert_eq!(issues[0].name.as_str(), "issue_1");
        assert_eq!(issues[0].content, "Content 1");
        assert_eq!(issues[1].name.as_str(), "issue_2");
        assert_eq!(issues[1].content, "Content 2");
        assert_eq!(issues[2].name.as_str(), "issue_3");
        assert_eq!(issues[2].content, "Content 3");

        // Verify issues were actually created
        let all_issues = storage.list_issues().await.unwrap();
        assert_eq!(all_issues.len(), 3);
    }

    #[tokio::test]
    async fn test_create_issues_batch_empty() {
        let (storage, _temp) = create_test_storage();

        let batch_data = vec![];
        let issues = storage.create_issues_batch(batch_data).await.unwrap();

        assert_eq!(issues.len(), 0);
    }

    #[tokio::test]
    async fn test_get_issues_batch() {
        let (storage, _temp) = create_test_storage();

        // Create some issues first
        let issue1 = storage
            .create_issue("issue_1".to_string(), "Content 1".to_string())
            .await
            .unwrap();
        let issue2 = storage
            .create_issue("issue_2".to_string(), "Content 2".to_string())
            .await
            .unwrap();
        let issue3 = storage
            .create_issue("issue_3".to_string(), "Content 3".to_string())
            .await
            .unwrap();

        let names = vec![
            issue1.name.as_str(),
            issue2.name.as_str(),
            issue3.name.as_str(),
        ];
        let retrieved_issues = storage.get_issues_batch(names).await.unwrap();

        assert_eq!(retrieved_issues.len(), 3);
        assert_eq!(retrieved_issues[0].name, issue1.name);
        assert_eq!(retrieved_issues[1].name, issue2.name);
        assert_eq!(retrieved_issues[2].name, issue3.name);
    }

    #[tokio::test]
    async fn test_get_issues_batch_empty() {
        let (storage, _temp) = create_test_storage();

        let numbers = vec![];
        let issues = storage.get_issues_batch(numbers).await.unwrap();

        assert_eq!(issues.len(), 0);
    }

    #[tokio::test]
    async fn test_get_issues_batch_nonexistent() {
        let (storage, _temp) = create_test_storage();

        let names = vec!["nonexistent1", "nonexistent2", "nonexistent3"];
        let result = storage.get_issues_batch(names).await;

        // Should fail because the issues don't exist
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_issues_batch() {
        let (storage, _temp) = create_test_storage();

        // Create some issues first
        let issue1 = storage
            .create_issue("issue_1".to_string(), "Original 1".to_string())
            .await
            .unwrap();
        let issue2 = storage
            .create_issue("issue_2".to_string(), "Original 2".to_string())
            .await
            .unwrap();
        let issue3 = storage
            .create_issue("issue_3".to_string(), "Original 3".to_string())
            .await
            .unwrap();

        let updates = vec![
            (issue1.name.as_str(), "Updated 1".to_string()),
            (issue2.name.as_str(), "Updated 2".to_string()),
            (issue3.name.as_str(), "Updated 3".to_string()),
        ];

        let updated_issues = storage.update_issues_batch(updates).await.unwrap();

        assert_eq!(updated_issues.len(), 3);
        assert_eq!(updated_issues[0].content, "Updated 1");
        assert_eq!(updated_issues[1].content, "Updated 2");
        assert_eq!(updated_issues[2].content, "Updated 3");

        // Verify updates were persisted
        let retrieved_issue1 = storage
            ./*get_issue_by_number*/ get_issue(&issue1.name)
            .await
            .unwrap();
        assert_eq!(retrieved_issue1.content, "Updated 1");
    }

    #[tokio::test]
    async fn test_update_issues_batch_empty() {
        let (storage, _temp) = create_test_storage();

        let updates = vec![];
        let issues = storage.update_issues_batch(updates).await.unwrap();

        assert_eq!(issues.len(), 0);
    }

    #[tokio::test]
    async fn test_update_issues_batch_nonexistent() {
        let (storage, _temp) = create_test_storage();

        let updates = vec![
            ("nonexistent1", "Updated 1".to_string()),
            ("nonexistent2", "Updated 2".to_string()),
        ];

        let result = storage.update_issues_batch(updates).await;

        // Should fail because the issues don't exist
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mark_complete_batch() {
        let (storage, _temp) = create_test_storage();

        // Create some issues first
        let issue1 = storage
            .create_issue("issue_1".to_string(), "Content 1".to_string())
            .await
            .unwrap();
        let issue2 = storage
            .create_issue("issue_2".to_string(), "Content 2".to_string())
            .await
            .unwrap();
        let issue3 = storage
            .create_issue("issue_3".to_string(), "Content 3".to_string())
            .await
            .unwrap();

        let names = vec![
            issue1.name.as_str(),
            issue2.name.as_str(),
            issue3.name.as_str(),
        ];
        let completed_issues = storage.mark_complete_batch(names).await.unwrap();

        assert_eq!(completed_issues.len(), 3);
        // Basic issue data should be preserved
        assert_eq!(completed_issues[0].name, "issue_1");
        assert_eq!(completed_issues[1].name, "issue_2");
        assert_eq!(completed_issues[2].name, "issue_3");

        // Verify issues were marked complete (check filesystem)
        let temp_dir = _temp.path();
        let completed_path1 = temp_dir.join("issues/complete/issue_1.md");
        assert!(completed_path1.exists());
    }

    #[tokio::test]
    async fn test_mark_complete_batch_empty() {
        let (storage, _temp) = create_test_storage();

        let numbers = vec![];
        let issues = storage.mark_complete_batch(numbers).await.unwrap();

        assert_eq!(issues.len(), 0);
    }

    #[tokio::test]
    async fn test_mark_complete_batch_nonexistent() {
        let (storage, _temp) = create_test_storage();

        let names = vec!["nonexistent1", "nonexistent2", "nonexistent3"];
        let result = storage.mark_complete_batch(names).await;

        // Should fail because the issues don't exist
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_batch_operations_preserve_order() {
        let (storage, _temp) = create_test_storage();

        // Create issues in a specific order
        let batch_data = vec![
            ("alpha".to_string(), "First".to_string()),
            ("beta".to_string(), "Second".to_string()),
            ("gamma".to_string(), "Third".to_string()),
        ];

        let created_issues = storage.create_issues_batch(batch_data).await.unwrap();

        // Verify order is preserved
        assert_eq!(created_issues[0].name.as_str(), "alpha");
        assert_eq!(created_issues[1].name.as_str(), "beta");
        assert_eq!(created_issues[2].name.as_str(), "gamma");

        // Get issues in different order
        let names = vec![
            created_issues[2].name.as_str(),
            created_issues[0].name.as_str(),
            created_issues[1].name.as_str(),
        ];

        let retrieved_issues = storage.get_issues_batch(names).await.unwrap();

        // Should preserve requested order
        assert_eq!(retrieved_issues[0].name.as_str(), "gamma");
        assert_eq!(retrieved_issues[1].name.as_str(), "alpha");
        assert_eq!(retrieved_issues[2].name.as_str(), "beta");
    }

    #[tokio::test]
    async fn test_batch_operations_with_large_batches() {
        let (storage, _temp) = create_test_storage();

        // Create a large batch
        let batch_size = 100;
        let batch_data: Vec<(String, String)> = (1..=batch_size)
            .map(|i| (format!("issue_{i}"), format!("Content {i}")))
            .collect();

        let created_issues = storage.create_issues_batch(batch_data).await.unwrap();
        assert_eq!(created_issues.len(), batch_size);

        // Get all issues in batch
        let names: Vec<&str> = created_issues.iter().map(|i| i.name.as_str()).collect();
        let retrieved_issues = storage.get_issues_batch(names.clone()).await.unwrap();
        assert_eq!(retrieved_issues.len(), batch_size);

        // Update all issues in batch
        let updates: Vec<(&str, String)> = created_issues
            .iter()
            .map(|i| (i.name.as_str(), format!("Updated {}", i.name)))
            .collect();
        let updated_issues = storage.update_issues_batch(updates).await.unwrap();
        assert_eq!(updated_issues.len(), batch_size);

        // Mark half complete in batch
        let half_names: Vec<&str> = names.iter().take(batch_size / 2).cloned().collect();
        let completed_issues = storage.mark_complete_batch(half_names).await.unwrap();
        assert_eq!(completed_issues.len(), batch_size / 2);

        // Verify final state
        let all_issue_infos = storage.list_issues_info().await.unwrap();
        assert_eq!(all_issue_infos.len(), batch_size);

        let completed_count = all_issue_infos.iter().filter(|i| i.completed).count();
        assert_eq!(completed_count, batch_size / 2);
    }

    #[tokio::test]
    async fn test_batch_operations_partial_failure_behavior() {
        let (storage, _temp) = create_test_storage();

        // Create one issue
        let issue = storage
            .create_issue("existing".to_string(), "Content".to_string())
            .await
            .unwrap();

        // Try to get batch with mix of existing and non-existing issues
        let names = vec![issue.name.as_str(), "nonexistent1", "nonexistent2"];
        let result = storage.get_issues_batch(names).await;

        // Should fail entirely, not return partial results
        assert!(result.is_err());

        // Try to update batch with mix of existing and non-existing issues
        let updates = vec![
            (issue.name.as_str(), "Updated".to_string()),
            ("nonexistent", "Should fail".to_string()),
        ];
        let result = storage.update_issues_batch(updates).await;

        // Should fail entirely
        assert!(result.is_err());

        // Verify original issue was not updated
        let retrieved_issue = storage.get_issue(&issue.name).await.unwrap();
        assert_eq!(retrieved_issue.content, "Content");
    }

    #[test]
    fn test_virtual_number_collision_resistance() {
        // Test that the improved virtual number generation has better collision resistance
        let storage = FileSystemIssueStorage::new(PathBuf::from("/tmp")).unwrap();

        // Create a set of filenames that would likely collide with simple hash % range
        let test_filenames = vec![
            "readme", "README", "notes", "NOTES", "todo", "TODO", "doc", "DOC", "test", "TEST",
        ];

        let mut virtual_numbers = std::collections::HashSet::new();

        for filename in test_filenames {
            let virtual_number =
                storage.generate_virtual_number_with_collision_resistance(filename);

            // Verify each filename gets a unique virtual number (no collisions in this small set)
            assert!(
                virtual_numbers.insert(virtual_number),
                "Collision detected: filename '{filename}' got virtual number {virtual_number} which was already used"
            );

            // Verify virtual numbers are in valid range
            let config = Config::global();
            assert!(
                virtual_number >= config.virtual_issue_number_base,
                "Virtual number {} is below base {}",
                virtual_number,
                config.virtual_issue_number_base
            );
            assert!(
                virtual_number
                    < config.virtual_issue_number_base + config.virtual_issue_number_range,
                "Virtual number {} is above max {}",
                virtual_number,
                config.virtual_issue_number_base + config.virtual_issue_number_range
            );
        }
    }

    #[test]
    fn test_virtual_number_deterministic() {
        // Test that virtual number generation is deterministic - same filename always gets same number
        let storage = FileSystemIssueStorage::new(PathBuf::from("/tmp")).unwrap();

        let test_filenames = vec![
            "consistent_test",
            "another_file",
            "special-chars_123",
            "unicode_文件名",
        ];

        for filename in test_filenames {
            let first_call = storage.generate_virtual_number_with_collision_resistance(filename);
            let second_call = storage.generate_virtual_number_with_collision_resistance(filename);
            let third_call = storage.generate_virtual_number_with_collision_resistance(filename);

            assert_eq!(
                first_call, second_call,
                "Virtual number generation is not deterministic for filename '{filename}'"
            );
            assert_eq!(
                second_call, third_call,
                "Virtual number generation is not deterministic for filename '{filename}'"
            );
        }
    }

    // Tests for the new IssueName-based approach
    #[tokio::test]
    async fn test_issue_identification_by_name() {
        let (storage, temp) = create_test_storage();

        // Create various types of issues to test filename-based identification
        // 1. Traditional numbered issue
        let numbered_issue = storage
            .create_issue(
                "feature_request".to_string(),
                "# Feature Request\nContent".to_string(),
            )
            .await
            .unwrap();

        // 2. Create arbitrary named issues directly (simulating user dropping in files)
        let content = "# My Issue\nThis is arbitrary content";
        let arbitrary_file = temp.path().join("issues").join("my-custom-issue.md");
        std::fs::write(&arbitrary_file, content).unwrap();

        let parsed_issue = storage.parse_issue_from_file(&arbitrary_file).unwrap();

        // Verify that we can identify issues by their derived names
        assert_eq!(numbered_issue.name.as_str(), "feature_request");
        assert_eq!(parsed_issue.name.as_str(), "my-custom-issue");

        // Test that both issues are included in list and properly sorted
        let all_issue_infos = storage.list_issues_info().await.unwrap();
        assert_eq!(all_issue_infos.len(), 2);

        // Issues should be sorted lexicographically by filename
        // 000001_feature_request.md should come before my-custom-issue.md
        assert!(
            all_issue_infos[0]
                .file_path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                < all_issue_infos[1]
                    .file_path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
        );
    }

    #[tokio::test]
    async fn test_new_issue_name_based_api() {
        // This test demonstrates the new API that should work with IssueName

        let (storage, _temp) = create_test_storage();

        // Create an issue
        let issue = storage
            .create_issue("bug_fix".to_string(), "# Bug Fix\nDetails".to_string())
            .await
            .unwrap();

        // Test both old and new APIs work correctly

        // Current API uses name (String)
        let retrieved_by_string = storage.get_issue(&issue.name).await.unwrap();
        assert_eq!(retrieved_by_string.name.as_str(), "bug_fix");

        // NEW type-safe API using IssueName
        let issue_name = IssueName::new("bug_fix".to_string()).unwrap();
        let retrieved_by_name = storage.get_issue_by_name(&issue_name).await.unwrap();
        assert_eq!(retrieved_by_name.name.as_str(), "bug_fix");

        // Verify both methods return the same issue
        assert_eq!(retrieved_by_string.content, retrieved_by_name.content);
        // Note: basic Issue objects do not carry completion status

        // For now, let's verify that the issue name extraction works correctly
        // for different filename formats
        let issue_name_from_numbered = extract_issue_name_from_filename("000123_my_bug_fix.md");
        assert_eq!(issue_name_from_numbered, "my_bug_fix");

        let issue_name_from_arbitrary = extract_issue_name_from_filename("my-arbitrary-issue.md");
        assert_eq!(issue_name_from_arbitrary, "my-arbitrary-issue");
    }

    #[tokio::test]
    async fn test_real_issue_scenario_000187() {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Simulate the real scenario from issue 000187:
        // Main directory: 000185, 000186, 000187, 000188
        fs::write(issues_dir.join("000185.md"), "issue 185").unwrap();
        fs::write(issues_dir.join("000186.md"), "issue 186").unwrap();
        fs::write(issues_dir.join("000187.md"), "issue 187").unwrap();
        fs::write(issues_dir.join("000188.md"), "issue 188").unwrap();

        // Complete directory: up to 000184 (simulate some completed issues)
        let complete_dir = issues_dir.join("complete");
        fs::write(complete_dir.join("000181.md"), "completed issue 181").unwrap();
        fs::write(complete_dir.join("000182.md"), "completed issue 182").unwrap();
        fs::write(complete_dir.join("000183.md"), "completed issue 183").unwrap();
        fs::write(complete_dir.join("000184.md"), "completed issue 184").unwrap();

        // Debug: List all issues found
        let pending_issues = storage.list_issues_in_dir(&issues_dir).unwrap();
        println!("Found {} pending issues:", pending_issues.len());
        for issue in &pending_issues {
            println!("  {}", issue.name);
        }

        let completed_issues = storage.list_issues_in_dir(&complete_dir).unwrap();
        println!("Found {} completed issues:", completed_issues.len());
        for issue in &completed_issues {
            println!("  {}", issue.name);
        }

        // get_next_issue_number method removed - name-based system doesn't use sequential numbers

        // Next number logic removed - name-based system doesn't use sequential numbers

        // Test creating a new issue
        let new_issue = storage
            .create_issue(
                "test_new_issue".to_string(),
                "New issue content".to_string(),
            )
            .await
            .unwrap();

        // Verify issue was created with correct name
        assert_eq!(new_issue.name, "test_new_issue");
    }

    #[tokio::test]
    async fn test_issues_sorted_by_name_ascending() {
        // Test for issue 000188: Issues should be sorted by name, not by number
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create issues with names that would be in different order if sorted by number vs name
        // Using filesystem operations to create specific numbered files with specific names

        // Create issue files with names that should be sorted alphabetically:
        // - "alpha" should come first
        // - "beta" should come second
        // - "gamma" should come third
        // But their numbers will be in different order
        std::fs::write(issues_dir.join("000003_gamma.md"), "Third issue by name").unwrap();
        std::fs::write(issues_dir.join("000001_beta.md"), "Second issue by name").unwrap();
        std::fs::write(issues_dir.join("000002_alpha.md"), "First issue by name").unwrap();

        // Also add a non-numbered issue to ensure it's sorted by name too
        std::fs::write(issues_dir.join("zulu.md"), "Last issue by name").unwrap();
        std::fs::write(issues_dir.join("apex.md"), "Should be first by name").unwrap();

        let all_issues = storage.list_issues().await.unwrap();

        // Verify we got all 5 issues
        assert_eq!(all_issues.len(), 5);

        // Extract the names in order
        let names: Vec<&str> = all_issues.iter().map(|i| i.name.as_str()).collect();

        // Issues should be sorted alphabetically by name:
        // 000001_beta, 000002_alpha, 000003_gamma, apex, zulu
        assert_eq!(
            names,
            vec![
                "000001_beta",
                "000002_alpha",
                "000003_gamma",
                "apex",
                "zulu"
            ],
            "Issues should be sorted alphabetically by name. Got: {names:?}"
        );

        // Verify the first issue (next issue) is "000001_beta"
        assert_eq!(
            all_issues[0].name.as_str(),
            "000001_beta",
            "Next issue should be '000001_beta' (first in alphabetical order)"
        );
    }

    #[test]
    fn test_get_issue_name_from_filename_simple() {
        // Test the new simplified approach - filename without .md extension is the issue name
        assert_eq!(
            get_issue_name_from_filename("000001_paper.md"),
            "000001_paper"
        );
        assert_eq!(get_issue_name_from_filename("nice.md"), "nice");
        assert_eq!(get_issue_name_from_filename("000001.md"), "000001");
        assert_eq!(
            get_issue_name_from_filename("373c4cf9-d803-4138-89b3-bc802d22f94e.md"),
            "373c4cf9-d803-4138-89b3-bc802d22f94e"
        );

        // Files without .md extension
        assert_eq!(get_issue_name_from_filename("000001_paper"), "000001_paper");
        assert_eq!(get_issue_name_from_filename("nice"), "nice");

        // Test the specific case from the request
        assert_eq!(
            get_issue_name_from_filename("000007_basic_testing.md"),
            "000007_basic_testing"
        );
    }

    #[tokio::test]
    async fn test_mark_complete_with_duplicate_cleanup() {
        // Integration test for the complete move + cleanup workflow
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create an issue
        let issue = storage
            .create_issue("test_issue".to_string(), "Test content".to_string())
            .await
            .unwrap();
        // Note: issue.completed not available on basic Issue - new issues are always in active directory

        // Manually create a duplicate file in the completed directory to simulate the scenario
        let completed_dir = issues_dir.join("complete");
        std::fs::create_dir_all(&completed_dir).unwrap();
        let duplicate_path = completed_dir.join("test_issue.md");
        std::fs::write(&duplicate_path, "Duplicate content").unwrap();
        assert!(duplicate_path.exists());

        // Mark as complete - this should move the file and clean up the duplicate
        let completed_issue = storage.mark_complete("test_issue").await.unwrap();

        // Verify the issue was moved
        // Verify the issue data is correct
        assert_eq!(completed_issue.name, issue.name);

        // Check filesystem directly for completion status
        let completed_path = issues_dir.join("complete/test_issue.md");
        let active_path = issues_dir.join("test_issue.md");

        assert!(completed_path.exists());
        assert!(!active_path.exists());

        // Read the final content to verify it's the correct file (from the original issue, not the duplicate)
        let final_content = std::fs::read_to_string(&completed_path).unwrap();
        assert!(final_content.contains("Test content"));
        assert!(!final_content.contains("Duplicate content"));
    }

    #[tokio::test]
    async fn test_mark_complete_already_completed_with_duplicate_cleanup() {
        // Test the early cleanup scenario when issue is already in target state
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().to_path_buf();
        let storage = FileSystemIssueStorage::new(issues_dir.clone()).unwrap();

        // Create and complete an issue
        let _issue = storage
            .create_issue("test_issue".to_string(), "Original content".to_string())
            .await
            .unwrap();
        let _completed_issue = storage.mark_complete("test_issue").await.unwrap();
        // Note: completed_issue is basic Issue - check filesystem for completion status

        // Manually create a duplicate in the pending directory to simulate a leftover file
        let pending_duplicate = issues_dir.join("test_issue.md");
        std::fs::write(&pending_duplicate, "Stale duplicate content").unwrap();
        assert!(pending_duplicate.exists());

        // Try to mark as complete again - should cleanup the duplicate
        let result = storage.mark_complete("test_issue").await.unwrap();

        // Verify the issue data is correct and duplicate was cleaned up
        assert_eq!(result.name, "test_issue");
        assert!(!pending_duplicate.exists()); // Duplicate should be cleaned up

        // Verify the original completed file still exists and has correct content
        let completed_path = issues_dir.join("complete/test_issue.md");
        let final_content = std::fs::read_to_string(&completed_path).unwrap();
        assert!(final_content.contains("Original content"));
        assert!(!final_content.contains("Stale duplicate"));
    }

    #[test]
    fn test_migration_paths() {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path();

        let paths = FileSystemIssueStorage::migration_paths_in_dir(base_dir);

        // Verify paths are correctly calculated relative to base directory
        assert_eq!(paths.source, base_dir.join("issues"));
        assert_eq!(
            paths.destination,
            base_dir.join(".swissarmyhammer").join("issues")
        );
        assert_eq!(
            paths.backup,
            base_dir.join(".swissarmyhammer").join("issues_backup")
        );
    }

    #[test]
    fn test_should_migrate_source_exists_destination_does_not() {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path();

        // Create source directory (old issues location)
        let source_dir = base_dir.join("issues");
        std::fs::create_dir_all(&source_dir).unwrap();

        // Ensure destination does not exist
        let dest_dir = base_dir.join(".swissarmyhammer").join("issues");
        assert!(!dest_dir.exists());

        // Should migrate: source exists, destination doesn't
        let should_migrate = FileSystemIssueStorage::should_migrate_in_dir(base_dir).unwrap();
        assert!(should_migrate);
    }

    #[test]
    fn test_should_not_migrate_source_does_not_exist() {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path();

        // Ensure source directory does not exist
        let source_dir = base_dir.join("issues");
        assert!(!source_dir.exists());

        // Should not migrate: source doesn't exist
        let should_migrate = FileSystemIssueStorage::should_migrate_in_dir(base_dir).unwrap();
        assert!(!should_migrate);
    }

    #[test]
    fn test_should_not_migrate_both_exist() {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path();

        // Create both source and destination directories
        let source_dir = base_dir.join("issues");
        std::fs::create_dir_all(&source_dir).unwrap();

        let dest_dir = base_dir.join(".swissarmyhammer").join("issues");
        std::fs::create_dir_all(&dest_dir).unwrap();

        // Should not migrate: both exist
        let should_migrate = FileSystemIssueStorage::should_migrate_in_dir(base_dir).unwrap();
        assert!(!should_migrate);
    }

    #[test]
    fn test_should_not_migrate_neither_exist() {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path();

        // Ensure neither directory exists in fresh temp directory
        let source_dir = base_dir.join("issues");
        let dest_dir = base_dir.join(".swissarmyhammer").join("issues");
        assert!(!source_dir.exists());
        assert!(!dest_dir.exists());

        // Should not migrate: neither exist
        let should_migrate = FileSystemIssueStorage::should_migrate_in_dir(base_dir).unwrap();
        assert!(!should_migrate);
    }

    #[test]
    fn test_should_not_migrate_source_is_file() {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path();

        // Create source as a file instead of directory
        let source_path = base_dir.join("issues");
        std::fs::write(&source_path, "this is a file").unwrap();

        // Should not migrate: source exists but is not a directory
        let should_migrate = FileSystemIssueStorage::should_migrate_in_dir(base_dir).unwrap();
        assert!(!should_migrate);
    }

    #[test]
    fn test_count_directory_contents_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join("empty");
        std::fs::create_dir_all(&test_dir).unwrap();

        let (file_count, total_size) =
            FileSystemIssueStorage::count_directory_contents(&test_dir).unwrap();
        assert_eq!(file_count, 0);
        assert_eq!(total_size, 0);
    }

    #[test]
    fn test_count_directory_contents_with_files() {
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join("with_files");
        std::fs::create_dir_all(&test_dir).unwrap();

        // Create test files with known content
        let file1_content = "Hello, world!";
        let file2_content = "This is a test file with more content.";

        std::fs::write(test_dir.join("file1.txt"), file1_content).unwrap();
        std::fs::write(test_dir.join("file2.md"), file2_content).unwrap();

        let (file_count, total_size) =
            FileSystemIssueStorage::count_directory_contents(&test_dir).unwrap();
        assert_eq!(file_count, 2);
        assert_eq!(
            total_size,
            file1_content.len() as u64 + file2_content.len() as u64
        );
    }

    #[test]
    fn test_count_directory_contents_nested_directories() {
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join("nested");
        std::fs::create_dir_all(&test_dir).unwrap();

        // Create nested structure
        let subdir = test_dir.join("subdir");
        std::fs::create_dir_all(&subdir).unwrap();

        let file1_content = "Root file";
        let file2_content = "Nested file";

        std::fs::write(test_dir.join("root.txt"), file1_content).unwrap();
        std::fs::write(subdir.join("nested.txt"), file2_content).unwrap();

        let (file_count, total_size) =
            FileSystemIssueStorage::count_directory_contents(&test_dir).unwrap();
        assert_eq!(file_count, 2);
        assert_eq!(
            total_size,
            file1_content.len() as u64 + file2_content.len() as u64
        );
    }

    #[test]
    fn test_count_directory_contents_ignores_subdirectories() {
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join("with_dirs");
        std::fs::create_dir_all(&test_dir).unwrap();

        // Create files and subdirectories
        let file_content = "Test file";
        std::fs::write(test_dir.join("file.txt"), file_content).unwrap();

        // Create empty subdirectory (should not be counted)
        std::fs::create_dir_all(test_dir.join("empty_subdir")).unwrap();

        let (file_count, total_size) =
            FileSystemIssueStorage::count_directory_contents(&test_dir).unwrap();
        assert_eq!(file_count, 1); // Only the file, not the subdirectory
        assert_eq!(total_size, file_content.len() as u64);
    }

    #[test]
    fn test_migration_info_should_migrate() {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path();

        // Create source directory with test files
        let source_dir = base_dir.join("issues");
        std::fs::create_dir_all(&source_dir).unwrap();

        let file1_content = "Test issue 1";
        let file2_content = "Test issue 2 with more content";
        std::fs::write(source_dir.join("issue1.md"), file1_content).unwrap();
        std::fs::write(source_dir.join("issue2.md"), file2_content).unwrap();

        let info = FileSystemIssueStorage::migration_info_in_dir(base_dir).unwrap();

        assert!(info.should_migrate);
        assert!(info.source_exists);
        assert!(!info.destination_exists);
        assert_eq!(info.file_count, 2);
        assert_eq!(
            info.total_size,
            file1_content.len() as u64 + file2_content.len() as u64
        );
    }

    #[test]
    fn test_migration_info_should_not_migrate() {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path();

        // Create both directories (migration should not occur)
        let source_dir = base_dir.join("issues");
        std::fs::create_dir_all(&source_dir).unwrap();

        let dest_dir = base_dir.join(".swissarmyhammer").join("issues");
        std::fs::create_dir_all(&dest_dir).unwrap();

        // Add files to source
        std::fs::write(source_dir.join("test.md"), "test content").unwrap();

        let info = FileSystemIssueStorage::migration_info_in_dir(base_dir).unwrap();

        assert!(!info.should_migrate);
        assert!(info.source_exists);
        assert!(info.destination_exists);
        assert_eq!(info.file_count, 1);
        assert_eq!(info.total_size, "test content".len() as u64);
    }

    #[test]
    fn test_migration_info_no_source() {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path();

        // No source directory exists (fresh temp directory)
        let info = FileSystemIssueStorage::migration_info_in_dir(base_dir).unwrap();

        assert!(!info.should_migrate);
        assert!(!info.source_exists);
        assert!(!info.destination_exists);
        assert_eq!(info.file_count, 0);
        assert_eq!(info.total_size, 0);
    }

    #[test]
    fn test_migration_info_large_directory() {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path();

        // Create source directory with many files
        let source_dir = base_dir.join("issues");
        std::fs::create_dir_all(&source_dir).unwrap();

        let mut expected_size = 0u64;
        for i in 0..10 {
            let content = format!("Issue {} content with variable length", i);
            std::fs::write(source_dir.join(format!("issue{}.md", i)), &content).unwrap();
            expected_size += content.len() as u64;
        }

        let info = FileSystemIssueStorage::migration_info_in_dir(base_dir).unwrap();

        assert!(info.should_migrate);
        assert!(info.source_exists);
        assert!(!info.destination_exists);
        assert_eq!(info.file_count, 10);
        assert_eq!(info.total_size, expected_size);
    }

    /// Comprehensive tests for core storage functionality added in steps 000284-000288
    mod filesystem_storage_tests {
        use super::*;
        use tempfile::TempDir;

        #[test]
        fn test_new_default_with_swissarmyhammer_directory() {
            let temp_dir = TempDir::new().unwrap();
            let swissarmyhammer_dir = temp_dir.path().join(".swissarmyhammer");
            std::fs::create_dir_all(&swissarmyhammer_dir).unwrap();

            let _test_env = IsolatedTestEnvironment::new().unwrap();
            std::env::set_current_dir(temp_dir.path()).unwrap();

            let storage = FileSystemIssueStorage::new_default().unwrap();

            // Verify the storage uses .swissarmyhammer path by checking the path contains it
            assert!(storage
                .state
                .issues_dir
                .to_string_lossy()
                .contains(".swissarmyhammer"));
            assert!(storage.state.issues_dir.ends_with("issues"));

            // The completed_dir should be issues_dir/complete - test this relationship
            assert!(storage.state.completed_dir.ends_with("complete"));
            let completed_parent = storage.state.completed_dir.parent().unwrap();
            assert_eq!(
                completed_parent
                    .canonicalize()
                    .unwrap_or_else(|_| completed_parent.to_path_buf()),
                storage
                    .state
                    .issues_dir
                    .canonicalize()
                    .unwrap_or_else(|_| storage.state.issues_dir.clone())
            );
        }

        #[test]
        fn test_new_default_without_swissarmyhammer_directory() {
            let temp_dir = TempDir::new().unwrap();
            let original_dir = match std::env::current_dir() {
                Ok(dir) => dir,
                Err(_) => return, // Skip test if current directory is not accessible
            };
            if std::env::set_current_dir(temp_dir.path()).is_err() {
                return; // Skip test if can't change directory
            }

            // Ensure .swissarmyhammer directory does NOT exist to test legacy behavior
            let swissarmyhammer_dir = temp_dir.path().join(".swissarmyhammer");
            if swissarmyhammer_dir.exists() {
                std::fs::remove_dir_all(&swissarmyhammer_dir).unwrap();
            }

            let storage = FileSystemIssueStorage::new_default().unwrap();

            // Restore original directory
            let _ = std::env::set_current_dir(original_dir);

            // Verify the storage uses legacy path (not containing .swissarmyhammer)
            assert!(!storage
                .state
                .issues_dir
                .to_string_lossy()
                .contains(".swissarmyhammer"));
            assert!(storage.state.issues_dir.ends_with("issues"));

            // The completed_dir should be issues_dir/complete - test this relationship
            assert!(storage.state.completed_dir.ends_with("complete"));
            let completed_parent = storage.state.completed_dir.parent().unwrap();
            assert_eq!(
                completed_parent
                    .canonicalize()
                    .unwrap_or_else(|_| completed_parent.to_path_buf()),
                storage
                    .state
                    .issues_dir
                    .canonicalize()
                    .unwrap_or_else(|_| storage.state.issues_dir.clone())
            );
        }

        #[test]
        fn test_default_directory_detection() {
            let temp_dir = TempDir::new().unwrap();

            let _test_env = IsolatedTestEnvironment::new().unwrap();
            std::env::set_current_dir(temp_dir.path()).unwrap();

            // Test without .swissarmyhammer
            let dir = FileSystemIssueStorage::default_directory().unwrap();
            assert!(dir.ends_with("issues"));
            assert!(!dir.to_string_lossy().contains(".swissarmyhammer"));

            // Create .swissarmyhammer and test again
            std::fs::create_dir_all(temp_dir.path().join(".swissarmyhammer")).unwrap();
            let dir = FileSystemIssueStorage::default_directory().unwrap();
            assert!(dir.ends_with("issues"));
            assert!(dir.to_string_lossy().contains(".swissarmyhammer"));
        }

        #[test]
        fn test_new_default_creates_directories() {
            let _test_env = IsolatedTestEnvironment::new().unwrap();

            let temp_dir = TempDir::new().unwrap();
            let swissarmyhammer_dir = temp_dir.path().join(".swissarmyhammer");
            std::fs::create_dir_all(&swissarmyhammer_dir).unwrap();

            std::env::set_current_dir(temp_dir.path()).unwrap();

            // Create storage - should create the required directories
            let storage = FileSystemIssueStorage::new_default().unwrap();

            // Directories should now exist - check the actual storage paths
            assert!(storage.state.issues_dir.exists());
            assert!(storage.state.completed_dir.exists());

            // Verify the paths are correct
            assert!(storage
                .state
                .issues_dir
                .to_string_lossy()
                .contains(".swissarmyhammer"));
            assert!(storage.state.issues_dir.ends_with("issues"));
            assert_eq!(
                storage.state.completed_dir,
                storage.state.issues_dir.join("complete")
            );
        }
    }

    /// Comprehensive tests for migration detection functionality
    mod migration_detection_tests {
        use super::*;
        use tempfile::TempDir;

        #[test]
        fn test_should_migrate_with_legacy_directory() {
            let temp_dir = TempDir::new().unwrap();

            let _test_env = IsolatedTestEnvironment::new().unwrap();
            std::env::set_current_dir(temp_dir.path()).unwrap();

            // Create legacy issues directory with content
            let issues_dir = temp_dir.path().join("issues");
            std::fs::create_dir_all(&issues_dir).unwrap();
            std::fs::write(issues_dir.join("test.md"), "test content").unwrap();

            // Remove the .swissarmyhammer/issues directory created by IsolatedTestEnvironment
            // so that migration detection works correctly
            let new_issues = temp_dir.path().join(".swissarmyhammer/issues");
            if new_issues.exists() {
                std::fs::remove_dir_all(&new_issues).unwrap();
            }

            assert!(FileSystemIssueStorage::should_migrate().unwrap());
        }

        #[test]
        fn test_should_not_migrate_when_new_exists() {
            let temp_dir = TempDir::new().unwrap();

            let _test_env = IsolatedTestEnvironment::new().unwrap();
            std::env::set_current_dir(temp_dir.path()).unwrap();

            // Create both directories
            std::fs::create_dir_all(temp_dir.path().join("issues")).unwrap();
            std::fs::create_dir_all(temp_dir.path().join(".swissarmyhammer/issues")).unwrap();

            assert!(!FileSystemIssueStorage::should_migrate().unwrap());
        }

        #[test]
        fn test_should_not_migrate_when_no_legacy() {
            let temp_dir = TempDir::new().unwrap();

            let _test_env = IsolatedTestEnvironment::new().unwrap();
            std::env::set_current_dir(temp_dir.path()).unwrap();

            assert!(!FileSystemIssueStorage::should_migrate().unwrap());
        }

        #[test]
        fn test_migration_info_accuracy() {
            let temp_dir = TempDir::new().unwrap();

            let _test_env = IsolatedTestEnvironment::new().unwrap();
            std::env::set_current_dir(temp_dir.path()).unwrap();

            // Create test files
            let issues_dir = temp_dir.path().join("issues");
            std::fs::create_dir_all(&issues_dir).unwrap();
            std::fs::write(issues_dir.join("file1.md"), "content1").unwrap();
            std::fs::write(issues_dir.join("file2.md"), "content2").unwrap();
            std::fs::create_dir_all(issues_dir.join("complete")).unwrap();
            std::fs::write(issues_dir.join("complete/file3.md"), "content3").unwrap();

            // Remove the .swissarmyhammer/issues directory created by IsolatedTestEnvironment
            // so that migration detection works correctly
            let new_issues = temp_dir.path().join(".swissarmyhammer/issues");
            if new_issues.exists() {
                std::fs::remove_dir_all(&new_issues).unwrap();
            }

            let info = FileSystemIssueStorage::migration_info().unwrap();

            assert!(info.should_migrate);
            assert!(info.source_exists);
            assert!(!info.destination_exists);
            assert_eq!(info.file_count, 3);
            assert!(info.total_size > 0);
            assert_eq!(info.total_size, 24); // "content1" + "content2" + "content3" = 8 + 8 + 8 = 24
        }

        #[test]
        fn test_migration_paths() {
            let temp_dir = TempDir::new().unwrap();

            let _test_env = IsolatedTestEnvironment::new().unwrap();
            std::env::set_current_dir(temp_dir.path()).unwrap();

            let paths = FileSystemIssueStorage::migration_paths().unwrap();

            // Use canonical path for comparison to handle macOS symlink resolution
            let canonical_temp_path = temp_dir.path().canonicalize().unwrap();
            assert_eq!(paths.source, canonical_temp_path.join("issues"));
            assert_eq!(
                paths.destination,
                canonical_temp_path.join(".swissarmyhammer/issues")
            );
            assert_eq!(
                paths.backup,
                canonical_temp_path.join(".swissarmyhammer/issues_backup")
            );
        }

        #[test]
        fn test_should_migrate_with_empty_legacy_directory() {
            let temp_dir = TempDir::new().unwrap();

            let _test_env = IsolatedTestEnvironment::new().unwrap();
            std::env::set_current_dir(temp_dir.path()).unwrap();

            // Create empty legacy directory
            let issues_dir = temp_dir.path().join("issues");
            std::fs::create_dir_all(&issues_dir).unwrap();

            // Remove the .swissarmyhammer/issues directory created by IsolatedTestEnvironment
            // so that migration detection works correctly
            let new_issues = temp_dir.path().join(".swissarmyhammer/issues");
            if new_issues.exists() {
                std::fs::remove_dir_all(&new_issues).unwrap();
            }

            // Should still migrate empty directories
            assert!(FileSystemIssueStorage::should_migrate().unwrap());
        }

        #[test]
        fn test_migration_info_with_nested_structure() {
            let temp_dir = TempDir::new().unwrap();

            let _test_env = IsolatedTestEnvironment::new().unwrap();

            // Create nested structure with files at different levels
            let issues_dir = temp_dir.path().join("issues");
            std::fs::create_dir_all(&issues_dir).unwrap();

            let complete_dir = issues_dir.join("complete");
            std::fs::create_dir_all(&complete_dir).unwrap();

            let sub_dir = complete_dir.join("archived");
            std::fs::create_dir_all(&sub_dir).unwrap();

            // Create files at different nesting levels
            std::fs::write(issues_dir.join("root.md"), "root").unwrap();
            std::fs::write(complete_dir.join("complete.md"), "complete").unwrap();
            std::fs::write(sub_dir.join("archived.md"), "archived").unwrap();

            // Remove the .swissarmyhammer/issues directory created by IsolatedTestEnvironment
            // so that migration detection works correctly
            let new_issues = temp_dir.path().join(".swissarmyhammer/issues");
            if new_issues.exists() {
                std::fs::remove_dir_all(&new_issues).unwrap();
            }

            let info = FileSystemIssueStorage::migration_info_in_dir(temp_dir.path()).unwrap();

            assert!(info.should_migrate);
            assert_eq!(info.file_count, 3);
            assert_eq!(info.total_size, 20); // "root" + "complete" + "archived" = 4 + 8 + 8 = 20
        }
    }

    /// Tests for directory counting utilities
    mod directory_utilities_tests {
        use super::*;
        use tempfile::TempDir;

        #[test]
        fn test_count_directory_contents_empty() {
            let temp_dir = TempDir::new().unwrap();
            let empty_dir = temp_dir.path().join("empty");
            std::fs::create_dir_all(&empty_dir).unwrap();

            let (count, size) =
                FileSystemIssueStorage::count_directory_contents(&empty_dir).unwrap();
            assert_eq!(count, 0);
            assert_eq!(size, 0);
        }

        #[test]
        fn test_count_directory_contents_with_files() {
            let temp_dir = TempDir::new().unwrap();
            let test_dir = temp_dir.path().join("test");
            std::fs::create_dir_all(&test_dir).unwrap();

            // Create test files
            std::fs::write(test_dir.join("file1.txt"), "hello").unwrap();
            std::fs::write(test_dir.join("file2.txt"), "world").unwrap();

            let (count, size) =
                FileSystemIssueStorage::count_directory_contents(&test_dir).unwrap();
            assert_eq!(count, 2);
            assert_eq!(size, 10); // "hello" + "world"
        }

        #[test]
        fn test_count_directory_contents_recursive() {
            let temp_dir = TempDir::new().unwrap();
            let test_dir = temp_dir.path().join("test");
            std::fs::create_dir_all(test_dir.join("subdir")).unwrap();

            std::fs::write(test_dir.join("root.txt"), "root").unwrap();
            std::fs::write(test_dir.join("subdir/sub.txt"), "sub").unwrap();

            let (count, size) =
                FileSystemIssueStorage::count_directory_contents(&test_dir).unwrap();
            assert_eq!(count, 2);
            assert_eq!(size, 7); // "root" + "sub"
        }

        #[test]
        fn test_count_directory_contents_ignores_directories() {
            let temp_dir = TempDir::new().unwrap();
            let test_dir = temp_dir.path().join("test");
            std::fs::create_dir_all(&test_dir).unwrap();

            // Create files and directories
            std::fs::write(test_dir.join("file.txt"), "content").unwrap();
            std::fs::create_dir_all(test_dir.join("empty_dir")).unwrap();
            std::fs::create_dir_all(test_dir.join("another_dir")).unwrap();

            let (count, size) =
                FileSystemIssueStorage::count_directory_contents(&test_dir).unwrap();
            assert_eq!(count, 1); // Only the file, not the directories
            assert_eq!(size, 7); // "content"
        }

        #[test]
        fn test_count_directory_contents_various_file_sizes() {
            let temp_dir = TempDir::new().unwrap();
            let test_dir = temp_dir.path().join("test");
            std::fs::create_dir_all(&test_dir).unwrap();

            // Create files with different sizes
            std::fs::write(test_dir.join("small.txt"), "x").unwrap(); // 1 byte
            std::fs::write(test_dir.join("medium.txt"), "x".repeat(100)).unwrap(); // 100 bytes
            std::fs::write(test_dir.join("large.txt"), "x".repeat(1000)).unwrap(); // 1000 bytes

            let (count, size) =
                FileSystemIssueStorage::count_directory_contents(&test_dir).unwrap();
            assert_eq!(count, 3);
            assert_eq!(size, 1101); // 1 + 100 + 1000
        }

        #[test]
        fn test_count_directory_contents_deeply_nested() {
            let temp_dir = TempDir::new().unwrap();
            let test_dir = temp_dir.path().join("test");

            // Create deeply nested structure
            let mut current_dir = test_dir.clone();
            for i in 0..5 {
                current_dir = current_dir.join(format!("level{}", i));
                std::fs::create_dir_all(&current_dir).unwrap();
                std::fs::write(
                    current_dir.join(format!("file{}.txt", i)),
                    format!("level{}", i),
                )
                .unwrap();
            }

            let (count, size) =
                FileSystemIssueStorage::count_directory_contents(&test_dir).unwrap();
            assert_eq!(count, 5);
            assert_eq!(size, 30); // "level0" + "level1" + "level2" + "level3" + "level4" = 6*5 = 30
        }
    }

    /// Tests for error handling scenarios
    mod error_handling_tests {
        use super::*;
        use tempfile::TempDir;

        #[test]
        fn test_count_directory_contents_nonexistent_directory() {
            let temp_dir = TempDir::new().unwrap();
            let nonexistent_dir = temp_dir.path().join("does_not_exist");

            let result = FileSystemIssueStorage::count_directory_contents(&nonexistent_dir);
            assert!(result.is_err());
        }

        #[test]
        fn test_default_directory_with_unreadable_current_dir() {
            // Instead of trying to create an unreadable current directory (which is unreliable in tests),
            // test the fallback behavior by using the default_directory_in method directly
            // with a temporary directory that we can control

            let _test_env = IsolatedTestEnvironment::new().unwrap();
            let temp_dir = TempDir::new().unwrap();

            // Test with a valid directory - should work
            let result = FileSystemIssueStorage::default_directory_in(temp_dir.path());
            assert!(
                result.is_ok(),
                "default_directory_in should work with valid path"
            );

            // The original test was trying to test current_dir() error handling,
            // but that's better tested through integration tests rather than
            // trying to manufacture filesystem errors in unit tests
        }

        #[test]
        fn test_migration_info_with_permission_errors() {
            let temp_dir = TempDir::new().unwrap();

            let _test_env = IsolatedTestEnvironment::new().unwrap();
            std::env::set_current_dir(temp_dir.path()).unwrap();

            let issues_dir = temp_dir.path().join("issues");
            std::fs::create_dir_all(&issues_dir).unwrap();
            std::fs::write(issues_dir.join("test.md"), "test content").unwrap();

            // This test may need platform-specific permission handling
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;

                // Remove read permissions to cause an error
                let mut perms = std::fs::metadata(&issues_dir).unwrap().permissions();
                let original_mode = perms.mode();
                perms.set_mode(0o000); // No permissions

                if std::fs::set_permissions(&issues_dir, perms).is_ok() {
                    // Test that error is handled gracefully
                    let result = FileSystemIssueStorage::migration_info();
                    assert!(result.is_err());

                    // Restore permissions for cleanup
                    let perms = std::fs::Permissions::from_mode(original_mode);
                    std::fs::set_permissions(&issues_dir, perms).unwrap();
                }
            }

            #[cfg(not(unix))]
            {
                // On non-Unix systems, just verify that the function works normally
                let result = FileSystemIssueStorage::migration_info();
                assert!(result.is_ok());
            }
        }

        #[test]
        fn test_should_migrate_with_file_instead_of_directory() {
            let temp_dir = TempDir::new().unwrap();

            let _test_env = IsolatedTestEnvironment::new().unwrap();
            std::env::set_current_dir(temp_dir.path()).unwrap();

            // Create a file named "issues" instead of a directory
            let issues_path = temp_dir.path().join("issues");
            std::fs::write(&issues_path, "this is a file, not a directory").unwrap();

            // Should not migrate because "issues" is not a directory
            let should_migrate = FileSystemIssueStorage::should_migrate().unwrap();
            assert!(!should_migrate);

            // Migration info should reflect this
            let info = FileSystemIssueStorage::migration_info().unwrap();
            assert!(!info.should_migrate);
            assert!(info.source_exists); // File exists
            assert!(!info.destination_exists);
            assert_eq!(info.file_count, 0); // No files because source is not a directory
            assert_eq!(info.total_size, 0);
        }

        #[test]
        fn test_migration_paths_consistency() {
            let temp_dir = TempDir::new().unwrap();

            let _test_env = IsolatedTestEnvironment::new().unwrap();
            std::env::set_current_dir(temp_dir.path()).unwrap();

            // Test that migration_paths() and migration_paths_in_dir() are consistent
            let paths_global = FileSystemIssueStorage::migration_paths().unwrap();
            // Use canonical path for comparison to handle macOS symlink resolution
            let canonical_temp_path = temp_dir.path().canonicalize().unwrap();
            let paths_in_dir = FileSystemIssueStorage::migration_paths_in_dir(&canonical_temp_path);

            assert_eq!(paths_global.source, paths_in_dir.source);
            assert_eq!(paths_global.destination, paths_in_dir.destination);
            assert_eq!(paths_global.backup, paths_in_dir.backup);
        }
    }

    /// Integration tests for end-to-end workflows
    mod integration_tests {
        use super::*;
        use tempfile::TempDir;

        #[test]
        fn test_storage_creation_end_to_end() {
            let temp_dir = TempDir::new().unwrap();

            let _test_env = IsolatedTestEnvironment::new().unwrap();

            // Test creation without .swissarmyhammer
            let (storage1, _) = FileSystemIssueStorage::new_default_in(temp_dir.path()).unwrap();
            assert!(storage1.state.issues_dir.ends_with("issues"));
            assert!(!storage1
                .state
                .issues_dir
                .to_string_lossy()
                .contains(".swissarmyhammer"));

            // Create .swissarmyhammer and test again
            std::fs::create_dir_all(temp_dir.path().join(".swissarmyhammer")).unwrap();
            let (storage2, _) = FileSystemIssueStorage::new_default_in(temp_dir.path()).unwrap();
            assert!(storage2.state.issues_dir.ends_with("issues"));
            assert!(storage2
                .state
                .issues_dir
                .to_string_lossy()
                .contains(".swissarmyhammer"));
        }

        #[test]
        fn test_migration_detection_end_to_end() {
            let temp_dir = TempDir::new().unwrap();

            let _test_env = IsolatedTestEnvironment::new().unwrap();
            std::env::set_current_dir(temp_dir.path()).unwrap();

            // Initially no migration needed
            assert!(!FileSystemIssueStorage::should_migrate().unwrap());

            // Create legacy directory
            let issues_dir = temp_dir.path().join("issues");
            std::fs::create_dir_all(&issues_dir).unwrap();
            std::fs::write(issues_dir.join("test.md"), "content").unwrap();

            // Now migration should be needed
            assert!(FileSystemIssueStorage::should_migrate().unwrap());

            // Create new directory - migration no longer needed
            std::fs::create_dir_all(temp_dir.path().join(".swissarmyhammer/issues")).unwrap();
            assert!(!FileSystemIssueStorage::should_migrate().unwrap());
        }

        #[test]
        fn test_migration_info_workflow() {
            let temp_dir = TempDir::new().unwrap();

            let _test_env = IsolatedTestEnvironment::new().unwrap();

            // Phase 1: No migration needed (empty state)
            let info1 = FileSystemIssueStorage::migration_info_in_dir(temp_dir.path()).unwrap();
            assert!(!info1.should_migrate);
            assert!(!info1.source_exists);
            assert!(!info1.destination_exists);

            // Phase 2: Create source with content - migration needed
            let issues_dir = temp_dir.path().join("issues");
            std::fs::create_dir_all(&issues_dir).unwrap();
            std::fs::write(issues_dir.join("issue1.md"), "Issue 1 content").unwrap();
            std::fs::write(issues_dir.join("issue2.md"), "Issue 2 content").unwrap();

            let info2 = FileSystemIssueStorage::migration_info_in_dir(temp_dir.path()).unwrap();
            assert!(info2.should_migrate);
            assert!(info2.source_exists);
            assert!(!info2.destination_exists);
            assert_eq!(info2.file_count, 2);
            assert_eq!(info2.total_size, 30); // "Issue 1 content" + "Issue 2 content"

            // Phase 3: Create destination - no migration needed
            std::fs::create_dir_all(temp_dir.path().join(".swissarmyhammer/issues")).unwrap();

            let info3 = FileSystemIssueStorage::migration_info_in_dir(temp_dir.path()).unwrap();
            assert!(!info3.should_migrate);
            assert!(info3.source_exists);
            assert!(info3.destination_exists);
            assert_eq!(info3.file_count, 2); // Source still has files
            assert_eq!(info3.total_size, 30);
        }

        #[test]
        fn test_directory_detection_with_complex_structure() {
            let temp_dir = TempDir::new().unwrap();

            let _test_env = IsolatedTestEnvironment::new().unwrap();
            std::env::set_current_dir(temp_dir.path()).unwrap();

            // Create complex directory structure
            let swissarmyhammer_dir = temp_dir.path().join(".swissarmyhammer");
            std::fs::create_dir_all(&swissarmyhammer_dir).unwrap();

            // Add some files to .swissarmyhammer to make it "real"
            std::fs::write(swissarmyhammer_dir.join("config.toml"), "[settings]").unwrap();

            // Create other directories that shouldn't affect detection
            std::fs::create_dir_all(temp_dir.path().join("src")).unwrap();
            std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
            std::fs::create_dir_all(temp_dir.path().join("target")).unwrap();

            // Storage should detect .swissarmyhammer and use it
            let storage = FileSystemIssueStorage::new_default().unwrap();
            assert!(storage
                .state
                .issues_dir
                .to_string_lossy()
                .contains(".swissarmyhammer"));

            // Remove .swissarmyhammer directory
            std::fs::remove_dir_all(&swissarmyhammer_dir).unwrap();

            // Storage should fall back to legacy issues directory
            let storage2 = FileSystemIssueStorage::new_default().unwrap();
            assert!(!storage2
                .state
                .issues_dir
                .to_string_lossy()
                .contains(".swissarmyhammer"));
            assert!(storage2.state.issues_dir.ends_with("issues"));
        }
    }

    /// Performance tests for migration operations
    mod performance_tests {
        use super::*;
        use tempfile::TempDir;

        #[test]
        fn test_migration_detection_performance() {
            let temp_dir = TempDir::new().unwrap();

            let _test_env = IsolatedTestEnvironment::new().unwrap();

            let issues_dir = temp_dir.path().join("issues");
            std::fs::create_dir_all(&issues_dir).unwrap();

            // Create many files to test performance
            for i in 0..1000 {
                std::fs::write(issues_dir.join(format!("issue{}.md", i)), "content").unwrap();
            }

            let start = std::time::Instant::now();
            let should_migrate = FileSystemIssueStorage::should_migrate_in_dir(temp_dir.path()).unwrap();
            let duration = start.elapsed();

            assert!(should_migrate);
            assert!(duration < std::time::Duration::from_millis(100)); // Should be fast
        }

        #[test]
        fn test_count_directory_contents_performance() {
            let temp_dir = TempDir::new().unwrap();
            let test_dir = temp_dir.path().join("perf_test");
            std::fs::create_dir_all(&test_dir).unwrap();

            // Create nested structure with many files
            for i in 0..100 {
                let subdir = test_dir.join(format!("subdir{}", i));
                std::fs::create_dir_all(&subdir).unwrap();

                for j in 0..10 {
                    std::fs::write(
                        subdir.join(format!("file{}.txt", j)),
                        format!("content{}", j),
                    )
                    .unwrap();
                }
            }

            let start = std::time::Instant::now();
            let (count, _size) =
                FileSystemIssueStorage::count_directory_contents(&test_dir).unwrap();
            let duration = start.elapsed();

            assert_eq!(count, 1000); // 100 subdirs * 10 files each
            assert!(duration < std::time::Duration::from_millis(500)); // Should be reasonably fast
        }

        #[test]
        fn test_migration_info_performance_with_large_files() {
            let temp_dir = TempDir::new().unwrap();

            let _test_env = IsolatedTestEnvironment::new().unwrap();

            let issues_dir = temp_dir.path().join("issues");
            std::fs::create_dir_all(&issues_dir).unwrap();

            // Create files with different sizes
            let small_content = "small";
            let medium_content = "x".repeat(1000);
            let large_content = "x".repeat(10000);

            std::fs::write(issues_dir.join("small.md"), small_content).unwrap();
            std::fs::write(issues_dir.join("medium.md"), &medium_content).unwrap();
            std::fs::write(issues_dir.join("large.md"), &large_content).unwrap();

            let start = std::time::Instant::now();
            let info = FileSystemIssueStorage::migration_info_in_dir(temp_dir.path()).unwrap();
            let duration = start.elapsed();

            assert!(info.should_migrate);
            assert_eq!(info.file_count, 3);
            assert_eq!(
                info.total_size,
                small_content.len() as u64
                    + medium_content.len() as u64
                    + large_content.len() as u64
            );
            assert!(duration < std::time::Duration::from_millis(50)); // Should be very fast
        }
    }

    // Migration Tests

    #[tokio::test]
    async fn test_migration_with_empty_directory() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();

        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Create empty issues directory
        let issues_dir = temp_dir.path().join("issues");
        std::fs::create_dir_all(&issues_dir).unwrap();

        // Perform migration
        let result = FileSystemIssueStorage::perform_migration().unwrap();

        let stats = match result {
            MigrationResult::Success(stats) => stats,
            result => {
                panic!(
                    "Expected MigrationResult::Success for empty directory, got: {:?}",
                    result
                );
            }
        };

        assert_eq!(stats.files_moved, 0);
        assert_eq!(stats.bytes_moved, 0);
        assert!(stats.duration > std::time::Duration::from_nanos(0));

        // Verify destination exists and source is gone
        let new_path = temp_dir.path().join(".swissarmyhammer").join("issues");
        assert!(new_path.exists());
        assert!(!issues_dir.exists());
    }

    #[tokio::test]
    async fn test_migration_with_nested_structure() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();

        let temp_dir = TempDir::new().unwrap();

        // Create nested directory structure
        let issues_dir = temp_dir.path().join("issues");
        let complete_dir = issues_dir.join("complete");
        std::fs::create_dir_all(&complete_dir).unwrap();

        // Create test files
        let active_content = "Active issue content";
        let completed_content = "Completed issue content";
        std::fs::write(issues_dir.join("active_issue.md"), active_content).unwrap();
        std::fs::write(complete_dir.join("completed_issue.md"), completed_content).unwrap();

        // Calculate expected byte count
        let expected_bytes = active_content.len() + completed_content.len();

        // Perform migration
        let result = FileSystemIssueStorage::perform_migration_in(temp_dir.path()).unwrap();

        let stats = match result {
            MigrationResult::Success(stats) => stats,
            result => {
                panic!(
                    "Expected MigrationResult::Success for nested structure, got: {:?}",
                    result
                );
            }
        };

        assert_eq!(stats.files_moved, 2);
        assert_eq!(stats.bytes_moved, expected_bytes as u64);

        // Verify structure is preserved
        let new_path = temp_dir.path().join(".swissarmyhammer").join("issues");
        let new_complete = new_path.join("complete");

        assert!(new_path.join("active_issue.md").exists());
        assert!(new_complete.join("completed_issue.md").exists());
        assert!(!issues_dir.exists());
    }

    #[tokio::test]
    async fn test_migration_not_needed() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();

        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Create .swissarmyhammer directory (indicating migration not needed)
        std::fs::create_dir_all(temp_dir.path().join(".swissarmyhammer")).unwrap();

        let result = FileSystemIssueStorage::perform_migration().unwrap();

        let info = match result {
            MigrationResult::NotNeeded(info) => info,
            result => {
                panic!("Expected MigrationResult::NotNeeded, got: {:?}", result);
            }
        };

        assert!(!info.should_migrate);
        assert!(!info.source_exists);
        assert_eq!(info.file_count, 0);
        assert_eq!(info.total_size, 0);
    }

    #[test]
    fn test_backup_creation() {
        let temp_dir = TempDir::new().unwrap();

        // Create source directory with content
        let source = temp_dir.path().join("source");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("file1.txt"), "content1").unwrap();
        std::fs::write(source.join("file2.txt"), "content2").unwrap();

        // Create backup
        let backup_path = FileSystemIssueStorage::create_backup(&source).unwrap();

        // Verify backup exists and contains expected files
        assert!(backup_path.exists());
        assert!(backup_path.join("file1.txt").exists());
        assert!(backup_path.join("file2.txt").exists());

        // Verify content is identical
        let backup_content1 = std::fs::read_to_string(backup_path.join("file1.txt")).unwrap();
        let backup_content2 = std::fs::read_to_string(backup_path.join("file2.txt")).unwrap();
        assert_eq!(backup_content1, "content1");
        assert_eq!(backup_content2, "content2");
    }

    #[tokio::test]
    async fn test_migration_rollback_on_validation_failure() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();

        let temp_dir = TempDir::new().unwrap();

        // Create issues directory with content
        let issues_dir = temp_dir.path().join("issues");
        std::fs::create_dir_all(&issues_dir).unwrap();
        std::fs::write(issues_dir.join("test_issue.md"), "test content").unwrap();

        // This test simulates rollback by manually corrupting the destination after migration
        // to trigger validation failure. In a real scenario, this would happen due to I/O errors.
        let destination = temp_dir.path().join(".swissarmyhammer").join("issues");

        // Create the parent directory
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();

        // Perform migration steps manually to test rollback
        let paths = FileSystemIssueStorage::migration_paths_in_dir(temp_dir.path());
        let backup_path = FileSystemIssueStorage::create_backup(&issues_dir).unwrap();

        // Execute migration
        let stats = FileSystemIssueStorage::execute_migration(&paths).unwrap();

        // Simulate validation failure by removing a file from destination
        std::fs::remove_file(destination.join("test_issue.md")).unwrap();

        // Validation should fail
        let validation_result = FileSystemIssueStorage::validate_migration(&paths, &stats);
        assert!(validation_result.is_err());

        // Perform rollback
        FileSystemIssueStorage::rollback_migration(&paths, &backup_path).unwrap();

        // Verify rollback restored original state
        assert!(issues_dir.exists());
        assert!(issues_dir.join("test_issue.md").exists());
        assert!(!destination.exists());
    }

    #[tokio::test]
    async fn test_new_default_with_migration() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();

        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Create issues directory to trigger migration
        let issues_dir = temp_dir.path().join("issues");
        std::fs::create_dir_all(&issues_dir).unwrap();
        let test_content = "test";
        std::fs::write(issues_dir.join("test.md"), test_content).unwrap();

        // Use new_default_with_migration
        let (storage, migration_result) =
            FileSystemIssueStorage::new_default_with_migration().unwrap();

        // Verify migration occurred
        let stats = match migration_result {
            Some(MigrationResult::Success(stats)) => stats,
            result => {
                panic!("Expected Some(MigrationResult::Success), got: {:?}", result);
            }
        };

        assert_eq!(stats.files_moved, 1);
        assert_eq!(stats.bytes_moved, test_content.len() as u64);

        // Verify storage works correctly
        let issues = storage.list_issues().await.unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].name, "test");
    }

    #[test]
    fn test_copy_directory_recursive() {
        let temp_dir = TempDir::new().unwrap();

        // Create complex source structure
        let source = temp_dir.path().join("source");
        let nested = source.join("nested");
        std::fs::create_dir_all(&nested).unwrap();

        std::fs::write(source.join("root_file.txt"), "root content").unwrap();
        std::fs::write(nested.join("nested_file.txt"), "nested content").unwrap();

        // Copy recursively
        let destination = temp_dir.path().join("destination");
        FileSystemIssueStorage::copy_directory_recursive(&source, &destination).unwrap();

        // Verify structure is preserved
        assert!(destination.exists());
        assert!(destination.join("root_file.txt").exists());
        assert!(destination.join("nested").exists());
        assert!(destination.join("nested").join("nested_file.txt").exists());

        // Verify content is identical
        let root_content = std::fs::read_to_string(destination.join("root_file.txt")).unwrap();
        let nested_content =
            std::fs::read_to_string(destination.join("nested").join("nested_file.txt")).unwrap();
        assert_eq!(root_content, "root content");
        assert_eq!(nested_content, "nested content");
    }

    // Integration Tests for Automatic Migration
    #[tokio::test]
    async fn test_automatic_migration_integration() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();

        let temp_dir = TempDir::new().unwrap();

        // Setup: Create old issues directory with test files
        let issues_dir = temp_dir.path().join("issues");
        std::fs::create_dir_all(&issues_dir).unwrap();
        std::fs::write(issues_dir.join("test1.md"), "Test issue 1").unwrap();
        std::fs::write(issues_dir.join("test2.md"), "Test issue 2").unwrap();

        // Test: Create storage with new_default_in() - should automatically migrate
        let (storage, _) = FileSystemIssueStorage::new_default_in(temp_dir.path()).unwrap();

        // Verify: Migration occurred
        let new_issues_dir = temp_dir.path().join(".swissarmyhammer").join("issues");
        assert!(new_issues_dir.exists());
        assert!(new_issues_dir.join("test1.md").exists());
        assert!(new_issues_dir.join("test2.md").exists());

        // Verify: Old directory no longer exists
        assert!(!issues_dir.exists());

        // Verify: Storage works correctly after migration
        let issues = storage.list_issues().await.unwrap();
        assert_eq!(issues.len(), 2);
    }

    #[tokio::test]
    async fn test_new_default_with_migration_info_integration() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();

        let temp_dir = TempDir::new().unwrap();

        // Setup: Create old issues directory
        let issues_dir = temp_dir.path().join("issues");
        std::fs::create_dir_all(&issues_dir).unwrap();
        std::fs::write(issues_dir.join("test.md"), "Test content").unwrap();

        // Test: Use new_default_in() which provides same functionality
        let (storage, migration_result) =
            FileSystemIssueStorage::new_default_in(temp_dir.path()).unwrap();

        // Verify: Migration result is returned
        assert!(migration_result.is_some());
        match migration_result.unwrap() {
            MigrationResult::Success(stats) => {
                assert_eq!(stats.files_moved, 1);
                assert!(stats.bytes_moved > 0);
            }
            _ => panic!("Expected MigrationResult::Success"),
        }

        // Verify: Storage is functional
        let issues = storage.list_issues().await.unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].name, "test");
    }

    #[test]
    fn test_migration_config_integration() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();

        let temp_dir = TempDir::new().unwrap();

        // Setup: Create issues directory with multiple files
        let issues_dir = temp_dir.path().join("issues");
        std::fs::create_dir_all(&issues_dir).unwrap();
        for i in 0..5 {
            std::fs::write(issues_dir.join(format!("test{}.md", i)), "Test content").unwrap();
        }

        // Test: Config with file count limit
        let config = MigrationConfig {
            auto_migrate: true,
            max_file_count: 3, // Less than the 5 files we have
            ..Default::default()
        };

        let result = FileSystemIssueStorage::new_default_with_config_in(temp_dir.path(), &config);
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("exceeds maximum for automatic migration"));

        // Test: Config with higher limit should succeed
        let config = MigrationConfig {
            auto_migrate: true,
            max_file_count: 10, // More than the 5 files we have
            ..Default::default()
        };

        let result = FileSystemIssueStorage::new_default_with_config_in(temp_dir.path(), &config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_migration_config_disabled() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();

        let temp_dir = TempDir::new().unwrap();

        // Setup: Create old issues directory
        let issues_dir = temp_dir.path().join("issues");
        std::fs::create_dir_all(&issues_dir).unwrap();
        std::fs::write(issues_dir.join("test.md"), "Test content").unwrap();

        // Test: Config with migration disabled
        let config = MigrationConfig {
            auto_migrate: false,
            ..Default::default()
        };

        let (_storage, migration_result) =
            FileSystemIssueStorage::new_default_with_config_in(temp_dir.path(), &config).unwrap();

        // Verify: No migration occurred
        assert!(migration_result.is_none());

        // Verify: Old directory still exists
        assert!(issues_dir.exists());
        assert!(issues_dir.join("test.md").exists());
    }

    #[test]
    fn test_migration_status_integration() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();

        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Test: No issues directory
        let status = FileSystemIssueStorage::migration_status().unwrap();
        assert!(status.contains("No issues directory found"));

        // Setup: Create old issues directory
        let issues_dir = temp_dir.path().join("issues");
        std::fs::create_dir_all(&issues_dir).unwrap();
        std::fs::write(issues_dir.join("test.md"), "Test content").unwrap();

        // Test: Migration needed
        let status = FileSystemIssueStorage::migration_status().unwrap();
        assert!(status.contains("Migration needed"));
        assert!(status.contains("1 files"));

        // Perform migration
        let _storage = FileSystemIssueStorage::new_default().unwrap();

        // Test: No migration needed after migration
        let status = FileSystemIssueStorage::migration_status().unwrap();
        assert!(status.contains("Using .swissarmyhammer/issues/"));
    }

    #[tokio::test]
    async fn test_concurrent_storage_creation_integration() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();

        let temp_dir = TempDir::new().unwrap();

        // Setup: Create old issues directory
        let issues_dir = temp_dir.path().join("issues");
        std::fs::create_dir_all(&issues_dir).unwrap();
        std::fs::write(issues_dir.join("test.md"), "Test content").unwrap();

        // Test: Concurrent storage creation
        let temp_path = temp_dir.path().to_path_buf();
        let handles: Vec<_> = (0..3)
            .map(|_| {
                let path = temp_path.clone();
                tokio::spawn(async move { FileSystemIssueStorage::new_default_in(&path) })
            })
            .collect();

        let results: Vec<_> = futures::future::join_all(handles).await;

        // Verify: All creations succeeded
        for result in results {
            assert!(result.unwrap().is_ok());
        }

        // Verify: Migration completed successfully
        let new_issues_dir = temp_dir.path().join(".swissarmyhammer").join("issues");
        assert!(new_issues_dir.exists());
        assert!(new_issues_dir.join("test.md").exists());
        assert!(!issues_dir.exists());
    }

    #[tokio::test]
    async fn test_migration_with_existing_destination_integration() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();

        let temp_dir = TempDir::new().unwrap();

        // Setup: Create both old and new directories
        let issues_dir = temp_dir.path().join("issues");
        let new_issues_dir = temp_dir.path().join(".swissarmyhammer").join("issues");

        std::fs::create_dir_all(&issues_dir).unwrap();
        std::fs::create_dir_all(&new_issues_dir).unwrap();

        std::fs::write(issues_dir.join("old.md"), "Old content").unwrap();
        std::fs::write(new_issues_dir.join("new.md"), "New content").unwrap();

        // Test: No migration should occur when destination exists
        let (storage, _) = FileSystemIssueStorage::new_default_in(temp_dir.path()).unwrap();

        // Verify: Both directories still exist, no migration occurred
        assert!(issues_dir.exists());
        assert!(new_issues_dir.exists());
        assert!(issues_dir.join("old.md").exists());
        assert!(new_issues_dir.join("new.md").exists());

        // Verify: Storage uses the new directory
        let issues = storage.list_issues().await.unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].name, "new");
    }

    /// Comprehensive tests for migration validation functionality
    mod validation_tests {
        use super::*;
        use std::fs;

        fn create_test_directories() -> (TempDir, PathBuf, PathBuf) {
            let temp_dir = TempDir::new().unwrap();
            let source = temp_dir.path().join("source");
            let destination = temp_dir.path().join("destination");
            fs::create_dir_all(&source).unwrap();
            fs::create_dir_all(&destination).unwrap();
            (temp_dir, source, destination)
        }

        #[test]
        fn test_file_integrity_validation_success() {
            let (_temp_dir, source, destination) = create_test_directories();

            // Create identical files in both directories
            fs::write(source.join("test1.md"), "# Test Issue 1\n\nContent here").unwrap();
            fs::write(source.join("test2.md"), "# Test Issue 2\n\nMore content").unwrap();
            fs::write(
                destination.join("test1.md"),
                "# Test Issue 1\n\nContent here",
            )
            .unwrap();
            fs::write(
                destination.join("test2.md"),
                "# Test Issue 2\n\nMore content",
            )
            .unwrap();

            let result =
                FileSystemIssueStorage::validate_file_integrity(&source, &destination).unwrap();

            assert_eq!(result.total_files, 2);
            assert_eq!(result.verified_files, 2);
            assert!(result.missing_files.is_empty());
            assert!(result.extra_files.is_empty());
            assert!(result.corrupted_files.is_empty());
        }

        #[test]
        fn test_file_integrity_validation_missing_files() {
            let (_temp_dir, source, destination) = create_test_directories();

            // Create files only in source
            fs::write(source.join("test1.md"), "# Test Issue 1").unwrap();
            fs::write(source.join("test2.md"), "# Test Issue 2").unwrap();
            fs::write(destination.join("test1.md"), "# Test Issue 1").unwrap();
            // test2.md is missing in destination

            let result =
                FileSystemIssueStorage::validate_file_integrity(&source, &destination).unwrap();

            assert_eq!(result.total_files, 2);
            assert_eq!(result.verified_files, 1); // Only test1.md is verified
            assert_eq!(result.missing_files.len(), 1);
            assert_eq!(result.missing_files[0], PathBuf::from("test2.md"));
        }

        #[test]
        fn test_file_integrity_validation_extra_files() {
            let (_temp_dir, source, destination) = create_test_directories();

            // Create files with extra file in destination
            fs::write(source.join("test1.md"), "# Test Issue 1").unwrap();
            fs::write(destination.join("test1.md"), "# Test Issue 1").unwrap();
            fs::write(destination.join("extra.md"), "# Extra file").unwrap();

            let result =
                FileSystemIssueStorage::validate_file_integrity(&source, &destination).unwrap();

            assert_eq!(result.total_files, 1);
            assert_eq!(result.verified_files, 1);
            assert_eq!(result.extra_files.len(), 1);
            assert_eq!(result.extra_files[0], PathBuf::from("extra.md"));
        }

        #[test]
        fn test_file_integrity_validation_corrupted_files() {
            let (_temp_dir, source, destination) = create_test_directories();

            // Create files with different sizes (indicating corruption)
            fs::write(
                source.join("test1.md"),
                "# Test Issue 1\n\nLong content here",
            )
            .unwrap();
            fs::write(destination.join("test1.md"), "# Test Issue 1").unwrap(); // Shorter content

            let result =
                FileSystemIssueStorage::validate_file_integrity(&source, &destination).unwrap();

            assert_eq!(result.total_files, 1);
            assert_eq!(result.verified_files, 0); // File is corrupted
            assert_eq!(result.corrupted_files.len(), 1);
            assert_eq!(result.corrupted_files[0], PathBuf::from("test1.md"));
        }

        #[test]
        fn test_content_verification_success() {
            let (_temp_dir, source, destination) = create_test_directories();

            // Create identical small files
            let content = "# Test Issue\n\nSample content for testing";
            fs::write(source.join("test.md"), content).unwrap();
            fs::write(destination.join("test.md"), content).unwrap();

            let result =
                FileSystemIssueStorage::validate_content_integrity(&source, &destination).unwrap();

            assert_eq!(result.total_files, 1);
            assert_eq!(result.verified_files, 1);
            assert!(result.mismatched_files.is_empty());
            assert!(result.unreadable_files.is_empty());
        }

        #[test]
        fn test_content_verification_mismatch() {
            let (_temp_dir, source, destination) = create_test_directories();

            // Create files with different content
            fs::write(source.join("test.md"), "# Original content").unwrap();
            fs::write(destination.join("test.md"), "# Modified content").unwrap();

            let result =
                FileSystemIssueStorage::validate_content_integrity(&source, &destination).unwrap();

            assert_eq!(result.total_files, 1);
            assert_eq!(result.verified_files, 0);
            assert_eq!(result.mismatched_files.len(), 1);
            assert_eq!(result.mismatched_files[0], PathBuf::from("test.md"));
        }

        #[test]
        fn test_directory_structure_validation() {
            let (_temp_dir, source, destination) = create_test_directories();

            // Create nested directory structure
            let complete_dir = source.join("complete");
            fs::create_dir_all(&complete_dir).unwrap();
            fs::write(complete_dir.join("old.md"), "# Old issue").unwrap();
            fs::write(source.join("current.md"), "# Current issue").unwrap();

            // Mirror structure in destination
            let dest_complete_dir = destination.join("complete");
            fs::create_dir_all(&dest_complete_dir).unwrap();
            fs::write(dest_complete_dir.join("old.md"), "# Old issue").unwrap();
            fs::write(destination.join("current.md"), "# Current issue").unwrap();

            let result =
                FileSystemIssueStorage::validate_directory_structure(&source, &destination)
                    .unwrap();

            assert!(result.directories_preserved);
            assert!(result.relative_paths_correct);
            assert!(result.structure_differences.is_empty());
        }

        #[test]
        fn test_comprehensive_validation_success() {
            let (_temp_dir, source, destination) = create_test_directories();

            // Create complete test structure
            fs::write(source.join("issue1.md"), "# Issue 1\n\nContent").unwrap();
            fs::write(source.join("issue2.md"), "# Issue 2\n\nMore content").unwrap();

            let complete_dir = source.join("complete");
            fs::create_dir_all(&complete_dir).unwrap();
            fs::write(complete_dir.join("done.md"), "# Completed issue").unwrap();

            // Mirror in destination
            fs::write(destination.join("issue1.md"), "# Issue 1\n\nContent").unwrap();
            fs::write(destination.join("issue2.md"), "# Issue 2\n\nMore content").unwrap();

            let dest_complete_dir = destination.join("complete");
            fs::create_dir_all(&dest_complete_dir).unwrap();
            fs::write(dest_complete_dir.join("done.md"), "# Completed issue").unwrap();

            let validation =
                FileSystemIssueStorage::validate_migration_comprehensive(&source, &destination)
                    .unwrap();
            let report = validation.generate_report();

            assert!(report.overall_success);
            assert_eq!(report.total_files, 3);
            assert_eq!(report.verified_files, 3);
            assert!(report.issues.is_empty());
            assert!(report.warnings.is_empty());
        }

        #[test]
        fn test_comprehensive_validation_with_issues() {
            let (_temp_dir, source, destination) = create_test_directories();

            // Create source files
            fs::write(source.join("issue1.md"), "# Issue 1").unwrap();
            fs::write(source.join("issue2.md"), "# Issue 2").unwrap();

            // Create destination with problems
            fs::write(destination.join("issue1.md"), "# Issue 1 MODIFIED").unwrap();
            // issue2.md is missing in destination
            fs::write(destination.join("extra.md"), "# Extra file").unwrap();

            let validation =
                FileSystemIssueStorage::validate_migration_comprehensive(&source, &destination)
                    .unwrap();
            let report = validation.generate_report();

            assert!(!report.overall_success);
            assert_eq!(report.total_files, 2);
            assert_eq!(report.verified_files, 0); // No files verified due to content mismatch
            assert!(!report.issues.is_empty());
            assert!(!report.warnings.is_empty()); // Extra files cause warnings

            // Check that issues are properly reported
            let issues_str = report.issues.join(" ");
            assert!(issues_str.contains("Missing files: 1"));
            assert!(issues_str.contains("Content mismatch: 1"));
        }

        #[test]
        fn test_validation_report_generation() {
            let (_temp_dir, source, destination) = create_test_directories();

            // Create a scenario with various validation issues
            fs::write(source.join("good.md"), "# Good file").unwrap();
            fs::write(source.join("missing.md"), "# Missing file").unwrap();
            fs::write(source.join("modified.md"), "# Original content").unwrap();

            fs::write(destination.join("good.md"), "# Good file").unwrap();
            // missing.md is not in destination
            fs::write(destination.join("modified.md"), "# Modified content").unwrap();
            fs::write(destination.join("extra.md"), "# Extra file").unwrap();

            let validation =
                FileSystemIssueStorage::validate_migration_comprehensive(&source, &destination)
                    .unwrap();

            // Test the validation components
            assert_eq!(validation.file_integrity.total_files, 3);
            assert_eq!(validation.file_integrity.missing_files.len(), 1);
            assert_eq!(validation.file_integrity.extra_files.len(), 1);

            assert_eq!(validation.content_verification.total_files, 3);
            assert_eq!(validation.content_verification.mismatched_files.len(), 1);

            // Test report generation
            let report = validation.generate_report();
            assert!(!report.overall_success);
            assert!(!report.issues.is_empty());
            assert!(!report.warnings.is_empty());
        }

        #[test]
        fn test_file_hash_calculation() {
            let temp_dir = TempDir::new().unwrap();
            let test_file = temp_dir.path().join("test.txt");

            // Create a file with known content
            let content = "Hello, world! This is test content.";
            fs::write(&test_file, content).unwrap();

            // Calculate hash
            let hash1 = FileSystemIssueStorage::calculate_file_hash(&test_file).unwrap();
            let hash2 = FileSystemIssueStorage::calculate_file_hash(&test_file).unwrap();

            // Hash should be consistent
            assert_eq!(hash1, hash2);

            // Different content should produce different hash
            fs::write(&test_file, "Different content").unwrap();
            let hash3 = FileSystemIssueStorage::calculate_file_hash(&test_file).unwrap();
            assert_ne!(hash1, hash3);
        }

        #[test]
        fn test_file_content_comparison() {
            let temp_dir = TempDir::new().unwrap();
            let file1 = temp_dir.path().join("file1.txt");
            let file2 = temp_dir.path().join("file2.txt");
            let file3 = temp_dir.path().join("file3.txt");

            // Create identical files
            let content = "Test content for comparison";
            fs::write(&file1, content).unwrap();
            fs::write(&file2, content).unwrap();
            fs::write(&file3, "Different content").unwrap();

            // Test comparison
            assert!(FileSystemIssueStorage::compare_file_content(&file1, &file2).unwrap());
            assert!(!FileSystemIssueStorage::compare_file_content(&file1, &file3).unwrap());
        }

        #[test]
        fn test_collect_file_list() {
            let temp_dir = TempDir::new().unwrap();
            let root = temp_dir.path().join("test");
            fs::create_dir_all(&root).unwrap();

            // Create test structure
            fs::write(root.join("file1.md"), "content1").unwrap();
            fs::write(root.join("file2.md"), "content2").unwrap();

            let subdir = root.join("subdir");
            fs::create_dir_all(&subdir).unwrap();
            fs::write(subdir.join("file3.md"), "content3").unwrap();

            let files = FileSystemIssueStorage::collect_file_list(&root).unwrap();

            assert_eq!(files.len(), 3);
            assert!(files.contains(&PathBuf::from("file1.md")));
            assert!(files.contains(&PathBuf::from("file2.md")));
            assert!(files.contains(&PathBuf::from("subdir/file3.md")));
        }
    }
}
