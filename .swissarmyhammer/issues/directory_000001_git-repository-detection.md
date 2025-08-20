# Git Repository Detection Implementation

Refer to /Users/wballard/github/sah-directory/ideas/directory.md

## Overview
Implement the foundation for Git repository-centric directory resolution by adding robust Git repository detection functionality.

## Technical Approach

Add new functions to `directory_utils.rs`:

```rust
/// Find the Git repository root starting from current directory
/// Walks up the directory tree looking for .git directory
pub fn find_git_repository_root() -> Option<PathBuf> {
    let current_dir = std::env::current_dir().ok()?;
    let mut path = current_dir.as_path();
    let mut depth = 0;

    loop {
        if depth >= MAX_DIRECTORY_DEPTH {
            break;
        }

        if path.join(".git").exists() {
            return Some(path.to_path_buf());
        }

        match path.parent() {
            Some(parent) => {
                path = parent;
                depth += 1;
            }
            None => break,
        }
    }

    None
}
```

## Error Types
Add new error variants to handle Git repository requirements:

```rust
#[derive(Error, Debug)]
pub enum SwissArmyHammerError {
    #[error("SwissArmyHammer must be run from within a Git repository")]
    NotInGitRepository,
    
    #[error("Failed to create .swissarmyhammer directory: {0}")]
    DirectoryCreation(std::io::Error),
    
    #[error("Git repository found but .swissarmyhammer directory is not accessible: {0}")]
    DirectoryAccess(std::io::Error),
}
```

## Tasks
1. Add `find_git_repository_root()` function to `directory_utils.rs`
2. Add new error variants to `error.rs` 
3. Add comprehensive unit tests covering:
   - Git repository detection from various depths
   - Handling of missing `.git` directories
   - MAX_DIRECTORY_DEPTH boundary testing
   - Edge cases (root directory, permission issues)
4. Test in environments with and without Git repositories

## Success Criteria
- `find_git_repository_root()` correctly identifies Git repository roots
- Handles edge cases gracefully (no infinite loops, respects depth limits)
- Comprehensive error handling with clear messages
- All tests pass including edge cases
- No changes to existing functionality (additive only)