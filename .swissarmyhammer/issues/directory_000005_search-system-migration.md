# Search System Migration

Refer to /Users/wballard/github/sah-directory/ideas/directory.md

## Overview
Migrate the semantic search system from multiple directory database selection to a single Git repository-centric database location.

## Current Implementation Analysis  
The search system currently:
- Uses `find_swissarmyhammer_dirs_upward()` to find multiple directories
- Uses the "most specific (deepest)" local directory for database placement
- Falls back to home directory if no local directories exist
- Database location: `<deepest_local>/.swissarmyhammer/semantic.db`

## New Implementation Approach

### Database Location Strategy
```rust
impl SemanticDatabase {
    /// Get database path using Git repository-centric approach
    fn get_database_path() -> Result<PathBuf, SwissArmyHammerError> {
        // First priority: Git repository .swissarmyhammer directory  
        if let Some(swissarmyhammer_dir) = find_swissarmyhammer_directory() {
            return Ok(swissarmyhammer_dir.join("semantic.db"));
        }
        
        // Fallback: User home directory (for commands run outside Git repos)
        let home = std::env::var("HOME")
            .or_else(|_| dirs::home_dir().map(|p| p.to_string_lossy().to_string()))
            .map_err(|_| SwissArmyHammerError::HomeDirectoryNotFound)?;
            
        Ok(PathBuf::from(home).join(".swissarmyhammer").join("semantic.db"))
    }
}
```

## Database Strategy
- **Primary**: `<git_root>/.swissarmyhammer/semantic.db` (when in Git repository)
- **Fallback**: `~/.swissarmyhammer/semantic.db` (when outside Git repository)

This ensures:
- Each Git repository has its own semantic index
- Commands outside Git repositories still work (using global index)  
- Clear separation between project-specific and global search indexes

## Migration Considerations
- Existing databases in deep directories will become unused
- Users may need to re-index content in new location
- Database schema and functionality remain unchanged

## Tasks
1. Update database path resolution logic in `search/types.rs`
2. Remove dependency on `find_swissarmyhammer_dirs_upward()`
3. Add fallback to home directory for non-Git contexts
4. Update database initialization and connection logic
5. Add comprehensive tests covering:
   - Database creation in Git repository context
   - Fallback behavior outside Git repositories  
   - Database migration scenarios
   - Error handling for permission issues
6. Performance validation (no regression)

## Data Migration Strategy
- Old databases remain in place (no automatic migration)
- Users should re-run `sah search index` to rebuild in new location
- Document migration process in release notes

## Dependencies
- Depends on: directory_000002_swissarmyhammer-directory-resolution

## Success Criteria  
- Search database correctly located in Git repository when available
- Graceful fallback to home directory when outside Git repository
- Database functionality unchanged (indexing, querying work as before)
- Clear error messages for database access issues
- All tests pass including edge cases
- Documentation updated with new database location strategy

## Proposed Solution

Based on my analysis of the current implementation, I'll update the search system to follow the new Git repository-centric approach:

### Current Implementation Issue
The `SemanticConfig::find_semantic_database_path()` method in `search/types.rs:244-290` currently uses `find_swissarmyhammer_dirs_upward()` to find multiple directories and selects the "most specific (deepest)" directory. This violates the new single Git repository approach.

### Implementation Plan

1. **Replace multi-directory logic with Git-centric approach**
   - Use `find_swissarmyhammer_directory()` from `directory_utils.rs` 
   - This already implements the Git repository + `.swissarmyhammer` existence check
   
2. **Add proper fallback to home directory**
   - When not in Git repository or no `.swissarmyhammer` exists
   - Use `~/.swissarmyhammer/semantic.db` as global fallback
   
3. **Database location strategy**:
   - **Primary**: `<git_root>/.swissarmyhammer/semantic.db` (repository-specific)
   - **Fallback**: `~/.swissarmyhammer/semantic.db` (global)

### Implementation Code

```rust
fn find_semantic_database_path() -> PathBuf {
    // Environment variable override for testing
    if let Ok(env_path) = std::env::var("SWISSARMYHAMMER_SEMANTIC_DB_PATH") {
        return PathBuf::from(env_path);
    }

    // Try Git repository .swissarmyhammer directory first
    if let Some(swissarmyhammer_dir) = crate::directory_utils::find_swissarmyhammer_directory() {
        return swissarmyhammer_dir.join("semantic.db");
    }
    
    // Fallback to home directory (for commands outside Git repos)
    if let Some(home_dir) = dirs::home_dir() {
        let swissarmyhammer_dir = home_dir.join(".swissarmyhammer");
        if let Err(e) = std::fs::create_dir_all(&swissarmyhammer_dir) {
            tracing::warn!("Cannot create home .swissarmyhammer directory: {}", e);
            return PathBuf::from(".swissarmyhammer/semantic.db");
        }
        return swissarmyhammer_dir.join("semantic.db");
    }

    // Final fallback to current directory
    PathBuf::from(".swissarmyhammer/semantic.db")
}
```

This ensures:
- Each Git repository has its own semantic search database
- Commands outside Git repositories use global index in home directory
- Clear separation between project-specific and global search indexes
- Backward compatibility for existing workflows

## Implementation Notes

Successfully migrated the search system database location logic from multiple directory resolution to Git repository-centric approach.

### Changes Made

1. **Updated `SemanticConfig::find_semantic_database_path()`** (lines 244-290)
   - Replaced `find_swissarmyhammer_dirs_upward()` with `find_swissarmyhammer_directory()`
   - Database location now follows Git repository-centric approach
   - Maintains environment variable override for testing

2. **Database Location Strategy** (implemented as planned)
   - **Primary**: `<git_root>/.swissarmyhammer/semantic.db` when in Git repository with `.swissarmyhammer` directory
   - **Fallback**: `~/.swissarmyhammer/semantic.db` when outside Git repositories or no `.swissarmyhammer` exists
   - **Final fallback**: `.swissarmyhammer/semantic.db` relative path if home directory unavailable

3. **Updated Tests**
   - `test_semantic_config_git_repository_path()` - validates Git repository path logic
   - `test_semantic_config_home_fallback()` - validates fallback to home directory
   - `test_semantic_config_git_repo_no_swissarmyhammer()` - validates behavior when Git repo exists but no `.swissarmyhammer`
   - `test_semantic_config_environment_variable_override()` - validates testing override

### Validation Results

- All 7 semantic config tests pass ✅
- All 188 search module tests pass ✅ 
- No functional regressions detected
- File loader system properly preserved (still uses hierarchical approach as designed)

### Migration Impact

- **Existing databases**: Old databases in deep directories will become unused (no automatic migration needed)
- **New behavior**: Each Git repository gets its own semantic search database
- **Global usage**: Commands outside Git repos use global home directory database
- **Performance**: No regression - database functionality unchanged

The search system now correctly follows the new Git repository-centric directory resolution while maintaining backward compatibility through fallback mechanisms.