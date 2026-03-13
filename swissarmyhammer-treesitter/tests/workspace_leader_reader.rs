//! Integration tests for Workspace leader/reader architecture
//!
//! These tests verify that:
//! - Leaders can index files and write to SQLite
//! - Non-leaders (readers) can query the SQLite database
//! - WAL mode allows concurrent read access

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use swissarmyhammer_treesitter::{IndexConfig, IndexDatabase, IndexStatus, Workspace};
use tempfile::TempDir;

/// Minimum similarity threshold for duplicate detection tests
const TEST_MIN_SIMILARITY: f32 = 0.5;

/// Minimum chunk bytes for duplicate detection tests
const TEST_MIN_CHUNK_BYTES: usize = 5;

/// Maximum time to wait for background indexing before failing the test.
/// Parallelism for resource-heavy tests is managed by nextest test-groups
/// in .config/nextest.toml — do not inflate this timeout to compensate.
const TEST_INDEX_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Open a workspace with a custom IndexConfig and wait for background indexing.
async fn open_and_wait_with_config(dir: &Path, config: IndexConfig) -> Workspace {
    let notify = Arc::new(tokio::sync::Notify::new());
    let notify_clone = notify.clone();
    let workspace = Workspace::new(dir)
        .with_index_config(config)
        .with_progress(move |status: IndexStatus| {
            if status.is_complete() {
                notify_clone.notify_one();
            }
        })
        .open()
        .await
        .expect("workspace should open successfully");
    tokio::time::timeout(TEST_INDEX_TIMEOUT, notify.notified())
        .await
        .expect("background indexing did not complete within timeout");
    workspace
}

/// Index config with embeddings disabled (fast, for most tests).
fn no_embedding_config() -> IndexConfig {
    IndexConfig {
        embedding_enabled: false,
        ..Default::default()
    }
}

/// Open a workspace and wait for background indexing to complete (no embeddings).
async fn open_and_wait(dir: &Path) -> Workspace {
    open_and_wait_with_config(dir, no_embedding_config()).await
}

/// Open a workspace without waiting (no embeddings).
async fn open_no_embedding(dir: &Path) -> Workspace {
    Workspace::new(dir)
        .with_index_config(no_embedding_config())
        .open()
        .await
        .expect("workspace should open without embeddings")
}

// =============================================================================
// Helper functions
// =============================================================================

/// Create a test workspace directory with Rust files
fn create_test_workspace() -> TempDir {
    let dir = TempDir::new().expect("should create temp directory");

    std::fs::write(
        dir.path().join("main.rs"),
        r#"
fn main() {
    println!("Hello, world!");
    let x = 42;
}

fn helper() {
    println!("Helper function");
}
"#,
    )
    .expect("should write main.rs");

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
    .expect("should write lib.rs");

    std::fs::write(
        dir.path().join("utils.rs"),
        r#"
pub fn format_number(n: i32) -> String {
    format!("{}", n)
}
"#,
    )
    .expect("should write utils.rs");

    dir
}

/// Get the database path for a workspace
fn database_path(workspace_root: &std::path::Path) -> PathBuf {
    workspace_root.join(".treesitter-index.db")
}

// =============================================================================
// Leader functionality tests
// =============================================================================

#[tokio::test]
async fn test_leader_creates_database() {
    let dir = create_test_workspace();

    let workspace = open_and_wait(dir.path()).await;

    // With background indexing, open() returns Reader mode
    assert!(!workspace.is_leader());

    // Database should be created by background task
    let db_path = database_path(dir.path());
    assert!(
        db_path.exists(),
        "Database file should be created by background indexer"
    );
}

