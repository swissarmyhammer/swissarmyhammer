# Git Worktree Command Foundations

## Overview
Add basic git worktree command wrappers to `GitOperations`. These foundational methods will be used by higher-level worktree operations in subsequent steps.

## Implementation

### Add Worktree Command Methods to `GitOperations` (`src/git.rs`)

```rust
impl GitOperations {
    /// List all worktrees in the repository
    pub fn list_worktrees(&self) -> Result<Vec<WorktreeInfo>> {
        let output = Command::new("git")
            .current_dir(&self.work_dir)
            .args(["worktree", "list", "--porcelain"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SwissArmyHammerError::git_command_failed(
                "worktree list",
                output.status.code().unwrap_or(-1),
                &stderr,
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let worktrees = self.parse_worktree_list(&stdout)?;
        Ok(worktrees)
    }

    /// Check if a worktree exists at the given path
    pub fn worktree_exists(&self, path: &Path) -> Result<bool> {
        let worktrees = self.list_worktrees()?;
        Ok(worktrees.iter().any(|wt| wt.path == path))
    }

    /// Add a new worktree
    pub fn add_worktree(&self, path: &Path, branch: &str) -> Result<()> {
        let output = Command::new("git")
            .current_dir(&self.work_dir)
            .args(["worktree", "add", path.to_str().unwrap(), branch])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SwissArmyHammerError::git_command_failed(
                "worktree add",
                output.status.code().unwrap_or(-1),
                &stderr,
            ));
        }

        Ok(())
    }

    /// Remove a worktree
    pub fn remove_worktree(&self, path: &Path) -> Result<()> {
        let output = Command::new("git")
            .current_dir(&self.work_dir)
            .args(["worktree", "remove", path.to_str().unwrap()])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SwissArmyHammerError::git_command_failed(
                "worktree remove",
                output.status.code().unwrap_or(-1),
                &stderr,
            ));
        }

        Ok(())
    }

    /// Parse worktree list output
    fn parse_worktree_list(&self, output: &str) -> Result<Vec<WorktreeInfo>> {
        let mut worktrees = Vec::new();
        let mut current_worktree: Option<WorktreeInfo> = None;

        for line in output.lines() {
            if let Some(path) = line.strip_prefix("worktree ") {
                if let Some(wt) = current_worktree.take() {
                    worktrees.push(wt);
                }
                current_worktree = Some(WorktreeInfo {
                    path: PathBuf::from(path),
                    branch: None,
                    commit: None,
                });
            } else if let Some(branch) = line.strip_prefix("branch ") {
                if let Some(ref mut wt) = current_worktree {
                    wt.branch = Some(branch.to_string());
                }
            } else if let Some(commit) = line.strip_prefix("HEAD ") {
                if let Some(ref mut wt) = current_worktree {
                    wt.commit = Some(commit.to_string());
                }
            }
        }

        if let Some(wt) = current_worktree {
            worktrees.push(wt);
        }

        Ok(worktrees)
    }
}
```

### Add WorktreeInfo Struct

```rust
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub commit: Option<String>,
}
```

## Dependencies
- Requires WORKTREE_000209 (config and base infrastructure)

## Testing
1. Add unit tests for command execution
2. Add tests for worktree list parsing
3. Test error handling for invalid operations

## Context
This step adds the low-level git worktree commands that will be used by the higher-level issue workflow operations. It doesn't change any existing functionality yet.