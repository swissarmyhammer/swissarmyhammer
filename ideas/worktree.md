# Worktree-Based Issue Management

## Overview

This specification outlines the migration from in-place branching to git worktree-based issue management for the Swiss Army Hammer MCP tools. The goal is to improve isolation between issues and provide cleaner workspace management.

## Current Implementation

The existing issue workflow uses in-place branching:

1. `issue_work` creates/switches to branch `issue/<issue_name>` 
2. Work is done in the main repository directory
3. `issue_merge` merges the branch back to main and optionally deletes the branch

**Current Git Operations** (from `git.rs:120-267`):
- `create_work_branch()` - Creates `issue/<issue_name>` branch from main
- `merge_issue_branch()` - Merges branch back to main with `--no-ff`
- `delete_branch()` - Removes branch after merge

## Proposed Worktree Implementation

### Directory Structure

```
project-root/
├── .swissarmyhammer/
│   └── worktrees/
│       ├── issue-<issue_name>/  # Worktree for each issue
│       └── ...
├── .git/
└── ... (main repo files)
```

### New Workflow Actions

1. ** Restore Current Working Directory**

When swissarmyhammer starts, it needs to record its current working directory for later use. 
Restoring the current working directory sets it back to this initial value.

We need to use this in implement.md adding a step before are_issues_complete.

### New Git Operations

1. **Create Worktree** (`create_work_worktree`)
   - Create branch `issue/<issue_name>` from main
   - Create worktree at `.swissarmyhammer/worktrees/issue-<issue_name>/`
   - Link worktree to the new branch

2. **Merge Worktree** (`merge_issue_worktree`)
   - Switch to main branch in main repo
   - Merge branch `issue/<issue_name>` to main
   - Remove worktree directory
   - Delete branch

3. **Use Worktree** (`use_worktree`)
    - Switch the current working directory to a specified worktree

### Implementation Changes

#### New Git Commands

```rust
impl GitOperations {
    pub fn create_work_worktree(&self, issue_name: &str) -> Result<String> {
        let branch_name = format!("issue/{issue_name}");
        let worktree_path = self.get_worktree_path(issue_name);
        
        // Ensure worktree directory exists
        self.ensure_worktree_base_dir()?;
        
        // Create branch and worktree
        if !self.branch_exists(&branch_name)? {
            self.create_branch_and_worktree(&branch_name, &worktree_path)?;
        } else {
            self.create_worktree_for_existing_branch(&branch_name, &worktree_path)?;
        }
        
        Ok(worktree_path)
    }
    
    pub fn merge_issue_worktree(&self, issue_name: &str) -> Result<()> {
        let branch_name = format!("issue/{issue_name}");
        let worktree_path = self.get_worktree_path(issue_name);
        
        // Merge branch to main
        self.merge_issue_branch(&branch_name)?;
        
        // Remove worktree
        self.remove_worktree(&worktree_path)?;
        
        Ok(())
    }
    
    pub fn get_current_worktree(&self) -> Result<Option<String>> {
        // List all worktrees and find the first one in .swissarmyhammer/worktrees/
        let output = Command::new("git")
            .current_dir(&self.work_dir)
            .args(["worktree", "list", "--porcelain"])
            .output()?;
            
        if !output.status.success() {
            return Ok(None);
        }
        
        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            if let Some(path) = line.strip_prefix("worktree ") {
                if path.contains("/.swissarmyhammer/worktrees/issue-") {
                    // Extract issue name from path
                    if let Some(issue_name) = path.split("issue-").nth(1) {
                        return Ok(Some(issue_name.to_string()));
                    }
                }
            }
        }
        
        Ok(None)
    }
    
    fn get_worktree_path(&self, issue_name: &str) -> String {
        format!("{}/.swissarmyhammer/worktrees/issue-{}", 
                self.work_dir.display(), issue_name)
    }
}
```

#### MCP Tool Updates

**WorkIssueTool** (`work/mod.rs:48-78`):
- Replace `create_work_branch()` call with `create_work_worktree()`
- Return worktree path (new) and branch name (existing)
- Set the current working directory to the worktree
- Update success message to indicate worktree location

**MergeIssueTool** (`merge/mod.rs:58-143`):
- Replace `merge_issue_branch()` call with `merge_issue_worktree()`
- Add worktree cleanup to success message
- Handle worktree removal errors gracefully

**CurrentIssueTool** (`current/mod.rs`):
- Replace branch detection logic with `get_current_worktree()`
- Return the first active worktree instead of current branch
- Update response to show worktree path and issue name


### Benefits

1. **Isolation**: Each issue has its own complete workspace
2. **Parallel Work**: Multiple issues can be worked on simultaneously
3. **Clean State**: No need to stash/commit when switching issues
4. **Simplified Workflow**: No branch switching in main repo

### Migration Considerations

1. **Backward Compatibility**: Do nothing for backward compatibility, the app is not released yet
2. **Error Handling**: Robust cleanup of failed worktree operations
3. **Path Management**: Ensure worktree paths don't conflict
4. **Integration**: Update CLI and MCP tools consistently

### File Changes Required

1. **Core Git Operations** (`swissarmyhammer/src/git.rs`)
   - Add worktree creation/removal methods
   - Update existing branch operations
   - Add worktree path utilities

2. **MCP Tools** (`swissarmyhammer/src/mcp/tools/issues/`)
   - Update `work/mod.rs` for worktree creation
   - Update `merge/mod.rs` for worktree cleanup
   - Update `current/mod.rs` to detect active worktree instead of current branch
   - Update response messages

3. **CLI Integration** (`swissarmyhammer-cli/src/issue.rs`)
   - CLI needs to 'just work' as it is calling the MCP tools
   - Handle worktree path display

4. **Tests**
   - Update integration tests for worktree operations
   - Add worktree-specific test cases


### Configuration

Add configuration option for worktree base directory:
```rust
pub struct WorktreeConfig {
    pub base_dir: String, // Default: ".swissarmyhammer/worktrees"
    pub auto_cleanup: bool, // Default: true
}
```

This specification provides a foundation for implementing worktree-based issue management that improves workspace isolation while maintaining the existing workflow patterns.

