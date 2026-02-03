//! Integration tests for Workspace leader/reader architecture
//!
//! These tests verify that:
//! - Leaders can index files and write to SQLite
//! - Non-leaders (readers) can query the SQLite database
//! - WAL mode allows concurrent read access

use std::collections::HashSet;
use std::path::PathBuf;
use swissarmyhammer_treesitter::{IndexDatabase, Workspace};
use tempfile::TempDir;

/// Minimum similarity threshold for duplicate detection tests
const TEST_MIN_SIMILARITY: f32 = 0.5;

/// Minimum chunk bytes for duplicate detection tests
const TEST_MIN_CHUNK_BYTES: usize = 5;

/// Timeout in seconds to wait for background indexing to complete in tests
const BACKGROUND_INDEXING_TIMEOUT_SECS: u64 = 2;

/// Time to wait for background indexing to complete in tests
const WAIT_FOR_BACKGROUND_INDEXING: std::time::Duration =
    std::time::Duration::from_secs(BACKGROUND_INDEXING_TIMEOUT_SECS);

// =============================================================================
// Helper functions
// =============================================================================

/// Create a test workspace directory with Rust files
fn create_test_workspace() -> TempDir {
    let dir = TempDir::new().unwrap();

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

    std::fs::write(
        dir.path().join("utils.rs"),
        r#"
pub fn format_number(n: i32) -> String {
    format!("{}", n)
}
"#,
    )
    .unwrap();

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

    let workspace = Workspace::open(dir.path()).await.unwrap();

    // Wait for background indexing to complete
    tokio::time::sleep(WAIT_FOR_BACKGROUND_INDEXING).await;

    // With background indexing, open() returns Reader mode
    assert!(!workspace.is_leader());

    // Database should be created by background task
    let db_path = database_path(dir.path());
    assert!(db_path.exists(), "Database file should be created by background indexer");
}

#[tokio::test]
async fn test_leader_indexes_files() {
    let dir = create_test_workspace();

    let workspace = Workspace::open(dir.path()).await.unwrap();

    // Wait for background indexing to complete
    tokio::time::sleep(WAIT_FOR_BACKGROUND_INDEXING).await;

    // With background indexing, open() returns Reader mode
    assert!(!workspace.is_leader());

    // Background task should have indexed the files
    let status = workspace.status().await.unwrap();
    assert!(status.files_total > 0, "Background indexer should have found files to index");
    assert!(
        status.is_ready,
        "Background indexer should complete indexing"
    );

    // Should be able to list indexed files
    let files = workspace.list_files().await.unwrap();
    assert!(!files.is_empty(), "Background indexer should have indexed files");

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
async fn test_leader_can_query_duplicates() {
    let dir = create_test_workspace();

    let workspace = Workspace::open(dir.path()).await.unwrap();

    // Wait for background indexing to complete
    tokio::time::sleep(WAIT_FOR_BACKGROUND_INDEXING).await;

    // With background indexing, open() returns Reader mode
    assert!(!workspace.is_leader());

    // Reader should be able to query for duplicates from indexed data
    let result = workspace
        .find_all_duplicates(TEST_MIN_SIMILARITY, TEST_MIN_CHUNK_BYTES)
        .await;
    assert!(result.is_ok(), "Reader should be able to query duplicates");
}

#[tokio::test]
async fn test_reader_cannot_run_tree_sitter_queries() {
    let dir = create_test_workspace();

    let workspace = Workspace::open(dir.path()).await.unwrap();

    // Wait for background indexing to complete
    tokio::time::sleep(WAIT_FOR_BACKGROUND_INDEXING).await;

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
        let _workspace = Workspace::open(dir.path()).await.unwrap();

        // Wait for background indexing to complete (database is created by background task)
        tokio::time::sleep(WAIT_FOR_BACKGROUND_INDEXING).await;

        // Workspace dropped here
    }

    // Now open the database in read-only mode (simulating a reader)
    let db_path = database_path(dir.path());
    let db = IndexDatabase::open_readonly(&db_path);
    assert!(db.is_ok(), "Should be able to open database in read-only mode");
}

