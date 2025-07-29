# Comprehensive Integration Tests for Worktree Workflow

## Overview
Add integration tests that verify the complete worktree workflow works correctly across MCP tools, CLI commands, and various edge cases.

## Implementation

### MCP Integration Tests (`swissarmyhammer-cli/tests/mcp_worktree_tests.rs`)

```rust
use common::{spawn_mcp_server, MCP_TEST_TIMEOUT};
use rmcp::client::ClientBuilder;

#[tokio::test]
async fn test_mcp_worktree_workflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo_path = temp_dir.path();
    
    // Initialize test repository
    initialize_test_repo(repo_path)?;
    
    // Start MCP server
    let mut server = spawn_mcp_server(repo_path)?;
    let mut client = ClientBuilder::new("stdio")?
        .stdin(server.stdin.take().unwrap())
        .stdout(server.stdout.take().unwrap())
        .build();
        
    client.initialize().await?;
    
    // Create an issue via MCP
    let create_result = client
        .call_tool(
            "issue_create",
            serde_json::json!({
                "name": "worktree-test",
                "content": "# Test Issue\n\nTesting worktree workflow"
            }),
        )
        .await?;
    assert!(create_result.content[0].text.contains("Created issue"));
    
    // Work on the issue (creates worktree)
    let work_result = client
        .call_tool(
            "issue_work",
            serde_json::json!({
                "name": "000001_worktree-test"
            }),
        )
        .await?;
    assert!(work_result.content[0].text.contains("Created worktree"));
    assert!(work_result.content[0].text.contains(".swissarmyhammer/worktrees/issue-"));
    
    // Check current issue
    let current_result = client
        .call_tool("issue_current", serde_json::json!({}))
        .await?;
    assert!(current_result.content[0].text.contains("worktree-test"));
    assert!(current_result.content[0].text.contains("Active issue worktrees"));
    
    // Mark complete
    let complete_result = client
        .call_tool(
            "issue_mark_complete",
            serde_json::json!({
                "name": "000001_worktree-test"
            }),
        )
        .await?;
    assert!(complete_result.content[0].text.contains("marked as complete"));
    
    // Merge with cleanup
    let merge_result = client
        .call_tool(
            "issue_merge",
            serde_json::json!({
                "name": "000001_worktree-test",
                "delete_branch": true
            }),
        )
        .await?;
    assert!(merge_result.content[0].text.contains("Merged"));
    assert!(merge_result.content[0].text.contains("cleaned up worktree"));
    
    // Verify worktree is gone
    let final_current = client
        .call_tool("issue_current", serde_json::json!({}))
        .await?;
    assert!(!final_current.content[0].text.contains("000001_worktree-test"));
    
    server.kill()?;
    Ok(())
}

#[tokio::test]
async fn test_mcp_multiple_worktrees() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo_path = temp_dir.path();
    initialize_test_repo(repo_path)?;
    
    let mut server = spawn_mcp_server(repo_path)?;
    let mut client = ClientBuilder::new("stdio")?
        .stdin(server.stdin.take().unwrap())
        .stdout(server.stdout.take().unwrap())
        .build();
        
    client.initialize().await?;
    
    // Create multiple issues
    for i in 1..=3 {
        client
            .call_tool(
                "issue_create",
                serde_json::json!({
                    "name": format!("feature-{}", i),
                    "content": format!("Feature {} content", i)
                }),
            )
            .await?;
    }
    
    // Create worktrees for all issues
    for i in 1..=3 {
        let result = client
            .call_tool(
                "issue_work",
                serde_json::json!({
                    "name": format!("00000{}_feature-{}", i, i)
                }),
            )
            .await?;
        assert!(result.content[0].text.contains("Created worktree"));
    }
    
    // Check current shows all worktrees
    let current_result = client
        .call_tool("issue_current", serde_json::json!({}))
        .await?;
    assert!(current_result.content[0].text.contains("feature-1"));
    assert!(current_result.content[0].text.contains("feature-2"));
    assert!(current_result.content[0].text.contains("feature-3"));
    
    server.kill()?;
    Ok(())
}
```

### CLI Integration Tests (`swissarmyhammer-cli/tests/cli_worktree_tests.rs`)

