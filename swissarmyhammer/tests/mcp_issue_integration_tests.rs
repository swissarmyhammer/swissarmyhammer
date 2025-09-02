use std::sync::Arc;
use swissarmyhammer::git::GitOperations;
use swissarmyhammer::issues::{FileSystemIssueStorage, IssueStorage};
use tempfile::TempDir;
use tokio::sync::RwLock;

// Import git2 utilities
use anyhow::Result;
use git2::{Repository, Signature};

// Performance test constants
const MAX_CREATION_TIME_SECS: u64 = 10;
const MAX_ALL_COMPLETE_TIME_MILLIS: u64 = 500;

/// Test helper to create a complete test environment
struct TestEnvironment {
    temp_dir: TempDir,
    issue_storage: Arc<RwLock<Box<dyn IssueStorage>>>,
    git_ops: Arc<tokio::sync::Mutex<Option<GitOperations>>>,
}

impl TestEnvironment {
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
        std::fs::write(path.join("README.md"), "# Test Project")?;

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
}

#[tokio::test]
async fn test_complete_issue_workflow() {
    let env = TestEnvironment::new().await;

    // Step 1: Create an issue
    let issue = env
        .issue_storage
        .write()
        .await
        .create_issue(
            "implement_feature".to_string(),
            "Implement the new authentication feature with JWT tokens".to_string(),
        )
        .await
        .unwrap();

    let issue_name = &issue.name;

    // Step 2: Check all complete (should be false)
    let issues = env
        .issue_storage
        .read()
        .await
        .list_issues_info()
        .await
        .unwrap();
    let active_issues: Vec<_> = issues.iter().filter(|i| !i.completed).collect();
    assert_eq!(active_issues.len(), 1);
    assert!(!active_issues[0].completed);

    // Step 3: Start working on the issue (test git operations)
    let git_ops = env.git_ops.lock().await;
    if let Some(git) = git_ops.as_ref() {
        let branch_name = git.create_work_branch_simple(issue.name.as_str()).unwrap();

        // Verify we're on the correct branch
        let current_branch = git.current_branch().unwrap();
        assert_eq!(current_branch, branch_name);
    }
    drop(git_ops);

    // Step 4: Update the issue with progress
    let updated_issue = env.issue_storage.write().await
        .update_issue(
            issue_name,
            format!("{}\n\nJWT authentication implementation completed. Added token generation and validation.", issue.content),
        )
        .await
        .unwrap();

    assert!(updated_issue
        .content
        .contains("JWT authentication implementation completed"));

    // Step 5: Mark issue as complete
    let _completed_issue = env
        .issue_storage
        .write()
        .await
        .complete_issue(issue_name)
        .await
        .unwrap();

    // Verify the issue is now completed by getting its extended info
    let completed_issue_info = env
        .issue_storage
        .read()
        .await
        .get_issue_info(issue_name)
        .await
        .unwrap();
    assert!(completed_issue_info.completed);

    // Step 6: Check all complete (should be true now)
    let issues = env
        .issue_storage
        .read()
        .await
        .list_issues_info()
        .await
        .unwrap();
    let active_issues: Vec<_> = issues.iter().filter(|i| !i.completed).collect();
    assert_eq!(active_issues.len(), 0);

    // Step 7: Merge the issue branch
    let git_ops = env.git_ops.lock().await;
    if let Some(git) = git_ops.as_ref() {
        // Merge the issue branch
        git.merge_issue_branch_auto(issue.name.as_str()).unwrap();

        // Verify we're on main
        let current_branch = git.current_branch().unwrap();
        let main_branch = git.main_branch().unwrap();
        assert_eq!(current_branch, main_branch);
    }
}

