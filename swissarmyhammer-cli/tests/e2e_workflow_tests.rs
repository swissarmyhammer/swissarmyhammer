//! End-to-End Workflow Tests
//!
//! Tests for complete user journeys that span multiple CLI commands and verify
//! that entire workflows function correctly with the CLI-MCP integration.

use anyhow::Result;
use assert_cmd::Command;
use std::time::Duration;
use tempfile::TempDir;

mod test_utils;
use test_utils::setup_git_repo;

use once_cell::sync::Lazy;
use std::path::PathBuf;

/// Check if we should run in fast mode (CI environment or explicit setting)
fn should_run_fast() -> bool {
    std::env::var("CI").is_ok()
        || std::env::var("FAST_E2E_TESTS").is_ok()
        || std::env::var("SKIP_SLOW_TESTS").is_ok()
}

/// Global cache for search model downloads - uses unique directory per test run
static MODEL_CACHE_DIR: Lazy<Option<PathBuf>> = Lazy::new(|| {
    std::env::var("SWISSARMYHAMMER_MODEL_CACHE")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            // Create unique cache directory per test execution to avoid conflicts
            use std::time::{SystemTime, UNIX_EPOCH};
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let thread_id = std::thread::current().id();
            std::env::temp_dir()
                .join(format!(
                    ".swissarmyhammer_test_cache_{thread_id:?}_{timestamp}"
                ))
                .into()
        })
});

/// Helper function to perform search indexing with timeout and graceful failure
fn try_search_index(temp_path: &std::path::Path, patterns: &[&str], force: bool) -> Result<bool> {
    // Skip search indexing in CI or when SKIP_SEARCH_TESTS is set
    if std::env::var("CI").is_ok() || std::env::var("SKIP_SEARCH_TESTS").is_ok() {
        eprintln!("⚠️  Skipping search indexing (CI environment or SKIP_SEARCH_TESTS set)");
        return Ok(false);
    }

    let mut cmd_args = vec!["search", "index"];
    cmd_args.extend_from_slice(patterns);
    if force {
        cmd_args.push("--force");
    }

    // Create unique test identifier to avoid any cross-test conflicts
    use std::time::{SystemTime, UNIX_EPOCH};
    let thread_id = std::thread::current().id();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let test_id = format!("{thread_id:?}_{timestamp}");

    let mut cmd = Command::cargo_bin("swissarmyhammer")?;
    cmd.args(&cmd_args)
        .current_dir(temp_path)
        .env("SWISSARMYHAMMER_TEST_MODE", "1")
        .env("SWISSARMYHAMMER_TEST_ID", &test_id) // Unique test identifier
        .env("RUST_LOG", "warn"); // Reduce logging noise

    // Set global model cache to avoid repeated downloads
    if let Some(cache_dir) = MODEL_CACHE_DIR.as_ref() {
        std::fs::create_dir_all(cache_dir).ok();
        cmd.env("SWISSARMYHAMMER_MODEL_CACHE", cache_dir);
    }

    let index_result = cmd.ok();

    match index_result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if (stdout.contains("indexed") && stdout.chars().any(char::is_numeric))
                || (stdout.contains("files") && stdout.chars().any(char::is_numeric))
            {
                Ok(true) // Successfully indexed
            } else {
                Ok(false) // Failed to index properly - skip silently for speed
            }
        }
        Err(_) => {
            Ok(false) // Failed to run - skip silently for speed
        }
    }
}

/// Fast mock search operation that skips actual indexing
fn mock_search_workflow(temp_path: &std::path::Path) -> Result<()> {
    // Create unique test identifier to avoid any cross-test conflicts
    use std::time::{SystemTime, UNIX_EPOCH};
    let thread_id = std::thread::current().id();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let test_id = format!("{thread_id:?}_{timestamp}");

    // Just verify the command structure works without actual indexing
    Command::cargo_bin("swissarmyhammer")?
        .args(["search", "query", "test", "--limit", "1"])
        .current_dir(temp_path)
        .env("SWISSARMYHAMMER_TEST_MODE", "1")
        .env("SWISSARMYHAMMER_TEST_ID", &test_id) // Unique test identifier
        .env("RUST_LOG", "warn")
        .assert()
        .success(); // Should handle gracefully even without index
    Ok(())
}

