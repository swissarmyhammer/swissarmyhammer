# Memoranda System Migration

Refer to /Users/wballard/github/sah-directory/ideas/directory.md

## Overview
Migrate the memoranda system from current working directory-based storage to Git repository-centric storage location.

## Current Implementation Analysis
The memoranda system currently:
- Uses `std::env::current_dir()?.join(".swissarmyhammer").join("memos")`  
- Simple current working directory approach with no upward traversal
- Creates `.swissarmyhammer/memos` in whatever directory the command is run from

## New Implementation Approach

### Storage Location Strategy
```rust
impl MemoriadaStorage {
    /// Get memos directory using Git repository-centric approach
    fn get_memos_directory() -> Result<PathBuf, SwissArmyHammerError> {
        // Primary: Git repository .swissarmyhammer directory
        let swissarmyhammer_dir = get_or_create_swissarmyhammer_directory()?;
        let memos_dir = swissarmyhammer_dir.join("memos");
        
        // Ensure memos subdirectory exists
        std::fs::create_dir_all(&memos_dir)
            .map_err(|e| SwissArmyHammerError::DirectoryCreation(e))?;
            
        Ok(memos_dir)
    }
}
```

## Storage Strategy  
- **Required**: `<git_root>/.swissarmyhammer/memos/` (Git repository required)
- **No Fallback**: Commands must be run within a Git repository

This ensures:
- Consistent memo location regardless of command execution directory
- All memos for a project are centralized in one location  
- Clear error when trying to use memos outside Git repository context

## Migration Impact
- **Breaking Change**: Memos created in various working directories will no longer be accessible
- Users need to consolidate memos to Git repository location
- Command behavior is now independent of current working directory

## Tasks
1. Update memo storage path resolution in `memoranda/storage.rs`  
2. Replace current directory logic with Git repository resolution
3. Remove working directory dependency from all memo operations
4. Add Git repository requirement validation
5. Update MCP tools (memo_create, memo_get, etc.) with new error handling
6. Add comprehensive tests covering:
   - Memo operations within Git repository
   - Error handling outside Git repository
   - Directory creation and permissions
   - Migration scenarios from old locations
7. Integration tests with real memo workflows

## CLI Integration
Update memo-related CLI commands:
- Add clear error messages when outside Git repository
- Guide users to run commands from within Git repository
- Document migration process for existing memos

## Dependencies
- Depends on: directory_000002_swissarmyhammer-directory-resolution

## Data Migration
- Old memo files remain in place (no automatic migration)
- Users must manually move memos from old locations to `<git_root>/.swissarmyhammer/memos/`
- Provide migration documentation and tooling

## Success Criteria
- Memo operations work consistently within Git repositories
- Clear error messages when outside Git repository context  
- Memo location is independent of command execution directory
- All existing memo functionality preserved
- Comprehensive testing covers all scenarios
- Users can successfully migrate existing memos to new location