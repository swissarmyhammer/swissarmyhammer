//! Session-root resolution guarantees for file tools.
//!
//! These tests pin down the contract that the working directory of a board *is*
//! the working directory of its agent session: every file tool resolves its
//! default search root and relative paths from `ToolContext::session_root` (the
//! board directory threaded in at construction), never from the process current
//! directory. The process CWD is `/` for the bundled GUI app, and a single app
//! process hosts multiple boards — so process CWD can never be a per-session
//! root.
//!
//! Two complementary checks:
//!
//! 1. A **real-path** test that reproduces the original "grep hung forever"
//!    failure shape: process CWD points away from the data, the session
//!    working dir points at a repo, and an unscoped grep must search the repo,
//!    return promptly, and skip `target/`, `.git/`, and `.gitignore`d paths.
//! 2. A **guard** test that statically forbids `std::env::current_dir()` in the
//!    file/shell tool handlers, so the fix cannot silently regress.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;
use swissarmyhammer_common::test_utils::CurrentDirGuard;
use swissarmyhammer_config::ModelConfig;
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{ToolContext, ToolRegistry};
use swissarmyhammer_tools::mcp::tools::files;

/// Build a registry with the file tools registered.
fn file_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    files::register_file_tools(&mut registry);
    registry
}

/// Build a tool context rooted at `working_dir` (the session/board directory).
fn context_rooted_at(working_dir: &std::path::Path) -> ToolContext {
    let git_ops: Arc<tokio::sync::Mutex<Option<GitOperations>>> =
        Arc::new(tokio::sync::Mutex::new(None));
    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(ModelConfig::default());
    ToolContext::new(tool_handlers, git_ops, agent_config)
        .with_working_dir(working_dir.to_path_buf())
}

fn response_text(result: &rmcp::model::CallToolResult) -> String {
    match &result.content[0].raw {
        rmcp::model::RawContent::Text(t) => t.text.clone(),
        _ => panic!("Expected text content"),
    }
}

/// Reproduces the production failure shape: the process CWD points away from the
/// data, the session working dir is the repo, and an unscoped grep must search
/// the repo (not the process CWD), return promptly, and honor `.gitignore` plus
/// skip `.git/` and `target/`.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn grep_unscoped_searches_session_root_and_honors_gitignore() {
    // Session working directory: a small repo with a match plus paths that must
    // be skipped.
    let repo = tempfile::TempDir::new().unwrap();
    let repo_path = repo.path();

    std::fs::write(
        repo_path.join("source.rs"),
        "fn builtin_partial() {}\n// builtin partial marker\n",
    )
    .unwrap();
    std::fs::write(repo_path.join(".gitignore"), "ignored.rs\n").unwrap();
    std::fs::write(repo_path.join("ignored.rs"), "builtin partial ignored\n").unwrap();

    // A `target/` directory the walker must not descend into (ripgrep skips it
    // because it is gitignored in real repos; here we add it to .gitignore too).
    std::fs::create_dir(repo_path.join("target")).unwrap();
    std::fs::write(
        repo_path.join("target/built.rs"),
        "builtin partial in target\n",
    )
    .unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(repo_path.join(".gitignore"))
        .and_then(|mut f| {
            use std::io::Write;
            writeln!(f, "target/")
        })
        .unwrap();

    // A `.git/` directory the walker must skip (hidden).
    std::fs::create_dir(repo_path.join(".git")).unwrap();
    std::fs::write(
        repo_path.join(".git/config"),
        "builtin partial in git dir\n",
    )
    .unwrap();

    // Pin the process CWD to a *different* temp dir that contains no matches, so
    // a search rooted at the process CWD would find nothing — proving the tool
    // rooted at the session working dir instead.
    let elsewhere = tempfile::TempDir::new().unwrap();
    let _cwd_guard = CurrentDirGuard::new(elsewhere.path())
        .expect("Failed to pin process CWD away from the repo");

    let registry = file_registry();
    let context = context_rooted_at(repo_path);
    let tool = registry.get_tool("files").unwrap();

    // Unscoped grep: no `path` supplied, exactly like the failing call.
    let arguments = json!({
        "op": "grep files",
        "pattern": "builtin.*partial",
        "output_mode": "content",
        "case_insensitive": true,
    })
    .as_object()
    .unwrap()
    .clone();

    let started = Instant::now();
    let result = tool
        .execute(arguments, &context)
        .await
        .expect("grep should succeed");
    let elapsed = started.elapsed();

    // Returns promptly — it searched a small repo, not the whole filesystem.
    assert!(
        elapsed < Duration::from_secs(10),
        "unscoped grep took too long ({elapsed:?}); it likely walked outside the session root"
    );

    let text = response_text(&result);

    // Found the match in the session root.
    assert!(
        text.contains("source.rs"),
        "expected a match in the session root, got: {text}"
    );

    // Skipped gitignored, target/, and .git/ paths.
    assert!(
        !text.contains("ignored.rs"),
        "gitignored file must be skipped, got: {text}"
    );
    assert!(
        !text.contains("target/") && !text.contains("built.rs"),
        "target/ must be skipped, got: {text}"
    );
    assert!(
        !text.contains(".git/") && !text.contains("git dir"),
        ".git/ must be skipped, got: {text}"
    );
}