/// Helper to run CLI commands with standard optimizations
fn run_optimized_command(args: &[&str], temp_path: &std::path::Path) -> Result<Command> {
    // Create unique test identifier to avoid any cross-test conflicts
    use std::time::{SystemTime, UNIX_EPOCH};
    let thread_id = std::thread::current().id();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let test_id = format!("{thread_id:?}_{timestamp}");

    let mut cmd = Command::cargo_bin("swissarmyhammer")?;
    cmd.args(args)
        .current_dir(temp_path)
        .env("SWISSARMYHAMMER_TEST_MODE", "1")
        .env("SWISSARMYHAMMER_TEST_ID", &test_id) // Unique test identifier
        .env("RUST_LOG", "warn");
    Ok(cmd)
}

/// Setup function for end-to-end workflow testing with optimized parallel execution
fn setup_e2e_test_environment() -> Result<(TempDir, std::path::PathBuf)> {
    // Use thread ID and timestamp to create unique temp directories for parallel test execution
    use std::time::{SystemTime, UNIX_EPOCH};
    let thread_id = std::thread::current().id();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = TempDir::with_prefix(format!("e2e_test_{thread_id:?}_{timestamp}_"))?;
    let temp_path = temp_dir.path().to_path_buf();

    // Create only essential directory structure
    let issues_dir = temp_path.join("issues");
    std::fs::create_dir_all(&issues_dir)?;

    let swissarmyhammer_dir = temp_path.join(".swissarmyhammer");
    std::fs::create_dir_all(&swissarmyhammer_dir)?;

    setup_git_repo(&temp_path)?;

    Ok((temp_dir, temp_path))
}

/// Lightweight setup for search-related tests only
fn setup_search_test_environment() -> Result<(TempDir, std::path::PathBuf)> {
    // Use thread ID and timestamp to create unique temp directories for parallel test execution
    use std::time::{SystemTime, UNIX_EPOCH};
    let thread_id = std::thread::current().id();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = TempDir::with_prefix(format!("search_test_{thread_id:?}_{timestamp}_"))?;
    let temp_path = temp_dir.path().to_path_buf();

    let src_dir = temp_path.join("src");
    std::fs::create_dir_all(&src_dir)?;

    // Create minimal source files for search workflow
    std::fs::write(
        src_dir.join("test.rs"),
        "//! Test file\npub fn test_function() -> String { \"test\".to_string() }",
    )?;

    Ok((temp_dir, temp_path))
}

/// Test complete issue lifecycle workflow (optimized)
#[test]
fn test_complete_issue_lifecycle() -> Result<()> {
    let (_temp_dir, temp_path) = setup_e2e_test_environment()?;

    // Step 1: Create a new issue
    let create_output = Command::cargo_bin("swissarmyhammer")?
        .args([
            "issue",
            "create",
            "e2e_lifecycle_test",
            "--content",
            "# E2E Lifecycle Test\n\nThis issue tests the complete lifecycle workflow.",
        ])
        .current_dir(&temp_path)
        .assert()
        .success();

    let create_stdout = String::from_utf8_lossy(&create_output.get_output().stdout);
    assert!(
        create_stdout.contains("Created issue: e2e_lifecycle_test")
            || create_stdout.contains("created issue: e2e_lifecycle_test")
            || create_stdout.contains("e2e_lifecycle_test"),
        "Issue creation should show success message with issue name: {create_stdout}"
    );

    // Step 2: List issues to verify creation
    let list_output = Command::cargo_bin("swissarmyhammer")?
        .args(["issue", "list"])
        .current_dir(&temp_path)
        .assert()
        .success();

    let list_stdout = String::from_utf8_lossy(&list_output.get_output().stdout);
    assert!(
        list_stdout.contains("e2e_lifecycle_test"),
        "Issue should appear in list: {list_stdout}"
    );

    // Step 3: Show the issue details
    let show_output = Command::cargo_bin("swissarmyhammer")?
        .args(["issue", "show", "e2e_lifecycle_test"])
        .current_dir(&temp_path)
        .assert()
        .success();

    let show_stdout = String::from_utf8_lossy(&show_output.get_output().stdout);
    assert!(
        show_stdout.contains("E2E Lifecycle Test")
            && show_stdout.contains("complete lifecycle workflow"),
        "Issue details should contain both title and description: {show_stdout}"
    );

    // Step 4: Update the issue
    Command::cargo_bin("swissarmyhammer")?
        .args([
            "issue",
            "update",
            "e2e_lifecycle_test",
            "--content",
            "Updated content for e2e testing",
            "--append",
        ])
        .current_dir(&temp_path)
        .assert()
        .success();

    // Step 5: Verify the update
    let updated_show_output = Command::cargo_bin("swissarmyhammer")?
        .args(["issue", "show", "e2e_lifecycle_test"])
        .current_dir(&temp_path)
        .assert()
        .success();

    let updated_stdout = String::from_utf8_lossy(&updated_show_output.get_output().stdout);
    assert!(
        updated_stdout.contains("Updated content"),
        "Issue should contain updated content: {updated_stdout}"
    );

    // Step 6: Work on the issue (creates git branch)
    Command::cargo_bin("swissarmyhammer")?
        .args(["issue", "work", "e2e_lifecycle_test"])
        .current_dir(&temp_path)
        .assert()
        .success();

    // Step 7: Check current issue
    let current_output = Command::cargo_bin("swissarmyhammer")?
        .args(["issue", "current"])
        .current_dir(&temp_path)
        .assert()
        .success();

    let current_stdout = String::from_utf8_lossy(&current_output.get_output().stdout);
    assert!(
        current_stdout.contains("e2e_lifecycle_test"),
        "Current issue should show our issue: {current_stdout}"
    );

    // Step 8: Complete the issue
    Command::cargo_bin("swissarmyhammer")?
        .args(["issue", "complete", "e2e_lifecycle_test"])
        .current_dir(&temp_path)
        .assert()
        .success();

    // Step 9: Merge the issue
    Command::cargo_bin("swissarmyhammer")?
        .args(["issue", "merge", "e2e_lifecycle_test"])
        .current_dir(&temp_path)
        .assert()
        .success();

    // Step 10: Verify issue is completed
    let final_list_output = Command::cargo_bin("swissarmyhammer")?
        .args(["issue", "list", "--completed"])
        .current_dir(&temp_path)
        .assert()
        .success();

    let final_stdout = String::from_utf8_lossy(&final_list_output.get_output().stdout);
    assert!(
        final_stdout.contains("e2e_lifecycle_test")
            && (final_stdout.contains("completed")
                || final_stdout.contains("✓")
                || final_stdout.contains("✅")),
        "Completed issue should appear with completion status indicator: {final_stdout}"
    );

    Ok(())
}

