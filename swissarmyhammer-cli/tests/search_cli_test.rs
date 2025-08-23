//! Integration tests for semantic search CLI commands

use anyhow::Result;

mod test_utils;
use test_utils::create_semantic_test_guard;

mod in_process_test_utils;
use in_process_test_utils::run_sah_command_in_process;

/// Test that the old --glob flag version no longer works (breaking change)
#[tokio::test]
async fn test_search_index_old_glob_flag_rejected() -> Result<()> {
    let result = run_sah_command_in_process(&["search", "index", "--glob", "**/*.rs"]).await?;

    assert!(
        result.exit_code != 0,
        "search index with --glob should now fail (breaking change)"
    );

    // The error should indicate that --glob is not a valid argument
    assert!(
        result.stderr.contains("unexpected argument") || result.stderr.contains("found argument"),
        "should show error about unexpected --glob argument: {}",
        result.stderr
    );

    Ok(())
}

/// Test that the new positional glob argument version works
#[tokio::test]
async fn test_search_index_positional_glob() -> Result<()> {
    let _guard = create_semantic_test_guard();

    // Use a pattern that won't match many files to avoid heavy indexing
    let result = run_sah_command_in_process(&["search", "index", "nonexistent/**/*.xyz"]).await?;

    // Check that it recognizes the correct arguments and starts the process
    // We don't care if it fails due to model initialization - just that it parses args correctly
    let combined_output = format!("{}{}", result.stdout, result.stderr);
    assert!(
        combined_output.contains("Indexing files matching: nonexistent/**/*.xyz")
            || combined_output.contains("Starting semantic search indexing")
            || combined_output.contains("Failed to initialize fastembed model")
            || combined_output.contains("Failed to create CLI context"),
        "should show that it parsed arguments correctly and attempted indexing: stdout={}, stderr={}", result.stdout, result.stderr
    );

    Ok(())
}

/// Test search index with force flag
#[tokio::test]
async fn test_search_index_with_force() -> Result<()> {
    let _guard = create_semantic_test_guard();

    // Use a pattern that won't match many files to avoid heavy indexing
    let result =
        run_sah_command_in_process(&["search", "index", "nonexistent/**/*.xyz", "--force"]).await?;

    // Should show that it's starting indexing with the correct glob pattern and force flag
    assert!(
        result
            .stdout
            .contains("Indexing files matching: nonexistent/**/*.xyz")
            || result.stderr.contains("Indexing files matching:")
            || result.stdout.contains("Starting semantic search indexing")
            || result.stderr.contains("Starting semantic search indexing"),
        "should show glob pattern or indexing start in output: stdout={}, stderr={}",
        result.stdout,
        result.stderr
    );
    assert!(
        result.stdout.contains("Force re-indexing: enabled")
            || result.stderr.contains("Force re-indexing: enabled"),
        "should show force flag is enabled: stdout={}, stderr={}",
        result.stdout,
        result.stderr
    );

    Ok(())
}

/// Test search query functionality
#[tokio::test]
async fn test_search_query() -> Result<()> {
    let _guard = create_semantic_test_guard();

    let result = run_sah_command_in_process(&["search", "query", "error handling"]).await?;

    // Should show that it's starting search with the correct query
    assert!(
        result.stdout.contains("Searching for: error handling")
            || result.stderr.contains("Searching for:"),
        "should show search query in output: stdout={}, stderr={}",
        result.stdout,
        result.stderr
    );

    Ok(())
}

/// Test search help output
#[tokio::test]
async fn test_search_help() -> Result<()> {
    let result = run_sah_command_in_process(&["search", "--help"]).await?;

    assert_eq!(result.exit_code, 0, "search help should succeed");

    assert!(
        result.stdout.contains("semantic search"),
        "help should mention semantic search"
    );
    assert!(
        result.stdout.contains("index") && result.stdout.contains("query"),
        "help should mention index and query subcommands"
    );

    Ok(())
}

/// Test search index help shows correct usage
#[tokio::test]
async fn test_search_index_help() -> Result<()> {
    let result = run_sah_command_in_process(&["search", "index", "--help"]).await?;

    assert_eq!(result.exit_code, 0, "search index help should succeed");

    // After our changes, this should show positional patterns argument syntax
    assert!(
        result.stdout.contains("PATTERNS")
            || result.stdout.contains("patterns")
            || result.stdout.contains("glob"),
        "help should show patterns parameter: {}",
        result.stdout
    );

    Ok(())
}
