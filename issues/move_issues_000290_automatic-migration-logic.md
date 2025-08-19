# Implement Automatic Migration Logic

## Overview
Implement the core automatic migration functionality to safely move issues from `./issues` to `.swissarmyhammer/issues` with validation, backup, and rollback capabilities.

Refer to /Users/wballard/github/sah-issues/ideas/move_issues.md

## Current State
After completing infrastructure steps 000284-000289, we have detection logic and need the actual migration implementation.

## Target Implementation

### Core Migration Function
```rust
impl FileSystemIssueStorage {
    /// Perform automatic migration with full safety checks
    pub fn perform_migration() -> Result<MigrationResult> {
        let info = Self::migration_info()?;
        
        if !info.should_migrate {
            return Ok(MigrationResult::NotNeeded(info));
        }
        
        tracing::info!(
            "Starting issues directory migration: {} files ({} bytes)",
            info.file_count,
            info.total_size
        );
        
        let paths = Self::migration_paths()?;
        
        // Create backup before migration
        let backup_path = Self::create_backup(&paths.source)?;
        
        match Self::execute_migration(&paths) {
            Ok(result) => {
                tracing::info!("Migration completed successfully");
                Ok(MigrationResult::Success(result))
            }
            Err(e) => {
                tracing::error!("Migration failed, attempting rollback: {}", e);
                Self::rollback_migration(&paths, &backup_path)?;
                Err(e)
            }
        }
    }
    
    /// Execute the actual file migration
    fn execute_migration(paths: &MigrationPaths) -> Result<MigrationStats> {
        let start_time = std::time::Instant::now();
        
        // Ensure destination parent directory exists
        if let Some(parent) = paths.destination.parent() {
            std::fs::create_dir_all(parent).map_err(SwissArmyHammerError::Io)?;
        }
        
        // Perform atomic move operation
        std::fs::rename(&paths.source, &paths.destination)
            .map_err(SwissArmyHammerError::Io)?;
        
        let duration = start_time.elapsed();
        let info = Self::migration_info()?; // Get final info
        
        Ok(MigrationStats {
            files_moved: info.file_count,
            bytes_moved: info.total_size,
            duration,
        })
    }
}
```

### Migration Result Types
```rust
#[derive(Debug)]
pub enum MigrationResult {
    Success(MigrationStats),
    NotNeeded(MigrationInfo),
    Failed(MigrationError),
}

#[derive(Debug)]
pub struct MigrationStats {
    pub files_moved: usize,
    pub bytes_moved: u64,
    pub duration: std::time::Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("Backup creation failed: {0}")]
    BackupFailed(#[source] SwissArmyHammerError),
    #[error("Migration execution failed: {0}")]
    ExecutionFailed(#[source] SwissArmyHammerError),
    #[error("Rollback failed: {0}")]
    RollbackFailed(#[source] SwissArmyHammerError),
    #[error("Validation failed: {reason}")]
    ValidationFailed { reason: String },
}
```

### Backup and Rollback Functions
```rust
impl FileSystemIssueStorage {
    /// Create backup of source directory before migration
    fn create_backup(source: &Path) -> Result<PathBuf> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_name = format!("issues_backup_{}", timestamp);
        let backup_path = source.parent()
            .ok_or_else(|| SwissArmyHammerError::invalid_path("Source has no parent"))?
            .join(&backup_name);
        
        Self::copy_directory_recursive(source, &backup_path)?;
        
        tracing::debug!("Created backup at: {}", backup_path.display());
        Ok(backup_path)
    }
    
    /// Rollback migration by restoring from backup
    fn rollback_migration(paths: &MigrationPaths, backup_path: &Path) -> Result<()> {
        tracing::warn!("Rolling back migration");
        
        // Remove partial destination if it exists
        if paths.destination.exists() {
            std::fs::remove_dir_all(&paths.destination)
                .map_err(SwissArmyHammerError::Io)?;
        }
        
        // Restore from backup
        std::fs::rename(backup_path, &paths.source)
            .map_err(SwissArmyHammerError::Io)?;
        
        tracing::info!("Migration rollback completed");
        Ok(())
    }
    
    /// Copy directory recursively for backup
    fn copy_directory_recursive(source: &Path, destination: &Path) -> Result<()> {
        std::fs::create_dir_all(destination).map_err(SwissArmyHammerError::Io)?;
        
        for entry in std::fs::read_dir(source).map_err(SwissArmyHammerError::Io)? {
            let entry = entry.map_err(SwissArmyHammerError::Io)?;
            let path = entry.path();
            let dest_path = destination.join(entry.file_name());
            
            if path.is_dir() {
                Self::copy_directory_recursive(&path, &dest_path)?;
            } else {
                std::fs::copy(&path, &dest_path).map_err(SwissArmyHammerError::Io)?;
            }
        }
        
        Ok(())
    }
}
```