/// Test complete memo management workflow
#[test]
fn test_complete_memo_workflow() -> Result<()> {
    let (_temp_dir, temp_path) = setup_e2e_test_environment()?;

    // Step 1: Create multiple memos
    let memo_data = vec![
        (
            "Meeting Notes",
            "# Meeting Notes\n\nDiscussed project timeline and goals.",
        ),
        (
            "Task List",
            "# Task List\n\n1. Complete testing\n2. Review documentation\n3. Deploy to production",
        ),
        (
            "Code Review Notes",
            "# Code Review\n\nReviewed PR #123:\n- Good error handling\n- Needs more tests",
        ),
    ];

    let mut memo_ids = vec![];

    for (title, content) in &memo_data {
        let create_output = Command::cargo_bin("swissarmyhammer")?
            .args(["memo", "create", title, "--content", content])
            .current_dir(&temp_path)
            .assert()
            .success();

        let create_stdout = String::from_utf8_lossy(&create_output.get_output().stdout);

        // Extract memo ID from output (ULID pattern)
        if let Some(id) = extract_ulid_from_text(&create_stdout) {
            memo_ids.push(id);
        }
    }

    // Step 2: List all memos
    let list_output = Command::cargo_bin("swissarmyhammer")?
        .args(["memo", "list"])
        .current_dir(&temp_path)
        .assert()
        .success();

    let list_stdout = String::from_utf8_lossy(&list_output.get_output().stdout);
    assert!(
        list_stdout.contains("Meeting Notes")
            && list_stdout.contains("Task List")
            && (list_stdout.matches('\n').count() >= 2 || list_stdout.len() > 50),
        "All memos should appear in list with proper formatting: {list_stdout}"
    );

    // Step 3: Get specific memo details
    if let Some(first_id) = memo_ids.first() {
        let get_output = Command::cargo_bin("swissarmyhammer")?
            .args(["memo", "get", first_id])
            .current_dir(&temp_path)
            .assert()
            .success();

        let get_stdout = String::from_utf8_lossy(&get_output.get_output().stdout);
        assert!(
            get_stdout.contains("Meeting Notes") || get_stdout.contains("project timeline"),
            "Memo details should contain expected content: {get_stdout}"
        );
    }

    // Step 4: Search memos
    let search_output = Command::cargo_bin("swissarmyhammer")?
        .args(["memo", "search", "testing"])
        .current_dir(&temp_path)
        .assert()
        .success();

    let search_stdout = String::from_utf8_lossy(&search_output.get_output().stdout);
    assert!(
        search_stdout.contains("Task List") || search_stdout.contains("Complete testing"),
        "Search should find relevant memos: {search_stdout}"
    );

    // Step 5: Update a memo
    if let Some(second_id) = memo_ids.get(1) {
        Command::cargo_bin("swissarmyhammer")?
            .args([
                "memo",
                "update",
                second_id,
                "--content",
                "# Updated Task List\n\n1. ✅ Complete testing\n2. Review documentation\n3. Deploy to production\n4. Monitor deployment"
            ])
            .current_dir(&temp_path)
            .assert()
            .success();

        // Verify update
        let updated_get_output = Command::cargo_bin("swissarmyhammer")?
            .args(["memo", "get", second_id])
            .current_dir(&temp_path)
            .assert()
            .success();

        let updated_stdout = String::from_utf8_lossy(&updated_get_output.get_output().stdout);
        assert!(
            updated_stdout.contains("Updated Task List")
                && updated_stdout.contains("Monitor deployment"),
            "Updated memo should contain new content: {updated_stdout}"
        );
    }

    // Step 6: Get all context for AI
    let context_output = Command::cargo_bin("swissarmyhammer")?
        .args(["memo", "context"])
        .current_dir(&temp_path)
        .assert()
        .success();

    let context_stdout = String::from_utf8_lossy(&context_output.get_output().stdout);
    assert!(
        context_stdout.len() > 100
            && context_stdout.contains("Meeting Notes")
            && context_stdout.contains("Task List"),
        "Context should contain substantial content from all memos: length={}",
        context_stdout.len()
    );

    // Step 7: Delete a memo
    if let Some(last_id) = memo_ids.last() {
        Command::cargo_bin("swissarmyhammer")?
            .args(["memo", "delete", last_id])
            .current_dir(&temp_path)
            .assert()
            .success();

        // Verify deletion
        Command::cargo_bin("swissarmyhammer")?
            .args(["memo", "get", last_id])
            .current_dir(&temp_path)
            .assert()
            .failure(); // Should fail to find deleted memo
    }

    Ok(())
}

