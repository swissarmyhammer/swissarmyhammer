//! Migration validation tools for SwissArmyHammer directory consolidation
//!
//! This module provides utilities to help users migrate from the old multiple-directory
//! system to the new Git repository-centric approach safely.
//!
//! # Features
//!
//! - Scan for existing `.swissarmyhammer` directories across the filesystem
//! - Detect conflicts and potential data loss scenarios  
//! - Generate migration recommendations and safety plans
//! - Validate migration readiness for specific Git repositories
//!
//! # Usage
//!
//! ```rust
//! use swissarmyhammer::migration::{scan_existing_directories, validate_migration_safety};
//!
//! // Scan current directory tree for all .swissarmyhammer directories
//! let scan_result = scan_existing_directories()?;
//! 
//! // Validate migration for a specific Git repository
//! let migration_plan = validate_migration_safety(&git_root_path)?;
//! ```

use crate::directory_utils::walk_files_with_extensions;
use crate::error::{Result, SwissArmyHammerError};
use crate::security::MAX_DIRECTORY_DEPTH;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Result of scanning for existing .swissarmyhammer directories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationScanResult {
    /// Git repositories found with their .swissarmyhammer status
    pub git_repositories: Vec<GitRepositoryInfo>,
    /// .swissarmyhammer directories not associated with Git repositories
    pub orphaned_directories: Vec<PathBuf>,
    /// Detected conflicts that could cause issues during migration
    pub conflicts: Vec<ConflictInfo>,
    /// Migration recommendations based on the scan
    pub recommendations: Vec<String>,
}

/// Information about a Git repository and its .swissarmyhammer directory status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRepositoryInfo {
    /// Path to the Git repository root
    pub path: PathBuf,
    /// Whether a .swissarmyhammer directory exists at the repository root
    pub has_swissarmyhammer: bool,
    /// Path to the .swissarmyhammer directory if it exists
    pub swissarmyhammer_path: Option<PathBuf>,
    /// Summary of content in the .swissarmyhammer directory
    pub content_summary: ContentSummary,
}

/// Summary of content found in a .swissarmyhammer directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSummary {
    /// Number of memo files found
    pub memos_count: usize,
    /// Number of todo files found
    pub todos_count: usize,
    /// Number of workflow files found
    pub workflows_count: usize,
    /// Whether a search database exists
    pub search_db_exists: bool,
    /// Other files that don't fit standard categories
    pub other_files: Vec<PathBuf>,
    /// Total size in bytes
    pub total_size_bytes: u64,
}

/// Information about a detected migration conflict
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictInfo {
    /// Path where the conflict occurs
    pub path: PathBuf,
    /// Type of conflict detected
    pub conflict_type: ConflictType,
    /// Human-readable description of the conflict
    pub description: String,
    /// Severity level of the conflict
    pub severity: ConflictSeverity,
}

/// Types of conflicts that can occur during migration
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConflictType {
    /// Same file exists in multiple locations with different content
    DuplicateData,
    /// Permission issues preventing access
    PermissionIssue,
    /// Path conflicts that prevent migration
    PathConflict,
    /// Migration would lose data
    DataLoss,
    /// Different versions of the same data exist
    VersionMismatch,
}

/// Severity levels for migration conflicts
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ConflictSeverity {
    /// Migration can proceed with warnings
    Low,
    /// Migration should be done carefully with user awareness
    Medium,
    /// Migration requires manual intervention
    High,
    /// Migration would cause data loss and should be blocked
    Critical,
}

/// A detailed plan for migrating .swissarmyhammer directories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationPlan {
    /// Source directories that will be consolidated
    pub source_directories: Vec<PathBuf>,
    /// Target directory where data will be consolidated
    pub target_directory: PathBuf,
    /// Specific actions to be taken during migration
    pub actions: Vec<MigrationAction>,
    /// Whether backup creation is recommended
    pub backup_needed: bool,
    /// Estimated duration for the migration
    pub estimated_duration: String,
}

