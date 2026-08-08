//! The fixtures the `code_context` test groups build on.

use std::path::PathBuf;
use std::sync::Arc;

use swissarmyhammer_code_context::CodeContextWorkspace;
use swissarmyhammer_config::model::ChatModelConfig;
use tokio::sync::Mutex as TokioMutex;

use crate::mcp::tools::code_context::index_discovered_files_async;

/// Build a ToolContext rooted at the given directory.
pub(super) fn make_context_with_dir(dir: PathBuf) -> crate::mcp::tool_registry::ToolContext {
    use crate::mcp::tool_handlers::ToolHandlers;
    let git_ops = Arc::new(TokioMutex::new(None));
    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(ChatModelConfig::default());
    let mut ctx = crate::mcp::tool_registry::ToolContext::new(tool_handlers, git_ops, agent_config);
    ctx.working_dir = Some(dir);
    ctx
}

/// Create a minimal Rust project in a temp dir and run full treesitter indexing.
///
/// Returns `(tempdir, context)` — the caller must hold `tempdir` to keep
/// the directory alive for the duration of the test.
pub(super) async fn create_indexed_project(
) -> (tempfile::TempDir, crate::mcp::tool_registry::ToolContext) {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let root = tmp.path();

    // Write source files with distinct symbols so operations have something to find.
    std::fs::create_dir_all(root.join("src")).unwrap();

    std::fs::write(
        root.join("src/main.rs"),
        r#"fn main() {
    greet("world");
}

fn greet(name: &str) {
    println!("Hello, {}!", name);
}
"#,
    )
    .unwrap();

    std::fs::write(
        root.join("src/lib.rs"),
        r#"/// A simple calculator struct.
pub struct Calculator {
    pub value: f64,
}

impl Calculator {
    /// Create a new Calculator with the given initial value.
    pub fn new(value: f64) -> Self {
        Self { value }
    }

    /// Add a number to the current value.
    pub fn add(&mut self, x: f64) -> f64 {
        self.value += x;
        self.value
    }
}
"#,
    )
    .unwrap();

    // Open the workspace — this runs startup_cleanup, marking files dirty.
    let ws = CodeContextWorkspace::open(root).expect("workspace open");

    // Run treesitter indexing so query operations have chunks to search.
    if let Some(shared_db) = ws.shared_db() {
        index_discovered_files_async(
            root,
            shared_db,
            swissarmyhammer_code_context::noop_reporter(),
            swissarmyhammer_code_context::new_shutdown_flag(),
        )
        .await;
    }

    let ctx = make_context_with_dir(root.to_path_buf());
    (tmp, ctx)
}

/// Extract the text content from the first item of a tool result.
pub(super) fn extract_text(result: &rmcp::model::CallToolResult) -> &str {
    match &result.content[0].raw {
        rmcp::model::RawContent::Text(t) => &t.text,
        _ => panic!("Expected text content"),
    }
}

/// Set up a tiny Rust project on disk and open the workspace.
///
/// Returns the tempdir (caller must keep it alive) and the shared DB ref.
/// `startup_cleanup` runs as part of `CodeContextWorkspace::open`, so the
/// `indexed_files` table is already populated with the two source files
/// (marked dirty).
pub(super) async fn make_tiny_indexable_project() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    swissarmyhammer_code_context::SharedDb,
) {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let root = tmp.path().to_path_buf();

    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/main.rs"),
        "fn main() {\n    println!(\"hi\");\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
    )
    .unwrap();

    let ws = CodeContextWorkspace::open(&root).expect("workspace open");
    let shared_db = ws.shared_db().expect("leader has shared db");
    (tmp, root, shared_db)
}

/// Count chunks in `ts_chunks` that have a non-NULL `embedding` blob.
pub(super) fn count_embedded_chunks(db: &swissarmyhammer_code_context::SharedDb) -> i64 {
    let conn = db.lock().unwrap_or_else(|p| p.into_inner());
    conn.query_row(
        "SELECT COUNT(*) FROM ts_chunks WHERE embedding IS NOT NULL",
        [],
        |r| r.get(0),
    )
    .unwrap()
}

/// Count total chunks regardless of embedding state.
pub(super) fn count_total_chunks(db: &swissarmyhammer_code_context::SharedDb) -> i64 {
    let conn = db.lock().unwrap_or_else(|p| p.into_inner());
    conn.query_row("SELECT COUNT(*) FROM ts_chunks", [], |r| r.get(0))
        .unwrap()
}

/// Read the `embedded` flag for a file row.
pub(super) fn read_embedded_flag(
    db: &swissarmyhammer_code_context::SharedDb,
    file_path: &str,
) -> Option<i64> {
    let conn = db.lock().unwrap_or_else(|p| p.into_inner());
    conn.query_row(
        "SELECT embedded FROM indexed_files WHERE file_path = ?",
        rusqlite::params![file_path],
        |r| r.get(0),
    )
    .ok()
}
