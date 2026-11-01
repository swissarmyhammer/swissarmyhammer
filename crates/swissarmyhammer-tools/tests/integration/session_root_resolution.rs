//! Session-root resolution guarantees for path-taking tools.
//!
//! These tests pin down the contract that the working directory of a board *is*
//! the working directory of its agent session: a tool resolves relative paths
//! from the working directory threaded into its `ToolContext` at construction,
//! never from the process current directory. The process CWD is `/` for the
//! bundled GUI app, and a single app process hosts multiple boards — so process
//! CWD can never be a per-session root.
//!
//! The real-path test drives the `git` tool's `get diff` file mode: the process
//! CWD points away from the data, the session working dir points at the files,
//! and relative `left`/`right` paths must resolve against the session root.

use std::sync::Arc;

use serde_json::json;
use swissarmyhammer_common::test_utils::CurrentDirGuard;
use swissarmyhammer_config::ChatModelConfig;
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{register_git_tools, ToolContext, ToolRegistry};

/// Build a registry with the git tool registered.
fn git_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    register_git_tools(&mut registry);
    registry
}

/// Build a tool context rooted at `working_dir` (the session/board directory).
fn context_rooted_at(working_dir: &std::path::Path) -> ToolContext {
    let git_ops: Arc<tokio::sync::Mutex<Option<GitOperations>>> =
        Arc::new(tokio::sync::Mutex::new(None));
    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(ChatModelConfig::default());
    ToolContext::new(tool_handlers, git_ops, agent_config)
        .with_working_dir(working_dir.to_path_buf())
}

fn response_text(result: &rmcp::model::CallToolResult) -> String {
    match &result.content[0].raw {
        rmcp::model::RawContent::Text(t) => t.text.clone(),
        _ => panic!("Expected text content"),
    }
}

/// Write the `left.rs` and `right.rs` pair that the diff tests compare.
fn write_diff_fixture(dir: &std::path::Path) {
    std::fs::write(dir.join("left.rs"), "fn old_name() {}\n").unwrap();
    std::fs::write(dir.join("right.rs"), "fn new_name() {}\n").unwrap();
}

/// Run the `git` tool `get diff` op on relative `left`/`right` paths, with
/// the tool context rooted at `session_root`.
async fn git_diff_tool_call(
    session_root: &std::path::Path,
    left: &str,
    right: &str,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    let registry = git_registry();
    let context = context_rooted_at(session_root);
    let tool = registry.get_tool("git").unwrap();

    let arguments = json!({
        "op": "get diff",
        "left": left,
        "right": right,
    })
    .as_object()
    .unwrap()
    .clone();

    tool.execute(arguments, &context).await
}

/// Relative `left`/`right` paths resolve against the session working
/// directory, not the process CWD.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn git_diff_relative_paths_resolve_against_session_root() {
    let repo = tempfile::TempDir::new().unwrap();
    write_diff_fixture(repo.path());

    // Process CWD points at an unrelated dir that holds neither file.
    let elsewhere = tempfile::TempDir::new().unwrap();
    let _cwd_guard = CurrentDirGuard::new(elsewhere.path())
        .expect("Failed to pin process CWD away from the repo");

    let result = git_diff_tool_call(repo.path(), "left.rs", "right.rs")
        .await
        .expect("diff with relative paths should resolve against the session root");
    let text = response_text(&result);
    assert!(
        text.contains("new_name"),
        "relative paths should resolve against the session root, got: {text}"
    );
}

/// A relative path that exists only under the process CWD must NOT be found:
/// the session root wins, so the read fails instead of leaking the CWD file.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn git_diff_does_not_fall_back_to_process_cwd() {
    let session = tempfile::TempDir::new().unwrap();

    // The files exist only under the process CWD.
    let process_cwd = tempfile::TempDir::new().unwrap();
    write_diff_fixture(process_cwd.path());
    let _cwd_guard = CurrentDirGuard::new(process_cwd.path()).expect("Failed to pin process CWD");

    git_diff_tool_call(session.path(), "left.rs", "right.rs")
        .await
        .expect_err("a file that exists only under the process CWD must not be read");
}
