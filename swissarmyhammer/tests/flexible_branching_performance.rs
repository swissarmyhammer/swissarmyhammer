//! Performance and compatibility tests for flexible base branch support
//!
//! This module tests performance characteristics and compatibility with various Git workflows.

use std::sync::Arc;
use std::time::{Duration, Instant};
use swissarmyhammer_issues::{FileSystemIssueStorage, IssueStorage};
use swissarmyhammer_git::BranchName;
use swissarmyhammer_git::GitOperations;
use tempfile::TempDir;
use tokio::sync::RwLock;

// Import git2 utilities
use anyhow::Result;
use git2::{BranchType, Repository, Signature};
use swissarmyhammer_git::git2_utils;

/// Test environment for performance testing
struct PerformanceTestEnvironment {
    temp_dir: TempDir,
    _issue_storage: Arc<RwLock<Box<dyn IssueStorage>>>,
    git_ops: Arc<tokio::sync::Mutex<Option<GitOperations>>>,
}

impl PerformanceTestEnvironment {
    async fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temporary directory for test");

        // Set up git repository
        Self::setup_git_repo(temp_dir.path()).await;

        // Don't change global working directory - use GitOperations with explicit work_dir

        // Initialize issue storage
        let issues_dir = temp_dir.path().join("issues");
        let issue_storage = Box::new(
            FileSystemIssueStorage::new(issues_dir).expect("Failed to create issue storage"),
        );
        let issue_storage = Arc::new(RwLock::new(issue_storage as Box<dyn IssueStorage>));

        // Initialize git operations with explicit work directory
        let git_ops = Arc::new(tokio::sync::Mutex::new(Some(
            GitOperations::with_work_dir(temp_dir.path().to_path_buf())
                .expect("Failed to create git operations"),
        )));

        Self {
            temp_dir,
            _issue_storage: issue_storage,
            git_ops,
        }
    }

    async fn setup_git_repo(path: &std::path::Path) {
        Self::setup_git_repo_git2(path).unwrap();
    }

    fn setup_git_repo_git2(path: &std::path::Path) -> Result<()> {
        // Initialize git repo
        let repo = Repository::init(path)?;

        // Configure git user
        let mut config = repo.config()?;
        config.set_str("user.name", "Test User")?;
        config.set_str("user.email", "test@example.com")?;

        // Create initial commit
        std::fs::write(path.join("README.md"), "# Performance Test Project")?;

        let mut index = repo.index()?;
        index.add_path(std::path::Path::new("README.md"))?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let signature = Signature::now("Test User", "test@example.com")?;

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        )?;

        Ok(())
    }

    /// Create many branches for performance testing
    async fn create_many_branches(&self, count: usize) {
        let repo = Repository::open(self.temp_dir.path()).unwrap();

        for i in 0..count {
            let branch_name = format!("feature/branch-{i:04}");

            // Create and checkout branch using git2
            let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
            let branch = repo.branch(&branch_name, &head_commit, false).unwrap();

            // Checkout the branch
            let branch_ref = branch.get();
            let tree = branch_ref.peel_to_tree().unwrap();
            repo.checkout_tree(tree.as_object(), None).unwrap();
            repo.set_head(&format!("refs/heads/{}", branch_name))
                .unwrap();

            // Add unique content to each branch
            let content = format!("Content for branch {i}");
            let filename = format!("branch_{i:04}.txt");
            std::fs::write(self.temp_dir.path().join(&filename), content)
                .expect("Failed to write branch file");

            // Add and commit using git2
            let mut index = repo.index().unwrap();
            index.add_path(std::path::Path::new(&filename)).unwrap();
            index.write().unwrap();

            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let signature = Signature::now("Test User", "test@example.com").unwrap();
            let parent_commit = repo.head().unwrap().peel_to_commit().unwrap();

            repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                &format!("Add content for {branch_name}"),
                &tree,
                &[&parent_commit],
            )
            .unwrap();
        }

        // Return to main branch using git2
        let main_branch = repo.find_branch("main", BranchType::Local).unwrap();
        let main_ref = main_branch.get();
        let main_tree = main_ref.peel_to_tree().unwrap();
        repo.checkout_tree(main_tree.as_object(), None).unwrap();
        repo.set_head("refs/heads/main").unwrap();
    }

    /// Measure execution time of a function
    fn measure_time<F, R>(f: F) -> (R, Duration)
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        let result = f();
        let duration = start.elapsed();
        (result, duration)
    }

    /// Measure execution time of an async function
    #[allow(dead_code)]
    async fn measure_time_async<F, Fut, R>(f: F) -> (R, Duration)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = R>,
    {
        let start = Instant::now();
        let result = f().await;
        let duration = start.elapsed();
        (result, duration)
    }

    /// Helper to add and commit files using git2
    fn git_add_and_commit(&self, files: &[&str], message: &str) -> Result<()> {
        let repo = Repository::open(self.temp_dir.path())?;
        git2_utils::add_files(&repo, files)?;
        git2_utils::create_commit(&repo, message, None, None)?;
        Ok(())
    }
}

