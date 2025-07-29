# Test Infrastructure for Worktree Operations

## Overview
Add test utilities and infrastructure to support testing worktree operations. This includes test helpers, fixtures, and mock implementations for isolated testing.

## Implementation

### Test Utilities Module (`src/test_utils.rs`)

Add worktree-specific test helpers:

```rust
#[cfg(test)]
pub mod worktree_test_utils {
    use super::*;
    use tempfile::TempDir;
    
    /// Create a test repository with worktree support
    pub struct TestRepoWithWorktrees {
        pub temp_dir: TempDir,
        pub repo_path: PathBuf,
        pub git_ops: GitOperations,
    }
    
    impl TestRepoWithWorktrees {
        pub fn new() -> Result<Self> {
            let temp_dir = TempDir::new()?;
            let repo_path = temp_dir.path().to_path_buf();
            
            // Initialize git repo
            Command::new("git")
                .current_dir(&repo_path)
                .args(["init"])
                .output()?;
                
            // Configure git
            Command::new("git")
                .current_dir(&repo_path)
                .args(["config", "user.name", "Test User"])
                .output()?;
                
            Command::new("git")
                .current_dir(&repo_path)
                .args(["config", "user.email", "test@example.com"])
                .output()?;
            
            // Create initial commit
            std::fs::write(repo_path.join("README.md"), "# Test Repo")?;
            Command::new("git")
                .current_dir(&repo_path)
                .args(["add", "."])
                .output()?;
            Command::new("git")
                .current_dir(&repo_path)
                .args(["commit", "-m", "Initial commit"])
                .output()?;
            
            let git_ops = GitOperations::new(repo_path.clone());
            
            Ok(Self {
                temp_dir,
                repo_path,
                git_ops,
            })
        }
        
        /// Create a test issue with worktree
        pub fn create_test_issue_worktree(&self, issue_name: &str) -> Result<PathBuf> {
            self.git_ops.create_work_worktree(issue_name)
        }
        
        /// Add a file to a worktree
        pub fn add_file_to_worktree(&self, issue_name: &str, filename: &str, content: &str) -> Result<()> {
            let worktree_path = self.git_ops.get_worktree_path(issue_name);
            std::fs::write(worktree_path.join(filename), content)?;
            
            // Stage and commit in worktree
            Command::new("git")
                .current_dir(&worktree_path)
                .args(["add", filename])
                .output()?;
            Command::new("git")
                .current_dir(&worktree_path)
                .args(["commit", "-m", &format!("Add {}", filename)])
                .output()?;
                
            Ok(())
        }
        
        /// Verify worktree state
        pub fn verify_worktree_exists(&self, issue_name: &str) -> bool {
            let path = self.git_ops.get_worktree_path(issue_name);
            path.exists() && self.git_ops.worktree_exists(&path).unwrap_or(false)
        }
    }
}
```

### Test Fixtures (`src/git.rs` - test module)

Add comprehensive test coverage:

