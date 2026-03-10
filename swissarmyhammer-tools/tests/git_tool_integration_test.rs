//! Integration tests for the git MCP tool's semantic diff operations.
//!
//! These tests exercise the higher-level diff functions (`execute_file_diff`,
//! `execute_auto_diff`) and the MCP tool dispatch layer (`execute_diff` on
//! `GitChangesTool`) with real git repositories. Complements the sem-core-level
//! tests in `git_diff_integration_test.rs`.

use std::process::Command;
use tempfile::TempDir;

use swissarmyhammer_tools::mcp::tools::git::diff::{
    execute_auto_diff, execute_file_diff, execute_inline_diff, DiffResponse,
};

/// Helper: set up a git repo with an initial Rust file and one commit.
/// Returns (TempDir, path to repo).
fn setup_rust_repo() -> TempDir {
    let tmp = TempDir::new().expect("Failed to create temp dir");
    let repo_path = tmp.path();

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
    };

    git(&["init"]);
    git(&["config", "user.email", "test@test.com"]);
    git(&["config", "user.name", "Test"]);

    let src_dir = repo_path.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();

    let initial = r#"fn process(x: i32) -> i32 {
    x + 1
}

fn helper() -> bool {
    true
}

fn stable() {
    println!("stable");
}
"#;
    std::fs::write(src_dir.join("lib.rs"), initial).unwrap();
    git(&["add", "."]);
    git(&["commit", "-m", "Initial commit"]);

    tmp
}

/// Helper: run a git command in a repo path, panic on failure.
fn git_cmd(repo_path: &std::path::Path, args: &[&str]) {
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
}

// ---------------------------------------------------------------------------
// execute_file_diff tests
// ---------------------------------------------------------------------------

#[test]
fn test_file_diff_disk_to_disk() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    // Write two versions of a file
    std::fs::write(
        dir.join("before.rs"),
        "fn greet() {\n    println!(\"hello\");\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("after.rs"),
        "fn greet(name: &str) {\n    println!(\"hello {}\", name);\n}\n",
    )
    .unwrap();

    let result = execute_file_diff("before.rs", "after.rs", dir).unwrap();
    let response: DiffResponse = serde_json::from_str(&result).unwrap();

    assert_eq!(response.summary.files, 1);
    assert_eq!(response.summary.modified, 1);
    assert_eq!(response.summary.added, 0);
    assert_eq!(response.summary.deleted, 0);

    let change = &response.changes[0];
    assert_eq!(change.entity_name, "greet");
    assert_eq!(change.change_type, "modified");
}

#[test]
fn test_file_diff_with_git_ref() {
    let tmp = setup_rust_repo();
    let repo_path = tmp.path();

    // Modify the file in the working tree
    let modified = r#"fn process(x: i32) -> i32 {
    x * 10
}

fn helper() -> bool {
    true
}

fn stable() {
    println!("stable");
}
"#;
    std::fs::write(repo_path.join("src/lib.rs"), modified).unwrap();

    // Compare HEAD version to working tree version
    let result = execute_file_diff("src/lib.rs@HEAD", "src/lib.rs", repo_path).unwrap();
    let response: DiffResponse = serde_json::from_str(&result).unwrap();

    assert_eq!(response.summary.modified, 1, "process() should be modified");
    assert_eq!(response.summary.added, 0);
    assert_eq!(response.summary.deleted, 0);

    let change = response
        .changes
        .iter()
        .find(|c| c.entity_name == "process")
        .expect("Should find process change");
    assert_eq!(change.change_type, "modified");
}

#[test]
fn test_file_diff_between_commits() {
    let tmp = setup_rust_repo();
    let repo_path = tmp.path();

    // Make a second commit with modifications
    let v2 = r#"fn process(x: i32) -> i32 {
    x * 2
}

fn helper() -> bool {
    false
}

