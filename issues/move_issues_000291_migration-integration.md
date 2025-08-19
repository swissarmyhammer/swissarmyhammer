# Integration of Automatic Migration with Storage Creation

## Overview
Integrate automatic migration logic with storage creation throughout the codebase, ensuring migration happens automatically when needed across CLI, MCP, and library usage.

Refer to /Users/wballard/github/sah-issues/ideas/move_issues.md

## Current State
After step 000290, we have working migration logic but need to integrate it with all storage creation points in the codebase.

## Target Implementation

### Update Core Storage Creation
```rust
impl FileSystemIssueStorage {
    /// Enhanced new_default that performs migration if needed
    pub fn new_default() -> Result<Self> {
        // Check if migration should occur and perform it
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
                    return Err(e.into());
                }
            }
        }
        
        // Proceed with normal storage creation
        let current_dir = std::env::current_dir().map_err(SwissArmyHammerError::Io)?;
        let issues_dir = Self::default_directory()?;
        Self::new(issues_dir)
    }
    
    /// Alternative constructor that reports migration results
    pub fn new_default_with_migration_info() -> Result<(Self, Option<MigrationResult>)> {
        let migration_result = if Self::should_migrate()? {
            Some(Self::perform_migration()?)
        } else {
            None
        };
        
        let storage = {
            let current_dir = std::env::current_dir().map_err(SwissArmyHammerError::Io)?;
            let issues_dir = Self::default_directory()?;
            Self::new(issues_dir)?
        };
        
        Ok((storage, migration_result))
    }
}
```

### CLI Integration Enhancement
```rust
// In swissarmyhammer-cli/src/mcp_integration.rs
fn create_issue_storage(
    current_dir: &std::path::Path,
) -> Result<IssueStorageArc, Box<dyn std::error::Error>> {
    // Set working directory for migration context
    let original_dir = std::env::current_dir()?;
    if current_dir != original_dir {
        std::env::set_current_dir(current_dir)?;
    }
    
    // Create storage with automatic migration
    let (storage, migration_result) = swissarmyhammer::issues::FileSystemIssueStorage::new_default_with_migration_info()?;
    
    // Log migration results for CLI users
    if let Some(result) = migration_result {
        match result {
            MigrationResult::Success(stats) => {
                eprintln!("✅ Migrated {} issues to .swissarmyhammer/issues", stats.files_moved);
            }
            MigrationResult::Failed(e) => {
                eprintln!("❌ Issue migration failed: {}", e);
                return Err(e.into());
            }
            MigrationResult::NotNeeded(_) => {
                // Silent for CLI - no need to inform about no migration
            }
        }
    }
    
    // Restore original directory
    if current_dir != original_dir {
        std::env::set_current_dir(original_dir)?;
    }
    
    Ok(Arc::new(RwLock::new(Box::new(storage))))
}
```

### MCP Server Integration Enhancement
```rust
// In swissarmyhammer-tools/src/mcp/server.rs
async fn initialize_issue_storage(work_dir: &Path) -> Result<Arc<RwLock<Box<dyn IssueStorage>>>> {
    // Set working directory context for migration
    let original_dir = std::env::current_dir()?;
    if work_dir != original_dir {
        std::env::set_current_dir(work_dir)?;
    }
    
    // Create storage with automatic migration
    let (storage, migration_result) = swissarmyhammer::issues::FileSystemIssueStorage::new_default_with_migration_info()?;
    
    // Log migration results
    if let Some(result) = migration_result {
        match result {
            MigrationResult::Success(stats) => {
                tracing::info!(
                    "MCP server performed automatic migration: {} files moved to .swissarmyhammer/issues",
                    stats.files_moved
                );
            }
            MigrationResult::Failed(e) => {
                tracing::error!("MCP server migration failed: {}", e);
                std::env::set_current_dir(original_dir)?;
                return Err(e.into());
            }
            MigrationResult::NotNeeded(_) => {
                tracing::debug!("No migration needed for MCP server");
            }
        }
    }
    
    // Restore original directory
    if work_dir != original_dir {
        std::env::set_current_dir(original_dir)?;
    }
    
    Ok(Arc::new(RwLock::new(Box::new(storage))))
}
```

### Migration Reporting and Logging
```rust
impl FileSystemIssueStorage {
    /// Get human-readable migration status
    pub fn migration_status() -> Result<String> {
        let info = Self::migration_info()?;
        
        if info.should_migrate {
            Ok(format!(
                "Migration needed: {} files ({:.1} KB) in ./issues/",
                info.file_count,
                info.total_size as f64 / 1024.0
            ))
        } else if info.source_exists && info.destination_exists {
            Ok("Both ./issues/ and .swissarmyhammer/issues/ exist - no migration needed".to_string())
        } else if info.destination_exists {
            Ok("Using .swissarmyhammer/issues/ directory".to_string())
        } else {
            Ok("No issues directory found - will create .swissarmyhammer/issues/".to_string())
        }
    }
}
```

