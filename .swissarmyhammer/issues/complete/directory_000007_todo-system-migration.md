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
## Proposed Solution

After analyzing the current implementation, I will implement the Git repository-centric approach by:

### 1. Update `get_todo_directory()` function in `todo/mod.rs`:
- Replace `directory_utils::find_repository_or_current_directory()` with `directory_utils::get_or_create_swissarmyhammer_directory()`  
- Remove fallback logic to current directory
- Return clear Git repository requirement error if not in a Git repository

### 2. Error Handling Strategy:
```rust
pub fn get_todo_directory() -> Result<PathBuf> {
    let swissarmyhammer_dir = directory_utils::get_or_create_swissarmyhammer_directory()
        .map_err(|e| SwissArmyHammerError::Other(format!(
            "Todo operations require a Git repository. Please run this command from within a Git repository. Error: {}",
            e
        )))?;
    
    let todo_dir = swissarmyhammer_dir.join("todo");
    
    // Ensure todo subdirectory exists  
    fs::create_dir_all(&todo_dir).map_err(|e| {
        SwissArmyHammerError::Other(format!("Failed to create todo directory: {e}"))
    })?;
    
    Ok(todo_dir)
}
```

### 3. Test Coverage Strategy:
- Test successful todo operations within Git repository using `IsolatedTestEnvironment` 
- Test clear error messages when run outside Git repository
- Verify directory creation and permissions
- Test all existing todo functionality still works within Git context

### 4. Implementation Steps:
1. Update `get_todo_directory()` function
2. Run existing tests to ensure no regression
3. Add new tests for Git repository requirement
4. Test CLI error messages manually
5. Verify all todo MCP tools work with new approach

This ensures todos are always stored in the Git repository's `.swissarmyhammer/todo/` directory with clear error messages when used outside a Git repository context.
## Implementation Completed

Successfully migrated the todo system from the fallback approach to strict Git repository-centric storage.

### Changes Made

#### 1. Updated `get_todo_directory()` Function
- **File**: `swissarmyhammer/src/todo/mod.rs`
- **Change**: Replaced `directory_utils::find_repository_or_current_directory()` with `directory_utils::get_or_create_swissarmyhammer_directory()`
- **Impact**: Removes fallback to current directory, requires Git repository

#### 2. Enhanced Error Messages
- Added clear error message: "Todo operations require a Git repository. Please run this command from within a Git repository."
- Provides actionable guidance to users when commands fail outside Git repository

#### 3. Automatic MCP Tool Coverage
- All todo MCP tools (`todo_create`, `todo_show`, `todo_mark_complete`) automatically inherit the new Git repository requirement
- No additional changes needed in MCP tools since they use `TodoStorage::new_default()` which calls the updated `get_todo_directory()`

#### 4. Comprehensive Testing
- Added new test: `test_get_todo_directory_git_repository_requirement`
- Tests error handling when run outside Git repository
- Verifies clear error messages are provided
- All existing tests continue to pass (15 total tests)

### Behavioral Changes
- **Before**: Todo operations worked in current directory if no Git repository found
- **After**: Todo operations require Git repository, fail with clear error message if not found
- **Storage Location**: Always `<git_root>/.swissarmyhammer/todo/` (no fallbacks)

### Verification Results
- ✅ All existing todo functionality preserved within Git repositories
- ✅ Clear error messages when run outside Git repository
- ✅ No breaking changes to MCP API
- ✅ 15 tests passing including new Git repository requirement test
- ✅ Code passes formatting (cargo fmt) and linting (cargo clippy)

### Migration Impact
This is a **breaking change** for users who previously used todo operations outside Git repositories. They will need to:
1. Move to a Git repository to use todo operations
2. Manually migrate existing `.swissarmyhammer/todo/` directories to their Git repository root if needed

The migration enforces consistency with other SwissArmyHammer features that expect Git repository context and eliminates the non-deterministic behavior of the fallback approach.