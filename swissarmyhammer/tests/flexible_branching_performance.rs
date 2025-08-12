//! Performance and compatibility tests for flexible base branch support
//!
//! This module tests performance characteristics and compatibility with various Git workflows.

use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use swissarmyhammer::git::GitOperations;
use swissarmyhammer::issues::{FileSystemIssueStorage, IssueStorage};
use tempfile::TempDir;
use tokio::sync::RwLock;

/// Test environment for performance testing
struct PerformanceTestEnvironment {
    temp_dir: TempDir,
    issue_storage: Arc<RwLock<Box<dyn IssueStorage>>>,
    git_ops: Arc<tokio::sync::Mutex<Option<GitOperations>>>,
}

impl PerformanceTestEnvironment {
    async fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temporary directory for test");

        // Set up git repository
        Self::setup_git_repo(temp_dir.path()).await;

        // Change to test directory
        std::env::set_current_dir(temp_dir.path()).expect("Failed to change to test directory");

        // Initialize issue storage
        let issues_dir = temp_dir.path().join("issues");
        let issue_storage = Box::new(
            FileSystemIssueStorage::new(issues_dir).expect("Failed to create issue storage"),
        );
        let issue_storage = Arc::new(RwLock::new(issue_storage as Box<dyn IssueStorage>));

        // Initialize git operations
        let git_ops = Arc::new(tokio::sync::Mutex::new(Some(
            GitOperations::with_work_dir(temp_dir.path().to_path_buf())
                .expect("Failed to create git operations"),
        )));

        Self {
            temp_dir,
            issue_storage,
            git_ops,
        }
    }

    async fn setup_git_repo(path: &std::path::Path) {
        // Initialize git repo
        Command::new("git")
            .current_dir(path)
            .args(["init"])
            .output()
            .unwrap();

        // Configure git
        Command::new("git")
            .current_dir(path)
            .args(["config", "user.name", "Test User"])
            .output()
            .unwrap();

        Command::new("git")
            .current_dir(path)
            .args(["config", "user.email", "test@example.com"])
            .output()
            .unwrap();

        // Create initial commit
        std::fs::write(path.join("README.md"), "# Performance Test Project")
            .expect("Failed to write README.md");
        Command::new("git")
            .current_dir(path)
            .args(["add", "README.md"])
            .output()
            .unwrap();

        Command::new("git")
            .current_dir(path)
            .args(["commit", "-m", "Initial commit"])
            .output()
            .unwrap();
    }

    /// Create many branches for performance testing
    async fn create_many_branches(&self, count: usize) {
        for i in 0..count {
            let branch_name = format!("feature/branch-{:04}", i);

            Command::new("git")
                .current_dir(self.temp_dir.path())
                .args(["checkout", "-b", &branch_name])
                .output()
                .unwrap();

            // Add unique content to each branch
            let content = format!("Content for branch {}", i);
            let filename = format!("branch_{:04}.txt", i);
            std::fs::write(self.temp_dir.path().join(&filename), content)
                .expect("Failed to write branch file");

            Command::new("git")
                .current_dir(self.temp_dir.path())
                .args(["add", &filename])
                .output()
                .unwrap();

            Command::new("git")
                .current_dir(self.temp_dir.path())
                .args(["commit", "-m", &format!("Add content for {}", branch_name)])
                .output()
                .unwrap();
        }

        // Return to main branch
        Command::new("git")
            .current_dir(self.temp_dir.path())
            .args(["checkout", "main"])
            .output()
            .unwrap();
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
}

