//! Integration tests for the swissarmyhammer-sem semantic diff engine.
//!
//! These tests exercise the semantic diff pipeline directly (not through the MCP
//! tool layer) to verify entity-level change detection for Rust code. Covers:
//! modified functions, added/deleted functions, reordering, renaming, struct
//! changes, multi-file diffs, direct parser registry usage, and real git repo
//! integration.

use std::process::Command;
use swissarmyhammer_sem::git_types::{FileChange, FileStatus};
use swissarmyhammer_sem::model::change::ChangeType;
use swissarmyhammer_sem::model::identity::match_entities;
use swissarmyhammer_sem::parser::differ::compute_semantic_diff;
use swissarmyhammer_sem::parser::plugins::create_default_registry;
use tempfile::TempDir;

/// Helper: runs compute_semantic_diff on a single file with before/after Rust content.
///
/// Returns the DiffResult for assertions.
fn diff_single_rust_file(
    before: &str,
    after: &str,
) -> swissarmyhammer_sem::parser::differ::DiffResult {
    let registry = create_default_registry();
    let file_change = FileChange {
        file_path: "src/lib.rs".to_string(),
        status: FileStatus::Modified,
        old_file_path: None,
        before_content: Some(before.to_string()),
        after_content: Some(after.to_string()),
    };
    compute_semantic_diff(&[file_change], &registry, None, None)
}

/// Helper: find a change by entity name in a DiffResult.
fn find_change_by_name<'a>(
    result: &'a swissarmyhammer_sem::parser::differ::DiffResult,
    name: &str,
) -> Option<&'a swissarmyhammer_sem::model::change::SemanticChange> {
    result.changes.iter().find(|c| c.entity_name == name)
}

// ---------------------------------------------------------------------------
// Test 1: Modified function detection
// ---------------------------------------------------------------------------