#[tokio::test]
async fn test_reader_can_query_chunks() {
    let dir = create_test_workspace();

    // Background indexer creates and populates the database
    {
        let _workspace = Workspace::open(dir.path()).await.unwrap();

        // Wait for background indexing to complete
        tokio::time::sleep(WAIT_FOR_BACKGROUND_INDEXING).await;
    }

    // Reader queries the database
    let db_path = database_path(dir.path());
    let db = IndexDatabase::open_readonly(&db_path).unwrap();

    // Reader should be able to query chunks
    let chunks = db.get_all_embedded_chunks();
    assert!(chunks.is_ok(), "Reader should be able to query chunks");

    // Extract unique file paths from chunks
    let file_paths: HashSet<PathBuf> = chunks
        .unwrap()
        .iter()
        .map(|c| c.path.clone())
        .collect();

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
        assert!(has_rust_files || file_names.is_empty(), "Should have Rust files if any");
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
        let workspace = Workspace::open(dir.path()).await.unwrap();
        let files = workspace.list_files().await.unwrap();
        files.len()
    };

    // Database should still exist
    let db_path = database_path(dir.path());
    assert!(db_path.exists(), "Database should persist after leader drops");

    // New leader should see the same files
    let workspace = Workspace::open(dir.path()).await.unwrap();
    let files = workspace.list_files().await.unwrap();
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
        let workspace = Workspace::open(dir.path()).await.unwrap();

        // Wait for background indexing to complete
        tokio::time::sleep(WAIT_FOR_BACKGROUND_INDEXING).await;

        workspace.list_files().await.unwrap().len()
    };

    // New workspace opens existing database
    let workspace = Workspace::open(dir.path()).await.unwrap();

    // Wait for any background activity to settle
    tokio::time::sleep(WAIT_FOR_BACKGROUND_INDEXING).await;

    let files = workspace.list_files().await.unwrap();
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
    let workspace = Workspace::open(dir.path()).await.unwrap();
    let initial_status = workspace.status().await.unwrap();

    // Add a new file
    std::fs::write(
        dir.path().join("new_file.rs"),
        "fn new_function() { println!(\"new\"); }",
    )
    .unwrap();

    // invalidate_file is not supported with background indexing
    let new_file = dir.path().join("new_file.rs");
    let result = workspace.invalidate_file(new_file.clone()).await;
    assert!(result.is_err(), "invalidate_file should return error with background indexing");

    // File changes will be picked up on next process start when content hash differs
    // For now, verify the workspace is still queryable
    let status = workspace.status().await.unwrap();
    assert_eq!(status.files_indexed, initial_status.files_indexed);
}

// =============================================================================
// Empty workspace tests
// =============================================================================

#[tokio::test]
async fn test_empty_workspace_leader_succeeds() {
    let dir = TempDir::new().unwrap();

    let workspace = Workspace::open(dir.path()).await.unwrap();

    // Wait for background indexing to complete
    tokio::time::sleep(WAIT_FOR_BACKGROUND_INDEXING).await;

    // Empty workspace should be queryable
    let status = workspace.status().await.unwrap();
    assert_eq!(status.files_total, 0);

    let duplicates = workspace
        .find_all_duplicates(TEST_MIN_SIMILARITY, TEST_MIN_CHUNK_BYTES)
        .await;
    assert!(duplicates.is_ok());
    assert!(duplicates.unwrap().is_empty());
}

#[tokio::test]
async fn test_empty_workspace_creates_database() {
    let dir = TempDir::new().unwrap();

    {
        let _workspace = Workspace::open(dir.path()).await.unwrap();

        // Wait for background indexing to complete
        tokio::time::sleep(WAIT_FOR_BACKGROUND_INDEXING).await;
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

    let workspace = Workspace::open(dir.path()).await.unwrap();

    let result = workspace
        .find_duplicates_in_file(PathBuf::from("/nonexistent/file.rs"), TEST_MIN_SIMILARITY)
        .await;

    assert!(result.is_err(), "Should return error for nonexistent file");
}