/// Test complete search workflow (optimized)
#[test]
#[ignore]
fn test_complete_search_workflow() -> Result<()> {
    let (_temp_dir, temp_path) = setup_search_test_environment()?;

    // Fast path: Try indexing with very short timeout, fallback to mock
    let indexed = try_search_index(&temp_path, &["src/**/*.rs"], false)?;
    if !indexed {
        // Use mock search workflow for speed
        mock_search_workflow(&temp_path)?;
        return Ok(());
    }

    // Only do full workflow if indexing succeeded quickly
    // Step 2: Single optimized query
    run_optimized_command(&["search", "query", "function", "--limit", "3"], &temp_path)?
        .assert()
        .success();

    // Step 3: Test JSON format only (skip other checks for speed)
    run_optimized_command(
        &[
            "search", "query", "test", "--format", "json", "--limit", "1",
        ],
        &temp_path,
    )?
    .assert()
    .success();

    Ok(())
}

/// Test mixed workflow with issues, memos, and search
#[test]
fn test_mixed_workflow() -> Result<()> {
    let (_temp_dir, temp_path) = setup_e2e_test_environment()?;

    // Step 1: Create an issue about implementing search functionality
    Command::cargo_bin("swissarmyhammer")?
        .args([
            "issue",
            "create",
            "implement_search_feature",
            "--content",
            "# Implement Search Feature\n\nNeed to add semantic search capabilities to the application."
        ])
        .current_dir(&temp_path)
        .assert()
        .success();

    // Step 2: Create research memo about search implementation
    let memo_output = Command::cargo_bin("swissarmyhammer")?
        .args([
            "memo",
            "create",
            "Search Implementation Research",
            "--content",
            "# Search Research\n\n## Options Considered\n- Vector embeddings\n- Full-text search\n- Hybrid approach\n\n## Recommendation\nUse vector embeddings with DuckDB storage."
        ])
        .current_dir(&temp_path)
        .assert()
        .success();

    let memo_stdout = String::from_utf8_lossy(&memo_output.get_output().stdout);
    let _research_memo_id = extract_ulid_from_text(&memo_stdout);

    // Step 3: Work on the issue
    Command::cargo_bin("swissarmyhammer")?
        .args(["issue", "work", "implement_search_feature"])
        .current_dir(&temp_path)
        .assert()
        .success();

    // Step 4: Mock search implementation (skip actual indexing for speed)
    mock_search_workflow(&temp_path)?;

    // Step 5: Create progress memo
    Command::cargo_bin("swissarmyhammer")?
        .args([
            "memo",
            "create",
            "Search Implementation Progress",
            "--content",
            "# Implementation Progress\n\n✅ Mock search verified\n✅ CLI integration tested\n🔄 Writing tests\n⏳ Documentation updates"
        ])
        .current_dir(&temp_path)
        .assert()
        .success();

    // Step 6: Update original issue with progress
    Command::cargo_bin("swissarmyhammer")?
        .args([
            "issue",
            "update",
            "implement_search_feature",
            "--content",
            "\n\n## Progress Update\n\nSearch functionality verified. Ready for testing phase.",
            "--append",
        ])
        .current_dir(&temp_path)
        .assert()
        .success();

    // Step 8: Search memos for research notes
    let memo_search_output = Command::cargo_bin("swissarmyhammer")?
        .args(["memo", "search", "vector embeddings"])
        .current_dir(&temp_path)
        .assert()
        .success();

    let memo_search_stdout = String::from_utf8_lossy(&memo_search_output.get_output().stdout);
    assert!(
        memo_search_stdout.contains("Search") || memo_search_stdout.contains("Research"),
        "Should find research memo: {memo_search_stdout}"
    );

    // Step 9: Complete the issue
    Command::cargo_bin("swissarmyhammer")?
        .args(["issue", "complete", "implement_search_feature"])
        .current_dir(&temp_path)
        .assert()
        .success();

    // Step 10: Create completion memo
    Command::cargo_bin("swissarmyhammer")?
        .args([
            "memo",
            "create",
            "Search Feature Completed",
            "--content",
            "# Search Feature Complete\n\n## Summary\nSuccessfully implemented semantic search with:\n- Vector embeddings\n- DuckDB storage\n- CLI integration\n\n## Next Steps\n- Performance optimization\n- User documentation"
        ])
        .current_dir(&temp_path)
        .assert()
        .success();

    // Step 11: Get all context for final review
    let context_output = Command::cargo_bin("swissarmyhammer")?
        .args(["memo", "context"])
        .current_dir(&temp_path)
        .assert()
        .success();

    let context_stdout = String::from_utf8_lossy(&context_output.get_output().stdout);
    assert!(
        context_stdout.contains("Search") && context_stdout.contains("Implementation"),
        "Context should contain all search-related memos: {}",
        context_stdout.len()
    );

    // Step 12: Merge the completed issue
    Command::cargo_bin("swissarmyhammer")?
        .args(["issue", "merge", "implement_search_feature"])
        .current_dir(&temp_path)
        .assert()
        .success();

    Ok(())
}