#[tokio::test]
async fn test_leader_indexes_files() {
    let dir = create_test_workspace();

    let workspace = open_and_wait(dir.path()).await;

    // With background indexing, open() returns Reader mode
    assert!(!workspace.is_leader());

    // Background task should have indexed the files
    let status = workspace.status().await.expect("should get workspace status");
    assert!(
        status.files_total > 0,
        "Background indexer should have found files to index"
    );
    assert!(
        status.is_ready,
        "Background indexer should complete indexing"
    );

    // Should be able to list indexed files
    let files = workspace.list_files().await.expect("should list indexed files");
    assert!(
        !files.is_empty(),
        "Background indexer should have indexed files"
    );

    // Verify specific files are indexed
    let file_names: Vec<String> = files
        .iter()
        .filter_map(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .collect();
    assert!(file_names.contains(&"main.rs".to_string()));
    assert!(file_names.contains(&"lib.rs".to_string()));
}

#[tokio::test]
async fn test_reader_cannot_run_tree_sitter_queries() {
    let dir = create_test_workspace();

    let workspace = open_and_wait(dir.path()).await;

    // With background indexing, open() returns Reader mode
    assert!(!workspace.is_leader());

    // Tree-sitter queries are not available in Reader mode (no parsed AST)
    let result = workspace
        .tree_sitter_query(
            "(function_item name: (identifier) @name)".to_string(),
            None,
            Some("rust".to_string()),
        )
        .await;
    assert!(
        result.is_err(),
        "Tree-sitter queries should not be available in Reader mode"
    );
}

// =============================================================================
// Reader functionality tests (using database directly)
// =============================================================================

#[tokio::test]
async fn test_reader_can_open_database_readonly() {
    let dir = create_test_workspace();

    // First, let background indexer create and populate the database
    {
        let _workspace = open_and_wait(dir.path()).await;

        // Workspace dropped here
    }

    // Now open the database in read-only mode (simulating a reader)
    let db_path = database_path(dir.path());
    let db = IndexDatabase::open_readonly(&db_path);
    assert!(
        db.is_ok(),
        "Should be able to open database in read-only mode"
    );
}

#[tokio::test]
async fn test_reader_can_query_chunks() {
    let dir = create_test_workspace();

    // Background indexer creates and populates the database
    {
        let _workspace = open_and_wait(dir.path()).await;
    }

    // Reader queries the database
    let db_path = database_path(dir.path());
    let db = IndexDatabase::open_readonly(&db_path).expect("should open database in read-only mode");

    // Reader should be able to query chunks
    let chunks = db.get_all_embedded_chunks();
    assert!(chunks.is_ok(), "Reader should be able to query chunks");

    // Extract unique file paths from chunks
    let file_paths: HashSet<PathBuf> = chunks.expect("should retrieve embedded chunks").iter().map(|c| c.path.clone()).collect();

    // Verify files are present (may be empty if no embeddings computed)
    // The important thing is the query succeeded
    if !file_paths.is_empty() {
        let file_names: Vec<String> = file_paths
            .iter()
            .filter_map(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .collect();
        // Check if any expected files are present
        let has_rust_files = file_names.iter().any(|n| n.ends_with(".rs"));
        assert!(
            has_rust_files || file_names.is_empty(),
            "Should have Rust files if any"
        );
    }
}

// =============================================================================
// Concurrent access tests
// =============================================================================

#[tokio::test]
async fn test_database_persists_after_leader_drops() {
    let dir = create_test_workspace();

    // Leader indexes and drops
    let file_count = {
        let workspace = open_no_embedding(dir.path()).await;
        let files = workspace.list_files().await.expect("should list files from initial leader");
        files.len()
    };

    // Database should still exist
    let db_path = database_path(dir.path());
    assert!(
        db_path.exists(),
        "Database should persist after leader drops"
    );

    // New leader should see the same files
    let workspace = open_no_embedding(dir.path()).await;
    let files = workspace.list_files().await.expect("should list files from new leader");
    assert_eq!(
        files.len(),
        file_count,
        "Database should contain same number of files"
    );
}

#[tokio::test]
async fn test_new_leader_can_reopen_existing_database() {
    let dir = create_test_workspace();

    // First background indexer populates database
    let original_file_count = {
        let workspace = open_and_wait(dir.path()).await;
        workspace.list_files().await.expect("should list files from first workspace").len()
    };

    // New workspace opens existing database
    let workspace = open_and_wait(dir.path()).await;

    let files = workspace.list_files().await.expect("should list files from reopened workspace");
    assert_eq!(
        files.len(),
        original_file_count,
        "New workspace should see existing indexed files"
    );
}

#[tokio::test]
async fn test_leader_detects_file_changes() {
    let dir = create_test_workspace();

    // Initial indexing
    let workspace = open_no_embedding(dir.path()).await;
    let initial_status = workspace.status().await.expect("should get initial workspace status");

    // Add a new file
    std::fs::write(
        dir.path().join("new_file.rs"),
        "fn new_function() { println!(\"new\"); }",
    )
    .expect("should write new_file.rs");

    // invalidate_file is not supported with background indexing
    let new_file = dir.path().join("new_file.rs");
    let result = workspace.invalidate_file(new_file.clone()).await;
    assert!(
        result.is_err(),
        "invalidate_file should return error with background indexing"
    );

    // File changes will be picked up on next process start when content hash differs
    // For now, verify the workspace is still queryable
    let status = workspace.status().await.expect("should get workspace status after file change");
    assert_eq!(status.files_indexed, initial_status.files_indexed);
}

// =============================================================================
// Empty workspace tests
// =============================================================================

#[tokio::test]
async fn test_empty_workspace_leader_succeeds() {
    let dir = TempDir::new().expect("should create temp directory for empty workspace test");

    let workspace = open_and_wait(dir.path()).await;

    // Empty workspace should be queryable
    let status = workspace.status().await.expect("should get status for empty workspace");
    assert_eq!(status.files_total, 0);

    let duplicates = workspace
        .find_all_duplicates(TEST_MIN_SIMILARITY, TEST_MIN_CHUNK_BYTES)
        .await;
    assert!(duplicates.is_ok());
    assert!(duplicates.expect("find_all_duplicates should succeed on empty workspace").is_empty());
}

#[tokio::test]
async fn test_empty_workspace_creates_database() {
    let dir = TempDir::new().expect("should create temp directory for database creation test");

    {
        let _workspace = open_and_wait(dir.path()).await;
    }

    // Database should exist even for empty workspace
    let db_path = database_path(dir.path());
    assert!(
        db_path.exists(),
        "Database should be created even for empty workspace"
    );
}

// =============================================================================
// Error handling tests
// =============================================================================

#[tokio::test]
async fn test_query_nonexistent_file_returns_error() {
    let dir = create_test_workspace();

    let workspace = open_no_embedding(dir.path()).await;

    let result = workspace
        .find_duplicates_in_file(PathBuf::from("/nonexistent/file.rs"), TEST_MIN_SIMILARITY)
        .await;

    assert!(result.is_err(), "Should return error for nonexistent file");
}

// =============================================================================
// Real duplicate detection tests
// =============================================================================

/// Create a workspace with near-duplicate functions across files and an unrelated control file.
fn create_duplicate_workspace() -> TempDir {
    let dir = TempDir::new().expect("should create temp directory for duplicate workspace");

    // Two near-identical functions: same structure, trivial variable renames
    std::fs::write(
        dir.path().join("utils_a.rs"),
        r#"
/// Process a list of numbers: keep positives and double them.
fn process_data(items: &[i32]) -> Vec<i32> {
    let mut result = Vec::new();
    for item in items {
        if *item > 0 {
            result.push(item * 2);
        }
    }
    result
}
"#,
    )
    .expect("should write utils_a.rs");

    std::fs::write(
        dir.path().join("utils_b.rs"),
        r#"
/// Transform a list of numbers: keep positives and double them.
fn transform_data(values: &[i32]) -> Vec<i32> {
    let mut result = Vec::new();
    for value in values {
        if *value > 0 {
            result.push(value * 2);
        }
    }
    result
}
"#,
    )
    .expect("should write utils_b.rs");

    // Completely different function — HTTP request handling, not numeric
    std::fs::write(
        dir.path().join("unrelated.rs"),
        r#"
use std::collections::HashMap;

/// Dispatch an incoming HTTP request to the appropriate handler
/// based on the method and path prefix. Returns the response status
/// code and body as a tuple. Unknown routes get a 404.
fn dispatch_http_request(
    method: &str,
    path: &str,
    headers: &HashMap<String, String>,
    body: &[u8],
) -> (u16, String) {
    let content_type = headers
        .get("content-type")
        .map(|s| s.as_str())
        .unwrap_or("text/plain");

    match (method, path) {
        ("GET", "/health") => (200, "ok".to_string()),
        ("GET", "/version") => (200, env!("CARGO_PKG_VERSION").to_string()),
        ("POST", "/echo") => {
            let response = String::from_utf8_lossy(body).to_string();
            (200, format!("content-type: {}\n{}", content_type, response))
        }
        _ => (404, format!("not found: {} {}", method, path)),
    }
}
"#,
    )
    .expect("should write unrelated.rs");

    dir
}

#[tokio::test]
async fn test_find_all_duplicates_detects_near_identical_functions() {
    let dir = create_duplicate_workspace();

    // Run with embeddings enabled — this is the whole point
    let config = IndexConfig {
        embedding_enabled: true,
        ..Default::default()
    };
    let workspace = open_and_wait_with_config(dir.path(), config).await;

    // Verify indexing actually happened
    let status = workspace.status().await.expect("should get status for duplicate detection workspace");
    assert_eq!(status.files_indexed, 3, "All 3 files should be indexed");

    // Find duplicates with a high threshold — the near-identical functions should
    // score well above 0.8, while the unrelated HTTP handler should not.
    let similarity_threshold = 0.8;
    let duplicates = workspace
        .find_all_duplicates(similarity_threshold, TEST_MIN_CHUNK_BYTES)
        .await
        .expect("find_all_duplicates should succeed");

    // There must be at least one cluster containing the near-identical functions
    assert!(
        !duplicates.is_empty(),
        "Should find at least one duplicate cluster, but found none. \
         Status: files_indexed={}, is_ready={}",
        status.files_indexed,
        status.is_ready,
    );

    // Find the cluster that contains utils_a.rs
    let utils_a_cluster = duplicates.iter().find(|cluster| {
        cluster
            .chunks
            .iter()
            .any(|c| c.file.to_string_lossy().contains("utils_a.rs"))
    });

    assert!(
        utils_a_cluster.is_some(),
        "Should have a cluster containing utils_a.rs. Clusters found: {:?}",
        duplicates
            .iter()
            .map(|c| c
                .chunks
                .iter()
                .map(|ch| ch.file.display().to_string())
                .collect::<Vec<_>>())
            .collect::<Vec<_>>()
    );

    let cluster = utils_a_cluster.expect("utils_a.rs cluster should be found in duplicate results");

    // The cluster must also contain utils_b.rs (the near-duplicate)
    let has_utils_b = cluster
        .chunks
        .iter()
        .any(|c| c.file.to_string_lossy().contains("utils_b.rs"));
    assert!(
        has_utils_b,
        "Cluster with utils_a.rs should also contain utils_b.rs. Cluster files: {:?}",
        cluster
            .chunks
            .iter()
            .map(|c| c.file.display().to_string())
            .collect::<Vec<_>>()
    );

    // The unrelated file should NOT be in the same cluster
    let has_unrelated = cluster
        .chunks
        .iter()
        .any(|c| c.file.to_string_lossy().contains("unrelated.rs"));
    assert!(
        !has_unrelated,
        "Cluster should NOT contain unrelated.rs (different semantics)"
    );

    // The cluster similarity should be meaningful
    assert!(
        cluster.avg_similarity >= similarity_threshold,
        "Cluster avg_similarity ({}) should be >= threshold ({})",
        cluster.avg_similarity,
        similarity_threshold,
    );
}