/// Specific actions to be taken during migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MigrationAction {
    /// Create a backup of existing data
    CreateBackup { 
        /// Path to back up
        path: PathBuf 
    },
    /// Move files from one location to another
    MoveFiles { 
        /// Source path to move from
        from: PathBuf, 
        /// Target path to move to
        to: PathBuf 
    },
    /// Merge multiple files into a single target file
    MergeFiles { 
        /// Source files to merge
        sources: Vec<PathBuf>, 
        /// Target file for merged content
        target: PathBuf 
    },
    /// Remove empty directories after migration
    RemoveEmpty { 
        /// Path to remove if empty
        path: PathBuf 
    },
    /// Update references in files that point to old locations
    UpdateReferences { 
        /// File to update
        file: PathBuf, 
        /// Changes to make in the file
        changes: Vec<String> 
    },
}

/// Scan for existing .swissarmyhammer directories and validate migration readiness
///
/// This function performs a comprehensive scan starting from the current directory,
/// looking for both Git repositories and .swissarmyhammer directories. It analyzes
/// the relationship between them and detects potential migration conflicts.
///
/// # Returns
///
/// Returns a `MigrationScanResult` containing:
/// - All Git repositories found and their .swissarmyhammer status
/// - Orphaned .swissarmyhammer directories not associated with Git repositories  
/// - Detected conflicts with severity levels
/// - Migration recommendations
///
/// # Errors
///
/// Returns an error if:
/// - Current directory cannot be accessed
/// - File system traversal fails
/// - Permission issues prevent scanning
pub fn scan_existing_directories() -> Result<MigrationScanResult> {
    let current_dir = std::env::current_dir().map_err(SwissArmyHammerError::Io)?;
    scan_existing_directories_from(&current_dir)
}

/// Scan for existing .swissarmyhammer directories starting from a specific path
///
/// Internal implementation that allows testing with different starting directories.
///
/// # Arguments
///
/// * `start_path` - The directory to start scanning from
///
/// # Returns
///
/// Returns a `MigrationScanResult` with comprehensive analysis
fn scan_existing_directories_from(start_path: &Path) -> Result<MigrationScanResult> {
    let mut git_repositories = Vec::new();
    let mut orphaned_directories = Vec::new();
    let mut conflicts = Vec::new();
    
    // Track all found .swissarmyhammer directories for conflict detection
    let mut all_swissarmyhammer_dirs = HashSet::new();
    let mut git_repo_paths = HashSet::new();

    // First pass: Find all Git repositories
    for entry in WalkDir::new(start_path)
        .max_depth(MAX_DIRECTORY_DEPTH)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        
        // Check if this is a Git repository
        if path.join(".git").exists() {
            git_repo_paths.insert(path.to_path_buf());
            
            let swissarmyhammer_path = path.join(".swissarmyhammer");
            let has_swissarmyhammer = swissarmyhammer_path.exists() && swissarmyhammer_path.is_dir();
            
            let content_summary = if has_swissarmyhammer {
                all_swissarmyhammer_dirs.insert(swissarmyhammer_path.clone());
                analyze_swissarmyhammer_content(&swissarmyhammer_path)?
            } else {
                ContentSummary::empty()
            };
            
            git_repositories.push(GitRepositoryInfo {
                path: path.to_path_buf(),
                has_swissarmyhammer,
                swissarmyhammer_path: if has_swissarmyhammer {
                    Some(swissarmyhammer_path)
                } else {
                    None
                },
                content_summary,
            });
        }
        
        // Check if this is a .swissarmyhammer directory
        if path.file_name() == Some(std::ffi::OsStr::new(".swissarmyhammer")) && path.is_dir() {
            all_swissarmyhammer_dirs.insert(path.to_path_buf());
        }
    }

    // Second pass: Identify orphaned .swissarmyhammer directories
    for swissarmyhammer_dir in &all_swissarmyhammer_dirs {
        let parent = swissarmyhammer_dir.parent().unwrap_or(swissarmyhammer_dir);
        
        // Check if this .swissarmyhammer directory belongs to a known Git repository
        if !git_repo_paths.contains(parent) {
            // Check if it's inside any Git repository
            let mut is_orphaned = true;
            for git_repo in &git_repo_paths {
                if swissarmyhammer_dir.starts_with(git_repo) && *swissarmyhammer_dir != git_repo.join(".swissarmyhammer") {
                    // This is a nested .swissarmyhammer inside a Git repo but not at the root
                    conflicts.push(ConflictInfo {
                        path: swissarmyhammer_dir.clone(),
                        conflict_type: ConflictType::PathConflict,
                        description: format!(
                            "SwissArmyHammer directory found at {} but Git repository root is at {}",
                            swissarmyhammer_dir.display(),
                            git_repo.display()
                        ),
                        severity: ConflictSeverity::Medium,
                    });
                    is_orphaned = false;
                    break;
                }
            }
            
            if is_orphaned {
                orphaned_directories.push(swissarmyhammer_dir.clone());
            }
        }
    }

    // Detect conflicts between directories
    detect_content_conflicts(&all_swissarmyhammer_dirs, &mut conflicts)?;

    // Generate recommendations
    let recommendations = generate_migration_recommendations(
        &git_repositories,
        &orphaned_directories,
        &conflicts,
    );

    Ok(MigrationScanResult {
        git_repositories,
        orphaned_directories,
        conflicts,
        recommendations,
    })
}

