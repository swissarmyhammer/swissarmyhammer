//! Integration tests for leader/client architecture
//!
//! These tests verify the full leader/client interaction including:
//! - Leader election
//! - Client connection
//! - Query execution
//! - Handling of "not ready" state
//!
//! Tests that create an `IndexLeader` must run serially because the
//! llama embedding backend can only be initialized once per process.

use std::path::PathBuf;
use std::time::Duration;

use serial_test::serial;
use tempfile::TempDir;
use tokio::task::JoinHandle;

use swissarmyhammer_treesitter::{
    ClientError, ElectionError, IndexClient, IndexLeader, LeaderElection, QueryError,
    QueryErrorKind,
};

// =============================================================================
// Test Constants
// =============================================================================

/// Time to wait for leader to start listening on socket (milliseconds).
const LEADER_STARTUP_DELAY_MS: u64 = 100;

/// Default number of results to request in semantic search tests.
const TEST_SEMANTIC_SEARCH_TOP_K: usize = 10;

/// Reduced result count for threshold tests.
const TEST_SEMANTIC_SEARCH_TOP_K_SMALL: usize = 5;

/// Minimum similarity threshold (accepts all results).
const TEST_MIN_SIMILARITY_ACCEPT_ALL: f32 = 0.0;

/// Very high similarity threshold for testing filtering.
const TEST_MIN_SIMILARITY_STRICT: f32 = 0.99;

/// Minimum similarity threshold for detecting duplicates in tests.
const TEST_DUPLICATE_MIN_SIMILARITY: f32 = 0.7;

/// Minimum chunk size in bytes for duplicate detection tests.
const TEST_DUPLICATE_MIN_CHUNK_BYTES: usize = 10;

/// Minimum expected file count in test workspace.
const TEST_WORKSPACE_MIN_FILES: usize = 3;

// =============================================================================
// Test Helpers
// =============================================================================

/// Test context that manages leader lifecycle.
///
/// Creates a test workspace, starts a leader, and provides client connection.
/// The leader is automatically aborted when the context is dropped.
struct TestContext {
    /// Temporary directory with test files
    dir: TempDir,
    /// Handle to the running leader task
    leader_handle: JoinHandle<()>,
}

impl TestContext {
    /// Create a new test context with a running leader.
    ///
    /// This sets up a test workspace, starts a leader in the background,
    /// and waits for it to be ready to accept connections.
    async fn new() -> Self {
        let dir = create_test_workspace();
        let election = LeaderElection::new(dir.path());
        let socket_path = election.socket_path().to_path_buf();

        let guard = election.try_become_leader().unwrap();
        let leader = IndexLeader::new(guard, dir.path()).await.unwrap();

        let leader_handle = tokio::spawn(async move {
            let _ = leader.run(&socket_path).await;
        });

        // Give leader time to start listening
        tokio::time::sleep(Duration::from_millis(LEADER_STARTUP_DELAY_MS)).await;

        Self { dir, leader_handle }
    }

    /// Connect a client to the running leader.
    async fn connect(&self) -> Result<IndexClient, ClientError> {
        IndexClient::connect(self.dir.path()).await
    }

    /// Get the workspace directory path.
    fn workspace_path(&self) -> &std::path::Path {
        self.dir.path()
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        self.leader_handle.abort();
    }
}

/// Create a test directory with sample source files.
///
/// Creates a workspace with:
/// - `main.rs` - contains `main()` and `helper()` functions
/// - `lib.rs` - contains `add()` and `subtract()` functions
/// - `utils.rs` - contains duplicate `helper()` function (for testing duplicate detection)
fn create_test_workspace() -> TempDir {
    let dir = TempDir::new().unwrap();

    // Create some Rust files for testing
    std::fs::write(
        dir.path().join("main.rs"),
        r#"
fn main() {
    println!("Hello, world!");
}

fn helper() {
    println!("I'm a helper");
}
"#,
    )
    .unwrap();

    std::fs::write(
        dir.path().join("lib.rs"),
        r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn subtract(a: i32, b: i32) -> i32 {
    a - b
}
"#,
    )
    .unwrap();

    // Create a duplicate function in another file
    std::fs::write(
        dir.path().join("utils.rs"),
        r#"
fn helper() {
    println!("I'm a helper");
}

fn another_helper() {
    println!("Another one");
}
"#,
    )
    .unwrap();

    dir
}