#[tokio::test]
async fn test_error_handling_scenarios() {
    let env = TestEnvironment::new().await;

    // Test creating issue with empty name (direct storage call generates ULID)
    let result = env
        .issue_storage
        .write()
        .await
        .create_issue("".to_string(), "Valid content".to_string())
        .await;
    assert!(result.is_ok());
    let issue = result.unwrap();
    // When name is empty, a ULID is generated
    assert!(!issue.name.is_empty());
    assert_eq!(issue.name.len(), 26); // ULID length

    // Test creating issue with dangerous characters in name (path traversal protection)
    let result = env
        .issue_storage
        .write()
        .await
        .create_issue(
            "../../../etc/passwd".to_string(),
            "Valid content".to_string(),
        )
        .await;
    assert!(result.is_err()); // Should fail due to invalid characters

    // Test working on non-existent issue
    let result = env
        .issue_storage
        .read()
        .await
        .get_issue("nonexistent_issue")
        .await;
    assert!(result.is_err());

    // Test marking non-existent issue complete
    let result = env
        .issue_storage
        .write()
        .await
        .complete_issue("nonexistent_issue")
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_concurrent_operations() {
    let env = TestEnvironment::new().await;

    // Create multiple issues concurrently
    let mut create_futures = Vec::new();

    for i in 1..=5 {
        let storage = env.issue_storage.clone();
        let future = async move {
            storage
                .write()
                .await
                .create_issue(format!("issue_{i}"), format!("Content for issue {i}"))
                .await
        };
        create_futures.push(future);
    }

    // Wait for all creates to complete
    let results = futures::future::join_all(create_futures).await;

    // Verify all succeeded
    for result in results {
        assert!(result.is_ok());
    }

    // Verify all issues were created
    let issues = env.issue_storage.read().await.list_issues().await.unwrap();
    assert_eq!(issues.len(), 5);
}

#[tokio::test]
async fn test_git_integration_edge_cases() {
    let env = TestEnvironment::new().await;

    // Create an issue
    let issue = env
        .issue_storage
        .write()
        .await
        .create_issue(
            "test_git_issue".to_string(),
            "Test git integration".to_string(),
        )
        .await
        .unwrap();

    // Work on the issue
    let git_ops = env.git_ops.lock().await;
    if let Some(git) = git_ops.as_ref() {
        let _branch_name = git.create_work_branch_simple(issue.name.as_str()).unwrap();
    }
    drop(git_ops);

    // Create some uncommitted changes
    std::fs::write(env.temp_dir.path().join("test.txt"), "uncommitted changes").unwrap();

    // Create another issue
    let issue2 = env
        .issue_storage
        .write()
        .await
        .create_issue(
            "another_issue".to_string(),
            "Another test issue".to_string(),
        )
        .await
        .unwrap();

    // Try to work on another issue (create_work_branch may handle uncommitted changes)
    let git_ops = env.git_ops.lock().await;
    if let Some(git) = git_ops.as_ref() {
        // Check if there are uncommitted changes
        let has_changes = git.has_uncommitted_changes().unwrap_or(false);
        assert!(has_changes);

        // The create_work_branch may succeed as it handles uncommitted changes
        let result = git.create_work_branch_simple(issue2.name.as_str());

        // We accept either success or failure here as it depends on git implementation
        let _ = result;
    }
    drop(git_ops);

    // Commit the changes using git2
    let repo = Repository::open(env.temp_dir.path()).unwrap();
    let mut index = repo.index().unwrap();
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .unwrap();
    index.write().unwrap();

    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let signature = Signature::now("Test User", "test@example.com").unwrap();
    let parent_commit = repo.head().unwrap().peel_to_commit().unwrap();

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Add test file",
        &tree,
        &[&parent_commit],
    )
    .unwrap();

    // Switch back to main branch first (required per issue 000184)
    let git_ops = env.git_ops.lock().await;
    if let Some(git) = git_ops.as_ref() {
        let main_branch = git.main_branch().unwrap();
        git.checkout_branch(&main_branch).unwrap();

        // Now working on another issue should succeed
        let result = git.create_work_branch_simple(issue2.name.as_str());
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_performance_with_many_issues() {
    let env = TestEnvironment::new().await;

    let start_time = std::time::Instant::now();

    // Create 50 issues
    for i in 1..=50 {
        let _ = env
            .issue_storage
            .write()
            .await
            .create_issue(
                format!("perf_issue_{i:03}"),
                format!("Performance test issue name {i}"),
            )
            .await
            .unwrap();
    }

    let creation_time = start_time.elapsed();

    // Check all complete (should be fast even with many issues)
    let all_complete_start = std::time::Instant::now();
    let issues = env.issue_storage.read().await.list_issues().await.unwrap();
    let all_complete_time = all_complete_start.elapsed();

    assert_eq!(issues.len(), 50);

    // Performance assertions (adjust as needed)
    assert!(creation_time < std::time::Duration::from_secs(MAX_CREATION_TIME_SECS));
    assert!(all_complete_time < std::time::Duration::from_millis(MAX_ALL_COMPLETE_TIME_MILLIS));
}

#[tokio::test]
async fn test_issue_file_structure() {
    let env = TestEnvironment::new().await;

    // Create an issue
    let issue = env
        .issue_storage
        .write()
        .await
        .create_issue(
            "test_structure".to_string(),
            "Test issue file structure".to_string(),
        )
        .await
        .unwrap();

    // Verify the issue file exists
    let issue_file = env
        .temp_dir
        .path()
        .join("issues")
        .join(format!("{}.md", issue.name));
    assert!(issue_file.exists());

    // Verify the content is correct
    let content = std::fs::read_to_string(&issue_file).unwrap();
    assert!(content.contains("Test issue file structure"));

    // Mark as complete
    let _ = env
        .issue_storage
        .write()
        .await
        .complete_issue(&issue.name)
        .await
        .unwrap();

    // Verify the issue was moved to complete directory
    let complete_file = env
        .temp_dir
        .path()
        .join("issues")
        .join("complete")
        .join(format!("{}.md", issue.name));
    assert!(complete_file.exists());
    assert!(!issue_file.exists());
}