/// Analyze the content of a .swissarmyhammer directory
///
/// # Arguments
///
/// * `path` - Path to the .swissarmyhammer directory
///
/// # Returns
///
/// Returns a `ContentSummary` with counts and details of found content
fn analyze_swissarmyhammer_content(path: &Path) -> Result<ContentSummary> {
    let mut content_summary = ContentSummary::empty();
    
    if !path.exists() || !path.is_dir() {
        return Ok(content_summary);
    }

    // Count memos
    let memos_dir = path.join("memos");
    if memos_dir.exists() {
        content_summary.memos_count = count_files_with_extensions(&memos_dir, &["md"])?;
    }

    // Count todos  
    let todo_dir = path.join("todo");
    if todo_dir.exists() {
        content_summary.todos_count = count_files_with_extensions(&todo_dir, &["yaml", "yml"])?;
    }

    // Count workflows
    let workflows_dir = path.join("workflows");
    if workflows_dir.exists() {
        content_summary.workflows_count = count_files_with_extensions(&workflows_dir, &["yaml", "yml"])?;
    }

    // Check for search database
    let search_db = path.join("search.db");
    content_summary.search_db_exists = search_db.exists();

    // Calculate total size and find other files
    content_summary.total_size_bytes = calculate_directory_size(path)?;
    content_summary.other_files = find_other_files(path)?;

    Ok(content_summary)
}

/// Count files with specific extensions in a directory
fn count_files_with_extensions(dir: &Path, extensions: &[&str]) -> Result<usize> {
    if !dir.exists() {
        return Ok(0);
    }

    let count = walk_files_with_extensions(dir, extensions).count();
    Ok(count)
}

/// Calculate total size of a directory in bytes
fn calculate_directory_size(path: &Path) -> Result<u64> {
    let mut total_size = 0;
    
    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Ok(metadata) = entry.metadata() {
                total_size += metadata.len();
            }
        }
    }
    
    Ok(total_size)
}

/// Find files that don't fit standard SwissArmyHammer categories
fn find_other_files(path: &Path) -> Result<Vec<PathBuf>> {
    let mut other_files = Vec::new();
    let standard_dirs = ["memos", "todo", "workflows"];
    let standard_files = ["search.db", "mcp.log"];

    for entry in fs::read_dir(path).map_err(SwissArmyHammerError::Io)? {
        let entry = entry.map_err(SwissArmyHammerError::Io)?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        
        if entry.file_type().map_err(SwissArmyHammerError::Io)?.is_file() {
            if !standard_files.iter().any(|&f| name_str == f) {
                other_files.push(entry.path());
            }
        } else if entry.file_type().map_err(SwissArmyHammerError::Io)?.is_dir()
            && !standard_dirs.iter().any(|&d| name_str == d)
        {
            other_files.push(entry.path());
        }
    }

    Ok(other_files)
}

