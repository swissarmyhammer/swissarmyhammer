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
    let stats = swissarmyhammer_code_context::startup_cleanup(&ws.db(), root)
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
    let status = swissarmyhammer_code_context::get_status(&ws.db()).expect("get_status failed");

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
    let stats1 = swissarmyhammer_code_context::startup_cleanup(&ws.db(), root)
        .expect("First startup_cleanup failed");
    assert_eq!(
        stats1.files_unchanged, 4,
        "Files should be unchanged after workspace.open() auto-discovered them"
    );

    // Second run: all files should be unchanged
    let stats2 = swissarmyhammer_code_context::startup_cleanup(&ws.db(), root)
        .expect("Second startup_cleanup failed");
    assert_eq!(stats2.files_added, 0, "Second run should not add new files");
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
    let stats1 = swissarmyhammer_code_context::startup_cleanup(&ws.db(), root)
        .expect("First startup_cleanup failed");
    assert_eq!(
        stats1.files_unchanged, 4,
        "Files should be unchanged after workspace.open() auto-discovered them"
    );

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
    let stats2 = swissarmyhammer_code_context::startup_cleanup(&ws.db(), root)
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
    let stats1 = swissarmyhammer_code_context::startup_cleanup(&ws.db(), root)
        .expect("First startup_cleanup failed");
    assert_eq!(
        stats1.files_unchanged, 4,
        "Files should be unchanged after workspace.open() auto-discovered them"
    );

    // Delete one file
    std::fs::remove_file(root.join("src/utils.rs")).unwrap();

    // Second run: should detect the deletion
    let stats2 = swissarmyhammer_code_context::startup_cleanup(&ws.db(), root)
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
    let status = swissarmyhammer_code_context::get_status(&ws.db()).expect("get_status failed");
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
    let stats1 = swissarmyhammer_code_context::startup_cleanup(&ws.db(), root)
        .expect("First startup_cleanup failed");
    assert_eq!(
        stats1.files_unchanged, 4,
        "Files should be unchanged after workspace.open() auto-discovered them"
    );

    // Add a new file
    let new_module = r#"pub fn new_function() -> String {
    "Hello from new module".to_string()
}
"#;
    std::fs::write(root.join("src/new_module.rs"), new_module).unwrap();

    // Second run: should detect the new file
    let stats2 = swissarmyhammer_code_context::startup_cleanup(&ws.db(), root)
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
    let status = swissarmyhammer_code_context::get_status(&ws.db()).expect("get_status failed");
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
    let _stats = swissarmyhammer_code_context::startup_cleanup(&ws.db(), root)
        .expect("startup_cleanup failed");

    let status = swissarmyhammer_code_context::get_status(&ws.db()).expect("get_status failed");

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
    let cleanup_stats = swissarmyhammer_code_context::startup_cleanup(&ws.db(), root)
        .expect("startup_cleanup failed");
    println!(
        "startup_cleanup: added={}, removed={}, dirty={}, unchanged={}",
        cleanup_stats.files_added,
        cleanup_stats.files_removed,
        cleanup_stats.files_dirty,
        cleanup_stats.files_unchanged
    );

    // Verify files are in DB before indexing
    let before = swissarmyhammer_code_context::get_status(&ws.db()).expect("get_status failed");
    println!(
        "Before indexing: total={}, ts_indexed={}, lsp_indexed={}",
        before.total_files, before.ts_indexed_files, before.lsp_indexed_files
    );
    assert_eq!(before.total_files, 4, "Should have 4 files in database");
    assert_eq!(
        before.ts_indexed_files, 0,
        "No files should be indexed initially"
    );

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
    let after = swissarmyhammer_code_context::get_status(&ws.db()).expect("get_status failed");
    println!(
        "After indexing: total={}, ts_indexed={}, lsp_indexed={}",
        after.total_files, after.ts_indexed_files, after.lsp_indexed_files
    );

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
    assert_eq!(
        after.ts_indexed_percent, 100.0,
        "All files should be 100% indexed"
    );
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
    let _stats = swissarmyhammer_code_context::startup_cleanup(&ws.db(), root)
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
    let db = &*db;

    // ---------------------------------------------------------------
    // 1. Every file should have ts_indexed=1 and lsp_indexed=0
    // ---------------------------------------------------------------
    let mut stmt = db
        .prepare("SELECT file_path, ts_indexed, lsp_indexed FROM indexed_files")
        .expect("Failed to prepare indexed_files query");
    let rows: Vec<(String, i64, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .expect("query_map failed")
        .collect::<Result<Vec<_>, _>>()
        .expect("row collection failed");

    assert!(!rows.is_empty(), "indexed_files should not be empty");
    for (path, ts, lsp) in &rows {
        assert_eq!(*ts, 1, "ts_indexed should be 1 for {}, got {}", path, ts);
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
        .query_row("SELECT COUNT(DISTINCT file_path) FROM ts_chunks", [], |r| {
            r.get(0)
        })
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

    assert!(!lib_texts.is_empty(), "Should have ts_chunks for lib.rs");

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

// ---------------------------------------------------------------------------
// Test 9: File change triggers re-indexing with updated chunks
// ---------------------------------------------------------------------------

/// Proves the full file change -> re-indexing cycle works:
///
/// 1. Create project, run startup_cleanup, spawn TS worker, wait for indexing.
/// 2. Verify all files have `ts_indexed=1` and chunks exist for `lib.rs`.
/// 3. Modify `src/lib.rs` on disk (add a new function `new_feature`).
/// 4. Run `startup_cleanup` again -- it detects the hash change and sets `ts_indexed=0`.
/// 5. Verify `lib.rs` is dirty while other files remain indexed.
/// 6. Spawn TS worker again, wait for re-indexing.
/// 7. Verify `ts_indexed=1` for all files again.
/// 8. Query `ts_chunks` for `lib.rs` -- the new content (`new_feature`) must appear.
#[test]
fn test_file_change_triggers_reindexing() {
    use std::thread;
    use std::time::Duration;

    let project = create_test_project();
    let root = project.path();

    // -- Phase 1: initial index --
    let ws = CodeContextWorkspace::open(root).expect("Failed to open workspace");
    assert!(ws.is_leader(), "Should be leader");

    let _stats = swissarmyhammer_code_context::startup_cleanup(&ws.db(), root)
        .expect("startup_cleanup failed");

    let db_path = root.join(".code-context").join("index.db");
    swissarmyhammer_code_context::indexing::spawn_indexing_worker(
        root.to_path_buf(),
        db_path.clone(),
        swissarmyhammer_code_context::indexing::IndexingConfig::default(),
    );
    thread::sleep(Duration::from_secs(2));

    // Verify: all files ts_indexed=1
    {
        let db = ws.db();
        let mut stmt = db
            .prepare("SELECT file_path, ts_indexed FROM indexed_files")
            .expect("prepare failed");
        let rows: Vec<(String, i64)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        for (path, ts) in &rows {
            assert_eq!(
                *ts, 1,
                "After initial index, ts_indexed should be 1 for {}",
                path
            );
        }
    }

    // Verify: chunks exist for lib.rs containing original content
    let (initial_lib_chunks, initial_chunk_count) = {
        let db = ws.db();
        let mut stmt = db
            .prepare("SELECT text FROM ts_chunks WHERE file_path LIKE '%lib.rs'")
            .unwrap();
        let chunks: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM ts_chunks WHERE file_path LIKE '%lib.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        (chunks, count)
    };
    assert!(
        !initial_lib_chunks.is_empty(),
        "lib.rs should have chunks after initial indexing"
    );
    let initial_text = initial_lib_chunks.join("\n");
    assert!(
        initial_text.contains("Config"),
        "Initial lib.rs chunks should contain 'Config'"
    );
    assert!(
        !initial_text.contains("new_feature"),
        "Initial lib.rs chunks should NOT contain 'new_feature'"
    );
    println!("Initial lib.rs chunk count: {}", initial_chunk_count);

    // -- Phase 2: modify lib.rs --
    let lib_rs_path = root.join("src/lib.rs");
    let modified_lib = r#"pub struct Config {
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

pub fn new_feature() -> bool {
    true
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
    std::fs::write(&lib_rs_path, modified_lib).unwrap();

    // -- Phase 3: detect the change via startup_cleanup --
    let cleanup_stats = swissarmyhammer_code_context::startup_cleanup(&ws.db(), root)
        .expect("startup_cleanup after modification failed");
    assert_eq!(
        cleanup_stats.files_dirty, 1,
        "Only lib.rs should be dirty after modification, got {} dirty",
        cleanup_stats.files_dirty
    );

    // Verify lib.rs is dirty, others are still indexed
    {
        let db = ws.db();
        let mut stmt = db
            .prepare("SELECT file_path, ts_indexed FROM indexed_files ORDER BY file_path")
            .unwrap();
        let rows: Vec<(String, i64)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        for (path, ts) in &rows {
            if path.contains("lib.rs") {
                assert_eq!(*ts, 0, "lib.rs should have ts_indexed=0 after modification");
            } else {
                assert_eq!(*ts, 1, "{} should still have ts_indexed=1", path);
            }
        }
    }

    // -- Phase 4: re-index --
    swissarmyhammer_code_context::indexing::spawn_indexing_worker(
        root.to_path_buf(),
        db_path,
        swissarmyhammer_code_context::indexing::IndexingConfig::default(),
    );
    thread::sleep(Duration::from_secs(2));

    // Verify: all files ts_indexed=1 again
    {
        let db = ws.db();
        let mut stmt = db
            .prepare("SELECT file_path, ts_indexed FROM indexed_files")
            .unwrap();
        let rows: Vec<(String, i64)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        for (path, ts) in &rows {
            assert_eq!(
                *ts, 1,
                "After re-indexing, ts_indexed should be 1 for {}",
                path
            );
        }
    }

    // Verify: new content appears in chunks for lib.rs
    let (reindexed_lib_chunks, final_chunk_count) = {
        let db = ws.db();
        let mut stmt = db
            .prepare("SELECT text FROM ts_chunks WHERE file_path LIKE '%lib.rs'")
            .unwrap();
        let chunks: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM ts_chunks WHERE file_path LIKE '%lib.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        (chunks, count)
    };
    assert!(
        !reindexed_lib_chunks.is_empty(),
        "lib.rs should have chunks after re-indexing"
    );
    let reindexed_text = reindexed_lib_chunks.join("\n");
    assert!(
        reindexed_text.contains("new_feature"),
        "Re-indexed lib.rs chunks should contain 'new_feature', got:\n{}",
        reindexed_text
    );
    assert!(
        reindexed_text.contains("Config"),
        "Re-indexed lib.rs chunks should still contain 'Config'"
    );
    println!(
        "Final lib.rs chunk count: {} (was {})",
        final_chunk_count, initial_chunk_count
    );
    assert!(
        final_chunk_count >= initial_chunk_count,
        "Chunk count should not decrease after re-indexing"
    );
}

// ---------------------------------------------------------------------------
// Test 10: File change resets both ts_indexed and lsp_indexed flags
// ---------------------------------------------------------------------------

/// Verifies that `startup_cleanup` resets BOTH `ts_indexed` and `lsp_indexed`
/// when a file's content hash changes, even if only one pipeline had run.
///
/// 1. Create project, index via TS worker, then manually set `lsp_indexed=1`.
/// 2. Modify `src/utils.rs` on disk.
/// 3. Run `startup_cleanup` -- should reset both flags to 0 for `utils.rs`.
/// 4. Other files should retain their indexed state.
#[test]
fn test_file_change_resets_both_index_flags() {
    use std::thread;
    use std::time::Duration;

    let project = create_test_project();
    let root = project.path();

    // -- Phase 1: initial index via TS worker --
    let ws = CodeContextWorkspace::open(root).expect("Failed to open workspace");
    assert!(ws.is_leader(), "Should be leader");

    let _stats = swissarmyhammer_code_context::startup_cleanup(&ws.db(), root)
        .expect("startup_cleanup failed");

    let db_path = root.join(".code-context").join("index.db");
    swissarmyhammer_code_context::indexing::spawn_indexing_worker(
        root.to_path_buf(),
        db_path,
        swissarmyhammer_code_context::indexing::IndexingConfig::default(),
    );
    thread::sleep(Duration::from_secs(2));

    // Verify ts_indexed=1 for all files after TS worker
    {
        let db = ws.db();
        let mut stmt = db
            .prepare("SELECT file_path, ts_indexed FROM indexed_files")
            .unwrap();
        let rows: Vec<(String, i64)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        for (path, ts) in &rows {
            assert_eq!(
                *ts, 1,
                "ts_indexed should be 1 for {} after TS worker",
                path
            );
        }
    }

    // Simulate LSP worker having run: set lsp_indexed=1 for all files
    {
        let db = ws.db();
        db.execute("UPDATE indexed_files SET lsp_indexed = 1", [])
            .expect("Failed to set lsp_indexed=1");
    }

    // Verify both flags are 1 for all files
    {
        let db = ws.db();
        let mut stmt = db
            .prepare("SELECT file_path, ts_indexed, lsp_indexed FROM indexed_files")
            .unwrap();
        let rows: Vec<(String, i64, i64)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        for (path, ts, lsp) in &rows {
            assert_eq!(*ts, 1, "ts_indexed should be 1 for {}", path);
            assert_eq!(*lsp, 1, "lsp_indexed should be 1 for {}", path);
        }
    }

    // -- Phase 2: modify utils.rs --
    let utils_path = root.join("src/utils.rs");
    let modified_utils = r#"pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

pub fn is_positive(n: i32) -> bool {
    n > 0
}

pub fn subtract(a: i32, b: i32) -> i32 {
    a - b
}
"#;
    std::fs::write(&utils_path, modified_utils).unwrap();

    // -- Phase 3: run startup_cleanup to detect the change --
    let cleanup_stats = swissarmyhammer_code_context::startup_cleanup(&ws.db(), root)
        .expect("startup_cleanup after modification failed");
    assert_eq!(
        cleanup_stats.files_dirty, 1,
        "Only utils.rs should be dirty"
    );
    assert_eq!(
        cleanup_stats.files_unchanged, 3,
        "3 other files should be unchanged"
    );

    // -- Phase 4: verify both flags reset for utils.rs, others unchanged --
    {
        let db = ws.db();
        let mut stmt = db
            .prepare(
                "SELECT file_path, ts_indexed, lsp_indexed FROM indexed_files ORDER BY file_path",
            )
            .unwrap();
        let rows: Vec<(String, i64, i64)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        for (path, ts, lsp) in &rows {
            if path.contains("utils.rs") {
                assert_eq!(
                    *ts, 0,
                    "ts_indexed should be 0 for utils.rs after modification"
                );
                assert_eq!(
                    *lsp, 0,
                    "lsp_indexed should be 0 for utils.rs after modification"
                );
            } else {
                assert_eq!(
                    *ts, 1,
                    "ts_indexed should still be 1 for {} (not modified)",
                    path
                );
                assert_eq!(
                    *lsp, 1,
                    "lsp_indexed should still be 1 for {} (not modified)",
                    path
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Test 11: LSP re-indexing after file change (requires rust-analyzer)
// ---------------------------------------------------------------------------

/// Proves the full LSP re-indexing cycle works after a file modification:
///
/// 1. Create project, run startup_cleanup, TS-index all files.
/// 2. Spawn rust-analyzer manually, initialize it, and LSP-index `src/lib.rs`.
/// 3. Verify `lsp_indexed = 1` for lib.rs and `lsp_symbols` rows exist.
/// 4. Modify `src/lib.rs` on disk (add `added_later` function).
/// 5. Run `startup_cleanup` -- detects hash change, resets both flags to 0.
/// 6. Re-run TS indexing, then LSP-index lib.rs again with the same server.
/// 7. Verify both flags back to 1 and `lsp_symbols` contains `added_later`.
///
/// Marked `#[ignore]` because it requires `rust-analyzer` to be installed.
/// Run with:
///   cargo test --test workspace_e2e_test -- test_lsp_reindexing_after_file_change --ignored --nocapture
#[test]
#[ignore]
fn test_lsp_reindexing_after_file_change() {
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::Duration;
    use swissarmyhammer_code_context::{detect_rust_analyzer, LspJsonRpcClient};

    // Skip gracefully if rust-analyzer is not installed
    if detect_rust_analyzer().is_none() {
        eprintln!("SKIP: rust-analyzer not found in PATH");
        return;
    }

    let project = create_test_project();
    let root = project.path();

    // -- Phase 1: open workspace, TS-index everything --
    let ws = CodeContextWorkspace::open(root).expect("Failed to open workspace");
    assert!(ws.is_leader(), "Should be leader");

    let _stats = swissarmyhammer_code_context::startup_cleanup(&ws.db(), root)
        .expect("startup_cleanup failed");

    let db_path = root.join(".code-context").join("index.db");
    swissarmyhammer_code_context::indexing::spawn_indexing_worker(
        root.to_path_buf(),
        db_path.clone(),
        swissarmyhammer_code_context::indexing::IndexingConfig::default(),
    );
    thread::sleep(Duration::from_secs(2));

    // Verify TS indexing completed
    {
        let db = ws.db();
        let ts_count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM indexed_files WHERE ts_indexed = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ts_count, 4, "All 4 files should be TS-indexed");
    }

    // -- Phase 2: spawn rust-analyzer and LSP-index lib.rs --
    let mut ra_child = Command::new("rust-analyzer")
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn rust-analyzer");

    let stdin = ra_child.stdin.take().expect("Failed to take stdin");
    let stdout = ra_child.stdout.take().expect("Failed to take stdout");
    let mut client = LspJsonRpcClient::new(stdin, stdout);

    client.initialize(root).expect("LSP initialize failed");

    let lib_rs_abs = root.join("src/lib.rs");
    let lib_content = std::fs::read_to_string(&lib_rs_abs).unwrap();
    client
        .send_did_open(&lib_rs_abs, "rust", &lib_content)
        .expect("didOpen failed");

    // Give rust-analyzer time to analyze the project
    thread::sleep(Duration::from_secs(5));

    let result = {
        let db = ws.db();
        client
            .collect_and_persist_file_symbols(&db, &lib_rs_abs, "src/lib.rs")
            .expect("collect_and_persist_file_symbols failed")
    };
    println!(
        "Initial LSP indexing: {} symbols for {}",
        result.symbol_count, result.file_path
    );
    if let Some(ref err) = result.error {
        eprintln!("LSP collection warning: {}", err);
    }

    // -- Phase 3: verify lsp_indexed = 1 for lib.rs --
    let initial_symbol_count = {
        let db = ws.db();
        let lsp_flag: i64 = db
            .query_row(
                "SELECT lsp_indexed FROM indexed_files WHERE file_path LIKE '%lib.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            lsp_flag, 1,
            "lib.rs should have lsp_indexed=1 after LSP indexing"
        );

        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM lsp_symbols WHERE file_path = 'src/lib.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        count
    };
    println!(
        "Initial lsp_symbols count for lib.rs: {}",
        initial_symbol_count
    );
    assert!(
        initial_symbol_count > 0,
        "lsp_symbols should have rows for lib.rs after LSP indexing"
    );

    // -- Phase 4: modify lib.rs on disk --
    let modified_lib = r#"pub struct Config {
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

pub fn added_later() -> i32 {
    99
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
    std::fs::write(&lib_rs_abs, modified_lib).unwrap();

    // -- Phase 5: startup_cleanup detects hash change, resets both flags --
    let cleanup_stats = swissarmyhammer_code_context::startup_cleanup(&ws.db(), root)
        .expect("startup_cleanup after modification failed");
    assert_eq!(
        cleanup_stats.files_dirty, 1,
        "Only lib.rs should be dirty, got {} dirty",
        cleanup_stats.files_dirty
    );

    {
        let db = ws.db();
        let (ts_flag, lsp_flag): (i64, i64) = db
            .query_row(
                "SELECT ts_indexed, lsp_indexed FROM indexed_files WHERE file_path LIKE '%lib.rs'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(ts_flag, 0, "ts_indexed should be 0 after file change");
        assert_eq!(lsp_flag, 0, "lsp_indexed should be 0 after file change");
    }

    // -- Phase 6: re-run TS worker, then LSP-index again --
    swissarmyhammer_code_context::indexing::spawn_indexing_worker(
        root.to_path_buf(),
        db_path,
        swissarmyhammer_code_context::indexing::IndexingConfig::default(),
    );
    thread::sleep(Duration::from_secs(2));

    // Re-open the modified file in rust-analyzer
    let modified_content = std::fs::read_to_string(&lib_rs_abs).unwrap();
    client
        .send_did_open(&lib_rs_abs, "rust", &modified_content)
        .expect("didOpen (modified) failed");

    // Give rust-analyzer time to re-analyze
    thread::sleep(Duration::from_secs(5));

    let result2 = {
        let db = ws.db();
        client
            .collect_and_persist_file_symbols(&db, &lib_rs_abs, "src/lib.rs")
            .expect("collect_and_persist_file_symbols (re-index) failed")
    };
    println!(
        "Re-index LSP: {} symbols for {}",
        result2.symbol_count, result2.file_path
    );

    // -- Phase 7: verify both flags back to 1 --
    {
        let db = ws.db();
        let (ts_flag, lsp_flag): (i64, i64) = db
            .query_row(
                "SELECT ts_indexed, lsp_indexed FROM indexed_files WHERE file_path LIKE '%lib.rs'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(ts_flag, 1, "ts_indexed should be 1 after re-indexing");
        assert_eq!(lsp_flag, 1, "lsp_indexed should be 1 after re-indexing");
    }

    // -- Phase 8: verify lsp_symbols contains `added_later` --
    {
        let db = ws.db();
        let mut stmt = db
            .prepare("SELECT name FROM lsp_symbols WHERE file_path = 'src/lib.rs'")
            .expect("prepare lsp_symbols query failed");
        let symbol_names: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        println!("LSP symbols after re-index: {:?}", symbol_names);

        assert!(
            symbol_names.iter().any(|n| n == "added_later"),
            "lsp_symbols should contain 'added_later' after re-indexing, got: {:?}",
            symbol_names
        );
        assert!(
            symbol_names.iter().any(|n| n == "Config"),
            "lsp_symbols should still contain 'Config', got: {:?}",
            symbol_names
        );
    }

    // -- Phase 9: clean shutdown --
    client.shutdown().expect("LSP shutdown failed");
    let _ = ra_child.wait();
}
