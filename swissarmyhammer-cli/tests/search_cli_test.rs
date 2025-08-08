//! Integration tests for semantic search CLI commands

use anyhow::Result;
use assert_cmd::Command;

mod test_utils;
use test_utils::create_semantic_test_guard;

/// Test that the old --glob flag version no longer works (breaking change)
#[test]
fn test_search_index_old_glob_flag_rejected() -> Result<()> {
    let output = Command::cargo_bin("swissarmyhammer")
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

    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .args(["search", "index", "**/*.rs"])
        .output()?;

    // The command should fail gracefully without a real API key, but still show the expected output
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should show that it's starting indexing with the correct glob pattern
    assert!(
        stdout.contains("Indexing files matching: **/*.rs")
            || stderr.contains("Indexing files matching:"),
        "should show glob pattern in output: stdout={stdout}, stderr={stderr}"
    );

    Ok(())
}

/// Test search index with force flag
#[test]
fn test_search_index_with_force() -> Result<()> {
    let _guard = create_semantic_test_guard();

    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .args(["search", "index", "**/*.py", "--force"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should show that it's starting indexing with the correct glob pattern and force flag
    assert!(
        stdout.contains("Indexing files matching: **/*.py")
            || stderr.contains("Indexing files matching:"),
        "should show glob pattern in output: stdout={stdout}, stderr={stderr}"
    );
    assert!(
        stdout.contains("Force re-indexing: enabled")
            || stderr.contains("Force re-indexing: enabled"),
        "should show force flag is enabled: stdout={stdout}, stderr={stderr}"
    );

    Ok(())
}

/// Test search query functionality
#[test]
fn test_search_query() -> Result<()> {
    let _guard = create_semantic_test_guard();

    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .args(["search", "query", "error handling"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should show that it's starting search with the correct query
    assert!(
        stdout.contains("Searching for: error handling") || stderr.contains("Searching for:"),
        "should show search query in output: stdout={stdout}, stderr={stderr}"
    );

    Ok(())
}

/// Test search help output
#[test]
fn test_search_help() -> Result<()> {
    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .args(["search", "--help"])
        .output()?;

    assert!(output.status.success(), "search help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("semantic search"),
        "help should mention semantic search"
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
    let output = Command::cargo_bin("swissarmyhammer")
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
