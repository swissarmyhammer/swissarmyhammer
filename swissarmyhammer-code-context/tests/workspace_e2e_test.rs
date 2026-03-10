//! End-to-end workspace tests with real filesystem discovery.
//!
//! These tests verify that the code_context tool works with actual file discovery
//! from the filesystem, not mocked data. Each test creates a temporary project
//! directory with real source files and verifies the indexing pipeline.

use swissarmyhammer_code_context::CodeContextWorkspace;
use tempfile::TempDir;

/// Helper: Create a temporary Rust project with source files
fn create_test_project() -> TempDir {
    let tmp = TempDir::new().expect("Failed to create temp dir");
    let root = tmp.path();

    // Create Cargo.toml
    let cargo_toml = r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
"#;
    std::fs::write(root.join("Cargo.toml"), cargo_toml).unwrap();

    // Create src/main.rs
    let main_rs = r#"use std::collections::HashMap;

fn main() {
    let mut map = HashMap::new();
    process_data(&mut map);
    print_result(&map);
}

fn process_data(data: &mut HashMap<String, i32>) {
    data.insert("key1".to_string(), 42);
    helper_function();
}

fn helper_function() {
    println!("Helper called");
}

fn print_result(data: &HashMap<String, i32>) {
    for (k, v) in data {
        println!("{}: {}", k, v);
    }
}
"#;
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/main.rs"), main_rs).unwrap();

    // Create src/lib.rs
    let lib_rs = r#"pub struct Config {
    pub name: String,
    pub value: i32,
}

impl Config {
    pub fn new(name: String, value: i32) -> Self {
        Self { name, value }
    }

    pub fn display(&self) -> String {
        format!("{}: {}", self.name, self.value)
    }
}

pub fn validate_config(config: &Config) -> bool {
    config.value > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config() {
        let cfg = Config::new("test".to_string(), 42);
        assert!(validate_config(&cfg));
    }
}
"#;
    std::fs::write(root.join("src/lib.rs"), lib_rs).unwrap();

    // Create src/utils.rs
    let utils_rs = r#"pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

pub fn is_positive(n: i32) -> bool {
    n > 0
}
"#;
    std::fs::write(root.join("src/utils.rs"), utils_rs).unwrap();

    tmp
}

// ---------------------------------------------------------------------------
// Test 1: Workspace initialization and startup_cleanup
// ---------------------------------------------------------------------------

#[test]
fn test_workspace_startup_discovers_files() {
    let project = create_test_project();
    let root = project.path();

    // Open the workspace (becomes leader, initializes DB)
    let ws = CodeContextWorkspace::open(root).expect("Failed to open workspace");
    assert!(ws.is_leader(), "First process should be leader");

    // startup_cleanup was already called automatically in workspace.open()
    // Call it again to verify it handles existing files correctly
    let stats = swissarmyhammer_code_context::startup_cleanup(ws.db(), root)
        .expect("startup_cleanup failed");

    // Files should be unchanged since they were already discovered in workspace.open()
    assert_eq!(
        stats.files_added, 0,
        "startup_cleanup should have been called automatically in workspace.open()"
    );
    assert_eq!(stats.files_removed, 0);
    assert_eq!(stats.files_dirty, 0);
    assert_eq!(
        stats.files_unchanged, 4,
        "Expected 4 unchanged files: Cargo.toml, main.rs, lib.rs, utils.rs, got {}",
        stats.files_unchanged
    );

    // Verify get_status returns the correct counts
    let status = swissarmyhammer_code_context::get_status(ws.db())
        .expect("get_status failed");

    assert_eq!(
        status.total_files, 4,
        "Status should report 4 total files, got {}",
        status.total_files
    );
}

// ---------------------------------------------------------------------------
// Test 2: Running startup_cleanup twice is idempotent
// ---------------------------------------------------------------------------

#[test]
fn test_startup_cleanup_idempotent() {
    let project = create_test_project();
    let root = project.path();

    let ws = CodeContextWorkspace::open(root).expect("Failed to open workspace");

    // First run: files already discovered by workspace.open()
    let stats1 = swissarmyhammer_code_context::startup_cleanup(ws.db(), root)
        .expect("First startup_cleanup failed");
    assert_eq!(stats1.files_unchanged, 4, "Files should be unchanged after workspace.open() auto-discovered them");

    // Second run: all files should be unchanged
    let stats2 = swissarmyhammer_code_context::startup_cleanup(ws.db(), root)
        .expect("Second startup_cleanup failed");
    assert_eq!(
        stats2.files_added, 0,
        "Second run should not add new files"
    );
    assert_eq!(
        stats2.files_removed, 0,
        "Second run should not remove files"
    );
    assert_eq!(
        stats2.files_dirty, 0,
        "Second run should not mark files dirty"
    );
    assert_eq!(
        stats2.files_unchanged, 4,
        "Second run should see all files unchanged"
    );
}

