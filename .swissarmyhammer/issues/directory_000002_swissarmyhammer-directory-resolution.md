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