### Migration Validation
```rust
impl FileSystemIssueStorage {
    /// Validate migration completed successfully
    fn validate_migration(paths: &MigrationPaths, expected_stats: &MigrationStats) -> Result<()> {
        // Check destination exists
        if !paths.destination.exists() {
            return Err(SwissArmyHammerError::validation_failed(
                "Destination directory does not exist after migration"
            ));
        }
        
        // Check source no longer exists
        if paths.source.exists() {
            return Err(SwissArmyHammerError::validation_failed(
                "Source directory still exists after migration"
            ));
        }
        
        // Validate file count and size
        let (actual_files, actual_size) = Self::count_directory_contents(&paths.destination)?;
        if actual_files != expected_stats.files_moved {
            return Err(SwissArmyHammerError::validation_failed(&format!(
                "File count mismatch: expected {}, got {}",
                expected_stats.files_moved, actual_files
            )));
        }
        
        if actual_size != expected_stats.bytes_moved {
            return Err(SwissArmyHammerError::validation_failed(&format!(
                "Size mismatch: expected {} bytes, got {} bytes",
                expected_stats.bytes_moved, actual_size
            )));
        }
        
        tracing::debug!("Migration validation passed");
        Ok(())
    }
}
```

### Integration with Storage Creation
```rust
impl FileSystemIssueStorage {
    /// Enhanced new_default with optional automatic migration
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
}
```

## Implementation Details

### Safety Features
- **Atomic Operations**: Use `fs::rename` for atomic directory moves
- **Backup Creation**: Always create backup before migration
- **Validation**: Verify migration completed correctly
- **Rollback**: Automatic rollback on failure
- **Logging**: Comprehensive logging of all operations

### Error Handling
- Handle filesystem errors gracefully
- Provide detailed error context
- Distinguish between recoverable and non-recoverable errors
- Log errors for debugging

### Performance Considerations
- Use atomic `fs::rename` when possible (same filesystem)
- Fall back to copy + delete for cross-filesystem moves
- Minimize filesystem operations during validation
- Provide progress feedback for large migrations

### Thread Safety
- Migration operations are atomic at filesystem level
- Use appropriate locking if needed for concurrent access
- Ensure backup creation is thread-safe

## Testing Requirements

### Unit Tests
```rust
#[cfg(test)]
mod migration_tests {
    #[test]
    fn test_migration_with_empty_directory() {
        // Test migration of empty directory
    }
    
    #[test]
    fn test_migration_with_nested_structure() {
        // Test migration preserves nested directory structure
    }
    
    #[test]
    fn test_migration_backup_creation() {
        // Test backup is created properly
    }
    
    #[test]
    fn test_migration_rollback() {
        // Test rollback works correctly
    }
    
    #[test]
    fn test_migration_validation() {
        // Test validation catches issues
    }
}
```

### Integration Tests
- Test complete migration workflows
- Test migration with large numbers of files
- Test migration failure scenarios
- Test concurrent access during migration

### Error Scenario Tests
- Test rollback when migration fails
- Test handling of filesystem permission errors
- Test behavior when destination already exists
- Test cross-filesystem migration scenarios

## Files to Modify
- `swissarmyhammer/src/issues/filesystem.rs`
- Add migration-specific error types
- Comprehensive test suite
- Update documentation

## Acceptance Criteria
- [ ] Migration safely moves all files and directories
- [ ] Backup is created before migration starts
- [ ] Migration is atomic (all-or-nothing)
- [ ] Validation ensures migration completed correctly
- [ ] Rollback restores original state on failure
- [ ] Comprehensive logging of migration process
- [ ] Thread-safe migration operations
- [ ] Performance acceptable for typical issue directories
- [ ] All edge cases handled gracefully

## Dependencies
- Depends on steps 000284-000289 (infrastructure)
- Required before automatic migration integration (step 000291)

## Estimated Effort
~500-600 lines including migration logic, safety features, and comprehensive tests.

## Notes
- Focus on safety and data integrity over performance
- Consider cross-platform filesystem behavior differences
- Test thoroughly with various directory sizes and structures
- Provide detailed logging for troubleshooting migration issues