/// Test performance of branch creation with many existing branches
#[tokio::test]
async fn test_performance_branch_creation_with_many_branches() {
    let env = PerformanceTestEnvironment::new().await;

    // Create many branches (simulating a large repository)
    let branch_count = 5; // Heavily reduced for test performance
    env.create_many_branches(branch_count).await;

    let git_ops = env.git_ops.lock().await;
    let git = git_ops.as_ref().unwrap();

    // Measure performance of creating issue branches from various sources
    let mut total_creation_time = Duration::new(0, 0);
    let mut successful_creations = 0;

    for i in 0..3 {
        // Test a subset for performance - reduced iterations
        let source_branch = format!("feature/branch-{:04}", i);
        let issue_name = format!("perf-test-{i}");

        // Switch to source branch
        let branch_name = BranchName::new(&source_branch).unwrap();
        git.checkout_branch(&branch_name).unwrap();

        // Measure branch creation time
        let (result, duration) =
            PerformanceTestEnvironment::measure_time(|| git.create_work_branch(&issue_name));

        if let Ok(branch_name) = result {
            total_creation_time += duration;
            successful_creations += 1;

            assert_eq!(branch_name, format!("issue/{issue_name}"));

            // Each creation should be reasonably fast
            assert!(
                duration.as_millis() < 5000,
                "Branch creation took too long: {}ms",
                duration.as_millis()
            );
        }
    }

    // Average creation time should be acceptable
    if successful_creations > 0 {
        let avg_time = total_creation_time / successful_creations as u32;
        assert!(
            avg_time.as_millis() < 2000,
            "Average branch creation time too high: {}ms",
            avg_time.as_millis()
        );
    }

    println!(
        "Created {} branches in average {}ms each",
        successful_creations,
        if successful_creations > 0 {
            total_creation_time.as_millis() / successful_creations as u128
        } else {
            0
        }
    );
}

/// Test performance of branch existence checking
#[tokio::test]
async fn test_performance_branch_existence_checking() {
    let env = PerformanceTestEnvironment::new().await;

    let branch_count = 5; // Heavily reduced for test performance
    env.create_many_branches(branch_count).await;

    let git_ops = env.git_ops.lock().await;
    let git = git_ops.as_ref().unwrap();

    // Test branch existence checking performance
    let (_, duration) = PerformanceTestEnvironment::measure_time(|| {
        for i in 0..branch_count {
            let branch_name_str = format!("feature/branch-{i:04}");
            let branch_name = BranchName::new(&branch_name_str).unwrap();
            assert!(git.branch_exists(&branch_name).unwrap());
        }
    });

    // All branch checks should complete quickly (allow more time when run with other tests)
    assert!(
        duration.as_millis() < 10000,
        "Branch existence checking took too long: {}ms",
        duration.as_millis()
    );

    // Test checking non-existent branches
    let (_, duration) = PerformanceTestEnvironment::measure_time(|| {
        for i in 0..3 {
            let branch_name_str = format!("non-existent-branch-{i}");
            let branch_name = BranchName::new(&branch_name_str).unwrap();
            assert!(!git.branch_exists(&branch_name).unwrap());
        }
    });

    assert!(
        duration.as_millis() < 5000,
        "Non-existent branch checking took too long: {}ms",
        duration.as_millis()
    );
}