// ---------------------------------------------------------------------------
// Test 3: File modification detection
// ---------------------------------------------------------------------------

#[test]
fn test_startup_cleanup_detects_modifications() {
    let project = create_test_project();
    let root = project.path();

    let ws = CodeContextWorkspace::open(root).expect("Failed to open workspace");

    // First run: files already discovered by workspace.open()
    let stats1 = swissarmyhammer_code_context::startup_cleanup(ws.db(), root)
        .expect("First startup_cleanup failed");
    assert_eq!(stats1.files_unchanged, 4, "Files should be unchanged after workspace.open() auto-discovered them");

    // Modify one file
    let main_rs_path = root.join("src/main.rs");
    let modified_content = r#"use std::collections::HashMap;

fn main() {
    let mut map = HashMap::new();
    process_data(&mut map);
    print_result(&map);
    println!("Modified!");
}

fn process_data(data: &mut HashMap<String, i32>) {
    data.insert("key1".to_string(), 42);
    helper_function();
}

fn helper_function() {
    println!("Helper called");
}

fn print_result(data: &HashMap<String, i32>) {
    for (k, v) in data {
        println!("{}: {}", k, v);
    }
}
"#;
    std::fs::write(&main_rs_path, modified_content).unwrap();

    // Second run: should detect the modification
    let stats2 = swissarmyhammer_code_context::startup_cleanup(ws.db(), root)
        .expect("Second startup_cleanup failed");
    assert_eq!(
        stats2.files_dirty, 1,
        "Should detect 1 modified file, got {}",
        stats2.files_dirty
    );
    assert_eq!(stats2.files_added, 0);
    assert_eq!(stats2.files_removed, 0);
    assert_eq!(stats2.files_unchanged, 3);
}

// ---------------------------------------------------------------------------
// Test 4: File deletion detection
// ---------------------------------------------------------------------------

#[test]
fn test_startup_cleanup_detects_deletions() {
    let project = create_test_project();
    let root = project.path();

    let ws = CodeContextWorkspace::open(root).expect("Failed to open workspace");

    // First run: files already discovered by workspace.open()
    let stats1 = swissarmyhammer_code_context::startup_cleanup(ws.db(), root)
        .expect("First startup_cleanup failed");
    assert_eq!(stats1.files_unchanged, 4, "Files should be unchanged after workspace.open() auto-discovered them");

    // Delete one file
    std::fs::remove_file(root.join("src/utils.rs")).unwrap();

    // Second run: should detect the deletion
    let stats2 = swissarmyhammer_code_context::startup_cleanup(ws.db(), root)
        .expect("Second startup_cleanup failed");
    assert_eq!(
        stats2.files_removed, 1,
        "Should detect 1 deleted file, got {}",
        stats2.files_removed
    );
    assert_eq!(stats2.files_added, 0);
    assert_eq!(stats2.files_dirty, 0);
    assert_eq!(stats2.files_unchanged, 3);

    // Verify status reflects the deletion
    let status = swissarmyhammer_code_context::get_status(ws.db())
        .expect("get_status failed");
    assert_eq!(
        status.total_files, 3,
        "Status should report 3 files after deletion"
    );
}

// ---------------------------------------------------------------------------
// Test 5: New file addition detection
// ---------------------------------------------------------------------------