fn stable() {
    println!("stable");
}
"#;
    std::fs::write(repo_path.join("src/lib.rs"), v2).unwrap();
    git_cmd(repo_path, &["add", "."]);
    git_cmd(repo_path, &["commit", "-m", "Second commit"]);

    // Compare first commit to second
    let result = execute_file_diff("src/lib.rs@HEAD~1", "src/lib.rs@HEAD", repo_path).unwrap();
    let response: DiffResponse = serde_json::from_str(&result).unwrap();

    // process and helper both changed
    assert_eq!(response.summary.modified, 2);
    assert!(response.changes.iter().any(|c| c.entity_name == "process"));
    assert!(response.changes.iter().any(|c| c.entity_name == "helper"));
    // stable should not appear
    assert!(!response.changes.iter().any(|c| c.entity_name == "stable"));
}

#[test]
fn test_file_diff_added_and_deleted_entities() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    std::fs::write(dir.join("old.rs"), "fn alpha() { 1 }\nfn beta() { 2 }\n").unwrap();
    std::fs::write(dir.join("new.rs"), "fn beta() { 2 }\nfn gamma() { 3 }\n").unwrap();

    let result = execute_file_diff("old.rs", "new.rs", dir).unwrap();
    let response: DiffResponse = serde_json::from_str(&result).unwrap();

    assert_eq!(response.summary.deleted, 1, "alpha should be deleted");
    assert_eq!(response.summary.added, 1, "gamma should be added");
    assert_eq!(response.summary.modified, 0, "beta is unchanged");

    assert!(response
        .changes
        .iter()
        .any(|c| c.entity_name == "alpha" && c.change_type == "deleted"));
    assert!(response
        .changes
        .iter()
        .any(|c| c.entity_name == "gamma" && c.change_type == "added"));
}

#[test]
fn test_file_diff_nonexistent_file_returns_error() {
    let tmp = TempDir::new().unwrap();
    let result = execute_file_diff("nope.rs", "also_nope.rs", tmp.path());
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// execute_auto_diff tests
// ---------------------------------------------------------------------------

#[test]
fn test_auto_diff_detects_working_tree_changes() {
    let tmp = setup_rust_repo();
    let repo_path = tmp.path();

    // Modify process() in working tree
    let modified = r#"fn process(x: i32) -> i32 {
    x * 100
}

fn helper() -> bool {
    true
}

fn stable() {
    println!("stable");
}
"#;
    std::fs::write(repo_path.join("src/lib.rs"), modified).unwrap();

    let result = execute_auto_diff(repo_path).unwrap();
    let response: DiffResponse = serde_json::from_str(&result).unwrap();

    assert!(
        response.summary.modified >= 1,
        "Should detect modified entities"
    );
    assert!(response
        .changes
        .iter()
        .any(|c| c.entity_name == "process" && c.change_type == "modified"));
    // stable and helper are unchanged
    assert!(!response.changes.iter().any(|c| c.entity_name == "stable"));
    assert!(!response.changes.iter().any(|c| c.entity_name == "helper"));
}

#[test]
fn test_auto_diff_clean_repo_returns_head_commit() {
    // When no staged or working-tree changes exist, detect_and_get_files falls
    // back to diffing the HEAD commit against its parent. For an initial commit
    // (no parent), all entities in the committed files appear as "added".
    let tmp = setup_rust_repo();
    let result = execute_auto_diff(tmp.path()).unwrap();
    let response: DiffResponse = serde_json::from_str(&result).unwrap();

    // The initial commit added src/lib.rs with 3 functions: process, helper, stable
    assert!(
        response.summary.added >= 3,
        "Expected at least 3 added entities from HEAD commit, got {}",
        response.summary.added
    );
    assert!(response
        .changes
        .iter()
        .any(|c| c.entity_name == "process" && c.change_type == "added"));
    assert!(response
        .changes
        .iter()
        .any(|c| c.entity_name == "helper" && c.change_type == "added"));
    assert!(response
        .changes
        .iter()
        .any(|c| c.entity_name == "stable" && c.change_type == "added"));
}

#[test]
fn test_auto_diff_new_file_added() {
    let tmp = setup_rust_repo();
    let repo_path = tmp.path();

    // Add a brand new file (untracked)
    std::fs::write(
        repo_path.join("src/new_module.rs"),
        "fn brand_new() -> u32 {\n    42\n}\n",
    )
    .unwrap();
    // Stage it so GitBridge sees it
    git_cmd(repo_path, &["add", "src/new_module.rs"]);

    let result = execute_auto_diff(repo_path).unwrap();
    let response: DiffResponse = serde_json::from_str(&result).unwrap();

    assert!(
        response.summary.added >= 1,
        "Should detect at least 1 added entity"
    );
    assert!(response
        .changes
        .iter()
        .any(|c| c.entity_name == "brand_new" && c.change_type == "added"));
}