/// Detect conflicts between different .swissarmyhammer directories
fn detect_content_conflicts(
    directories: &HashSet<PathBuf>,
    conflicts: &mut Vec<ConflictInfo>,
) -> Result<()> {
    // Build a map of content signatures to detect duplicates
    let mut content_map: HashMap<String, Vec<PathBuf>> = HashMap::new();

    for dir in directories {
        // Check for permission issues
        if fs::read_dir(dir).is_err() {
            conflicts.push(ConflictInfo {
                path: dir.clone(),
                conflict_type: ConflictType::PermissionIssue,
                description: format!(
                    "Cannot read directory {} - permission denied",
                    dir.display()
                ),
                severity: ConflictSeverity::High,
            });
            continue;
        }

        // Create a signature based on directory content
        let signature = create_content_signature(dir)?;
        content_map.entry(signature).or_default().push(dir.clone());
    }

    // Detect duplicate content signatures
    for (signature, dirs) in content_map {
        if dirs.len() > 1 && !signature.is_empty() {
            conflicts.push(ConflictInfo {
                path: dirs[0].clone(), // Use first directory as reference
                conflict_type: ConflictType::DuplicateData,
                description: format!(
                    "Similar content found in {} directories: {}",
                    dirs.len(),
                    dirs.iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                severity: ConflictSeverity::Medium,
            });
        }
    }

    Ok(())
}

/// Create a signature representing the content of a .swissarmyhammer directory
fn create_content_signature(dir: &Path) -> Result<String> {
    let content_summary = analyze_swissarmyhammer_content(dir)?;
    
    // Create a simple signature based on content counts and structure
    let signature = format!(
        "memos:{}_todos:{}_workflows:{}_search_db:{}_other:{}",
        content_summary.memos_count,
        content_summary.todos_count,
        content_summary.workflows_count,
        content_summary.search_db_exists,
        content_summary.other_files.len()
    );

    // Return empty signature if directory is essentially empty
    if content_summary.memos_count == 0
        && content_summary.todos_count == 0
        && content_summary.workflows_count == 0
        && !content_summary.search_db_exists
        && content_summary.other_files.is_empty()
    {
        Ok(String::new())
    } else {
        Ok(signature)
    }
}

/// Generate migration recommendations based on scan results
fn generate_migration_recommendations(
    git_repositories: &[GitRepositoryInfo],
    orphaned_directories: &[PathBuf],
    conflicts: &[ConflictInfo],
) -> Vec<String> {
    let mut recommendations = Vec::new();

    // Count repositories with and without .swissarmyhammer
    let repos_with_swissarmyhammer = git_repositories.iter().filter(|r| r.has_swissarmyhammer).count();
    let repos_without_swissarmyhammer = git_repositories.len() - repos_with_swissarmyhammer;

    // Basic status
    if git_repositories.is_empty() {
        recommendations.push("No Git repositories found. SwissArmyHammer requires Git repository context.".to_string());
    } else {
        recommendations.push(format!(
            "Found {} Git repositories: {} with .swissarmyhammer directories, {} without",
            git_repositories.len(),
            repos_with_swissarmyhammer,
            repos_without_swissarmyhammer
        ));
    }

    // Orphaned directories
    match orphaned_directories.len() {
        0 => {
            if !git_repositories.is_empty() {
                recommendations.push("✓ No orphaned .swissarmyhammer directories found".to_string());
            }
        }
        1 => recommendations.push(format!(
            "⚠ Found 1 orphaned .swissarmyhammer directory: {}",
            orphaned_directories[0].display()
        )),
        n => {
            recommendations.push(format!("⚠ Found {} orphaned .swissarmyhammer directories", n));
            for dir in orphaned_directories.iter().take(5) {
                recommendations.push(format!("  - {}", dir.display()));
            }
            if orphaned_directories.len() > 5 {
                recommendations.push(format!("  ... and {} more", orphaned_directories.len() - 5));
            }
        }
    }

    // Conflicts
    let critical_conflicts = conflicts.iter().filter(|c| c.severity == ConflictSeverity::Critical).count();
    let high_conflicts = conflicts.iter().filter(|c| c.severity == ConflictSeverity::High).count();
    let medium_conflicts = conflicts.iter().filter(|c| c.severity == ConflictSeverity::Medium).count();

    if conflicts.is_empty() {
        recommendations.push("✓ No migration conflicts detected".to_string());
    } else {
        recommendations.push(format!(
            "⚠ Detected {} conflicts: {} critical, {} high, {} medium severity",
            conflicts.len(),
            critical_conflicts,
            high_conflicts,
            medium_conflicts
        ));
    }

    // Migration readiness assessment
    if critical_conflicts > 0 {
        recommendations.push("🛑 Migration blocked due to critical conflicts - manual intervention required".to_string());
    } else if high_conflicts > 0 || orphaned_directories.len() > 3 {
        recommendations.push("⚠ Migration possible but requires careful planning and manual steps".to_string());
    } else if medium_conflicts > 0 || !orphaned_directories.is_empty() {
        recommendations.push("⚠ Migration recommended with backup and validation".to_string());
    } else if repos_with_swissarmyhammer == git_repositories.len() {
        recommendations.push("✓ All Git repositories already have .swissarmyhammer directories at correct locations".to_string());
    } else {
        recommendations.push("✓ Migration ready - can proceed with standard migration process".to_string());
    }

    // Specific actions
    if !orphaned_directories.is_empty() {
        recommendations.push("📋 Next steps for orphaned directories:".to_string());
        recommendations.push("  1. Review content in each orphaned directory".to_string());
        recommendations.push("  2. Identify which Git repository should contain the data".to_string());
        recommendations.push("  3. Move or merge data into the appropriate Git repository".to_string());
        recommendations.push("  4. Remove empty orphaned directories".to_string());
    }

    if critical_conflicts > 0 || high_conflicts > 0 {
        recommendations.push("📋 Next steps for conflicts:".to_string());
        recommendations.push("  1. Review each conflict listed above".to_string());
        recommendations.push("  2. Resolve permission issues".to_string());
        recommendations.push("  3. Back up any important data".to_string());
        recommendations.push("  4. Manually resolve data conflicts".to_string());
    }

    recommendations
}

/// Validate that migration can proceed safely for a specific Git repository
///
/// # Arguments
///
/// * `git_root` - Path to the Git repository root
///
/// # Returns
///
/// Returns a `MigrationPlan` with specific actions needed for migration
///
/// # Errors
///
/// Returns an error if:
/// - The path is not a Git repository
/// - Permission issues prevent analysis  
/// - Critical conflicts prevent migration
pub fn validate_migration_safety(git_root: &Path) -> Result<MigrationPlan> {
    // Verify this is actually a Git repository
    if !git_root.join(".git").exists() {
        return Err(SwissArmyHammerError::git_repository_not_found(
            &git_root.display().to_string(),
        ));
    }

    let target_directory = git_root.join(".swissarmyhammer");
    let mut source_directories = Vec::new();
    let mut actions = Vec::new();
    
    // Check if target directory already exists
    let target_exists = target_directory.exists() && target_directory.is_dir();
    
    // Find any .swissarmyhammer directories within this Git repository
    for entry in WalkDir::new(git_root)
        .max_depth(MAX_DIRECTORY_DEPTH)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        
        if path.file_name() == Some(std::ffi::OsStr::new(".swissarmyhammer"))
            && path.is_dir()
            && path != target_directory
        {
            source_directories.push(path.to_path_buf());
        }
    }

    // Determine if backup is needed
    let backup_needed = target_exists || !source_directories.is_empty();

    // Create backup action if needed
    if backup_needed {
        actions.push(MigrationAction::CreateBackup {
            path: git_root.to_path_buf(),
        });
    }

    // Create target directory if it doesn't exist
    if !target_exists {
        // This will be handled by get_or_create_swissarmyhammer_directory
    }

    // Plan to move content from source directories
    for source_dir in &source_directories {
        actions.push(MigrationAction::MoveFiles {
            from: source_dir.clone(),
            to: target_directory.clone(),
        });
        
        // Plan to remove empty source directory
        actions.push(MigrationAction::RemoveEmpty {
            path: source_dir.clone(),
        });
    }

    // Estimate duration
    let estimated_duration = if source_directories.is_empty() {
        "Immediate (no migration needed)".to_string()
    } else if source_directories.len() == 1 {
        "1-2 minutes".to_string()
    } else {
        format!("{}-{} minutes", source_directories.len(), source_directories.len() * 2)
    };

    Ok(MigrationPlan {
        source_directories,
        target_directory,
        actions,
        backup_needed,
        estimated_duration,
    })
}

