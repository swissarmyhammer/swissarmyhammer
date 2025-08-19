# Manual Migration CLI Command (Optional)

## Overview
Add an optional manual migration CLI command for users who want explicit control over the migration process or need to troubleshoot migration issues.

Refer to /Users/wballard/github/sah-issues/ideas/move_issues.md

## Current State
After step 000291, automatic migration works but users may need manual control for various scenarios.

## Target Implementation

### CLI Command Structure
```rust
// In swissarmyhammer-cli/src/commands/mod.rs
#[derive(Debug, Parser)]
pub enum Commands {
    // ... existing commands
    
    #[command(about = "Migrate issues directory to .swissarmyhammer/issues")]
    Migrate {
        #[command(subcommand)]
        subcommand: MigrateSubcommand,
    },
}

#[derive(Debug, Parser)]
pub enum MigrateSubcommand {
    #[command(about = "Show migration status and preview")]
    Status,
    
    #[command(about = "Perform migration with confirmation")]
    Run {
        #[arg(long, help = "Skip confirmation prompt")]
        force: bool,
        
        #[arg(long, help = "Create backup before migration")]
        backup: bool,
        
        #[arg(long, help = "Dry run - show what would be migrated")]
        dry_run: bool,
    },
    
    #[command(about = "Check if migration is possible")]
    Check,
    
    #[command(about = "Clean up migration artifacts")]
    Cleanup,
}
```

### Migration Status Command
```rust
pub async fn handle_migrate_status() -> Result<(), Box<dyn std::error::Error>> {
    let info = swissarmyhammer::issues::FileSystemIssueStorage::migration_info()?;
    
    println!("📊 Migration Status");
    println!();
    
    if info.should_migrate {
        println!("✅ Migration needed");
        println!("   Source: ./issues/ ({} files, {:.1} KB)", 
                info.file_count, info.total_size as f64 / 1024.0);
        println!("   Target: .swissarmyhammer/issues/");
        println!();
        println!("Run 'sah migrate run' to perform migration");
    } else if info.source_exists && info.destination_exists {
        println!("⚠️  Both directories exist");
        println!("   Legacy: ./issues/");
        println!("   Current: .swissarmyhammer/issues/");
        println!();
        println!("Manual intervention may be required");
    } else if info.destination_exists {
        println!("✅ Already using .swissarmyhammer/issues/");
    } else {
        println!("ℹ️  No issues directory found");
        println!("   Will create .swissarmyhammer/issues/ when needed");
    }
    
    Ok(())
}
```

### Migration Run Command
```rust
pub async fn handle_migrate_run(
    force: bool,
    backup: bool,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let info = swissarmyhammer::issues::FileSystemIssueStorage::migration_info()?;
    
    if !info.should_migrate {
        println!("No migration needed");
        return Ok(());
    }
    
    println!("🚀 Migration Plan");
    println!("   Source: ./issues/");
    println!("   Target: .swissarmyhammer/issues/");
    println!("   Files: {}", info.file_count);
    println!("   Size: {:.1} KB", info.total_size as f64 / 1024.0);
    if backup {
        println!("   Backup: Yes");
    }
    println!();
    
    if dry_run {
        println!("🧪 Dry Run - No files will be moved");
        return Ok(());
    }
    
    if !force {
        println!("Proceed with migration? (y/N)");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().to_lowercase().starts_with('y') {
            println!("Migration cancelled");
            return Ok(());
        }
    }
    
    println!("🔄 Starting migration...");
    
    let result = if backup {
        swissarmyhammer::issues::FileSystemIssueStorage::perform_migration_with_backup()?
    } else {
        swissarmyhammer::issues::FileSystemIssueStorage::perform_migration()?
    };
    
    match result {
        MigrationResult::Success(stats) => {
            println!("✅ Migration completed successfully!");
            println!("   Files moved: {}", stats.files_moved);
            println!("   Data transferred: {:.1} KB", stats.bytes_moved as f64 / 1024.0);
            println!("   Duration: {:.2}s", stats.duration.as_secs_f64());
        }
        MigrationResult::Failed(e) => {
            println!("❌ Migration failed: {}", e);
            return Err(e.into());
        }
        MigrationResult::NotNeeded(_) => {
            println!("ℹ️  No migration was needed");
        }
    }
    
    Ok(())
}
```