```rust
#[test]
fn test_cli_worktree_workflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo_path = temp_dir.path();
    initialize_test_repo(repo_path)?;
    
    // Create issue via CLI
    let output = Command::cargo_bin("swissarmyhammer")?
        .current_dir(repo_path)
        .args(["issue", "create", "cli-test", "--content", "Test issue"])
        .output()?;
    assert!(output.status.success());
    
    // Work on issue
    let output = Command::cargo_bin("swissarmyhammer")?
        .current_dir(repo_path)
        .args(["issue", "work", "000001_cli-test"])
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Created worktree"));
    assert!(stdout.contains("cd "));
    
    // Check current
    let output = Command::cargo_bin("swissarmyhammer")?
        .current_dir(repo_path)
        .args(["issue", "current"])
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cli-test"));
    
    // Complete and merge
    let output = Command::cargo_bin("swissarmyhammer")?
        .current_dir(repo_path)
        .args(["issue", "complete", "000001_cli-test"])
        .output()?;
    assert!(output.status.success());
    
    let output = Command::cargo_bin("swissarmyhammer")?
        .current_dir(repo_path)
        .args(["issue", "merge", "000001_cli-test", "--delete-branch"])
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Merged"));
    
    Ok(())
}

#[test]
fn test_cli_worktree_list() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo_path = temp_dir.path();
    initialize_test_repo(repo_path)?;
    
    // Create multiple issues and worktrees
    for i in 1..=3 {
        Command::cargo_bin("swissarmyhammer")?
            .current_dir(repo_path)
            .args(["issue", "create", &format!("task-{}", i)])
            .output()?;
            
        Command::cargo_bin("swissarmyhammer")?
            .current_dir(repo_path)
            .args(["issue", "work", &format!("00000{}_task-{}", i, i)])
            .output()?;
    }
    
    // List worktrees
    let output = Command::cargo_bin("swissarmyhammer")?
        .current_dir(repo_path)
        .args(["issue", "worktrees"])
        .output()?;
    assert!(output.status.success());
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("task-1"));
    assert!(stdout.contains("task-2"));
    assert!(stdout.contains("task-3"));
    assert!(stdout.contains(".swissarmyhammer/worktrees/"));
    
    Ok(())
}
```

### Edge Case Tests (`tests/worktree_edge_cases.rs`)

```rust
#[test]
fn test_worktree_with_uncommitted_changes() -> Result<()> {
    let test_repo = TestRepoWithWorktrees::new()?;
    
    // Create worktree
    let worktree_path = test_repo.create_test_issue_worktree("EDGE-001")?;
    
    // Add uncommitted changes
    std::fs::write(worktree_path.join("uncommitted.txt"), "Changes")?;
    
    // Try to merge - should fail
    let result = test_repo.git_ops.merge_issue_worktree("EDGE-001", false);
    assert!(result.is_err());
    
    Ok(())
}

#[test]
fn test_corrupted_worktree_recovery() -> Result<()> {
    let test_repo = TestRepoWithWorktrees::new()?;
    
    // Create worktree
    let worktree_path = test_repo.create_test_issue_worktree("EDGE-002")?;
    
    // Corrupt worktree by removing .git file
    let git_file = worktree_path.join(".git");
    std::fs::remove_file(git_file)?;
    
    // Try to recover
    test_repo.git_ops.recover_worktree_operation("EDGE-002")?;
    
    // Verify we can recreate
    let new_path = test_repo.git_ops.create_work_worktree("EDGE-002")?;
    assert!(new_path.exists());
    
    Ok(())
}

#[test]
fn test_concurrent_worktree_operations() -> Result<()> {
    let test_repo = TestRepoWithWorktrees::new()?;
    let git_ops = Arc::new(test_repo.git_ops);
    
    // Spawn multiple threads creating worktrees
    let mut handles = vec![];
    
    for i in 1..=5 {
        let git_ops_clone = git_ops.clone();
        let handle = std::thread::spawn(move || {
            git_ops_clone.create_work_worktree(&format!("CONCURRENT-{:03}", i))
        });
        handles.push(handle);
    }
    
    // Wait for all to complete
    for handle in handles {
        let result = handle.join().unwrap();
        assert!(result.is_ok());
    }
    
    // Verify all worktrees exist
    let worktrees = git_ops.list_issue_worktrees()?;
    assert_eq!(worktrees.len(), 5);
    
    Ok(())
}
```

## Dependencies
- Requires all previous worktree implementation steps
- Requires WORKTREE_000219 (test infrastructure)

## Testing Areas
1. Complete workflow from issue creation to merge
2. Multiple simultaneous worktrees
3. Error recovery scenarios
4. CLI and MCP consistency
5. Cross-platform behavior
6. Performance with many worktrees

## Context
This step adds comprehensive integration tests to ensure the worktree workflow works correctly end-to-end across all components and handles edge cases gracefully.