#[test]
fn test_modified_function_detection() {
    let before = r#"
fn process(x: i32) -> i32 {
    x + 1
}
"#;
    let after = r#"
fn process(x: i32) -> i32 {
    x * 2
}
"#;

    let result = diff_single_rust_file(before, after);

    assert_eq!(
        result.modified_count, 1,
        "Expected exactly 1 modified entity"
    );
    assert_eq!(result.added_count, 0);
    assert_eq!(result.deleted_count, 0);

    let change =
        find_change_by_name(&result, "process").expect("Should find a change for 'process'");
    assert_eq!(change.change_type, ChangeType::Modified);
    assert!(
        change.before_content.is_some(),
        "before_content should be populated"
    );
    assert!(
        change.after_content.is_some(),
        "after_content should be populated"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Added and deleted functions
// ---------------------------------------------------------------------------

#[test]
fn test_added_and_deleted_functions() {
    let before = r#"
fn alpha() {
    println!("alpha");
}

fn beta() {
    println!("beta");
}
"#;
    let after = r#"
fn beta() {
    println!("beta");
}

fn gamma() {
    println!("gamma");
}
"#;

    let result = diff_single_rust_file(before, after);

    // alpha should be deleted
    let alpha = find_change_by_name(&result, "alpha").expect("Should find a change for 'alpha'");
    assert_eq!(alpha.change_type, ChangeType::Deleted);

    // gamma should be added
    let gamma = find_change_by_name(&result, "gamma").expect("Should find a change for 'gamma'");
    assert_eq!(gamma.change_type, ChangeType::Added);

    // beta is unchanged -- should NOT appear in changes
    assert!(
        find_change_by_name(&result, "beta").is_none(),
        "beta is unchanged and should not appear in changes"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Function moved within file (reordered)
// ---------------------------------------------------------------------------

#[test]
fn test_function_reorder_no_modification() {
    let before = r#"
fn first() {
    println!("first");
}

fn second() {
    println!("second");
}
"#;
    let after = r#"
fn second() {
    println!("second");
}

fn first() {
    println!("first");
}
"#;

    let result = diff_single_rust_file(before, after);

    // Content of both functions is identical; only order changed.
    // Neither should be classified as Modified (content hash is the same).
    let first_change = find_change_by_name(&result, "first");
    let second_change = find_change_by_name(&result, "second");

    // If a change exists for either, it should NOT be Modified -- at most Moved/Renamed
    // but within the same file the engine should recognise them as unchanged.
    if let Some(c) = first_change {
        assert_ne!(
            c.change_type,
            ChangeType::Modified,
            "first() content unchanged -- should not be Modified"
        );
    }
    if let Some(c) = second_change {
        assert_ne!(
            c.change_type,
            ChangeType::Modified,
            "second() content unchanged -- should not be Modified"
        );
    }
}

// ---------------------------------------------------------------------------
// Test 4: Function renamed (same body, different name)
// ---------------------------------------------------------------------------

#[test]
fn test_function_renamed() {
    let before = r#"
fn old_name(x: i32) -> i32 {
    x + 1
}
"#;
    let after = r#"
fn new_name(x: i32) -> i32 {
    x + 1
}
"#;

    let result = diff_single_rust_file(before, after);

    // The entity ID includes the name, so old_name and new_name have different IDs.
    // Phase 2 (content hash match) should detect a Renamed change, OR
    // Phase 3 (fuzzy similarity) should detect it if content hash differs
    // due to slight extraction differences.
    //
    // If the engine cannot detect the rename, it will produce an Added + Deleted pair.
    let has_renamed = result
        .changes
        .iter()
        .any(|c| c.change_type == ChangeType::Renamed);
    let has_add_delete = result.added_count >= 1 && result.deleted_count >= 1;

    assert!(
        has_renamed || has_add_delete,
        "Renaming should produce either a Renamed change or an Added+Deleted pair. Got: {:?}",
        result
            .changes
            .iter()
            .map(|c| (&c.entity_name, c.change_type))
            .collect::<Vec<_>>()
    );

    // Regardless of classification, no Modified change should exist (body is the same)
    assert_eq!(
        result.modified_count, 0,
        "Body is identical; should not be Modified"
    );
}

// ---------------------------------------------------------------------------
// Test 5: Struct modification (field added)
// ---------------------------------------------------------------------------

#[test]
fn test_struct_modification() {
    let before = r#"
struct Config {
    host: String,
}
"#;
    let after = r#"
struct Config {
    host: String,
    port: u16,
}
"#;

    let result = diff_single_rust_file(before, after);

    let config_change =
        find_change_by_name(&result, "Config").expect("Should find a change for 'Config'");
    assert_eq!(
        config_change.change_type,
        ChangeType::Modified,
        "Config struct should be Modified after adding a field"
    );
}

// ---------------------------------------------------------------------------
// Test 6: Multiple files
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_files() {
    let registry = create_default_registry();

    let file1 = FileChange {
        file_path: "src/foo.rs".to_string(),
        status: FileStatus::Modified,
        old_file_path: None,
        before_content: Some("fn foo() { 1 }".to_string()),
        after_content: Some("fn foo() { 2 }".to_string()),
    };

    let file2 = FileChange {
        file_path: "src/bar.rs".to_string(),
        status: FileStatus::Modified,
        old_file_path: None,
        before_content: Some("fn bar() { 1 }".to_string()),
        after_content: Some("fn bar() { 2 }".to_string()),
    };

    let result = compute_semantic_diff(&[file1, file2], &registry, None, None);

    assert_eq!(result.file_count, 2, "Should report 2 files with changes");
    assert_eq!(result.modified_count, 2, "Should have 2 modified entities");

    // Verify both entity names appear
    let names: Vec<&str> = result
        .changes
        .iter()
        .map(|c| c.entity_name.as_str())
        .collect();
    assert!(names.contains(&"foo"), "Should contain change for foo");
    assert!(names.contains(&"bar"), "Should contain change for bar");
}

// ---------------------------------------------------------------------------
// Test 7: Inline text comparison using ParserRegistry directly
// ---------------------------------------------------------------------------

#[test]
fn test_parser_registry_direct_entity_extraction() {
    let registry = create_default_registry();
    let plugin = registry
        .get_plugin("example.rs")
        .expect("Registry should have a plugin for .rs files");

    let code_before = r#"
fn compute(a: i32, b: i32) -> i32 {
    a + b
}

fn helper() -> bool {
    true
}
"#;

    let code_after = r#"
fn compute(a: i32, b: i32) -> i32 {
    a * b
}

fn helper() -> bool {
    true
}

fn new_fn() -> String {
    String::from("hello")
}
"#;

    let before_entities = plugin.extract_entities(code_before, "example.rs");
    let after_entities = plugin.extract_entities(code_after, "example.rs");

    // Before should have 2 entities, after should have 3
    assert!(
        before_entities.len() >= 2,
        "Should extract at least 2 entities from before code, got {}",
        before_entities.len()
    );
    assert!(
        after_entities.len() >= 3,
        "Should extract at least 3 entities from after code, got {}",
        after_entities.len()
    );

    // Use match_entities directly
    let sim_fn = |a: &swissarmyhammer_sem::model::entity::SemanticEntity,
                  b: &swissarmyhammer_sem::model::entity::SemanticEntity|
     -> f64 { plugin.compute_similarity(a, b) };

    let match_result = match_entities(
        &before_entities,
        &after_entities,
        "example.rs",
        Some(&sim_fn),
        None,
        None,
    );

    // compute should be modified
    let compute_change = match_result
        .changes
        .iter()
        .find(|c| c.entity_name == "compute")
        .expect("Should find change for 'compute'");
    assert_eq!(compute_change.change_type, ChangeType::Modified);

    // new_fn should be added
    let new_fn_change = match_result
        .changes
        .iter()
        .find(|c| c.entity_name == "new_fn")
        .expect("Should find change for 'new_fn'");
    assert_eq!(new_fn_change.change_type, ChangeType::Added);

    // helper should not appear (unchanged)
    assert!(
        match_result
            .changes
            .iter()
            .find(|c| c.entity_name == "helper")
            .is_none(),
        "helper is unchanged and should not appear in changes"
    );
}

// ---------------------------------------------------------------------------
// Test 8: Real git repo integration (using execute_auto_diff with shell git)
// ---------------------------------------------------------------------------

#[test]
fn test_real_git_repo_integration() {
    use swissarmyhammer_tools::mcp::tools::git::diff::{execute_auto_diff, DiffResponse};

    let tmp = TempDir::new().expect("Failed to create temp dir");
    let repo_path = tmp.path();

    // Helper to run git commands in the temp repo
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo_path)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@test.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@test.com")
            .output()
            .unwrap_or_else(|e| panic!("git {:?} failed: {e}", args));
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        output
    };

    // Initialize repo
    git(&["init"]);
    git(&["config", "user.email", "test@test.com"]);
    git(&["config", "user.name", "Test"]);

    // Write initial Rust file with two functions
    let src_dir = repo_path.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    let file_path = src_dir.join("lib.rs");

    let initial_content = r#"fn process(x: i32) -> i32 {
    x + 1
}

fn unchanged() {
    println!("stable");
}
"#;
    std::fs::write(&file_path, initial_content).unwrap();

    git(&["add", "."]);
    git(&["commit", "-m", "Initial commit"]);

    // Modify the file: change process() body
    let modified_content = r#"fn process(x: i32) -> i32 {
    x * 2 + 10
}

fn unchanged() {
    println!("stable");
}
"#;
    std::fs::write(&file_path, modified_content).unwrap();

    // Use execute_auto_diff (shell git) to detect and diff changes
    let result_json = execute_auto_diff(repo_path).expect("auto-diff should succeed");
    let response: DiffResponse = serde_json::from_str(&result_json).unwrap();

    // The process function should be modified
    let process_change = response
        .changes
        .iter()
        .find(|c| c.entity_name == "process")
        .expect("Should find a change for 'process'");
    assert_eq!(
        process_change.change_type, "modified",
        "process() body changed -- should be modified"
    );

    // unchanged() should NOT appear in changes
    assert!(
        !response
            .changes
            .iter()
            .any(|c| c.entity_name == "unchanged"),
        "unchanged() should not appear in diff results"
    );
}