impl ContentSummary {
    /// Create an empty content summary
    pub fn empty() -> Self {
        Self {
            memos_count: 0,
            todos_count: 0,
            workflows_count: 0,
            search_db_exists: false,
            other_files: Vec::new(),
            total_size_bytes: 0,
        }
    }

    /// Check if the content summary represents an empty directory
    pub fn is_empty(&self) -> bool {
        self.memos_count == 0
            && self.todos_count == 0
            && self.workflows_count == 0
            && !self.search_db_exists
            && self.other_files.is_empty()
    }

    /// Get a human-readable size string
    pub fn human_readable_size(&self) -> String {
        let size = self.total_size_bytes;
        if size < 1024 {
            format!("{} B", size)
        } else if size < 1024 * 1024 {
            format!("{:.1} KB", size as f64 / 1024.0)
        } else if size < 1024 * 1024 * 1024 {
            format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", size as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }
}

impl ConflictSeverity {
    /// Get a human-readable description of the severity
    pub fn description(&self) -> &'static str {
        match self {
            ConflictSeverity::Low => "Minor issue - migration can proceed with warnings",
            ConflictSeverity::Medium => "Moderate issue - migration should be done carefully",
            ConflictSeverity::High => "Serious issue - manual intervention required",
            ConflictSeverity::Critical => "Critical issue - migration blocked, data loss risk",
        }
    }

    /// Get an emoji symbol for the severity
    pub fn symbol(&self) -> &'static str {
        match self {
            ConflictSeverity::Low => "⚠️",
            ConflictSeverity::Medium => "⚠️",
            ConflictSeverity::High => "❌",
            ConflictSeverity::Critical => "🛑",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_content_summary_empty() {
        let summary = ContentSummary::empty();
        assert!(summary.is_empty());
        assert_eq!(summary.memos_count, 0);
        assert_eq!(summary.human_readable_size(), "0 B");
    }

    #[test]
    fn test_content_summary_human_readable_size() {
        let mut summary = ContentSummary::empty();
        
        summary.total_size_bytes = 500;
        assert_eq!(summary.human_readable_size(), "500 B");
        
        summary.total_size_bytes = 1536; // 1.5 KB
        assert_eq!(summary.human_readable_size(), "1.5 KB");
        
        summary.total_size_bytes = 2_097_152; // 2 MB
        assert_eq!(summary.human_readable_size(), "2.0 MB");
        
        summary.total_size_bytes = 3_221_225_472; // ~3 GB
        assert_eq!(summary.human_readable_size(), "3.0 GB");
    }

    #[test]
    fn test_conflict_severity_properties() {
        assert_eq!(ConflictSeverity::Low.symbol(), "⚠️");
        assert_eq!(ConflictSeverity::Medium.symbol(), "⚠️");
        assert_eq!(ConflictSeverity::High.symbol(), "❌");
        assert_eq!(ConflictSeverity::Critical.symbol(), "🛑");
        
        assert!(ConflictSeverity::Critical.description().contains("Critical"));
        assert!(ConflictSeverity::Low.description().contains("Minor"));
    }

    #[test]
    fn test_create_content_signature_empty() {
        let temp_dir = TempDir::new().unwrap();
        let swissarmyhammer_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir(&swissarmyhammer_dir).unwrap();
        
        let signature = create_content_signature(&swissarmyhammer_dir).unwrap();
        assert_eq!(signature, "");
    }

    #[test]
    fn test_analyze_swissarmyhammer_content_empty() {
        let temp_dir = TempDir::new().unwrap();
        let swissarmyhammer_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir(&swissarmyhammer_dir).unwrap();
        
        let summary = analyze_swissarmyhammer_content(&swissarmyhammer_dir).unwrap();
        assert!(summary.is_empty());
        assert_eq!(summary.memos_count, 0);
        assert_eq!(summary.todos_count, 0);
        assert_eq!(summary.workflows_count, 0);
        assert!(!summary.search_db_exists);
        assert!(summary.other_files.is_empty());
    }

    #[test]
    fn test_analyze_swissarmyhammer_content_with_files() {
        let temp_dir = TempDir::new().unwrap();
        let swissarmyhammer_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir(&swissarmyhammer_dir).unwrap();
        
        // Create memos directory with test files
        let memos_dir = swissarmyhammer_dir.join("memos");
        fs::create_dir(&memos_dir).unwrap();
        fs::write(memos_dir.join("test1.md"), "memo content").unwrap();
        fs::write(memos_dir.join("test2.md"), "another memo").unwrap();
        
        // Create todo directory with test files
        let todo_dir = swissarmyhammer_dir.join("todo");
        fs::create_dir(&todo_dir).unwrap();
        fs::write(todo_dir.join("tasks.yaml"), "task list").unwrap();
        
        // Create search database
        fs::write(swissarmyhammer_dir.join("search.db"), "database").unwrap();
        
        let summary = analyze_swissarmyhammer_content(&swissarmyhammer_dir).unwrap();
        assert!(!summary.is_empty());
        assert_eq!(summary.memos_count, 2);
        assert_eq!(summary.todos_count, 1);
        assert_eq!(summary.workflows_count, 0);
        assert!(summary.search_db_exists);
        assert!(summary.total_size_bytes > 0);
    }

    #[test]
    fn test_scan_existing_directories_no_git_repos() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create orphaned .swissarmyhammer directory
        let orphaned_dir = temp_dir.path().join("project").join(".swissarmyhammer");
        fs::create_dir_all(&orphaned_dir).unwrap();
        
        let scan_result = scan_existing_directories_from(temp_dir.path()).unwrap();
        
        assert!(scan_result.git_repositories.is_empty());
        assert_eq!(scan_result.orphaned_directories.len(), 1);
        assert_eq!(scan_result.orphaned_directories[0], orphaned_dir);
        assert!(scan_result.recommendations.iter().any(|r| r.contains("No Git repositories found")));
    }

    #[test] 
    fn test_scan_existing_directories_with_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo");
        fs::create_dir(&repo_path).unwrap();
        
        // Create Git repository
        fs::create_dir(repo_path.join(".git")).unwrap();
        
        // Create .swissarmyhammer directory at repo root
        let swissarmyhammer_dir = repo_path.join(".swissarmyhammer");
        fs::create_dir(&swissarmyhammer_dir).unwrap();
        
        let scan_result = scan_existing_directories_from(temp_dir.path()).unwrap();
        
        assert_eq!(scan_result.git_repositories.len(), 1);
        assert!(scan_result.git_repositories[0].has_swissarmyhammer);
        assert_eq!(scan_result.git_repositories[0].path, repo_path);
        assert_eq!(scan_result.git_repositories[0].swissarmyhammer_path, Some(swissarmyhammer_dir));
        assert!(scan_result.orphaned_directories.is_empty());
    }

