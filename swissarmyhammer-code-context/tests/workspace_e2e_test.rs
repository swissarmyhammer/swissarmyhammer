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

    // Run startup_cleanup to discover files
    let stats = swissarmyhammer_code_context::startup_cleanup(ws.db(), root)
        .expect("startup_cleanup failed");

    // Should discover 4 files: Cargo.toml, main.rs, lib.rs, utils.rs
    assert_eq!(
        stats.files_added, 4,
        "Expected 4 files to be discovered, got {}",
        stats.files_added
    );
    assert_eq!(stats.files_removed, 0);
    assert_eq!(stats.files_dirty, 0);
    assert_eq!(stats.files_unchanged, 0);

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

    // First run: discovers files
    let stats1 = swissarmyhammer_code_context::startup_cleanup(ws.db(), root)
        .expect("First startup_cleanup failed");
    assert_eq!(stats1.files_added, 4);

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

    // First run: discover files
    let stats1 = swissarmyhammer_code_context::startup_cleanup(ws.db(), root)
        .expect("First startup_cleanup failed");
    assert_eq!(stats1.files_added, 4);

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

    // First run: discover files
    let stats1 = swissarmyhammer_code_context::startup_cleanup(ws.db(), root)
        .expect("First startup_cleanup failed");
    assert_eq!(stats1.files_added, 4);

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

    // First run: discover files
    let stats1 = swissarmyhammer_code_context::startup_cleanup(ws.db(), root)
        .expect("First startup_cleanup failed");
    assert_eq!(stats1.files_added, 4);

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
