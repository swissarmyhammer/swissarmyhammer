use std::fs;
use std::sync::Arc;
use swissarmyhammer_issues::{FileSystemIssueStorage, IssueStorage};
use tempfile::TempDir;
use tokio::sync::RwLock;

#[tokio::test]
async fn debug_issue_completion_detection() {
    let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    let issues_dir = temp_dir.path().join("issues");

    let issue_storage = Box::new(
        FileSystemIssueStorage::new(issues_dir.clone()).expect("Failed to create issue storage"),
    );
    let issue_storage = Arc::new(RwLock::new(issue_storage as Box<dyn IssueStorage>));

    // Create the standard directory structure
    let complete_dir = issues_dir.join("complete");
    fs::create_dir_all(&complete_dir).unwrap();

    // Create a file directly in the complete directory
    let completed_issue_file = complete_dir.join("000001_direct_completed.md");
    fs::write(
        &completed_issue_file,
        "# Direct Completed Issue\n\nThis is directly in complete dir",
    )
    .unwrap();

    // Create a regular active issue
    let _active_issue = issue_storage
        .write()
        .await
        .create_issue(
            "active_issue".to_string(),
            "This is a regular active issue".to_string(),
        )
        .await
        .unwrap();

    // List all issues and debug their status
    let all_issue_infos = issue_storage.read().await.list_issues_info().await.unwrap();

    println!("Total issues found: {}", all_issue_infos.len());
    for issue_info in &all_issue_infos {
        println!(
            "Issue: name='{}', completed={}, path='{}'",
            issue_info.issue.name,
            issue_info.completed,
            issue_info.file_path.display()
        );
    }

    // Verify completion counts
    let completed_count = all_issue_infos.iter().filter(|i| i.completed).count();
    let active_count = all_issue_infos.iter().filter(|i| !i.completed).count();

    println!("Completed count: {completed_count}, Active count: {active_count}");

    // We should have 1 completed (direct in complete dir) and 1 active (regular issue)
    assert_eq!(completed_count, 1, "Should have exactly 1 completed issue");
    assert_eq!(active_count, 1, "Should have exactly 1 active issue");
}