### Configuration and Control
```rust
/// Configuration for migration behavior
#[derive(Debug, Clone)]
pub struct MigrationConfig {
    pub auto_migrate: bool,
    pub create_backup: bool,
    pub require_confirmation: bool,
    pub max_file_count: usize,
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

impl FileSystemIssueStorage {
    /// Create storage with custom migration configuration
    pub fn new_default_with_config(config: &MigrationConfig) -> Result<(Self, Option<MigrationResult>)> {
        if !config.auto_migrate {
            return Ok((Self::new_default_without_migration()?, None));
        }
        
        let info = Self::migration_info()?;
        
        // Check size and file count limits
        if info.file_count > config.max_file_count {
            return Err(SwissArmyHammerError::validation_failed(&format!(
                "Too many files for automatic migration: {} > {}",
                info.file_count, config.max_file_count
            )));
        }
        
        if info.total_size > config.max_total_size {
            return Err(SwissArmyHammerError::validation_failed(&format!(
                "Directory too large for automatic migration: {} > {} bytes",
                info.total_size, config.max_total_size
            )));
        }
        
        Self::new_default_with_migration_info()
    }
    
    /// Create storage without any migration attempts
    fn new_default_without_migration() -> Result<Self> {
        let current_dir = std::env::current_dir().map_err(SwissArmyHammerError::Io)?;
        let issues_dir = Self::default_directory()?;
        Self::new(issues_dir)
    }
}
```

## Implementation Details

### Migration Triggering
- Automatic migration occurs on first storage creation
- Migration is atomic and happens once per repository
- Failed migrations prevent storage creation
- Configuration controls migration behavior

### Error Handling
- Migration failures are treated as storage creation failures
- Rollback ensures no partial state
- Comprehensive error reporting for debugging
- Graceful degradation when possible

### Logging and User Communication
- CLI provides user-friendly migration messages
- MCP server logs migration activities
- Library users can get detailed migration results
- Different verbosity levels for different contexts

### Thread Safety
- Migration is atomic at the filesystem level
- Concurrent storage creation handles migration properly
- Race conditions avoided through filesystem semantics

## Testing Requirements

### Integration Tests
```rust
#[cfg(test)]
mod integration_tests {
    #[test]
    fn test_cli_automatic_migration() {
        // Test CLI commands trigger migration correctly
    }
    
    #[test]
    fn test_mcp_automatic_migration() {
        // Test MCP server handles migration
    }
    
    #[test]
    fn test_library_automatic_migration() {
        // Test library usage triggers migration
    }
    
    #[test]
    fn test_concurrent_storage_creation() {
        // Test multiple processes don't interfere with migration
    }
    
    #[test]
    fn test_migration_configuration() {
        // Test migration configuration controls behavior
    }
}
```

### CLI-Specific Tests
- Test CLI migration messages are user-friendly
- Test CLI handles migration failures appropriately
- Test CLI working directory context handling
- Test CLI batch operations with migration

### MCP-Specific Tests
- Test MCP server startup with migration
- Test MCP tool operations after migration
- Test MCP error handling for migration failures
- Test MCP logging of migration activities

## Files to Modify
- `swissarmyhammer/src/issues/filesystem.rs`
- `swissarmyhammer-cli/src/mcp_integration.rs`
- `swissarmyhammer-tools/src/mcp/server.rs`
- Add configuration structures
- Update integration tests
- Add CLI and MCP-specific tests

## Acceptance Criteria
- [ ] Storage creation automatically performs migration when needed
- [ ] CLI users see appropriate migration messages
- [ ] MCP server handles migration transparently
- [ ] Library users can control migration behavior
- [ ] Migration configuration provides fine-grained control
- [ ] Thread safety maintained across all integration points
- [ ] Error handling provides clear diagnostics
- [ ] All existing functionality preserved
- [ ] Performance acceptable for typical usage

## Dependencies
- Depends on step 000290 (automatic migration logic)
- Should be done after infrastructure tests (step 000289)

## Estimated Effort
~400-500 lines including integration points, configuration, and comprehensive testing.

## Notes
- Focus on seamless user experience
- Provide appropriate feedback for different interfaces
- Consider long-term maintenance of migration code
- Ensure migration can be disabled if needed for troubleshooting