/// A relative `path` resolves against the session working directory, not the
/// process CWD.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn grep_relative_path_resolves_against_session_root() {
    let repo = tempfile::TempDir::new().unwrap();
    let sub = repo.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join("hit.txt"), "needle here\n").unwrap();

    // Process CWD points at an unrelated dir with no `sub/` underneath.
    let elsewhere = tempfile::TempDir::new().unwrap();
    let _cwd_guard = CurrentDirGuard::new(elsewhere.path())
        .expect("Failed to pin process CWD away from the repo");

    let registry = file_registry();
    let context = context_rooted_at(repo.path());
    let tool = registry.get_tool("files").unwrap();

    let arguments = json!({
        "op": "grep files",
        "pattern": "needle",
        "path": "sub",
        "output_mode": "content",
    })
    .as_object()
    .unwrap()
    .clone();

    let result = tool
        .execute(arguments, &context)
        .await
        .expect("grep with relative path should succeed");
    let text = response_text(&result);
    assert!(
        text.contains("hit.txt"),
        "relative path should resolve against the session root, got: {text}"
    );
}

/// An unscoped grep whose session root resolves to the filesystem root must be
/// refused outright rather than walking the whole machine.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn grep_unscoped_refuses_filesystem_root() {
    let registry = file_registry();
    let context = context_rooted_at(std::path::Path::new("/"));
    let tool = registry.get_tool("files").unwrap();

    let arguments = json!({
        "op": "grep files",
        "pattern": "anything",
        "output_mode": "content",
    })
    .as_object()
    .unwrap()
    .clone();

    let err = tool
        .execute(arguments, &context)
        .await
        .expect_err("grep rooted at `/` with no path must be refused");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("filesystem root"),
        "grep should refuse the filesystem root, got: {msg}"
    );
}

/// An unscoped glob whose session root resolves to the filesystem root must be
/// refused too — the same guard grep applies, so glob can't stream the whole
/// filesystem.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn glob_unscoped_refuses_filesystem_root() {
    let registry = file_registry();
    let context = context_rooted_at(std::path::Path::new("/"));
    let tool = registry.get_tool("files").unwrap();

    // A scoped-enough pattern that passes `validate_glob_pattern` (it has a
    // directory prefix) so execution reaches the filesystem-root guard rather
    // than the broad-pattern rejection.
    let arguments = json!({
        "op": "glob files",
        "pattern": "src/**/*.rs",
    })
    .as_object()
    .unwrap()
    .clone();

    let err = tool
        .execute(arguments, &context)
        .await
        .expect_err("glob rooted at `/` with no path must be refused");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("filesystem root"),
        "glob should refuse the filesystem root, got: {msg}"
    );
}

/// Static guard: file and shell tool handlers must never call
/// `std::env::current_dir()`. Default roots and relative-path resolution come
/// from `ToolContext::session_root`, which is the single sanctioned place that
/// fallback lives. Test modules (`#[cfg(test)]`) are excluded, and the shell
/// command-history storage singleton (`shell/state.rs`) is exempt because its
/// `.shell/` directory is a process-level storage concern with its own
/// documented GUI-CWD-readonly fallback — not a per-tool-call operating root.
#[test]
fn tool_handlers_do_not_call_current_dir() {
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    let tools_root = std::path::Path::new(crate_dir).join("src/mcp/tools");

    // Directories whose handlers resolve an operating root or relative paths.
    let scanned_dirs = ["files", "shell"];
    // Files exempt from the ban (with the reason captured above).
    let exempt_files = ["state.rs"];

    let mut offenders: Vec<String> = Vec::new();

    for dir in scanned_dirs {
        let dir_path = tools_root.join(dir);
        visit_rs_files(&dir_path, &mut |path| {
            if exempt_files
                .iter()
                .any(|name| path.file_name().and_then(|n| n.to_str()) == Some(*name))
            {
                return;
            }

            let src = std::fs::read_to_string(path).expect("read tool source");

            // Only scan the non-test portion of each file. Each handler module
            // keeps its unit tests in a single trailing `#[cfg(test)] mod tests`,
            // and tests legitimately set up temp/cwd fixtures.
            let production = match src.find("#[cfg(test)]") {
                Some(idx) => &src[..idx],
                None => &src[..],
            };

            if production.contains("std::env::current_dir")
                || production.contains("env::current_dir")
            {
                offenders.push(path.display().to_string());
            }
        });
    }

    assert!(
        offenders.is_empty(),
        "tool handlers must resolve roots from ToolContext::session_root, \
         not std::env::current_dir(). Offending files: {offenders:?}"
    );
}

/// Recursively visit every `.rs` file under `dir`, calling `f` on each.
fn visit_rs_files(dir: &std::path::Path, f: &mut impl FnMut(&std::path::Path)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_rs_files(&path, f);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            f(&path);
        }
    }
}
