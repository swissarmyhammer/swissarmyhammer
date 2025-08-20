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