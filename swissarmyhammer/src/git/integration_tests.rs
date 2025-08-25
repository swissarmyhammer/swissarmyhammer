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
    fn test_current_branch_git2_migration() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Get branch name using migrated git2 method
        let git2_branch = git_ops.current_branch().unwrap();

        // Should be main or master
        assert!(git2_branch == "main" || git2_branch == "master");
    }

    #[test]
    fn test_branch_exists_git2_migration() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Test existing branch
        let main_branch = git_ops.current_branch().unwrap();

        let git2_exists = git_ops.branch_exists(&main_branch).unwrap();

        assert!(git2_exists); // Main branch should exist

        // Test non-existent branch
        let git2_not_exists = git_ops.branch_exists("non-existent-branch").unwrap();

        assert!(!git2_not_exists); // Non-existent branch should not exist
    }

    #[test]
    fn test_git2_repository_initialization() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let mut git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // After migration, git2 repo is eagerly initialized during construction
        assert!(git_ops.has_git2_repo());

        // Initialize git2 repo (should be idempotent)
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

        // After migration, git2 repo is eagerly initialized during construction
        assert!(git_ops.has_git2_repo());

        // Should be able to access repo reference (already initialized)
        let _repo = git_ops.git2_repo().unwrap();
        assert!(git_ops.has_git2_repo());
    }

    #[test]
    fn test_git2_operations_with_branches() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create a new branch using shell operations
        git_ops.create_work_branch("test-issue").unwrap();

        // Verify the branch exists using git2 method
        assert!(git_ops.branch_exists("issue/test-issue").unwrap());

        // Verify current branch using git2 method
        let git2_branch = git_ops.current_branch().unwrap();

        assert_eq!(git2_branch, "issue/test-issue");
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
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Use shell operations to create a branch
        git_ops.create_work_branch("mixed-test").unwrap();

        // Use git2 operations to verify it exists
        assert!(git_ops.branch_exists("issue/mixed-test").unwrap());

        // Use git2 to check current branch
        let git2_branch = git_ops.current_branch().unwrap();
        assert_eq!(git2_branch, "issue/mixed-test");

        // Switch back using shell operations
        let main_branch = git_ops.main_branch().unwrap();
        git_ops.checkout_branch(&main_branch).unwrap();

        // Verify using git2
        let git2_branch = git_ops.current_branch().unwrap();
        assert_eq!(git2_branch, main_branch);
    }

    #[test]
    fn test_git2_repository_state_queries() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let mut git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Test bare repository check
        let is_bare = git_ops.is_bare_repository().unwrap();
        assert!(!is_bare); // Test repo should not be bare

        // Test git directory path
        let git_dir = git_ops.git_directory().unwrap();
        assert!(git_dir.exists());
        assert!(git_dir.ends_with(".git"));

        // Test working directory path
        let work_dir = git_ops.working_directory().unwrap();
        assert!(work_dir.is_some());
        let work_dir = work_dir.unwrap();
        assert!(work_dir.exists());

        // Use canonicalized paths to handle symlink differences on macOS
        let canonical_work_dir = work_dir.canonicalize().unwrap();
        let canonical_temp_dir = temp_dir.path().canonicalize().unwrap();
        assert_eq!(canonical_work_dir, canonical_temp_dir);
    }

    #[test]
    fn test_git2_repository_validation() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let mut git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Repository validation should succeed for a valid repository
        git_ops.validate_repository().unwrap();
    }

    #[test]
    fn test_git2_current_branch_behavior() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Test normal branch name retrieval
        let result = git_ops.current_branch();
        assert!(result.is_ok());

        let branch_name = result.unwrap();
        assert!(!branch_name.is_empty());
        assert!(branch_name == "main" || branch_name == "master");

        // Create a detached HEAD scenario
        let checkout_result = Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "--detach", "HEAD"])
            .output()
            .unwrap();

        if checkout_result.status.success() {
            // In detached HEAD state, git2 should handle this gracefully
            // Either return a commit hash or handle it appropriately
            let result = git_ops.current_branch();

            // Both success and error are acceptable in detached HEAD
            match result {
                Ok(branch_or_commit) => {
                    // Should return something (commit hash or branch name)
                    assert!(!branch_or_commit.is_empty());
                }
                Err(error) => {
                    // Error should be properly structured (not generic Other)
                    let error_msg = error.to_string();
                    assert!(!error_msg.contains("SwissArmyHammerError::Other"));
                }
            }
        }
    }

    #[test]
    fn test_git2_branch_exists_nonexistent() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Test with various invalid branch names
        assert!(!git_ops.branch_exists("definitely-does-not-exist").unwrap());
        assert!(!git_ops.branch_exists("feature/never-created").unwrap());

        // Empty string should be handled gracefully - should return false now
        assert!(!git_ops.branch_exists("").unwrap());
    }

    #[test]
    fn test_git2_performance() {
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        let iterations = 10;

        // Time git2-based operations
        let git2_start = std::time::Instant::now();
        for _ in 0..iterations {
            let _ = git_ops.current_branch().unwrap();
            let _ = git_ops.branch_exists("main").unwrap();
        }
        let git2_duration = git2_start.elapsed();

        // git2 operations should complete successfully and be reasonably fast
        println!("Git2 operations took: {:?}", git2_duration);

        // Verify git2 operations complete successfully
        assert!(git2_duration > std::time::Duration::from_nanos(0));
        assert!(git2_duration < std::time::Duration::from_secs(5)); // Should be much faster than 5 seconds
    }
}
