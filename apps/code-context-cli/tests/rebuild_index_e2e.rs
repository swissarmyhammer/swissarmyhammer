//! End-to-end test for the `rebuild index` CLI op with progress wiring.
//!
//! These tests spawn the real `code-context` binary via `assert_cmd` against a
//! temporary workspace containing a couple of source files. They exercise the
//! full path: clap parsing -> `run_operation` -> `ToolContext` with a progress
//! sink -> `CodeContextTool::execute("rebuild index", ...)` -> renderer task
//! drain -> printed `CallToolResult`.
//!
//! The assertions deliberately ignore live progress-bar control codes and only
//! check the final summary line emitted after the renderer task exits. That
//! mirrors the task spec — bars are TTY chrome, the summary is the contract.
//!
//! Two cases are covered:
//!
//! 1. Default invocation: the `IndicatifRenderer` is wired in but `indicatif`
//!    auto-degrades to plain output for the non-TTY stdout that `assert_cmd`
//!    captures. The summary must still land on stdout.
//! 2. `--no-progress`: the `NullRenderer` is wired in. The summary must be
//!    identical and the output must contain no ANSI escape sequences (a quick
//!    structural check that the null renderer produced nothing extra).

use assert_cmd::Command;
use tempfile::TempDir;

/// Build a temporary workspace with a Cargo.toml and a couple of Rust source
/// files. Returns the `TempDir` guard so the caller controls cleanup.
///
/// The contents mirror the minimal layout the MCP code_context tool expects
/// for discovery: a project root with `Cargo.toml` plus at least one source
/// file the tree-sitter indexer can chunk. We keep the files tiny so the
/// rebuild completes well under any nextest timeout.
fn make_workspace() -> TempDir {
    let tmp = TempDir::new().expect("create temp workspace");
    let root = tmp.path();

    std::fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "rebuild-index-e2e"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("write Cargo.toml");

    std::fs::create_dir_all(root.join("src")).expect("mkdir src");
    std::fs::write(
        root.join("src/main.rs"),
        r#"fn main() {
    greet("world");
}

fn greet(name: &str) {
    println!("hello, {name}");
}
"#,
    )
    .expect("write src/main.rs");

    std::fs::write(
        root.join("src/lib.rs"),
        r#"pub struct Greeter {
    pub name: String,
}

impl Greeter {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    pub fn greet(&self) -> String {
        format!("hello, {}", self.name)
    }
}
"#,
    )
    .expect("write src/lib.rs");

    tmp
}

/// Default invocation: progress is wired but stdout is not a TTY under
/// `assert_cmd`, so `indicatif` degrades to plain output. The exit code must
/// be 0 and the captured stdout must contain the summary fields the tool
/// emits (`files_indexed`, `chunks_written`, `elapsed_ms`).
#[test]
fn rebuild_index_default_progress_prints_summary() {
    let workspace = make_workspace();

    let output = Command::cargo_bin("code-context")
        .expect("locate code-context binary")
        .current_dir(workspace.path())
        // This test asserts the rebuild summary contract (field presence), not
        // semantic embeddings. Skip the multi-GB embedding-model load so the
        // run stays fast and hermetic — on a clean machine the model would be
        // downloaded from HuggingFace and dominate the test time.
        .env("SAH_DISABLE_EMBEDDING", "1")
        .args(["rebuild", "index", "--layer", "treesitter"])
        .output()
        .expect("spawn code-context");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "rebuild index failed: status={:?}\nstdout=\n{}\nstderr=\n{}",
        output.status,
        stdout,
        stderr,
    );

    // The tool prints the serialized `RebuildIndexResult` as JSON text. We
    // only assert on the field names, not the values: the exact counts depend
    // on tree-sitter chunking heuristics and can change without breaking the
    // contract this test covers.
    for field in ["files_indexed", "chunks_written", "elapsed_ms", "layer"] {
        assert!(
            stdout.contains(field),
            "expected `{}` in summary output, got:\n{}",
            field,
            stdout,
        );
    }
    assert!(
        stdout.contains("treesitter"),
        "expected `treesitter` layer in summary, got:\n{}",
        stdout,
    );
}

/// `--no-progress` swaps the `IndicatifRenderer` for the `NullRenderer`. The
/// summary line must be identical to the default case and the captured output
/// must contain no ANSI escape sequences (a structural check that no extra
/// progress chrome leaked through).
#[test]
fn rebuild_index_no_progress_prints_clean_summary() {
    let workspace = make_workspace();

    let output = Command::cargo_bin("code-context")
        .expect("locate code-context binary")
        .current_dir(workspace.path())
        // See the default-progress test: the embeddings are irrelevant to the
        // summary contract under test, so skip the model load.
        .env("SAH_DISABLE_EMBEDDING", "1")
        .args(["--no-progress", "rebuild", "index", "--layer", "treesitter"])
        .output()
        .expect("spawn code-context");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "rebuild index --no-progress failed: status={:?}\nstdout=\n{}\nstderr=\n{}",
        output.status,
        stdout,
        stderr,
    );

    for field in ["files_indexed", "chunks_written", "elapsed_ms", "layer"] {
        assert!(
            stdout.contains(field),
            "expected `{}` in summary output, got:\n{}",
            field,
            stdout,
        );
    }

    // The null renderer emits no characters at all. `indicatif` would also
    // suppress its bar on a non-TTY pipe, but the null path is the hard
    // guarantee — assert that no ANSI escape sequence (CSI `\x1b[`) made it
    // into either stream.
    assert!(
        !stdout.contains('\u{1b}'),
        "stdout contained ANSI escape sequence under --no-progress:\n{:?}",
        stdout,
    );
    assert!(
        !stderr.contains('\u{1b}'),
        "stderr contained ANSI escape sequence under --no-progress:\n{:?}",
        stderr,
    );
}
