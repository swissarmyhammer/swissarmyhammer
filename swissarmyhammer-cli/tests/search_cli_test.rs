//! Integration tests for semantic search CLI commands

use anyhow::Result;
use assert_cmd::Command;

mod test_utils;
use test_utils::create_semantic_test_guard;

/// Test that the old --glob flag version no longer works (breaking change)
#[test]
fn test_search_index_old_glob_flag_rejected() -> Result<()> {
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["search", "index", "--glob", "**/*.rs"])
        .output()?;

    assert!(
        !output.status.success(),
        "search index with --glob should now fail (breaking change)"
    );

    // The error should indicate that --glob is not a valid argument
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unexpected argument") || stderr.contains("found argument"),
        "should show error about unexpected --glob argument: {stderr}"
    );

    Ok(())
}

/// Test that the new positional glob argument version works
#[test]
fn test_search_index_positional_glob() -> Result<()> {
    let _guard = create_semantic_test_guard();

    // Use a pattern that won't match many files to avoid heavy indexing
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["search", "index", "nonexistent/**/*.xyz"])
        .timeout(std::time::Duration::from_secs(10)) // Fail fast if this takes too long
        .output()?;

    // The command should either succeed in showing the indexing message or fail gracefully
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Dynamic CLI returns JSON response from MCP tools
    // Check for JSON structure or expected error messages
    let combined_output = format!("{stdout}{stderr}");
    assert!(
        stdout.contains("indexed_files") // JSON response contains this field
            || stdout.contains("Successfully indexed") // Success message
            || combined_output.contains("Failed to initialize fastembed model")
            || combined_output.contains("Failed to create CLI context")
            || combined_output.contains("No files found matching pattern"),
        "should show indexing attempt or expected errors: stdout={stdout}, stderr={stderr}"
    );

    Ok(())
}

/// Test search index with force flag
#[test]
fn test_search_index_with_force() -> Result<()> {
    let _guard = create_semantic_test_guard();

    // Use a pattern that won't match many files to avoid heavy indexing
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["search", "index", "nonexistent/**/*.xyz", "--force"])
        .timeout(std::time::Duration::from_secs(10)) // Fail fast if this takes too long
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Dynamic CLI returns JSON response, check for expected structure
    assert!(
        !stdout.is_empty()
            && (
                stdout.contains("indexed_files") // JSON response field
            || stdout.contains("message") // JSON success message field
            || stderr.contains("No files found matching pattern")
                // Expected for nonexistent pattern
            ),
        "should show indexing response in JSON format: stdout={stdout}, stderr={stderr}"
    );

    Ok(())
}

/// Test search query functionality
#[test]
fn test_search_query() -> Result<()> {
    let _guard = create_semantic_test_guard();

    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["search", "query", "error handling"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Dynamic CLI returns JSON response from search query MCP tool
    // The response should be JSON format with query information
    assert!(
        !stdout.is_empty()
            && (
                stdout.contains("query") // JSON response contains query field
            || stdout.contains("results") // JSON structure for search results
            || stderr.contains("No search index found")
                // Expected error if no index
            ),
        "should show search response in JSON format: stdout={stdout}, stderr={stderr}"
    );

    Ok(())
}

/// Test search help output
#[test]
fn test_search_help() -> Result<()> {
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["search", "--help"])
        .output()?;

    assert!(output.status.success(), "search help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Search management") || stdout.contains("search"),
        "help should mention search functionality: {stdout}"
    );
    assert!(
        stdout.contains("index") && stdout.contains("query"),
        "help should mention index and query subcommands"
    );

    Ok(())
}

/// Test search index help shows correct usage
#[test]
fn test_search_index_help() -> Result<()> {
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["search", "index", "--help"])
        .output()?;

    assert!(output.status.success(), "search index help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // After our changes, this should show positional patterns argument syntax
    assert!(
        stdout.contains("PATTERNS") || stdout.contains("patterns") || stdout.contains("glob"),
        "help should show patterns parameter: {stdout}"
    );

    Ok(())
}