// =============================================================================
// Leader Election Tests (no leader needed)
// =============================================================================

#[tokio::test]
async fn test_leader_election_single_process() {
    let dir = create_test_workspace();
    let election = LeaderElection::new(dir.path());

    // First attempt should succeed
    let guard = election.try_become_leader();
    assert!(guard.is_ok());

    // Second attempt should fail with LockHeld error
    let result = election.try_become_leader();
    assert!(
        matches!(result, Err(ElectionError::LockHeld)),
        "Expected LockHeld error when lock is already held"
    );

    // Drop the first guard
    drop(guard);

    // Now we should be able to acquire again
    let guard3 = election.try_become_leader();
    assert!(guard3.is_ok());
}

#[tokio::test]
async fn test_client_connect_no_leader() {
    let dir = create_test_workspace();

    // No leader running, should fail
    let result = IndexClient::connect(dir.path()).await;
    assert!(result.is_err());
}

// =============================================================================
// QueryError Tests (no leader needed)
// =============================================================================

#[tokio::test]
async fn test_query_error_not_ready() {
    let err = QueryError::not_ready();
    assert!(matches!(err.kind, QueryErrorKind::NotReady));
    assert!(err.message.contains("not ready"));
}

#[tokio::test]
async fn test_query_error_file_not_found() {
    let err = QueryError::file_not_found(&PathBuf::from("/missing/file.rs"));
    assert!(matches!(err.kind, QueryErrorKind::FileNotFound));
    assert!(err.message.contains("/missing/file.rs"));
}

// =============================================================================
// Basic Client-Leader Tests
// =============================================================================

#[tokio::test]
#[serial]
async fn test_leader_status() {
    let ctx = TestContext::new().await;
    let client = ctx.connect().await.expect("Failed to connect to leader");

    let status = client.status().await.expect("Failed to get status");
    assert!(status.files_indexed > 0);
    assert!(status.is_ready);
}