### Migration Check Command
```rust
pub async fn handle_migrate_check() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Checking migration prerequisites...");
    
    let info = swissarmyhammer::issues::FileSystemIssueStorage::migration_info()?;
    let paths = swissarmyhammer::issues::FileSystemIssueStorage::migration_paths()?;
    
    // Check source directory
    if info.source_exists {
        println!("✅ Source directory exists: {}", paths.source.display());
        println!("   Files: {}", info.file_count);
        println!("   Size: {:.1} KB", info.total_size as f64 / 1024.0);
    } else {
        println!("❌ Source directory does not exist: {}", paths.source.display());
    }
    
    // Check destination
    if info.destination_exists {
        println!("⚠️  Destination already exists: {}", paths.destination.display());
    } else {
        println!("✅ Destination available: {}", paths.destination.display());
    }
    
    // Check parent directory permissions
    if let Some(parent) = paths.destination.parent() {
        if parent.exists() {
            match std::fs::metadata(parent) {
                Ok(metadata) => {
                    if metadata.permissions().readonly() {
                        println!("❌ Parent directory is read-only: {}", parent.display());
                    } else {
                        println!("✅ Parent directory is writable: {}", parent.display());
                    }
                }
                Err(e) => {
                    println!("⚠️  Cannot check parent directory permissions: {}", e);
                }
            }
        } else {
            println!("ℹ️  Parent directory will be created: {}", parent.display());
        }
    }
    
    // Overall assessment
    if info.should_migrate {
        println!();
        println!("🎯 Migration is recommended and should succeed");
    } else {
        println!();
        println!("ℹ️  No migration needed at this time");
    }
    
    Ok(())
}
```

### Migration Cleanup Command
```rust
pub async fn handle_migrate_cleanup() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧹 Cleaning up migration artifacts...");
    
    let current_dir = std::env::current_dir()?;
    let swissarmyhammer_dir = current_dir.join(".swissarmyhammer");
    
    // Look for backup directories
    let mut backup_count = 0;
    let mut backup_size = 0u64;
    
    if swissarmyhammer_dir.exists() {
        for entry in std::fs::read_dir(&swissarmyhammer_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("issues_backup_") {
                        let metadata = std::fs::metadata(&path)?;
                        backup_size += metadata.len();
                        backup_count += 1;
                        
                        println!("Found backup: {}", name);
                    }
                }
            }
        }
    }
    
    if backup_count == 0 {
        println!("No migration artifacts found");
        return Ok(());
    }
    
    println!();
    println!("Found {} backup(s) using {:.1} KB", backup_count, backup_size as f64 / 1024.0);
    println!("Remove backups? (y/N)");
    
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if !input.trim().to_lowercase().starts_with('y') {
        println!("Cleanup cancelled");
        return Ok(());
    }
    
    // Remove backups
    let mut removed_count = 0;
    for entry in std::fs::read_dir(&swissarmyhammer_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("issues_backup_") {
                    std::fs::remove_dir_all(&path)?;
                    removed_count += 1;
                    println!("Removed: {}", name);
                }
            }
        }
    }
    
    println!("✅ Removed {} backup(s)", removed_count);
    Ok(())
}
```

## Implementation Details

### User Experience Design
- Clear status information with emojis for visual clarity
- Confirmation prompts to prevent accidental operations
- Dry-run capability for safe preview
- Detailed error messages and troubleshooting guidance

### Safety Features
- Default to confirmation required (unless --force)
- Optional backup creation with --backup flag
- Comprehensive pre-migration checks
- Cleanup command for managing backup artifacts

### Integration with Existing Infrastructure
- Reuse migration logic from step 000290
- Integrate with CLI command structure
- Use existing error handling patterns
- Maintain consistency with other CLI commands

## Testing Requirements

### CLI Command Tests
```rust
#[cfg(test)]
mod migrate_command_tests {
    #[test]
    fn test_migrate_status_command() {
        // Test status display for various scenarios
    }
    
    #[test]
    fn test_migrate_run_with_confirmation() {
        // Test interactive confirmation flow
    }
    
    #[test]
    fn test_migrate_run_force() {
        // Test forced migration without confirmation
    }
    
    #[test]
    fn test_migrate_dry_run() {
        // Test dry run doesn't modify anything
    }
    
    #[test]
    fn test_migrate_check_prerequisites() {
        // Test prerequisite checking
    }
    
    #[test]
    fn test_migrate_cleanup() {
        // Test backup cleanup functionality
    }
}
```

### Integration Tests
- Test CLI migration commands with real directory structures
- Test error handling for various failure scenarios
- Test user interaction flows
- Test cleanup of migration artifacts

### User Experience Tests
- Test help text and command documentation
- Test error message clarity and usefulness
- Test confirmation prompts and cancellation
- Test progress reporting for long operations

## Files to Modify
- `swissarmyhammer-cli/src/commands/mod.rs`
- Add migration command module
- Update CLI command dispatcher
- Add comprehensive CLI tests
- Update CLI help documentation

## Acceptance Criteria
- [ ] Manual migration commands provide full control over process
- [ ] Status command shows clear migration information
- [ ] Run command safely performs migration with confirmation
- [ ] Check command validates migration prerequisites
- [ ] Cleanup command manages migration artifacts
- [ ] Dry-run capability allows safe preview
- [ ] Force option enables non-interactive migration
- [ ] Comprehensive error handling and user guidance
- [ ] Help text and documentation are clear and complete

## Dependencies
- Depends on steps 000290-000291 (migration logic and integration)
- Optional step - automatic migration works without this

## Estimated Effort
~600-700 lines including all commands, user interaction, and comprehensive testing.

## Notes
- This is an optional step that provides manual control
- Focus on user experience and safety
- Consider edge cases and error recovery scenarios
- Design for both interactive and script usage
- Maintain consistency with existing CLI command patterns