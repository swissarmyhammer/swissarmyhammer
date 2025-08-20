# Migration Validation Tools Implementation  

Refer to /Users/wballard/github/sah-directory/ideas/directory.md

## Overview
Create validation and migration tools to help users transition from the old multiple-directory system to the new Git repository-centric approach safely.

## Technical Approach

Add migration utilities to help users consolidate multiple `.swissarmyhammer` directories:

```rust
/// Scan for existing .swissarmyhammer directories and validate migration readiness
pub fn scan_existing_directories() -> MigrationScanResult {
    // Find all existing .swissarmyhammer directories
    // Identify conflicts and data that would be lost
    // Provide recommendations
}

/// Validate that migration can proceed safely
pub fn validate_migration_safety(git_root: &Path) -> Result<MigrationPlan, SwissArmyHammerError> {
    // Check for existing data
    // Identify merge conflicts
    // Create consolidation plan
}

#[derive(Debug)]
pub struct MigrationScanResult {
    pub git_repositories: Vec<GitRepositoryInfo>,
    pub orphaned_directories: Vec<PathBuf>,
    pub conflicts: Vec<ConflictInfo>,
    pub recommendations: Vec<String>,
}

#[derive(Debug)]  
pub struct ConflictInfo {
    pub path: PathBuf,
    pub conflict_type: ConflictType,
    pub description: String,
}
```

## CLI Integration
Add `sah doctor --migration` command that:
1. Scans current directory structure
2. Reports existing `.swissarmyhammer` directories  
3. Identifies potential migration issues
4. Provides clear guidance for consolidation

## Migration Safety Features
- Dry-run mode for validation
- Backup creation before any moves
- Conflict detection and resolution strategies
- Roll-back capability if issues occur

## Tasks
1. Implement directory scanning utilities
2. Add migration validation logic
3. Create conflict detection system
4. Add CLI command integration (`sah doctor --migration`)
5. Comprehensive testing covering:
   - Multiple nested `.swissarmyhammer` directories
   - Conflicting file scenarios
   - Permission issues
   - Backup and restore functionality
6. Documentation for migration process

## Dependencies  
- Depends on: directory_000002_swissarmyhammer-directory-resolution

## Success Criteria
- Tools accurately identify all existing `.swissarmyhammer` directories
- Clear migration recommendations with step-by-step instructions
- Safe validation prevents data loss scenarios  
- Comprehensive conflict detection and resolution
- Users can confidently migrate without losing data
- All tests pass including complex directory hierarchies
## Proposed Solution

Based on the directory utilities analysis and new Git-centric approach, I will implement migration validation tools as follows:

### Data Structures

```rust
#[derive(Debug, Clone)]
pub struct MigrationScanResult {
    pub git_repositories: Vec<GitRepositoryInfo>,
    pub orphaned_directories: Vec<PathBuf>,
    pub conflicts: Vec<ConflictInfo>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GitRepositoryInfo {
    pub path: PathBuf,
    pub has_swissarmyhammer: bool,
    pub swissarmyhammer_path: Option<PathBuf>,
    pub content_summary: ContentSummary,
}

#[derive(Debug, Clone)]
pub struct ContentSummary {
    pub memos_count: usize,
    pub todos_count: usize,
    pub workflows_count: usize,
    pub search_db_exists: bool,
    pub other_files: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ConflictInfo {
    pub path: PathBuf,
    pub conflict_type: ConflictType,
    pub description: String,
    pub severity: ConflictSeverity,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConflictType {
    DuplicateData,         // Same file exists in multiple locations
    PermissionIssue,       // Can't read/write to location
    PathConflict,          // Path issues preventing migration
    DataLoss,             // Migration would lose data
    VersionMismatch,      // Different versions of same data
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConflictSeverity {
    Low,     // Migration can proceed with warnings
    Medium,  // Migration should be done carefully
    High,    // Migration requires manual intervention
    Critical, // Migration would cause data loss
}
```

### Core Functions

```rust
/// Scan for existing .swissarmyhammer directories and validate migration readiness
pub fn scan_existing_directories() -> Result<MigrationScanResult> {
    // 1. Find all Git repositories in current directory and subdirectories
    // 2. For each Git repo, check for .swissarmyhammer directory
    // 3. Find orphaned .swissarmyhammer directories (not in Git repos)
    // 4. Analyze content and detect conflicts
    // 5. Generate recommendations
}

/// Validate that migration can proceed safely for a specific Git repository
pub fn validate_migration_safety(git_root: &Path) -> Result<MigrationPlan, SwissArmyHammerError> {
    // 1. Check current .swissarmyhammer directory at git root
    // 2. Find any other .swissarmyhammer directories that would be consolidated
    // 3. Detect file conflicts and data overlap
    // 4. Create step-by-step migration plan
}

#[derive(Debug)]
pub struct MigrationPlan {
    pub source_directories: Vec<PathBuf>,
    pub target_directory: PathBuf,
    pub actions: Vec<MigrationAction>,
    pub backup_needed: bool,
    pub estimated_duration: String,
}

#[derive(Debug)]
pub enum MigrationAction {
    CreateBackup { path: PathBuf },
    MoveFiles { from: PathBuf, to: PathBuf },
    MergeFiles { sources: Vec<PathBuf>, target: PathBuf },
    RemoveEmpty { path: PathBuf },
    UpdateReferences { file: PathBuf, changes: Vec<String> },
}
```

### CLI Integration

Will add `--migration` flag to the existing doctor command:
- `sah doctor --migration`: Scan and report migration status
- Integrate with existing doctor check system
- Use consistent colored output and check format
- Return appropriate exit codes (0=ready, 1=warnings, 2=conflicts)

### Implementation Plan

1. Create migration validation module in `swissarmyhammer/src/migration/`
2. Add migration checks to doctor command in `swissarmyhammer-cli/src/doctor/checks.rs`
3. Implement comprehensive test suite with various directory scenarios
4. Add CLI flag parsing and integration

### Safety Features

- Dry-run mode for all validation (no actual changes)
- Detailed conflict reporting with severity levels
- Backup recommendation before any actions
- Clear rollback instructions
- Comprehensive logging of all decisions

This approach builds on the existing directory utilities and doctor command patterns while providing the migration validation tools needed for the new Git-centric directory approach.