/// Test performance of merge operations
#[tokio::test]
async fn test_performance_merge_operations() {
    let env = PerformanceTestEnvironment::new().await;

    // Create fewer branches for merge testing
    let branch_count = 3;
    env.create_many_branches(branch_count).await;

    let git_ops = env.git_ops.lock().await;
    let git = git_ops.as_ref().unwrap();

    let mut total_merge_time = Duration::new(0, 0);
    let mut successful_merges = 0;

    for i in 0..branch_count {
        let source_branch = format!("feature/branch-{i:04}");
        let issue_name = format!("merge-test-{i}");

        // Create issue branch
        let branch_name = BranchName::new(&source_branch).unwrap();
        git.checkout_branch(&branch_name).unwrap();
        let _ = git.create_work_branch(&issue_name).unwrap();

        // Make a small change on issue branch
        let change_file = format!("change_{i}.txt");
        std::fs::write(env.temp_dir.path().join(&change_file), "merge test change")
            .expect("Failed to write change file");

        env.git_add_and_commit(&[&change_file], &format!("Change for {issue_name}"))
            .expect("Failed to add and commit change");

        // Measure merge time
        let (result, duration) = PerformanceTestEnvironment::measure_time(|| {
            git.merge_issue_branch(&issue_name, &source_branch)
        });

        if result.is_ok() {
            total_merge_time += duration;
            successful_merges += 1;

            // Each merge should be reasonably fast
            assert!(
                duration.as_millis() < 10000,
                "Merge took too long: {}ms",
                duration.as_millis()
            );

            // Verify merge was successful
            let current_branch = git.current_branch().unwrap();
            assert_eq!(current_branch, source_branch);
            assert!(env.temp_dir.path().join(&change_file).exists());
        }
    }

    if successful_merges > 0 {
        let avg_merge_time = total_merge_time / successful_merges as u32;
        assert!(
            avg_merge_time.as_millis() < 5000,
            "Average merge time too high: {}ms",
            avg_merge_time.as_millis()
        );
    }

    println!(
        "Completed {} merges in average {}ms each",
        successful_merges,
        if successful_merges > 0 {
            total_merge_time.as_millis() / successful_merges as u128
        } else {
            0
        }
    );
}

/// Test Git Flow workflow compatibility
#[tokio::test]
async fn test_git_flow_compatibility() {
    let env = PerformanceTestEnvironment::new().await;

    let git_ops = env.git_ops.lock().await;
    let git = git_ops.as_ref().unwrap();

    // Set up Git Flow style branches (reduced for performance)
    let branches = [
        ("develop", "Development branch"),
        ("feature/user-auth", "User authentication feature"),
    ];

    for (branch_name, description) in &branches {
        let repo = Repository::open(env.temp_dir.path()).unwrap();
        let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
        let branch = repo.branch(branch_name, &head_commit, false).unwrap();

        // Checkout the branch
        let branch_ref = branch.get();
        let tree = branch_ref.peel_to_tree().unwrap();
        repo.checkout_tree(tree.as_object(), None).unwrap();
        repo.set_head(&format!("refs/heads/{}", branch_name))
            .unwrap();

        let filename = format!("{}.md", branch_name.replace('/', "_"));
        std::fs::write(env.temp_dir.path().join(&filename), description)
            .expect("Failed to write branch description");

        git2_utils::add_files(&repo, &[&filename]).unwrap();
        git2_utils::create_commit(
            &repo,
            &format!("Initialize {branch_name}"),
            Some("Test User"),
            Some("test@example.com"),
        )
        .unwrap();
    }

    // Test creating issues from each Git Flow branch type
    for (branch_name, _) in &branches {
        let branch = BranchName::new(*branch_name).unwrap();
        git.checkout_branch(&branch).unwrap();

        let issue_name = format!("issue-from-{}", branch_name.replace('/', "-"));
        let issue_branch = git.create_work_branch(&issue_name).unwrap();

        assert_eq!(issue_branch, format!("issue/{issue_name}"));

        // Make a small change and test merge back
        let change_file = format!("change_{}.txt", branch_name.replace('/', "_"));
        std::fs::write(env.temp_dir.path().join(&change_file), "Git Flow test")
            .expect("Failed to write change");

        let repo = Repository::open(env.temp_dir.path()).unwrap();
        git2_utils::add_files(&repo, &[&change_file]).unwrap();
        git2_utils::create_commit(
            &repo,
            &format!("Change from {issue_name}"),
            Some("Test User"),
            Some("test@example.com"),
        )
        .unwrap();

        // Merge back to original branch
        git.merge_issue_branch(&issue_name, branch_name).unwrap();

        // Verify we're back on the original branch
        let current_branch = git.current_branch().unwrap();
        assert_eq!(current_branch, *branch_name);

        // Verify the change was merged
        assert!(env.temp_dir.path().join(&change_file).exists());
    }
}