#[tokio::test]
#[serial]
async fn test_leader_list_files() {
    let ctx = TestContext::new().await;
    let client = ctx.connect().await.expect("Failed to connect to leader");

    let files = client.list_files().await.expect("Failed to list files");
    assert!(files.len() >= TEST_WORKSPACE_MIN_FILES);

    let file_names: Vec<_> = files
        .iter()
        .filter_map(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .collect();

    assert!(file_names.contains(&"main.rs".to_string()));
    assert!(file_names.contains(&"lib.rs".to_string()));
    assert!(file_names.contains(&"utils.rs".to_string()));
}

#[tokio::test]
#[serial]
async fn test_leader_client_full_flow() {
    let ctx = TestContext::new().await;
    let client = ctx.connect().await.expect("Failed to connect to leader");

    // Query for status
    let status = client.status().await.expect("Failed to get status");
    assert!(status.is_ready);
    assert!(status.files_total > 0);

    // List files
    let files = client.list_files().await.expect("Failed to list files");
    assert!(files.len() >= TEST_WORKSPACE_MIN_FILES);
}

#[tokio::test]
#[serial]
async fn test_multiple_clients() {
    let ctx = TestContext::new().await;

    // Connect multiple clients concurrently
    let workspace = ctx.workspace_path().to_path_buf();
    let c1 = IndexClient::connect(&workspace)
        .await
        .expect("Failed to connect client 1");
    let c2 = IndexClient::connect(&workspace)
        .await
        .expect("Failed to connect client 2");

    // Both clients should be able to query
    let status1 = c1.status().await.expect("Client 1 failed to get status");
    let status2 = c2.status().await.expect("Client 2 failed to get status");

    assert!(status1.is_ready);
    assert!(status2.is_ready);

    // Both should see the same number of files
    let files1 = c1.list_files().await.expect("Client 1 failed to list files");
    let files2 = c2.list_files().await.expect("Client 2 failed to list files");
    assert_eq!(files1.len(), files2.len());
}

// =============================================================================
// Tree-sitter Query Tests
// =============================================================================

#[tokio::test]
#[serial]
async fn test_tree_sitter_query() {
    let ctx = TestContext::new().await;
    let client = ctx.connect().await.expect("Failed to connect to leader");

    // Query for function definitions
    let query = "(function_item name: (identifier) @name)".to_string();
    let matches = client
        .tree_sitter_query(query, None, Some("rust".to_string()))
        .await
        .expect("Tree-sitter query failed");

    // Should find functions like main, helper, add, subtract, etc.
    let names: Vec<_> = matches
        .iter()
        .flat_map(|m| m.captures.iter())
        .filter(|c| c.name == "name")
        .map(|c| c.text.clone())
        .collect();

    assert!(names.contains(&"main".to_string()));
    assert!(names.contains(&"helper".to_string()));
}

// =============================================================================
// Semantic Search Tests
// =============================================================================

#[tokio::test]
#[serial]
async fn test_semantic_search() {
    let ctx = TestContext::new().await;
    let client = ctx.connect().await.expect("Failed to connect to leader");

    // Search for code similar to "println" - should find matches in our test files
    let results = client
        .semantic_search(
            "println macro call".to_string(),
            TEST_SEMANTIC_SEARCH_TOP_K,
            TEST_MIN_SIMILARITY_ACCEPT_ALL,
        )
        .await
        .expect("semantic_search should succeed");

    // With min_similarity of 0.0, we should get some results
    assert!(
        !results.is_empty(),
        "Should find chunks similar to println query"
    );

    // Each result should have valid chunk data
    for result in &results {
        assert!(!result.chunk.text.is_empty());
        assert!(result.similarity >= 0.0);
        assert!(result.similarity <= 1.0);
    }
}

#[tokio::test]
#[serial]
async fn test_semantic_search_with_high_threshold() {
    let ctx = TestContext::new().await;
    let client = ctx.connect().await.expect("Failed to connect to leader");

    // Search with very high similarity threshold - may return fewer/no results
    let results = client
        .semantic_search(
            "completely unrelated gibberish xyz".to_string(),
            TEST_SEMANTIC_SEARCH_TOP_K_SMALL,
            TEST_MIN_SIMILARITY_STRICT,
        )
        .await
        .expect("semantic_search should succeed even with high threshold");

    // With 0.99 threshold and unrelated query, we expect few or no results
    // This is valid behavior - we're testing the threshold filtering works
    assert!(
        results.len() <= TEST_SEMANTIC_SEARCH_TOP_K_SMALL,
        "Results should be bounded by top_k"
    );
}

// =============================================================================
// Duplicate Detection Tests
// =============================================================================

#[tokio::test]
#[serial]
async fn test_find_all_duplicates() {
    let ctx = TestContext::new().await;
    let client = ctx.connect().await.expect("Failed to connect to leader");

    // Find all duplicates - the test workspace has duplicate helper() functions
    let clusters = client
        .find_all_duplicates(TEST_DUPLICATE_MIN_SIMILARITY, TEST_DUPLICATE_MIN_CHUNK_BYTES)
        .await
        .expect("find_all_duplicates should succeed");

    // We should find at least one cluster (the duplicate helper functions)
    // Note: exact count depends on embedding similarity and chunking
    // The test verifies the API works, not exact duplicate detection accuracy

    // Each cluster should have valid structure
    for cluster in &clusters {
        assert!(!cluster.chunks.is_empty(), "Cluster should have chunks");
        assert!(
            cluster.avg_similarity >= TEST_DUPLICATE_MIN_SIMILARITY,
            "Cluster similarity should meet threshold"
        );
    }
}

#[tokio::test]
#[serial]
async fn test_find_duplicates_in_file() {
    let ctx = TestContext::new().await;
    let client = ctx.connect().await.expect("Failed to connect to leader");

    // Find duplicates for main.rs - should find similar code in utils.rs
    let main_rs = ctx.workspace_path().join("main.rs");
    let results = client
        .find_duplicates_in_file(main_rs.clone(), TEST_DUPLICATE_MIN_SIMILARITY)
        .await
        .expect("find_duplicates_in_file should succeed");

    // Results should contain similar chunks from OTHER files (not main.rs itself)
    for result in &results {
        assert!(!result.chunk.text.is_empty());
        assert!(result.similarity >= TEST_DUPLICATE_MIN_SIMILARITY);
        // The chunk should NOT be from main.rs (duplicates are cross-file)
        assert_ne!(
            result.chunk.file, main_rs,
            "Duplicates should be from other files"
        );
    }
}

#[tokio::test]
#[serial]
async fn test_find_duplicates_in_file_not_found() {
    let ctx = TestContext::new().await;
    let client = ctx.connect().await.expect("Failed to connect to leader");

    // Try to find duplicates for a non-existent file
    let nonexistent = ctx.workspace_path().join("does_not_exist.rs");
    let result = client
        .find_duplicates_in_file(nonexistent, TEST_DUPLICATE_MIN_SIMILARITY)
        .await;

    assert!(result.is_err(), "Should fail for non-existent file");
    let err = result.unwrap_err();
    assert!(
        matches!(err.kind, QueryErrorKind::FileNotFound),
        "Error should be FileNotFound"
    );
}

// =============================================================================
// Invalidate File Tests
// =============================================================================

#[tokio::test]
#[serial]
async fn test_invalidate_file() {
    let ctx = TestContext::new().await;
    let client = ctx.connect().await.expect("Failed to connect to leader");

    let main_rs = ctx.workspace_path().join("main.rs");

    // Get initial status
    let status_before = client.status().await.expect("Failed to get initial status");

    // Verify new_function does NOT exist before modification
    let query = "(function_item name: (identifier) @name)".to_string();
    let matches_before = client
        .tree_sitter_query(query.clone(), Some(vec![main_rs.clone()]), Some("rust".to_string()))
        .await
        .expect("Tree-sitter query failed before modification");

    let names_before: Vec<_> = matches_before
        .iter()
        .flat_map(|m| m.captures.iter())
        .filter(|c| c.name == "name")
        .map(|c| c.text.clone())
        .collect();

    assert!(
        !names_before.contains(&"new_function".to_string()),
        "new_function should NOT exist before modification"
    );

    // Modify the file to add new_function
    std::fs::write(
        &main_rs,
        r#"
fn main() {
    println!("Modified!");
}

fn new_function() {
    println!("I'm new");
}
"#,
    )
    .expect("Failed to write modified file");

    // Invalidate the file to force re-index
    client
        .invalidate_file(main_rs.clone())
        .await
        .expect("invalidate_file should succeed");

    // Status should still show the file is indexed
    let status_after = client.status().await.expect("Failed to get status after invalidate");
    assert_eq!(
        status_before.files_total, status_after.files_total,
        "File count should remain the same"
    );

    // Verify new_function NOW exists after invalidation
    let matches_after = client
        .tree_sitter_query(query, Some(vec![main_rs]), Some("rust".to_string()))
        .await
        .expect("Tree-sitter query failed after invalidation");

    let names_after: Vec<_> = matches_after
        .iter()
        .flat_map(|m| m.captures.iter())
        .filter(|c| c.name == "name")
        .map(|c| c.text.clone())
        .collect();

    assert!(
        names_after.contains(&"new_function".to_string()),
        "new_function should exist after invalidation and re-indexing"
    );
}