#[test]
fn test_auto_diff_multiple_files_changed() {
    let tmp = setup_rust_repo();
    let repo_path = tmp.path();

    // Modify existing file
    let modified_lib = r#"fn process(x: i32) -> i32 {
    x - 1
}

fn helper() -> bool {
    true
}

fn stable() {
    println!("stable");
}
"#;
    std::fs::write(repo_path.join("src/lib.rs"), modified_lib).unwrap();

    // Add a new file
    std::fs::write(
        repo_path.join("src/utils.rs"),
        "fn utility() -> String {\n    String::new()\n}\n",
    )
    .unwrap();
    // Stage both changes so detect_and_get_files sees them together.
    // It checks staged first; if only utils.rs is staged, lib.rs (unstaged)
    // would be missed because the staged scope takes priority.
    git_cmd(repo_path, &["add", "src/lib.rs", "src/utils.rs"]);

    let result = execute_auto_diff(repo_path).unwrap();
    let response: DiffResponse = serde_json::from_str(&result).unwrap();

    assert!(
        response.summary.files >= 2,
        "Should report at least 2 files"
    );
    assert!(response.changes.iter().any(|c| c.entity_name == "process"));
    assert!(response.changes.iter().any(|c| c.entity_name == "utility"));
}

#[test]
fn test_auto_diff_not_a_git_repo() {
    let tmp = TempDir::new().unwrap();
    let result = execute_auto_diff(tmp.path());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("git repository"));
}

// ---------------------------------------------------------------------------
// execute_inline_diff (additional edge cases not covered by unit tests)
// ---------------------------------------------------------------------------

#[test]
fn test_inline_diff_python() {
    let before = "def greet():\n    print('hello')\n";
    let after = "def greet(name):\n    print(f'hello {name}')\n";

    let result = execute_inline_diff(before, after, "python").unwrap();
    let response: DiffResponse = serde_json::from_str(&result).unwrap();

    assert_eq!(response.summary.modified, 1);
    assert_eq!(response.changes[0].entity_name, "greet");
}

#[test]
fn test_inline_diff_multiple_changes() {
    let before = r#"fn a() { 1 }
fn b() { 2 }
fn c() { 3 }
"#;
    let after = r#"fn a() { 10 }
fn c() { 3 }
fn d() { 4 }
"#;

    let result = execute_inline_diff(before, after, "rust").unwrap();
    let response: DiffResponse = serde_json::from_str(&result).unwrap();

    // a modified, b deleted, d added, c unchanged
    assert_eq!(response.summary.modified, 1);
    assert_eq!(response.summary.deleted, 1);
    assert_eq!(response.summary.added, 1);

    assert!(response
        .changes
        .iter()
        .any(|c| c.entity_name == "a" && c.change_type == "modified"));
    assert!(response
        .changes
        .iter()
        .any(|c| c.entity_name == "b" && c.change_type == "deleted"));
    assert!(response
        .changes
        .iter()
        .any(|c| c.entity_name == "d" && c.change_type == "added"));
    assert!(!response.changes.iter().any(|c| c.entity_name == "c"));
}

// ---------------------------------------------------------------------------
// MCP tool dispatch layer tests
// ---------------------------------------------------------------------------

use std::sync::Arc;
use swissarmyhammer_config::model::ModelConfig;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolContext};
use swissarmyhammer_tools::mcp::tools::git::changes::GitChangesTool;
use tokio::sync::Mutex as TokioMutex;

/// Create a minimal ToolContext for integration tests.
fn make_test_context(working_dir: Option<std::path::PathBuf>) -> ToolContext {
    let git_ops = Arc::new(TokioMutex::new(None));
    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(ModelConfig::default());
    let mut ctx = ToolContext::new(tool_handlers, git_ops, agent_config);
    ctx.working_dir = working_dir;
    ctx
}

