# SwissArmyHammer Directory Resolution Implementation

Refer to /Users/wballard/github/sah-directory/ideas/directory.md

## Overview
Implement the new Git-centric SwissArmyHammer directory resolution functions that replace the multiple directory approach with a single repository-based approach.

## Technical Approach

Add core directory resolution functions to `directory_utils.rs`:

```rust
/// Find the SwissArmyHammer directory for the current Git repository
/// Returns None if not in a Git repository or if no .swissarmyhammer directory exists
pub fn find_swissarmyhammer_directory() -> Option<PathBuf> {
    let git_root = find_git_repository_root()?;
    let swissarmyhammer_dir = git_root.join(".swissarmyhammer");
    
    if swissarmyhammer_dir.exists() && swissarmyhammer_dir.is_dir() {
        Some(swissarmyhammer_dir)
    } else {
        None
    }
}

/// Get or create the SwissArmyHammer directory for the current Git repository
/// Returns error if not in a Git repository
pub fn get_or_create_swissarmyhammer_directory() -> Result<PathBuf, SwissArmyHammerError> {
    let git_root = find_git_repository_root()
        .ok_or(SwissArmyHammerError::NotInGitRepository)?;
    
    let swissarmyhammer_dir = git_root.join(".swissarmyhammer");
    
    if !swissarmyhammer_dir.exists() {
        std::fs::create_dir_all(&swissarmyhammer_dir)
            .map_err(|e| SwissArmyHammerError::DirectoryCreation(e))?;
    }
    
    Ok(swissarmyhammer_dir)
}
```

## Directory Structure
These functions enforce the new structure:
```
project-root/               # Git repository root
├── .git/                   # Git directory  
├── .swissarmyhammer/       # SwissArmyHammer directory (ONLY HERE)
│   ├── memos/             # Memoranda storage
│   ├── todo/              # Todo lists  
│   ├── runs/              # Workflow run storage
│   ├── search.db          # Semantic search database
│   └── workflows/         # Local workflows (optional)
└── src/                    # Project source code
```

## Tasks
1. Implement `find_swissarmyhammer_directory()` function
2. Implement `get_or_create_swissarmyhammer_directory()` function  
3. Add comprehensive unit tests covering:
   - Successful directory detection at Git repository root
   - Creation of missing `.swissarmyhammer` directories
   - Error handling for non-Git contexts
   - Permission handling for directory creation
   - Validation of directory vs file conflicts
4. Integration tests with real Git repositories
5. Documentation and examples

## Dependencies
- Depends on: directory_000001_git-repository-detection

## Success Criteria
- Functions correctly identify and create `.swissarmyhammer` directories at Git repository roots
- Clear error messages when not in Git repository
- No data loss or corruption during directory creation
- All tests pass including error scenarios
- Functions are ready for integration by other components
## Proposed Solution

I have implemented the Git-centric SwissArmyHammer directory resolution functions as specified in the issue requirements and ideas/directory.md document.

### Implementation Details

**Core Functions Added to `directory_utils.rs`:**

1. **`find_swissarmyhammer_directory() -> Option<PathBuf>`**
   - Locates existing `.swissarmyhammer` directories at Git repository roots only
   - Returns `None` if not in a Git repository or if no `.swissarmyhammer` directory exists
   - Uses the existing `find_git_repository_root()` function for Git repository detection
   - Validates that `.swissarmyhammer` exists and is a directory (not a file)

2. **`get_or_create_swissarmyhammer_directory() -> Result<PathBuf, SwissArmyHammerError>`**
   - Creates `.swissarmyhammer` directory at Git repository root if it doesn't exist
   - Returns appropriate errors for non-Git contexts or creation failures
   - Uses existing error types: `NotInGitRepository` and `DirectoryCreation`

### Key Design Decisions

1. **Git Repository Requirement**: Both functions enforce that SwissArmyHammer must be run within a Git repository context
2. **Single Source of Truth**: Each Git repository has exactly one `.swissarmyhammer` directory at the repository root
3. **Working Directory Independence**: Functions work consistently regardless of current working directory within the repository
4. **Comprehensive Error Handling**: Clear error messages for all failure scenarios using existing error types
5. **Existing Pattern Consistency**: Leverages existing `find_git_repository_root()` and follows codebase patterns

### Comprehensive Test Coverage

Added 12 comprehensive unit tests covering:
- ✅ Successful directory detection at Git repository root
- ✅ Creation of missing `.swissarmyhammer` directories  
- ✅ Error handling for non-Git contexts
- ✅ Permission handling for directory creation conflicts
- ✅ Validation of directory vs file conflicts
- ✅ Testing from subdirectories (repository root detection)
- ✅ Multiple Git repository scenarios (nested repos)
- ✅ Depth limit edge cases
- ✅ Existing directory preservation

### Integration Readiness

The implemented functions are now ready for integration by other components:
- Memoranda system can use `get_or_create_swissarmyhammer_directory()` for consistent memo storage
- Search system can use it for semantic search database location
- Todo system can migrate from current directory fallback to Git repository enforcement
- File loading system can use `find_swissarmyhammer_directory()` for directory resolution

### Benefits Achieved

1. **Consistency**: All components will now use the same directory resolution logic
2. **Predictability**: `.swissarmyhammer` always at Git repository root, never in subdirectories
3. **No Path Dependencies**: Commands work the same regardless of current directory
4. **Simplified Logic**: No need to handle multiple directories or precedence rules
5. **Clear Error Messages**: Users know they need to be in a Git repository
6. **Test Coverage**: Comprehensive edge case coverage ensures reliability

The implementation follows all coding standards and patterns established in the codebase, uses existing error types, and maintains backward compatibility requirements.