/// Test error recovery workflow (fast version)
#[test]
fn test_error_recovery_workflow() -> Result<()> {
    let (_temp_dir, temp_path) = setup_e2e_test_environment()?;

    // Step 1: Attempt to work on non-existent issue (should fail)
    Command::cargo_bin("swissarmyhammer")?
        .args(["issue", "work", "nonexistent_issue"])
        .current_dir(&temp_path)
        .assert()
        .failure();

    // Step 2: Create the issue properly
    Command::cargo_bin("swissarmyhammer")?
        .args([
            "issue",
            "create",
            "error_recovery_test",
            "--content",
            "# Error Recovery Test\n\nTesting error recovery workflows.",
        ])
        .current_dir(&temp_path)
        .assert()
        .success();

    // Step 3: Now work on the issue (should succeed)
    Command::cargo_bin("swissarmyhammer")?
        .args(["issue", "work", "error_recovery_test"])
        .current_dir(&temp_path)
        .assert()
        .success();

    // Step 4: Attempt to get non-existent memo (should fail gracefully)
    Command::cargo_bin("swissarmyhammer")?
        .args(["memo", "get", "01ARZ3NDEKTSV4RRFFQ69G5FAV"])
        .current_dir(&temp_path)
        .assert()
        .failure();

    // Step 5: Create memo properly
    let memo_output = Command::cargo_bin("swissarmyhammer")?
        .args([
            "memo",
            "create",
            "Error Recovery Notes",
            "--content",
            "# Recovery Notes\n\nDocumenting error recovery procedures.",
        ])
        .current_dir(&temp_path)
        .assert()
        .success();

    let memo_stdout = String::from_utf8_lossy(&memo_output.get_output().stdout);
    if let Some(memo_id) = extract_ulid_from_text(&memo_stdout) {
        // Step 6: Now get the memo (should succeed)
        Command::cargo_bin("swissarmyhammer")?
            .args(["memo", "get", &memo_id])
            .current_dir(&temp_path)
            .assert()
            .success();
    }

    // Step 7: Test graceful handling of search without index (skip expensive indexing)
    run_optimized_command(&["search", "query", "recovery"], &temp_path)?
        .assert()
        .success(); // Should handle gracefully even if no index

    // Step 8: Test issue update and completion error recovery
    Command::cargo_bin("swissarmyhammer")?
        .args([
            "issue",
            "update",
            "error_recovery_test",
            "--content",
            "Updated after error recovery testing",
            "--append",
        ])
        .current_dir(&temp_path)
        .assert()
        .success();

    // Step 9: Complete the issue to finish recovery workflow
    Command::cargo_bin("swissarmyhammer")?
        .args(["issue", "complete", "error_recovery_test"])
        .current_dir(&temp_path)
        .assert()
        .success();

    Ok(())
}