#[tokio::test]
async fn test_mcp_dispatch_inline_diff() {
    let tool = GitChangesTool::new();
    let context = make_test_context(None);

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("get diff"));
    args.insert("left_text".to_string(), serde_json::json!("fn foo() { 1 }"));
    args.insert(
        "right_text".to_string(),
        serde_json::json!("fn foo() { 2 }"),
    );
    args.insert("language".to_string(), serde_json::json!("rust"));

    let result = tool.execute(args, &context).await;
    assert!(
        result.is_ok(),
        "MCP dispatch should succeed: {:?}",
        result.err()
    );

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    let text = match &call_result.content[0].raw {
        rmcp::model::RawContent::Text(t) => &t.text,
        _ => panic!("Expected text content"),
    };
    let response: DiffResponse = serde_json::from_str(text).unwrap();
    assert_eq!(response.summary.modified, 1);
    assert_eq!(response.changes[0].entity_name, "foo");
}

#[tokio::test]
async fn test_mcp_dispatch_auto_diff() {
    let tmp = setup_rust_repo();
    let repo_path = tmp.path();

    // Modify working tree
    let modified = r#"fn process(x: i32) -> i32 {
    x * 99
}

fn helper() -> bool {
    true
}

fn stable() {
    println!("stable");
}
"#;
    std::fs::write(repo_path.join("src/lib.rs"), modified).unwrap();

    let tool = GitChangesTool::new();
    let context = make_test_context(Some(repo_path.to_path_buf()));

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("get diff"));

    let result = tool.execute(args, &context).await;
    assert!(
        result.is_ok(),
        "Auto-diff via MCP should succeed: {:?}",
        result.err()
    );

    let call_result = result.unwrap();
    let text = match &call_result.content[0].raw {
        rmcp::model::RawContent::Text(t) => &t.text,
        _ => panic!("Expected text content"),
    };
    let response: DiffResponse = serde_json::from_str(text).unwrap();
    assert!(response.summary.modified >= 1);
    assert!(response.changes.iter().any(|c| c.entity_name == "process"));
}

#[tokio::test]
async fn test_mcp_dispatch_file_diff() {
    let tmp = setup_rust_repo();
    let repo_path = tmp.path();

    // Modify and commit a second version
    let v2 = r#"fn process(x: i32) -> i32 {
    x + 100
}

fn helper() -> bool {
    true
}

fn stable() {
    println!("stable");
}
"#;
    std::fs::write(repo_path.join("src/lib.rs"), v2).unwrap();
    git_cmd(repo_path, &["add", "."]);
    git_cmd(repo_path, &["commit", "-m", "v2"]);

    let tool = GitChangesTool::new();
    let context = make_test_context(Some(repo_path.to_path_buf()));

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("get diff"));
    args.insert("left".to_string(), serde_json::json!("src/lib.rs@HEAD~1"));
    args.insert("right".to_string(), serde_json::json!("src/lib.rs@HEAD"));

    let result = tool.execute(args, &context).await;
    assert!(
        result.is_ok(),
        "File-mode diff via MCP should succeed: {:?}",
        result.err()
    );

    let call_result = result.unwrap();
    let text = match &call_result.content[0].raw {
        rmcp::model::RawContent::Text(t) => &t.text,
        _ => panic!("Expected text content"),
    };
    let response: DiffResponse = serde_json::from_str(text).unwrap();
    assert_eq!(response.summary.modified, 1);
    assert_eq!(response.changes[0].entity_name, "process");
}

#[tokio::test]
async fn test_mcp_dispatch_unknown_op() {
    let tool = GitChangesTool::new();
    let context = make_test_context(None);

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("bogus operation"));

    let result = tool.execute(args, &context).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.message.contains("Unknown operation"));
}

#[tokio::test]
async fn test_mcp_dispatch_missing_required_params() {
    let tool = GitChangesTool::new();
    let context = make_test_context(None);

    // Inline mode with left_text but missing right_text
    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("get diff"));
    args.insert("left_text".to_string(), serde_json::json!("fn foo() {}"));

    let result = tool.execute(args, &context).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.message.contains("right_text"));
}