#[test]
fn test_startup_cleanup_detects_new_files() {
    let project = create_test_project();
    let root = project.path();

    let ws = CodeContextWorkspace::open(root).expect("Failed to open workspace");

    // First run: files already discovered by workspace.open()
    let stats1 = swissarmyhammer_code_context::startup_cleanup(ws.db(), root)
        .expect("First startup_cleanup failed");
    assert_eq!(stats1.files_unchanged, 4, "Files should be unchanged after workspace.open() auto-discovered them");

    // Add a new file
    let new_module = r#"pub fn new_function() -> String {
    "Hello from new module".to_string()
}
"#;
    std::fs::write(root.join("src/new_module.rs"), new_module).unwrap();

    // Second run: should detect the new file
    let stats2 = swissarmyhammer_code_context::startup_cleanup(ws.db(), root)
        .expect("Second startup_cleanup failed");
    assert_eq!(
        stats2.files_added, 1,
        "Should detect 1 new file, got {}",
        stats2.files_added
    );
    assert_eq!(stats2.files_removed, 0);
    assert_eq!(stats2.files_dirty, 0);
    assert_eq!(stats2.files_unchanged, 4);

    // Verify status reflects the new file
    let status = swissarmyhammer_code_context::get_status(ws.db())
        .expect("get_status failed");
    assert_eq!(
        status.total_files, 5,
        "Status should report 5 files after adding new file"
    );
}

// ---------------------------------------------------------------------------
// Test 6: Status reflects file counts
// ---------------------------------------------------------------------------

#[test]
fn test_get_status_reflects_file_counts() {
    let project = create_test_project();
    let root = project.path();

    let ws = CodeContextWorkspace::open(root).expect("Failed to open workspace");
    let _stats = swissarmyhammer_code_context::startup_cleanup(ws.db(), root)
        .expect("startup_cleanup failed");

    let status = swissarmyhammer_code_context::get_status(ws.db())
        .expect("get_status failed");

    // After startup_cleanup, status should accurately report counts
    assert_eq!(status.total_files, 4);
    assert_eq!(status.ts_indexed_files, 0, "No files indexed yet");
    assert_eq!(status.lsp_indexed_files, 0, "No files indexed yet");
    assert_eq!(status.ts_indexed_percent, 0.0);
    assert_eq!(status.lsp_indexed_percent, 0.0);
}

// ---------------------------------------------------------------------------
// Test 7: Indexing worker marks files as indexed
// ---------------------------------------------------------------------------

#[test]
fn test_indexing_worker_marks_files_indexed() {
    use std::thread;
    use std::time::Duration;

    let project = create_test_project();
    let root = project.path();

    // Open workspace
    let ws = CodeContextWorkspace::open(root).expect("Failed to open workspace");
    assert!(ws.is_leader(), "Should be leader");

    // Populate dirty files
    let cleanup_stats = swissarmyhammer_code_context::startup_cleanup(ws.db(), root)
        .expect("startup_cleanup failed");
    println!("startup_cleanup: added={}, removed={}, dirty={}, unchanged={}",
        cleanup_stats.files_added, cleanup_stats.files_removed, cleanup_stats.files_dirty, cleanup_stats.files_unchanged);

    // Verify files are in DB before indexing
    let before = swissarmyhammer_code_context::get_status(ws.db())
        .expect("get_status failed");
    println!("Before indexing: total={}, ts_indexed={}, lsp_indexed={}",
        before.total_files, before.ts_indexed_files, before.lsp_indexed_files);
    assert_eq!(before.total_files, 4, "Should have 4 files in database");
    assert_eq!(before.ts_indexed_files, 0, "No files should be indexed initially");

    // Explicitly spawn the indexing worker (no longer auto-started by workspace open)
    let db_path = root.join(".code-context").join("index.db");
    swissarmyhammer_code_context::indexing::spawn_indexing_worker(
        root.to_path_buf(),
        db_path,
        swissarmyhammer_code_context::indexing::IndexingConfig::default(),
    );

    // Give indexing worker time to run (it's in a background thread)
    thread::sleep(Duration::from_secs(2));

    // Check status after indexing worker runs
    let after = swissarmyhammer_code_context::get_status(ws.db())
        .expect("get_status failed");
    println!("After indexing: total={}, ts_indexed={}, lsp_indexed={}",
        after.total_files, after.ts_indexed_files, after.lsp_indexed_files);

    // Indexing worker should have marked files as indexed
    // Even if tree-sitter parsing is placeholder, files should be marked ts_indexed=1
    if after.ts_indexed_files == 0 {
        panic!("Indexing worker did not mark any files as indexed!");
    }

    // Should have indexed all 4 files
    assert_eq!(
        after.ts_indexed_files, 4,
        "All 4 files should be marked as indexed"
    );
    assert_eq!(after.ts_indexed_percent, 100.0, "All files should be 100% indexed");
}

// ---------------------------------------------------------------------------
// Test 8: TS indexing produces real chunks and leaves LSP unindexed
// ---------------------------------------------------------------------------

