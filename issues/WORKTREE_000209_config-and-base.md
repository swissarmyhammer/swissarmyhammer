# Worktree Configuration and Base Infrastructure

## Overview
Add configuration and base infrastructure for worktree-based issue management. This is the foundation step that establishes the configuration structure and directory utilities needed for subsequent worktree operations.

## Implementation

### 1. Update Config struct (`src/config.rs`)
Add worktree configuration fields to the existing `Config` struct:

```rust
pub struct Config {
    // ... existing fields ...
    
    /// Base directory for worktrees (relative to repo root)
    pub worktree_base_dir: String,
    
    /// Whether to automatically clean up worktrees after merge
    pub worktree_auto_cleanup: bool,
}
```

Default values:
- `worktree_base_dir`: `.swissarmyhammer/worktrees`
- `worktree_auto_cleanup`: `true`

### 2. Environment Variable Support
Add environment variable support for the new config fields:
- `SWISSARMYHAMMER_WORKTREE_BASE_DIR`
- `SWISSARMYHAMMER_WORKTREE_AUTO_CLEANUP`

### 3. Directory Structure Utilities (`src/git.rs`)
Add utility methods to `GitOperations` for managing worktree directories:

```rust
impl GitOperations {
    /// Get the base directory for all worktrees
    fn get_worktree_base_dir(&self) -> PathBuf {
        let config = Config::global();
        self.work_dir.join(&config.worktree_base_dir)
    }
    
    /// Ensure the worktree base directory exists
    fn ensure_worktree_base_dir(&self) -> Result<()> {
        let base_dir = self.get_worktree_base_dir();
        if !base_dir.exists() {
            std::fs::create_dir_all(&base_dir)
                .context("Failed to create worktree base directory")?;
        }
        Ok(())
    }
    
    /// Get the path for a specific issue worktree
    fn get_worktree_path(&self, issue_name: &str) -> PathBuf {
        self.get_worktree_base_dir().join(format!("issue-{}", issue_name))
    }
}
```

## Testing
1. Add unit tests for config loading with new fields
2. Add tests for directory utility methods
3. Verify environment variable override works correctly

## Context
This step establishes the foundation for worktree operations without changing any existing functionality. It only adds new configuration options and utility methods that will be used in subsequent steps.