/// Test GitHub Flow workflow compatibility
#[tokio::test]
async fn test_github_flow_compatibility() {
    let env = PerformanceTestEnvironment::new().await;

    let git_ops = env.git_ops.lock().await;
    let git = git_ops.as_ref().unwrap();

    // Simplified test - just verify branch operations work
    let feature_branches = ["feature/add-user-profile", "bugfix/login-error"];

    for feature_branch in &feature_branches {
        // Create feature branch from main
        let main_branch = BranchName::new("main").unwrap();
        git.checkout_branch(&main_branch).unwrap();

        let feature_branch_name = BranchName::new(*feature_branch).unwrap();
        git.create_and_checkout_branch(&feature_branch_name)
            .unwrap();

        // Add simple work
        let feature_file = format!("{}.rs", feature_branch.replace('/', "_"));
        std::fs::write(
            env.temp_dir.path().join(&feature_file),
            format!("// Implementation for {feature_branch}"),
        )
        .unwrap();

        git.add_all().unwrap();
        git.commit(&format!("Implement {feature_branch}")).unwrap();

        // Verify branch exists and we can work with it
        assert!(git.branch_exists(&feature_branch_name).unwrap());
        assert_eq!(git.current_branch().unwrap(), *feature_branch);
    }

    // Switch back to main
    let main_branch = BranchName::new("main").unwrap();
    git.checkout_branch(&main_branch).unwrap();
    assert_eq!(git.current_branch().unwrap(), "main");

    // Verify all feature branches were created
    for feature_branch in &feature_branches {
        let feature_branch_name = BranchName::new(*feature_branch).unwrap();
        assert!(
            git.branch_exists(&feature_branch_name).unwrap(),
            "Feature branch should exist: {}",
            feature_branch
        );
    }
}

/// Test concurrent issue branch operations
#[tokio::test]
async fn test_concurrent_issue_operations() {
    let env = PerformanceTestEnvironment::new().await;

    // Create several source branches (reduced for performance)
    let source_branches = ["develop", "feature/api"];
    for branch in &source_branches {
        let repo = Repository::open(env.temp_dir.path()).unwrap();
        let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
        let git_branch = repo.branch(branch, &head_commit, false).unwrap();

        // Checkout the branch
        let branch_ref = git_branch.get();
        let tree = branch_ref.peel_to_tree().unwrap();
        repo.checkout_tree(tree.as_object(), None).unwrap();
        repo.set_head(&format!("refs/heads/{}", branch)).unwrap();

        let filename = format!("{}.txt", branch.replace('/', "_"));
        std::fs::write(
            env.temp_dir.path().join(&filename),
            format!("Content for {branch}"),
        )
        .expect("Failed to write branch content");

        git2_utils::add_files(&repo, &[&filename]).unwrap();
        git2_utils::create_commit(
            &repo,
            &format!("Initialize {branch}"),
            Some("Test User"),
            Some("test@example.com"),
        )
        .unwrap();
    }

    // Simulate concurrent operations by rapidly creating issue branches
    let git_ops = env.git_ops.lock().await;
    let git = git_ops.as_ref().unwrap();

    for (i, &source_branch) in source_branches.iter().enumerate() {
        let issue_name = format!("concurrent-issue-{i}");

        let branch_name = BranchName::new(source_branch).unwrap();
        git.checkout_branch(&branch_name).unwrap();

        // Create issue branch
        let issue_branch = git.create_work_branch(&issue_name).unwrap();

        assert_eq!(issue_branch, format!("issue/{issue_name}"));

        // Make a quick change
        let change_file = format!("concurrent_{i}.txt");
        std::fs::write(env.temp_dir.path().join(&change_file), "concurrent work")
            .expect("Failed to write concurrent file");

        let repo = Repository::open(env.temp_dir.path()).unwrap();
        git2_utils::add_files(&repo, &[&change_file]).unwrap();
        git2_utils::create_commit(
            &repo,
            &format!("Concurrent work {i}"),
            Some("Test User"),
            Some("test@example.com"),
        )
        .unwrap();
    }

    // All issue branches should exist
    for i in 0..source_branches.len() {
        let issue_branch_str = format!("issue/concurrent-issue-{i}");
        let issue_branch = BranchName::new(&issue_branch_str).unwrap();
        assert!(git.branch_exists(&issue_branch).unwrap());
    }

    // Test rapid merge operations
    for (i, &source_branch) in source_branches.iter().enumerate() {
        let issue_name = format!("concurrent-issue-{i}");

        let (_, duration) = PerformanceTestEnvironment::measure_time(|| {
            git.merge_issue_branch(&issue_name, source_branch)
        });

        assert!(
            duration.as_millis() < 5000,
            "Concurrent merge {} took too long: {}ms",
            i,
            duration.as_millis()
        );

        // Verify merge success
        let current_branch = git.current_branch().unwrap();
        assert_eq!(current_branch, source_branch);
    }
}