```rust
#[cfg(test)]
mod worktree_tests {
    use super::*;
    use crate::test_utils::worktree_test_utils::TestRepoWithWorktrees;
    
    #[test]
    fn test_create_worktree_new_issue() -> Result<()> {
        let test_repo = TestRepoWithWorktrees::new()?;
        
        // Create worktree for new issue
        let worktree_path = test_repo.git_ops.create_work_worktree("TEST-001")?;
        
        // Verify worktree was created
        assert!(worktree_path.exists());
        assert!(test_repo.verify_worktree_exists("TEST-001"));
        
        // Verify branch was created
        assert!(test_repo.git_ops.branch_exists("issue/TEST-001")?);
        
        Ok(())
    }
    
    #[test]
    fn test_create_worktree_existing_branch() -> Result<()> {
        let test_repo = TestRepoWithWorktrees::new()?;
        
        // First create a branch manually
        Command::new("git")
            .current_dir(&test_repo.repo_path)
            .args(["branch", "issue/TEST-002"])
            .output()?;
        
        // Create worktree for existing branch
        let worktree_path = test_repo.git_ops.create_work_worktree("TEST-002")?;
        
        assert!(worktree_path.exists());
        assert!(test_repo.verify_worktree_exists("TEST-002"));
        
        Ok(())
    }
    
    #[test]
    fn test_merge_worktree_with_cleanup() -> Result<()> {
        let test_repo = TestRepoWithWorktrees::new()?;
        
        // Create and modify worktree
        test_repo.create_test_issue_worktree("TEST-003")?;
        test_repo.add_file_to_worktree("TEST-003", "feature.txt", "New feature")?;
        
        // Merge with cleanup
        test_repo.git_ops.merge_issue_worktree("TEST-003", true)?;
        
        // Verify worktree was cleaned up
        assert!(!test_repo.verify_worktree_exists("TEST-003"));
        
        // Verify branch was deleted
        assert!(!test_repo.git_ops.branch_exists("issue/TEST-003")?);
        
        // Verify changes were merged
        let feature_file = test_repo.repo_path.join("feature.txt");
        assert!(feature_file.exists());
        assert_eq!(std::fs::read_to_string(feature_file)?, "New feature");
        
        Ok(())
    }
    
    #[test]
    fn test_worktree_detection() -> Result<()> {
        let test_repo = TestRepoWithWorktrees::new()?;
        
        // Create multiple worktrees
        test_repo.create_test_issue_worktree("TEST-004")?;
        test_repo.create_test_issue_worktree("TEST-005")?;
        
        // List issue worktrees
        let worktrees = test_repo.git_ops.list_issue_worktrees()?;
        assert_eq!(worktrees.len(), 2);
        
        let issue_names: Vec<_> = worktrees.iter()
            .map(|wt| wt.issue_name.as_str())
            .collect();
        assert!(issue_names.contains(&"TEST-004"));
        assert!(issue_names.contains(&"TEST-005"));
        
        Ok(())
    }
    
    #[test]
    fn test_error_when_merging_from_worktree() -> Result<()> {
        let test_repo = TestRepoWithWorktrees::new()?;
        
        // Create worktree
        let worktree_path = test_repo.create_test_issue_worktree("TEST-006")?;
        
        // Change to worktree directory
        std::env::set_current_dir(&worktree_path)?;
        
        // Try to merge from within worktree
        let result = test_repo.git_ops.merge_issue_worktree("TEST-006", false);
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cannot merge worktree while inside it"));
        
        Ok(())
    }
}
```

### Integration Test Helpers (`tests/worktree_integration_tests.rs`)

```rust
use swissarmyhammer::test_utils::create_test_environment;

#[test]
fn test_full_worktree_workflow() -> Result<()> {
    let (temp_dir, repo_path) = create_test_environment()?;
    
    // Initialize issue storage
    let issue_storage = FileSystemIssueStorage::new(repo_path.join("issues"));
    
    // Create an issue
    issue_storage.create_issue("TEST-007", "Test issue content").await?;
    
    // Work on issue (creates worktree)
    let git_ops = GitOperations::new(repo_path.clone());
    let worktree_path = git_ops.create_work_worktree("TEST-007")?;
    
    // Make changes in worktree
    std::fs::write(worktree_path.join("implementation.rs"), "fn solution() {}")?;
    
    // Commit in worktree
    Command::new("git")
        .current_dir(&worktree_path)
        .args(["add", "."])
        .output()?;
    Command::new("git")
        .current_dir(&worktree_path)
        .args(["commit", "-m", "Implement solution"])
        .output()?;
    
    // Mark issue as complete
    issue_storage.mark_issue_complete("TEST-007").await?;
    
    // Merge and cleanup
    git_ops.merge_issue_worktree("TEST-007", true)?;
    
    // Verify end state
    assert!(!worktree_path.exists());
    assert!(repo_path.join("implementation.rs").exists());
    
    Ok(())
}
```

## Dependencies
- Requires all previous worktree implementation steps

## Testing
1. Unit tests for each worktree operation
2. Integration tests for full workflows
3. Error scenario tests
4. Concurrent worktree tests
5. Cross-platform compatibility tests

## Context
This step provides comprehensive test infrastructure to ensure worktree operations work correctly across various scenarios and edge cases.