/// Test performance of branch creation with many existing branches
#[tokio::test]
async fn test_performance_branch_creation_with_many_branches() {
    let env = PerformanceTestEnvironment::new().await;

    // Create many branches (simulating a large repository)
    let branch_count = 50; // Reduced for CI performance
    env.create_many_branches(branch_count).await;

    let git_ops = env.git_ops.lock().await;
    let git = git_ops.as_ref().unwrap();

    // Measure performance of creating issue branches from various sources
    let mut total_creation_time = Duration::new(0, 0);
    let mut successful_creations = 0;

    for i in 0..10 {
        // Test a subset for performance
        let source_branch = format!("feature/branch-{:04}", i * 5);
        let issue_name = format!("perf-test-{}", i);

        // Switch to source branch
        git.checkout_branch(&source_branch).unwrap();

        // Measure branch creation time
        let (result, duration) = PerformanceTestEnvironment::measure_time(|| {
            git.create_work_branch_with_source(&issue_name, None)
        });

        if let Ok((branch_name, detected_source)) = result {
            total_creation_time += duration;
            successful_creations += 1;

            assert_eq!(branch_name, format!("issue/{}", issue_name));
            assert_eq!(detected_source, source_branch);

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

    let branch_count = 30; // Reduced for CI
    env.create_many_branches(branch_count).await;

    let git_ops = env.git_ops.lock().await;
    let git = git_ops.as_ref().unwrap();

    // Test branch existence checking performance
    let (_, duration) = PerformanceTestEnvironment::measure_time(|| {
        for i in 0..branch_count {
            let branch_name = format!("feature/branch-{:04}", i);
            assert!(git.branch_exists(&branch_name).unwrap());
        }
    });

    // All branch checks should complete quickly
    assert!(
        duration.as_millis() < 3000,
        "Branch existence checking took too long: {}ms",
        duration.as_millis()
    );

    // Test checking non-existent branches
    let (_, duration) = PerformanceTestEnvironment::measure_time(|| {
        for i in 0..10 {
            let branch_name = format!("non-existent-branch-{}", i);
            assert!(!git.branch_exists(&branch_name).unwrap());
        }
    });

    assert!(
        duration.as_millis() < 1000,
        "Non-existent branch checking took too long: {}ms",
        duration.as_millis()
    );
}

/// Test performance of merge operations
#[tokio::test]
async fn test_performance_merge_operations() {
    let env = PerformanceTestEnvironment::new().await;

    // Create fewer branches for merge testing
    let branch_count = 5;
    env.create_many_branches(branch_count).await;

    let git_ops = env.git_ops.lock().await;
    let git = git_ops.as_ref().unwrap();

    let mut total_merge_time = Duration::new(0, 0);
    let mut successful_merges = 0;

    for i in 0..branch_count {
        let source_branch = format!("feature/branch-{:04}", i);
        let issue_name = format!("merge-test-{}", i);

        // Create issue branch
        git.checkout_branch(&source_branch).unwrap();
        let (_, _) = git
            .create_work_branch_with_source(&issue_name, None)
            .unwrap();

        // Make a small change on issue branch
        let change_file = format!("change_{}.txt", i);
        std::fs::write(env.temp_dir.path().join(&change_file), "merge test change")
            .expect("Failed to write change file");

        Command::new("git")
            .current_dir(env.temp_dir.path())
            .args(["add", &change_file])
            .output()
            .unwrap();

        Command::new("git")
            .current_dir(env.temp_dir.path())
            .args(["commit", "-m", &format!("Change for {}", issue_name)])
            .output()
            .unwrap();

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

    // Set up Git Flow style branches
    let branches = [
        ("develop", "Development branch"),
        ("release/v1.0", "Release branch for version 1.0"),
        ("hotfix/security-fix", "Security hotfix"),
        ("feature/user-auth", "User authentication feature"),
    ];

    for (branch_name, description) in &branches {
        Command::new("git")
            .current_dir(env.temp_dir.path())
            .args(["checkout", "-b", branch_name])
            .output()
            .unwrap();

        let filename = format!("{}.md", branch_name.replace('/', "_"));
        std::fs::write(env.temp_dir.path().join(&filename), description)
            .expect("Failed to write branch description");

        Command::new("git")
            .current_dir(env.temp_dir.path())
            .args(["add", &filename])
            .output()
            .unwrap();

        Command::new("git")
            .current_dir(env.temp_dir.path())
            .args(["commit", "-m", &format!("Initialize {}", branch_name)])
            .output()
            .unwrap();
    }

    // Test creating issues from each Git Flow branch type
    for (branch_name, _) in &branches {
        git.checkout_branch(branch_name).unwrap();

        let issue_name = format!("issue-from-{}", branch_name.replace('/', "-"));
        let (issue_branch, source_branch) = git
            .create_work_branch_with_source(&issue_name, None)
            .unwrap();

        assert_eq!(issue_branch, format!("issue/{}", issue_name));
        assert_eq!(source_branch, *branch_name);

        // Make a small change and test merge back
        let change_file = format!("change_{}.txt", branch_name.replace('/', "_"));
        std::fs::write(env.temp_dir.path().join(&change_file), "Git Flow test")
            .expect("Failed to write change");

        Command::new("git")
            .current_dir(env.temp_dir.path())
            .args(["add", &change_file])
            .output()
            .unwrap();

        Command::new("git")
            .current_dir(env.temp_dir.path())
            .args(["commit", "-m", &format!("Change from {}", issue_name)])
            .output()
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

    // GitHub Flow: feature branches off main, merged back to main
    let feature_branches = [
        "feature/add-user-profile",
        "feature/improve-search",
        "feature/mobile-responsive",
        "bugfix/login-error",
        "enhancement/performance-boost",
    ];

    for feature_branch in &feature_branches {
        // Create feature branch from main
        git.checkout_branch("main").unwrap();
        Command::new("git")
            .current_dir(env.temp_dir.path())
            .args(["checkout", "-b", feature_branch])
            .output()
            .unwrap();

        // Add feature work
        let feature_file = format!("{}.rs", feature_branch.replace('/', "_"));
        std::fs::write(
            env.temp_dir.path().join(&feature_file),
            format!("// Implementation for {}", feature_branch),
        )
        .expect("Failed to write feature file");

        Command::new("git")
            .current_dir(env.temp_dir.path())
            .args(["add", &feature_file])
            .output()
            .unwrap();

        Command::new("git")
            .current_dir(env.temp_dir.path())
            .args(["commit", "-m", &format!("Implement {}", feature_branch)])
            .output()
            .unwrap();

        // Create issue branch for additional work on the feature
        let issue_name = format!("tests-for-{}", feature_branch.replace("feature/", ""));
        let (issue_branch, source_branch) = git
            .create_work_branch_with_source(&issue_name, None)
            .unwrap();

        assert_eq!(issue_branch, format!("issue/{}", issue_name));
        assert_eq!(source_branch, *feature_branch);

        // Add tests
        let test_file = format!("test_{}.rs", feature_branch.replace('/', "_"));
        std::fs::write(
            env.temp_dir.path().join(&test_file),
            format!("// Tests for {}", feature_branch),
        )
        .expect("Failed to write test file");

        Command::new("git")
            .current_dir(env.temp_dir.path())
            .args(["add", &test_file])
            .output()
            .unwrap();

        Command::new("git")
            .current_dir(env.temp_dir.path())
            .args(["commit", "-m", &format!("Add tests for {}", feature_branch)])
            .output()
            .unwrap();

        // Merge issue back to feature branch
        git.merge_issue_branch(&issue_name, feature_branch).unwrap();

        // Verify both files exist on feature branch
        assert!(env.temp_dir.path().join(&feature_file).exists());
        assert!(env.temp_dir.path().join(&test_file).exists());

        // In GitHub Flow, feature branch would then be merged to main via PR
        // We can simulate this by merging to main
        git.checkout_branch("main").unwrap();
        Command::new("git")
            .current_dir(env.temp_dir.path())
            .args([
                "merge",
                "--no-ff",
                feature_branch,
                "-m",
                &format!("Merge {}", feature_branch),
            ])
            .output()
            .unwrap();

        // Clean up feature branch (typical in GitHub Flow)
        Command::new("git")
            .current_dir(env.temp_dir.path())
            .args(["branch", "-d", feature_branch])
            .output()
            .unwrap();
    }

    // Verify all features were merged to main
    git.checkout_branch("main").unwrap();
    for feature_branch in &feature_branches {
        let feature_file = format!("{}.rs", feature_branch.replace('/', "_"));
        let test_file = format!("test_{}.rs", feature_branch.replace('/', "_"));

        assert!(
            env.temp_dir.path().join(&feature_file).exists(),
            "Feature file missing for {}",
            feature_branch
        );
        assert!(
            env.temp_dir.path().join(&test_file).exists(),
            "Test file missing for {}",
            feature_branch
        );
    }
}

/// Test concurrent issue branch operations
#[tokio::test]
async fn test_concurrent_issue_operations() {
    let env = PerformanceTestEnvironment::new().await;

    // Create several source branches
    let source_branches = ["develop", "feature/api", "feature/ui"];
    for branch in &source_branches {
        Command::new("git")
            .current_dir(env.temp_dir.path())
            .args(["checkout", "-b", branch])
            .output()
            .unwrap();

        std::fs::write(
            env.temp_dir
                .path()
                .join(&format!("{}.txt", branch.replace('/', "_"))),
            format!("Content for {}", branch),
        )
        .expect("Failed to write branch content");

        Command::new("git")
            .current_dir(env.temp_dir.path())
            .args(["add", "."])
            .output()
            .unwrap();

        Command::new("git")
            .current_dir(env.temp_dir.path())
            .args(["commit", "-m", &format!("Initialize {}", branch)])
            .output()
            .unwrap();
    }

    // Simulate concurrent operations by rapidly creating issue branches
    let git_ops = env.git_ops.lock().await;
    let git = git_ops.as_ref().unwrap();

    for (i, &source_branch) in source_branches.iter().enumerate() {
        let issue_name = format!("concurrent-issue-{}", i);

        git.checkout_branch(source_branch).unwrap();

        // Create issue branch
        let (issue_branch, detected_source) = git
            .create_work_branch_with_source(&issue_name, None)
            .unwrap();

        assert_eq!(issue_branch, format!("issue/{}", issue_name));
        assert_eq!(detected_source, source_branch);

        // Make a quick change
        let change_file = format!("concurrent_{}.txt", i);
        std::fs::write(env.temp_dir.path().join(&change_file), "concurrent work")
            .expect("Failed to write concurrent file");

        Command::new("git")
            .current_dir(env.temp_dir.path())
            .args(["add", &change_file])
            .output()
            .unwrap();

        Command::new("git")
            .current_dir(env.temp_dir.path())
            .args(["commit", "-m", &format!("Concurrent work {}", i)])
            .output()
            .unwrap();
    }

    // All issue branches should exist
    for i in 0..source_branches.len() {
        let issue_branch = format!("issue/concurrent-issue-{}", i);
        assert!(git.branch_exists(&issue_branch).unwrap());
    }

    // Test rapid merge operations
    for (i, &source_branch) in source_branches.iter().enumerate() {
        let issue_name = format!("concurrent-issue-{}", i);

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

/// Test memory usage with large number of operations
#[tokio::test]
async fn test_memory_usage_stability() {
    let env = PerformanceTestEnvironment::new().await;

    // Create moderate number of branches
    env.create_many_branches(20).await;

    let git_ops = env.git_ops.lock().await;
    let git = git_ops.as_ref().unwrap();

    // Perform many operations to test memory stability
    for cycle in 0..5 {
        for branch_num in 0..10 {
            let source_branch = format!("feature/branch-{:04}", branch_num);
            let issue_name = format!("memory-test-{}-{}", cycle, branch_num);

            // Create issue branch
            git.checkout_branch(&source_branch).unwrap();
            let (_, _) = git
                .create_work_branch_with_source(&issue_name, None)
                .unwrap();

            // Make change and merge immediately
            let change_file = format!("memory_{}_{}.txt", cycle, branch_num);
            std::fs::write(env.temp_dir.path().join(&change_file), "memory test")
                .expect("Failed to write memory test file");

            Command::new("git")
                .current_dir(env.temp_dir.path())
                .args(["add", &change_file])
                .output()
                .unwrap();

            Command::new("git")
                .current_dir(env.temp_dir.path())
                .args([
                    "commit",
                    "-m",
                    &format!("Memory test {}-{}", cycle, branch_num),
                ])
                .output()
                .unwrap();

            // Merge and clean up
            git.merge_issue_branch(&issue_name, &source_branch).unwrap();

            // Clean up issue branch
            let issue_branch = format!("issue/{}", issue_name);
            git.delete_branch(&issue_branch).unwrap();
        }
    }

    // If we get here without running out of memory or crashing, the test passes
    println!("Completed memory stability test with {} cycles", 5);
}