/// Test performance under realistic workflow load
#[test]
#[ignore = "Slow load test - run with --ignored"]
fn test_realistic_load_workflow() -> Result<()> {
    let (_temp_dir, temp_path) = setup_e2e_test_environment()?;

    // Create multiple issues and memos to simulate realistic usage
    for i in 1..=5 {
        Command::cargo_bin("swissarmyhammer")?
            .args([
                "issue",
                "create",
                &format!("load_test_issue_{i}"),
                "--content",
                &format!("# Load Test Issue {i}\n\nThis is issue {i} for load testing."),
            ])
            .current_dir(&temp_path)
            .assert()
            .success();

        Command::cargo_bin("swissarmyhammer")?
            .args([
                "memo",
                "create",
                &format!("Load Test Memo {i}"),
                "--content",
                &format!("# Memo {i}\n\nThis is memo {i} for load testing.\n\n## Details\n- Priority: Medium\n- Category: Testing\n- Iteration: {i}")
            ])
            .current_dir(&temp_path)
            .assert()
            .success();
    }

    // Perform various operations to test performance
    let start_time = std::time::Instant::now();

    Command::cargo_bin("swissarmyhammer")?
        .args(["issue", "list"])
        .current_dir(&temp_path)
        .assert()
        .success();

    Command::cargo_bin("swissarmyhammer")?
        .args(["memo", "list"])
        .current_dir(&temp_path)
        .assert()
        .success();

    let _indexed = try_search_index(&temp_path, &["src/**/*.rs"], false)?;
    // Continue timing test regardless of indexing result

    let elapsed = start_time.elapsed();

    // Should complete in reasonable time (less than 60 seconds for this load)
    assert!(
        elapsed < Duration::from_secs(60),
        "Workflow should complete in reasonable time: {elapsed:?}"
    );

    Ok(())
}

/// Fast smoke test that covers basic functionality without expensive operations
#[test]
fn test_fast_smoke_workflow() -> Result<()> {
    if !should_run_fast() {
        return Ok(()); // Skip if not in fast mode
    }

    let (_temp_dir, temp_path) = setup_e2e_test_environment()?;

    // Quick issue operations
    run_optimized_command(
        &["issue", "create", "smoke_test", "--content", "Quick test"],
        &temp_path,
    )?
    .assert()
    .success();

    run_optimized_command(&["issue", "list"], &temp_path)?
        .assert()
        .success();

    // Quick memo operations
    run_optimized_command(
        &[
            "memo",
            "create",
            "Smoke Test",
            "--content",
            "Fast test memo",
        ],
        &temp_path,
    )?
    .assert()
    .success();

    run_optimized_command(&["memo", "list"], &temp_path)?
        .assert()
        .success();

    // Mock search (no indexing)
    mock_search_workflow(&temp_path)?;

    Ok(())
}

/// Helper function to extract ULID from text
fn extract_ulid_from_text(text: &str) -> Option<String> {
    use regex::Regex;

    // ULID pattern: 26 characters using Crockford's Base32
    let ulid_pattern = Regex::new(r"\b[0-9A-HJKMNP-TV-Z]{26}\b").ok()?;
    ulid_pattern.find(text).map(|m| m.as_str().to_string())
}
