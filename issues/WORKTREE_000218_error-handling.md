# Robust Error Handling for Worktree Operations

## Overview
Add comprehensive error handling for worktree operations to ensure graceful failure recovery and clear user feedback when things go wrong.

## Implementation

### Enhanced Error Types (`src/error.rs`)

Add worktree-specific error variants:

```rust
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SwissArmyHammerError {
    // ... existing variants ...
    
    #[error("Worktree operation failed: {0}")]
    WorktreeError(String),
    
    #[error("Cannot perform operation from within worktree: {0}")]
    WorktreeLocationError(String),
    
    #[error("Worktree '{name}' already exists at {path}")]
    WorktreeExists { name: String, path: String },
    
    #[error("Worktree has uncommitted changes: {0}")]
    WorktreeHasChanges(String),
}
```

### Worktree Error Recovery (`src/git.rs`)

Add recovery mechanisms for common failure scenarios:

```rust
impl GitOperations {
    /// Recover from failed worktree operations
    pub fn recover_worktree_operation(&self, issue_name: &str) -> Result<()> {
        let worktree_path = self.get_worktree_path(issue_name);
        
        // Check if worktree is in a bad state
        match self.diagnose_worktree_issue(&worktree_path) {
            WorktreeIssue::Locked => {
                self.unlock_worktree(&worktree_path)?;
            }
            WorktreeIssue::Corrupted => {
                self.force_remove_worktree(&worktree_path)?;
            }
            WorktreeIssue::OrphanedDirectory => {
                self.cleanup_orphaned_directory(&worktree_path)?;
            }
            WorktreeIssue::None => {
                // No issues found
            }
        }
        
        Ok(())
    }
    
    /// Diagnose worktree issues
    fn diagnose_worktree_issue(&self, path: &Path) -> WorktreeIssue {
        // Check for lock file
        let lock_file = self.work_dir.join(".git/worktrees")
            .join(path.file_name().unwrap_or_default())
            .join("locked");
        if lock_file.exists() {
            return WorktreeIssue::Locked;
        }
        
        // Check if directory exists but not in git worktree list
        if path.exists() {
            if let Ok(worktrees) = self.list_worktrees() {
                if !worktrees.iter().any(|wt| wt.path == path) {
                    return WorktreeIssue::OrphanedDirectory;
                }
            }
        }
        
        WorktreeIssue::None
    }
    
    /// Force remove a corrupted worktree
    fn force_remove_worktree(&self, path: &Path) -> Result<()> {
        // Try git worktree remove --force
        let output = Command::new("git")
            .current_dir(&self.work_dir)
            .args(["worktree", "remove", "--force", path.to_str().unwrap()])
            .output()?;
            
        // If that fails, manually clean up
        if !output.status.success() {
            self.manual_worktree_cleanup(path)?;
        }
        
        Ok(())
    }
    
    /// Manual worktree cleanup as last resort
    fn manual_worktree_cleanup(&self, path: &Path) -> Result<()> {
        // Remove from git config
        let worktree_name = path.file_name()
            .ok_or_else(|| SwissArmyHammerError::WorktreeError(
                "Invalid worktree path".to_string()
            ))?;
            
        let git_worktree_dir = self.work_dir
            .join(".git/worktrees")
            .join(worktree_name);
            
        if git_worktree_dir.exists() {
            std::fs::remove_dir_all(&git_worktree_dir)
                .context("Failed to remove git worktree metadata")?;
        }
        
        // Remove physical directory
        if path.exists() {
            std::fs::remove_dir_all(path)
                .context("Failed to remove worktree directory")?;
        }
        
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
enum WorktreeIssue {
    None,
    Locked,
    Corrupted,
    OrphanedDirectory,
}
```

### User-Friendly Error Messages

Add context-aware error handling in tools:

```rust
impl WorkIssueTool {
    fn handle_worktree_error(e: SwissArmyHammerError, issue_name: &str) -> McpError {
        match e {
            SwissArmyHammerError::WorktreeExists { name, path } => {
                McpError::invalid_params(
                    format!(
                        "Worktree for issue '{}' already exists at {}. \
                         Use 'cd {}' to navigate to it.",
                        name, path, path
                    ),
                    None
                )
            }
            SwissArmyHammerError::WorktreeLocationError(msg) => {
                McpError::invalid_params(
                    format!(
                        "Cannot perform this operation from within a worktree. \
                         Please change to the main repository first: {}",
                        msg
                    ),
                    None
                )
            }
            _ => McpErrorHandler::handle_error(e, "worktree operation")
        }
    }
}
```

### Add Validation Helpers

```rust
impl GitOperations {
    /// Validate worktree name
    pub fn validate_worktree_name(name: &str) -> Result<()> {
        if name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']) {
            return Err(SwissArmyHammerError::WorktreeError(
                format!("Invalid characters in worktree name: '{}'", name)
            ));
        }
        Ok(())
    }
    
    /// Check if safe to create worktree
    pub fn check_worktree_prerequisites(&self) -> Result<()> {
        // Check git version supports worktrees
        let output = Command::new("git")
            .args(["--version"])
            .output()?;
            
        let version = String::from_utf8_lossy(&output.stdout);
        // Git 2.5+ required for worktrees
        
        // Check for uncommitted changes in main
        if self.has_uncommitted_changes()? {
            return Err(SwissArmyHammerError::WorktreeError(
                "Main repository has uncommitted changes. Please commit or stash first.".to_string()
            ));
        }
        
        Ok(())
    }
}
```

## Dependencies
- Requires previous worktree implementation steps

## Testing
1. Test recovery from locked worktrees
2. Test cleanup of corrupted worktrees
3. Test validation of worktree names
4. Test user-friendly error messages
5. Test manual cleanup fallbacks

## Context
This step adds robust error handling to ensure worktree operations can recover from various failure scenarios and provide clear guidance to users when issues occur.