    #[test]
    fn test_scan_detects_nested_swissarmyhammer_conflict() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo");
        fs::create_dir(&repo_path).unwrap();
        
        // Create Git repository
        fs::create_dir(repo_path.join(".git")).unwrap();
        
        // Create nested .swissarmyhammer directory (not at root)
        let nested_dir = repo_path.join("src").join(".swissarmyhammer");
        fs::create_dir_all(&nested_dir).unwrap();
        
        let scan_result = scan_existing_directories_from(temp_dir.path()).unwrap();
        
        assert_eq!(scan_result.git_repositories.len(), 1);
        assert!(!scan_result.git_repositories[0].has_swissarmyhammer);
        assert!(scan_result.orphaned_directories.is_empty());
        assert!(!scan_result.conflicts.is_empty());
        
        let conflict = &scan_result.conflicts[0];
        assert_eq!(conflict.conflict_type, ConflictType::PathConflict);
        assert_eq!(conflict.severity, ConflictSeverity::Medium);
        assert!(conflict.description.contains("but Git repository root is at"));
    }

    #[test]
    fn test_validate_migration_safety_no_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        let non_repo_path = temp_dir.path().join("not-a-repo");
        fs::create_dir(&non_repo_path).unwrap();
        
        let result = validate_migration_safety(&non_repo_path);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SwissArmyHammerError::GitRepositoryNotFound { .. }));
    }

    #[test]
    fn test_validate_migration_safety_clean_repo() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo");
        fs::create_dir(&repo_path).unwrap();
        
        // Create Git repository
        fs::create_dir(repo_path.join(".git")).unwrap();
        
        let migration_plan = validate_migration_safety(&repo_path).unwrap();
        
        assert!(migration_plan.source_directories.is_empty());
        assert_eq!(migration_plan.target_directory, repo_path.join(".swissarmyhammer"));
        assert!(!migration_plan.backup_needed);
        assert_eq!(migration_plan.estimated_duration, "Immediate (no migration needed)");
    }

    #[test]
    fn test_validate_migration_safety_with_existing_target() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo");
        fs::create_dir(&repo_path).unwrap();
        
        // Create Git repository
        fs::create_dir(repo_path.join(".git")).unwrap();
        
        // Create existing .swissarmyhammer directory
        fs::create_dir(repo_path.join(".swissarmyhammer")).unwrap();
        
        let migration_plan = validate_migration_safety(&repo_path).unwrap();
        
        assert!(migration_plan.source_directories.is_empty());
        assert!(migration_plan.backup_needed);
        assert!(migration_plan.actions.iter().any(|a| matches!(a, MigrationAction::CreateBackup { .. })));
    }

    #[test]
    fn test_validate_migration_safety_with_sources() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo");
        fs::create_dir(&repo_path).unwrap();
        
        // Create Git repository
        fs::create_dir(repo_path.join(".git")).unwrap();
        
        // Create source .swissarmyhammer directory in subdirectory
        let source_dir = repo_path.join("src").join(".swissarmyhammer");
        fs::create_dir_all(&source_dir).unwrap();
        
        let migration_plan = validate_migration_safety(&repo_path).unwrap();
        
        assert_eq!(migration_plan.source_directories.len(), 1);
        assert_eq!(migration_plan.source_directories[0], source_dir);
        assert!(migration_plan.backup_needed);
        assert_eq!(migration_plan.estimated_duration, "1-2 minutes");
        
        // Check for move and remove actions
        let has_move_action = migration_plan.actions.iter().any(|a| {
            matches!(a, MigrationAction::MoveFiles { from, to } 
                if from == &source_dir && to == &repo_path.join(".swissarmyhammer"))
        });
        assert!(has_move_action);
        
        let has_remove_action = migration_plan.actions.iter().any(|a| {
            matches!(a, MigrationAction::RemoveEmpty { path } 
                if path == &source_dir)
        });
        assert!(has_remove_action);
    }

    #[test]
    fn test_migration_plan_duration_estimation() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo");
        fs::create_dir(&repo_path).unwrap();
        
        // Create Git repository
        fs::create_dir(repo_path.join(".git")).unwrap();
        
        // Create multiple source directories
        for i in 1..=3 {
            let source_dir = repo_path.join(format!("dir{}", i)).join(".swissarmyhammer");
            fs::create_dir_all(&source_dir).unwrap();
        }
        
        let migration_plan = validate_migration_safety(&repo_path).unwrap();
        
        assert_eq!(migration_plan.source_directories.len(), 3);
        assert_eq!(migration_plan.estimated_duration, "3-6 minutes");
    }
}