#[test]
fn test_ts_indexing_produces_chunks_and_leaves_lsp_unindexed() {
    use std::thread;
    use std::time::Duration;

    let project = create_test_project();
    let root = project.path();

    // Open workspace and populate dirty files
    let ws = CodeContextWorkspace::open(root).expect("Failed to open workspace");
    assert!(ws.is_leader(), "Should be leader");
    let _stats = swissarmyhammer_code_context::startup_cleanup(ws.db(), root)
        .expect("startup_cleanup failed");

    // Spawn the indexing worker and wait for it to finish
    let db_path = root.join(".code-context").join("index.db");
    swissarmyhammer_code_context::indexing::spawn_indexing_worker(
        root.to_path_buf(),
        db_path,
        swissarmyhammer_code_context::indexing::IndexingConfig::default(),
    );
    thread::sleep(Duration::from_secs(2));

    let db = ws.db();

    // ---------------------------------------------------------------
    // 1. Every file should have ts_indexed=1 and lsp_indexed=0
    // ---------------------------------------------------------------
    let mut stmt = db
        .prepare("SELECT file_path, ts_indexed, lsp_indexed FROM indexed_files")
        .expect("Failed to prepare indexed_files query");
    let rows: Vec<(String, i64, i64)> = stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .expect("query_map failed")
        .collect::<Result<Vec<_>, _>>()
        .expect("row collection failed");

    assert!(!rows.is_empty(), "indexed_files should not be empty");
    for (path, ts, lsp) in &rows {
        assert_eq!(
            *ts, 1,
            "ts_indexed should be 1 for {}, got {}",
            path, ts
        );
        assert_eq!(
            *lsp, 0,
            "lsp_indexed should still be 0 for {}, got {}",
            path, lsp
        );
    }

    // ---------------------------------------------------------------
    // 2. ts_chunks table should have rows
    // ---------------------------------------------------------------
    let total_chunks: i64 = db
        .query_row("SELECT COUNT(*) FROM ts_chunks", [], |r| r.get(0))
        .expect("Failed to count ts_chunks");
    assert!(
        total_chunks > 0,
        "ts_chunks should contain rows after indexing, got 0"
    );
    println!("Total ts_chunks rows: {}", total_chunks);

    // ---------------------------------------------------------------
    // 3. Chunks should span multiple distinct files (at least the .rs files)
    // ---------------------------------------------------------------
    let distinct_files: i64 = db
        .query_row(
            "SELECT COUNT(DISTINCT file_path) FROM ts_chunks",
            [],
            |r| r.get(0),
        )
        .expect("Failed to count distinct file_path in ts_chunks");
    assert!(
        distinct_files >= 3,
        "ts_chunks should cover at least 3 distinct .rs files, got {}",
        distinct_files
    );
    println!("Distinct files with chunks: {}", distinct_files);

    // ---------------------------------------------------------------
    // 4. Chunks from lib.rs should contain real parsed content
    //    (e.g. the struct name "Config" or function "validate_config")
    // ---------------------------------------------------------------
    let mut stmt = db
        .prepare("SELECT text FROM ts_chunks WHERE file_path LIKE '%lib.rs'")
        .expect("Failed to prepare lib.rs chunks query");
    let lib_texts: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .expect("query_map failed")
        .collect::<Result<Vec<_>, _>>()
        .expect("row collection failed");

    assert!(
        !lib_texts.is_empty(),
        "Should have ts_chunks for lib.rs"
    );

    let all_lib_text = lib_texts.join("\n");
    let has_config = all_lib_text.contains("Config");
    let has_validate = all_lib_text.contains("validate_config");
    assert!(
        has_config || has_validate,
        "lib.rs chunks should contain 'Config' or 'validate_config', but got:\n{}",
        all_lib_text
    );

    // ---------------------------------------------------------------
    // 5. Every chunk row should have valid byte/line metadata
    // ---------------------------------------------------------------
    let bad_chunks: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM ts_chunks
             WHERE start_byte IS NULL
                OR end_byte IS NULL
                OR start_line IS NULL
                OR end_line IS NULL
                OR text IS NULL
                OR end_byte < start_byte
                OR end_line < start_line",
            [],
            |r| r.get(0),
        )
        .expect("Failed to query for bad chunks");
    assert_eq!(
        bad_chunks, 0,
        "All chunks should have valid non-null metadata with end >= start, found {} bad rows",
        bad_chunks
    );
}
