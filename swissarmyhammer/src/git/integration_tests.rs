//! Integration tests comparing shell-based and git2-based operations
//!
//! These tests validate that git2-based operations produce equivalent
//! results to their shell-based counterparts, ensuring compatibility
//! during the migration process.

#[cfg(test)]
mod tests {
    use super::super::GitOperations;
    use crate::test_utils::IsolatedTestEnvironment;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    // Helper to create a temporary git repository with initial commit
    fn create_test_git_repo() -> crate::Result<TempDir> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();

        // Initialize git repo
        let output = Command::new("git")
            .current_dir(repo_path)
            .args(["init"])
            .output()?;

        if !output.status.success() {
            return Err(crate::SwissArmyHammerError::Other(
                "Failed to initialize git repository".to_string(),
            ));
        }

        // Set up user config for tests
        Command::new("git")
            .current_dir(repo_path)
            .args(["config", "user.name", "Test User"])
            .output()?;

        Command::new("git")
            .current_dir(repo_path)
            .args(["config", "user.email", "test@example.com"])
            .output()?;

        // Create initial commit
        fs::write(repo_path.join("README.md"), "# Test Repository")?;

        Command::new("git")
            .current_dir(repo_path)
            .args(["add", "README.md"])
            .output()?;

        Command::new("git")
            .current_dir(repo_path)
            .args(["commit", "-m", "Initial commit"])
            .output()?;

        Ok(temp_dir)
    }

    #[test]
    fn test_current_branch_shell_vs_git2() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let mut git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Get branch name using shell method
        let shell_branch = git_ops.current_branch().unwrap();

        // Get branch name using git2 method
        let git2_branch = git_ops.current_branch_git2().unwrap();

        // They should be identical
        assert_eq!(shell_branch, git2_branch);

        // Should be main or master
        assert!(shell_branch == "main" || shell_branch == "master");
    }

    #[test]
    fn test_branch_exists_shell_vs_git2() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let mut git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Test existing branch
        let main_branch = git_ops.current_branch().unwrap();

        let shell_exists = git_ops.branch_exists(&main_branch).unwrap();
        let git2_exists = git_ops.branch_exists_git2(&main_branch).unwrap();

        assert_eq!(shell_exists, git2_exists);
        assert!(shell_exists); // Main branch should exist

        // Test non-existent branch
        let shell_not_exists = git_ops.branch_exists("non-existent-branch").unwrap();
        let git2_not_exists = git_ops.branch_exists_git2("non-existent-branch").unwrap();

        assert_eq!(shell_not_exists, git2_not_exists);
        assert!(!shell_not_exists); // Non-existent branch should not exist
    }

    #[test]
    fn test_git2_repository_initialization() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let mut git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Initially git2 repo should not be initialized
        assert!(!git_ops.has_git2_repo());

        // Initialize git2 repo
        git_ops.init_git2().unwrap();
        assert!(git_ops.has_git2_repo());

        // Should be able to access repo reference
        let _repo = git_ops.git2_repo().unwrap();
    }

    #[test]
    fn test_git2_repo_auto_initialization() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let mut git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Initially git2 repo should not be initialized
        assert!(!git_ops.has_git2_repo());

        // Calling git2_repo() should auto-initialize
        let _repo = git_ops.git2_repo().unwrap();
        assert!(git_ops.has_git2_repo());
    }

    #[test]
    fn test_git2_operations_with_branches() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let mut git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create a new branch using shell operations
        git_ops.create_work_branch("test-issue").unwrap();

        // Verify the branch exists using both methods
        assert!(git_ops.branch_exists("issue/test-issue").unwrap());
        assert!(git_ops.branch_exists_git2("issue/test-issue").unwrap());

        // Verify current branch using both methods
        let shell_branch = git_ops.current_branch().unwrap();
        let git2_branch = git_ops.current_branch_git2().unwrap();

        assert_eq!(shell_branch, git2_branch);
        assert_eq!(shell_branch, "issue/test-issue");
    }

    #[test]
    fn test_git2_error_handling() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = TempDir::new().unwrap();

        // Try to create GitOperations in non-git directory (should fail)
        let result = GitOperations::with_work_dir(temp_dir.path().to_path_buf());
        assert!(result.is_err());

        // Error should be about not being in a git repository
        if let Err(error) = result {
            let error_msg = error.to_string();
            assert!(
                error_msg.to_lowercase().contains("git")
                    || error_msg.to_lowercase().contains("repository")
            );
        }
    }

    #[test]
    fn test_git2_repo_persistence() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let mut git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Initialize git2 repo
        git_ops.init_git2().unwrap();

        // Multiple calls should reuse the same repository instance
        let repo1_path = git_ops.git2_repo().unwrap().path().to_path_buf();
        let repo2_path = git_ops.git2_repo().unwrap().path().to_path_buf();

        // They should point to the same repository (same path)
        assert_eq!(repo1_path, repo2_path);
    }

    #[test]
    fn test_mixed_shell_git2_operations() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let mut git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Use shell operations to create a branch
        git_ops.create_work_branch("mixed-test").unwrap();

        // Use git2 operations to verify it exists
        assert!(git_ops.branch_exists_git2("issue/mixed-test").unwrap());

        // Use git2 to check current branch
        let git2_branch = git_ops.current_branch_git2().unwrap();
        assert_eq!(git2_branch, "issue/mixed-test");

        // Switch back using shell operations
        let main_branch = git_ops.main_branch().unwrap();
        git_ops.checkout_branch(&main_branch).unwrap();

        // Verify using git2
        let git2_branch = git_ops.current_branch_git2().unwrap();
        assert_eq!(git2_branch, main_branch);
    }
}
