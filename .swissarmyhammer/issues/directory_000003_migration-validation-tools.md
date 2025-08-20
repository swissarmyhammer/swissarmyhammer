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