# Todo System Migration

Refer to /Users/wballard/github/sah-directory/ideas/directory.md

## Overview
Migrate the todo system from its current "Git repository root OR current directory" fallback approach to a strict Git repository-centric approach.

## Current Implementation Analysis
The todo system currently:
- Uses `directory_utils::find_repository_or_current_directory()` 
- Falls back to current directory if no Git repository found
- Stores todos in `<base_dir>/.swissarmyhammer/todo/`

## New Implementation Approach

### Storage Location Strategy
```rust
impl TodoSystem {
    /// Get todo directory using strict Git repository approach
    fn get_todo_directory() -> Result<PathBuf, SwissArmyHammerError> {
        // Require Git repository - no fallback to current directory
        let swissarmyhammer_dir = get_or_create_swissarmyhammer_directory()?;
        let todo_dir = swissarmyhammer_dir.join("todo");
        
        // Ensure todo subdirectory exists  
        std::fs::create_dir_all(&todo_dir)
            .map_err(|e| SwissArmyHammerError::DirectoryCreation(e))?;
            
        Ok(todo_dir)
    }
}
```

## Storage Strategy
- **Required**: `<git_root>/.swissarmyhammer/todo/` (Git repository required)
- **No Fallback**: Remove fallback to current directory
- **Clear Errors**: Commands outside Git repository fail with helpful message

## Behavioral Changes
- **Before**: Works in current directory if no Git repository found
- **After**: Requires Git repository, fails with clear error message if not found
- **Consistency**: Todo location independent of command execution directory

## Tasks
1. Update todo directory resolution in `todo/mod.rs`
2. Replace `find_repository_or_current_directory()` with `get_or_create_swissarmyhammer_directory()`
3. Remove fallback logic to current directory
4. Add Git repository requirement validation
5. Update all todo MCP tools with new error handling  
6. Add comprehensive tests covering:
   - Todo operations within Git repository
   - Error handling outside Git repository
   - Directory creation and permissions
   - Migration from current directory approach
7. Update CLI error messages

## Error Handling Updates
```rust
// Replace fallback error handling with clear Git requirement
let base_dir = get_or_create_swissarmyhammer_directory()
    .map_err(|e| SwissArmyHammerError::TodoDirectoryCreation(format!(
        "Todo operations require a Git repository. Please run this command from within a Git repository. Error: {}", 
        e
    )))?;
```

## Dependencies  
- Depends on: directory_000002_swissarmyhammer-directory-resolution

## Migration Impact
- **Breaking Change**: Todo operations outside Git repository will fail
- Users must run todo commands from within Git repository
- Existing todos in non-Git directories will become inaccessible

## Data Migration
- Todos created in current directory locations will remain in place
- Users should manually move `.swissarmyhammer/todo/` to Git repository root
- Provide clear migration documentation

## Success Criteria
- Todo operations work reliably within Git repositories
- Clear error messages guide users to run commands in Git repository
- No silent fallbacks or unexpected behavior
- All todo functionality preserved within Git context
- Comprehensive testing validates all scenarios
- Migration documentation helps users transition existing todos