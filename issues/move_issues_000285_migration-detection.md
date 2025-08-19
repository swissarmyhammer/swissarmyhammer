# Migration Detection and Helper Functions

## Overview
Add migration detection and helper functions to identify when automatic migration should occur and facilitate the migration process.

Refer to /Users/wballard/github/sah-issues/ideas/move_issues.md

## Current State
After step 000284, we have updated core storage logic but need migration detection capabilities.

## Target Implementation

### Migration Detection Function
```rust
impl FileSystemIssueStorage {
    /// Check if automatic migration should be performed
    pub fn should_migrate() -> Result<bool> {
        let current_dir = std::env::current_dir().map_err(SwissArmyHammerError::Io)?;
        let old_issues = current_dir.join("issues");
        let new_issues_parent = current_dir.join(".swissarmyhammer");
        let new_issues = new_issues_parent.join("issues");
        
        // Migrate if old exists and new location doesn't exist
        Ok(old_issues.exists() && 
           old_issues.is_dir() && 
           !new_issues.exists())
    }
    
    /// Get migration paths for validation
    pub fn migration_paths() -> Result<MigrationPaths> {
        let current_dir = std::env::current_dir().map_err(SwissArmyHammerError::Io)?;
        Ok(MigrationPaths {
            source: current_dir.join("issues"),
            destination: current_dir.join(".swissarmyhammer").join("issues"),
            backup: current_dir.join(".swissarmyhammer").join("issues_backup"),
        })
    }
}
```

### Supporting Types
```rust
#[derive(Debug, Clone)]
pub struct MigrationPaths {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub backup: PathBuf,
}

#[derive(Debug)]
pub struct MigrationInfo {
    pub should_migrate: bool,
    pub source_exists: bool,
    pub destination_exists: bool,
    pub file_count: usize,
    pub total_size: u64,
}
```

### Migration Information Function
```rust
impl FileSystemIssueStorage {
    /// Gather information about potential migration
    pub fn migration_info() -> Result<MigrationInfo> {
        let paths = Self::migration_paths()?;
        let should_migrate = Self::should_migrate()?;
        
        let source_exists = paths.source.exists();
        let destination_exists = paths.destination.exists();
        
        let (file_count, total_size) = if source_exists {
            Self::count_directory_contents(&paths.source)?
        } else {
            (0, 0)
        };
        
        Ok(MigrationInfo {
            should_migrate,
            source_exists,
            destination_exists,
            file_count,
            total_size,
        })
    }
    
    /// Count files and total size in a directory recursively
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
        Ok((file_count, total_size))
    }
}
```

## Implementation Details

### Migration Detection Logic
1. Check if `./issues/` directory exists and is a directory
2. Check if `.swissarmyhammer/issues/` does NOT exist
3. Return true only if migration should occur (source exists, destination doesn't)

### Information Gathering
- Count files recursively in source directory
- Calculate total disk usage
- Validate migration preconditions
- Provide detailed information for user confirmation

### Error Handling
- Handle I/O errors gracefully
- Provide meaningful error messages
- Include context for debugging

### Logging Integration
- Log migration detection activities with tracing
- Include file counts and sizes in debug logs
- Log decision making process

## Testing Requirements

### Unit Tests
- Test migration detection with various directory states:
  - Source exists, destination doesn't exist (should migrate)
  - Source doesn't exist (should not migrate)
  - Both exist (should not migrate)
  - Neither exist (should not migrate)
- Test migration info gathering accuracy
- Test file counting with nested directories
- Test error handling for permission issues

### Integration Tests
- Test with real directory structures
- Test with large numbers of files
- Test with various file sizes
- Test permission handling

## Files to Modify
- `swissarmyhammer/src/issues/filesystem.rs`
- Add new types to module
- Comprehensive unit tests
- Update module documentation

## Acceptance Criteria
- [ ] Migration detection correctly identifies when to migrate
- [ ] Migration info provides accurate file counts and sizes
- [ ] Helper functions support both manual and automatic migration
- [ ] Comprehensive error handling for all edge cases
- [ ] Full unit test coverage for all scenarios
- [ ] Integration with logging system
- [ ] No false positives in migration detection

## Dependencies
- Depends on step 000284 (core storage update)

## Estimated Effort
~300-400 lines of code changes including tests and supporting functions.