//! End-to-end real-pipeline tests for code-context ops other than `search code`.
//!
//! The audit (kanban `01KREPHGT14TY08K2JBCNFXEJP`) found every test under
//! `swissarmyhammer-code-context/src/ops/` is fixture-only — they raw-SQL
//! pre-seed `ts_chunks` / `lsp_symbols` / `lsp_call_edges` and assert on
//! the math, not on what users actually get when they call the op against
//! a workspace indexed by the real production pipeline. Card 4
//! (`semantic_search_e2e.rs`) closed that gap for `search code`; this file
//! does the same for `find_duplicates`, `grep_code`, and the LSP-layered
//! ops (`search_symbol`, `get_callgraph`, `get_blastradius`) exercised by
//! [`qwen_embedding_lsp_layered_e2e`].
//!
//! Modeled directly on `semantic_search_e2e.rs`. Same fixture pattern: create
//! a temp workspace, open a real [`CodeContextWorkspace`] so
//! `startup_cleanup` populates `indexed_files`, then drive
//! [`index_discovered_files_async`] (the production indexer used by the MCP
//! server bootstrap and the file watcher). After indexing, call the op
//! through the MCP tool registry and assert on the user-facing JSON result.
//!
//! ## LSP-layered ops
//!
//! `search_symbol`, `get_callgraph`, `get_blastradius`, and the other ops
//! that read `lsp_symbols` / `lsp_call_edges` need a running LSP daemon to
//! populate those tables. The lower-level LSP-to-DB persistence path is
//! covered by `swissarmyhammer-code-context/tests/integration_test.rs::test_real_lsp_document_symbols`;
//! the MCP-tool-layer assertions on top of that live below in
//! [`qwen_embedding_lsp_layered_e2e`]. That test spawns a real
//! `rust-analyzer` process via `LspJsonRpcClient`, drives the LSP indexing
//! through `collect_and_persist_file_symbols` +
//! `collect_and_persist_call_edges`, and then issues `search symbol`,
//! `get callgraph`, and `get blastradius` through the MCP tool registry.
//!
//! ## Embedding model dependency
//!
//! `find_duplicates` filters on `ts_chunks.embedding IS NOT NULL`. Without
//! the embedder, the production indexer leaves `embedded=0` and the op
//! returns zero groups. All three tests therefore drive
//! `index_discovered_files_async`, which loads `Embedder::default()`
//! (qwen-embedding — ~600 MB GGUF on Linux / Apple Neural Engine
//! `.mlpackage` on macOS arm64). Following the convention in
//! `.config/nextest.toml`, all three tests are named `qwen_embedding_*` so
//! the default-filter excludes them; run them via the `embedding-models`
//! profile:
//!
//! ```text
//! cargo nextest run --profile embedding-models --ignore-default-filter \
//!     -p swissarmyhammer-tools --test tools_tests \
//!     qwen_embedding_find_duplicates_e2e
//! cargo nextest run --profile embedding-models --ignore-default-filter \
//!     -p swissarmyhammer-tools --test tools_tests \
//!     qwen_embedding_grep_code_e2e
//! cargo nextest run --profile embedding-models --ignore-default-filter \
//!     -p swissarmyhammer-tools --test tools_tests \
//!     qwen_embedding_lsp_layered_e2e
//! ```
//!
//! [`qwen_embedding_lsp_layered_e2e`] additionally requires `rust-analyzer`
//! on `$PATH`; it skips with a `println!` and exits 0 when the binary is
//! not installed.

use std::path::Path;
use std::sync::Arc;

use serde_json::json;
use swissarmyhammer_code_context::{
    detect_rust_analyzer, CodeContextWorkspace, LspJsonRpcClient, SharedDb,
};
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_config::ModelConfig;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{ToolContext, ToolRegistry};
use swissarmyhammer_tools::mcp::tools::code_context::{
    index_discovered_files_async, register_code_context_tools,
};
use tokio::sync::Mutex as TokioMutex;

/// File names used inside the temp workspace. `stats_a.rs` and `stats_b.rs`
/// share a near-identical statistics function so `find_duplicates` has a
/// real duplicate group to report. `unrelated.rs` holds content that
/// should NOT match.
const FILE_STATS_A: &str = "src/stats_a.rs";
const FILE_STATS_B: &str = "src/stats_b.rs";
const FILE_UNRELATED: &str = "src/unrelated.rs";

/// A unique identifier present in `unrelated.rs` and nowhere else. Used by
/// the `grep_code` test to assert that the regex matched the right chunk.
/// Plain English so it can't be misclassified as a sensitive value.
const GREP_NEEDLE: &str = "this is a test sentinel marker for grep_code";

/// Build a [`ToolContext`] rooted at `dir` so MCP operations resolve the
/// workspace under that directory rather than the test's CWD.
fn make_context_with_dir(dir: &Path) -> ToolContext {
    let git_ops = Arc::new(TokioMutex::new(None));
    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(ModelConfig::default());
    let mut ctx = ToolContext::new(tool_handlers, git_ops, agent_config);
    ctx.working_dir = Some(dir.to_path_buf());
    ctx
}

/// Write three Rust files into `root`. `stats_a.rs` and `stats_b.rs` carry
/// near-identical statistics code so the embedder produces similar vectors
/// for them; `unrelated.rs` carries different content with a unique
/// [`GREP_NEEDLE`] for the grep assertion.
///
/// Both stats files are over 100 bytes (the default `min_chunk_bytes` for
/// `find_duplicates`) and share the same function body so cosine similarity
/// between their chunks lands well above any reasonable threshold.
fn write_sources(root: &Path) {
    std::fs::create_dir_all(root.join("src")).expect("create src/");

    // Two near-duplicate statistics modules. Both define `mean_squared_error`
    // with the same body — only the module-level doc comment differs. This
    // is the canonical copy-paste-with-minor-edit pattern that
    // `find_duplicates` is designed to surface.
    let stats_body = r#"
/// Compute the mean squared error between predicted and observed series.
/// Returns 0.0 for empty inputs and panics on length mismatch.
pub fn mean_squared_error(predicted: &[f64], observed: &[f64]) -> f64 {
    assert_eq!(predicted.len(), observed.len(), "length mismatch");
    if predicted.is_empty() {
        return 0.0;
    }
    let mut total = 0.0;
    for (p, o) in predicted.iter().zip(observed.iter()) {
        let delta = p - o;
        total += delta * delta;
    }
    total / predicted.len() as f64
}

/// Compute the arithmetic mean of a slice of f64 values.
pub fn arithmetic_mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut total = 0.0;
    for v in values {
        total += v;
    }
    total / values.len() as f64
}

/// Compute the population variance of a slice of f64 values.
fn population_variance(values: &[f64]) -> f64 {
    let mu = arithmetic_mean(values);
    if values.is_empty() {
        return 0.0;
    }
    let mut total = 0.0;
    for v in values {
        let delta = v - mu;
        total += delta * delta;
    }
    total / values.len() as f64
}
"#;

    std::fs::write(
        root.join(FILE_STATS_A),
        format!("//! Statistics helpers — variant A.\n{}", stats_body),
    )
    .expect("write stats_a.rs");

    std::fs::write(
        root.join(FILE_STATS_B),
        format!(
            "//! Statistics helpers — variant B (copy-pasted from A).\n{}",
            stats_body
        ),
    )
    .expect("write stats_b.rs");

    // A semantically unrelated file with the grep needle baked into the
    // body of a clearly-named function, so we can assert the regex match
    // is the right chunk.
    std::fs::write(
        root.join(FILE_UNRELATED),
        format!(
            r#"//! Unrelated module — no statistics, no duplication.

/// Returns a fixed sentinel marker string used by integration tests
/// to confirm `grep_code` matched the intended chunk.
pub fn sentinel() -> &'static str {{
    "{}"
}}

/// Compute the greatest common divisor via Euclid's algorithm.
pub fn gcd(mut a: i64, mut b: i64) -> i64 {{
    while b != 0 {{
        let t = b;
        b = a % b;
        a = t;
    }}
    a.abs()
}}
"#,
            GREP_NEEDLE
        ),
    )
    .expect("write unrelated.rs");
}

/// Count `ts_chunks` rows with a non-NULL embedding blob. Used as a
/// post-index sanity check — if this is zero, the embedder silently
/// failed and the rest of the test would assert on stale data.
fn count_embedded_chunks(db: &SharedDb) -> i64 {
    let conn = db.lock().unwrap_or_else(|p| p.into_inner());
    conn.query_row(
        "SELECT COUNT(*) FROM ts_chunks WHERE embedding IS NOT NULL",
        [],
        |r| r.get(0),
    )
    .expect("count embedded chunks")
}

/// Extract the text content of the first item of a `CallToolResult`.
/// Panics if the first content item is not a text block — every code-context
/// MCP op returns JSON-as-text.
fn extract_text(result: &rmcp::model::CallToolResult) -> &str {
    match &result.content[0].raw {
        rmcp::model::RawContent::Text(t) => &t.text,
        _ => panic!("Expected text content"),
    }
}

/// Set up an isolated temp workspace, write the test sources, and drive
/// the real production indexer end-to-end. Returns the workspace root and
/// the leader's shared DB so the caller can issue MCP-tool requests
/// against it.
///
/// Keeps the [`IsolatedTestEnvironment`] alive via the returned tuple so
/// the guarded HOME/CWD overrides remain in effect for the rest of the
/// test. The `TempDir` is also returned to keep the workspace directory
/// alive for the lifetime of the test.
async fn index_real_workspace() -> (IsolatedTestEnvironment, tempfile::TempDir, SharedDb) {
    let env = IsolatedTestEnvironment::new().expect("create isolated test environment");

    let tmp = tempfile::TempDir::new().expect("create workspace tempdir");
    let root = tmp.path().to_path_buf();

    write_sources(&root);

    let ws = CodeContextWorkspace::open(&root).expect("open workspace");
    let shared_db = ws.shared_db().expect("leader has shared db");

    // Drive the REAL production indexer — loads `Embedder::default()` and
    // writes real embedding blobs into `ts_chunks`.
    index_discovered_files_async(
        &root,
        Arc::clone(&shared_db),
        swissarmyhammer_code_context::noop_reporter(),
    )
    .await;

    // Sanity: at least one embedding must exist, otherwise the embedder
    // silently no-op'd and the duplicate test below would falsely pass
    // with zero groups.
    let embedded = count_embedded_chunks(&shared_db);
    assert!(
        embedded > 0,
        "real indexer must write at least one non-NULL embedding, got {embedded} — \
         the qwen embedder probably failed to load"
    );

    (env, tmp, shared_db)
}

// ---------------------------------------------------------------------------
// find_duplicates e2e
// ---------------------------------------------------------------------------

/// End-to-end: real indexer over two near-duplicate source files, then call
/// `find_duplicates` through the MCP tool path and assert the duplicate
/// group surfaces the other file.
///
/// This is the test that would have caught the original
/// "every chunk has `embedding IS NULL`" bug — `find_duplicates` filters
/// chunks on `embedding IS NOT NULL`, so without real embeddings it would
/// return zero groups and the assertion below would fail.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn qwen_embedding_find_duplicates_e2e() {
    let (_env, tmp, _db) = index_real_workspace().await;
    let root = tmp.path();

    let mut registry = ToolRegistry::new();
    register_code_context_tools(&mut registry);
    let context = make_context_with_dir(root);
    let tool = registry
        .get_tool("code_context")
        .expect("code_context tool");

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("find duplicates"));
    args.insert("file_path".to_string(), json!(FILE_STATS_A));
    // Default 0.85 is conservative for near-identical text but the qwen
    // embedder's exact cosine on these chunks is not contractually fixed.
    // Use a permissive threshold — we still rely on the duplicate file
    // being in `duplicates`, not on the absolute similarity number.
    args.insert("min_similarity".to_string(), json!(0.5));
    // The stats body is well over 100 bytes, but be explicit about the
    // size floor so the test isn't accidentally invalidated if the
    // default changes.
    args.insert("min_chunk_bytes".to_string(), json!(50));

    let result = tool
        .execute(args, &context)
        .await
        .expect("find duplicates dispatch should succeed");

    let body = extract_text(&result);
    assert!(
        !body.contains("Index not ready"),
        "find duplicates must not return the readiness placeholder — \
         the indexer above just ran (got: {body})"
    );

    let parsed: serde_json::Value = serde_json::from_str(body).unwrap_or_else(|e| {
        panic!(
            "find duplicates result must be JSON-encoded FindDuplicatesResult, \
             got error {e} for body: {body}"
        )
    });

    let file_field = parsed
        .get("file")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("result must have a `file` string, body was: {body}"));
    assert_eq!(
        file_field, FILE_STATS_A,
        "result.file should echo the queried file path"
    );

    let groups = parsed
        .get("groups")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("result must have a `groups` array, body was: {body}"));

    assert!(
        !groups.is_empty(),
        "real indexer + two copy-pasted source files should produce at least one \
         duplicate group; got 0 — body was: {body}"
    );

    // At least one group must point at the sibling stats file. The body
    // text of `stats_a.rs` is the source; `stats_b.rs` is the duplicate.
    let mentions_sibling = groups.iter().any(|group| {
        group
            .get("duplicates")
            .and_then(|d| d.as_array())
            .is_some_and(|dups| {
                dups.iter().any(|dup| {
                    dup.get("chunk")
                        .and_then(|c| c.get("file_path"))
                        .and_then(|p| p.as_str())
                        .is_some_and(|p| p == FILE_STATS_B)
                })
            })
    });
    assert!(
        mentions_sibling,
        "expected at least one duplicate group whose `duplicates` includes {FILE_STATS_B}, \
         body was: {body}"
    );
}

// ---------------------------------------------------------------------------
// grep_code e2e
// ---------------------------------------------------------------------------

/// End-to-end: real indexer over the same workspace, then issue a `grep
/// code` op through the MCP tool path with a regex that uniquely matches
/// one chunk and assert that chunk surfaces.
///
/// Unlike `find_duplicates`, `grep_code` only reads `ts_chunks.text` — it
/// does not depend on embeddings. Running it after the same real-pipeline
/// indexing pass proves the chunk-text storage path is wired up and the
/// op works end-to-end through the MCP dispatcher.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn qwen_embedding_grep_code_e2e() {
    let (_env, tmp, _db) = index_real_workspace().await;
    let root = tmp.path();

    let mut registry = ToolRegistry::new();
    register_code_context_tools(&mut registry);
    let context = make_context_with_dir(root);
    let tool = registry
        .get_tool("code_context")
        .expect("code_context tool");

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("grep code"));
    args.insert("pattern".to_string(), json!(GREP_NEEDLE));

    let result = tool
        .execute(args, &context)
        .await
        .expect("grep code dispatch should succeed");

    let body = extract_text(&result);
    assert!(
        !body.contains("Index not ready"),
        "grep code must not return the readiness placeholder — \
         the indexer above just ran (got: {body})"
    );

    let parsed: serde_json::Value = serde_json::from_str(body).unwrap_or_else(|e| {
        panic!(
            "grep code result must be JSON-encoded GrepResult, \
             got error {e} for body: {body}"
        )
    });

    let pattern = parsed
        .get("pattern")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("result must have a `pattern` string, body was: {body}"));
    assert_eq!(
        pattern, GREP_NEEDLE,
        "result.pattern should echo the regex we sent"
    );

    let matches = parsed
        .get("matches")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("result must have a `matches` array, body was: {body}"));

    assert!(
        !matches.is_empty(),
        "real indexer + a unique needle planted in unrelated.rs should produce \
         at least one match; got 0 — body was: {body}"
    );

    // The needle is unique to `unrelated.rs`, so every match must come
    // from that file. (Per-chunk granularity: there might be one chunk
    // matched, but its `file_path` is non-negotiable.)
    for (idx, m) in matches.iter().enumerate() {
        let file_path = m
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("match[{idx}] must have a `file_path`, body was: {body}"));
        assert_eq!(
            file_path, FILE_UNRELATED,
            "match[{idx}] should be in {FILE_UNRELATED}, got {file_path} — body was: {body}"
        );

        // Each match must also report at least one (start, end) match
        // position pointing into the chunk text — that's the structural
        // contract of `GrepMatch`.
        let positions = m
            .get("matches")
            .and_then(|v| v.as_array())
            .unwrap_or_else(|| {
                panic!("match[{idx}] must have a `matches` positions array, body was: {body}")
            });
        assert!(
            !positions.is_empty(),
            "match[{idx}] must report at least one position; body was: {body}"
        );
    }
}

// ---------------------------------------------------------------------------
// LSP-layered ops e2e (search_symbol / get_callgraph / get_blastradius)
// ---------------------------------------------------------------------------

/// File name used for the LSP-layered test fixture. A `src/main.rs` with a
/// known call graph: `main -> foo -> helper`, `main -> bar -> helper`,
/// `bar -> helper`. Picked so `get_blastradius` on `helper` has more than
/// one inbound caller to compute.
const LSP_MAIN_RS: &str = "src/main.rs";

/// Source code for the LSP-layered e2e test fixture. Mirrors the shape of
/// `KNOWN_CALL_GRAPH_RS` from the lower-level integration test in
/// `swissarmyhammer-code-context/tests/integration_test.rs` so the same
/// call-graph reasoning applies: `helper` has two inbound callers
/// (`foo` and `bar`), each of which has one inbound caller (`main`).
const LSP_MAIN_RS_BODY: &str = r#"fn main() {
    foo();
    bar();
}

fn foo() {
    helper();
}

fn bar() {
    helper();
}

fn helper() {}
"#;

/// Minimal `Cargo.toml` for the LSP-layered fixture project. rust-analyzer
/// refuses to analyze a directory that has no Cargo manifest, so this file
/// is mandatory for the test to produce LSP symbols.
const LSP_CARGO_TOML: &str = r#"[package]
name = "lsp-layered-fixture"
version = "0.1.0"
edition = "2021"
"#;

/// Write the LSP-layered fixture project into `root`: `Cargo.toml` plus
/// `src/main.rs` with the known call graph. The project is a binary crate
/// because `main` is the natural root for rust-analyzer's call hierarchy.
fn write_lsp_sources(root: &Path) {
    std::fs::create_dir_all(root.join("src")).expect("create src/");
    std::fs::write(root.join("Cargo.toml"), LSP_CARGO_TOML).expect("write Cargo.toml");
    std::fs::write(root.join(LSP_MAIN_RS), LSP_MAIN_RS_BODY).expect("write src/main.rs");
}

/// Poll the database until at least `min_symbols` rows exist in
/// `lsp_symbols` for the given file, or `timeout` is reached.
///
/// Returns the final row count. rust-analyzer can take several seconds to
/// produce documentSymbol results on a cold cache; this loop tolerates that
/// without burning the entire test budget on a fixed sleep.
fn wait_for_lsp_symbol_rows(
    db: &SharedDb,
    rel_path: &str,
    min_symbols: usize,
    timeout: std::time::Duration,
) -> usize {
    let start = std::time::Instant::now();
    let poll_interval = std::time::Duration::from_millis(250);
    loop {
        let count: i64 = {
            let conn = db.lock().unwrap_or_else(|p| p.into_inner());
            conn.query_row(
                "SELECT COUNT(*) FROM lsp_symbols WHERE file_path = ?1",
                [rel_path],
                |r| r.get(0),
            )
            .unwrap_or(0)
        };
        if count as usize >= min_symbols {
            return count as usize;
        }
        if start.elapsed() >= timeout {
            return count as usize;
        }
        std::thread::sleep(poll_interval);
    }
}

/// Spawn `rust-analyzer`, initialize the LSP session against `root`, open
/// `src/main.rs`, and drive both `collect_and_persist_file_symbols` and
/// `collect_and_persist_call_edges` against the leader's shared DB.
///
/// Returns the final number of `lsp_call_edges` rows written. The caller is
/// expected to assert on LSP symbols (always present after this returns) and
/// to gate edge-dependent assertions on the returned count being > 0.
fn drive_lsp_persistence(root: &Path, db: &SharedDb) -> i64 {
    use std::process::{Command, Stdio};

    let main_rs_path = root.join(LSP_MAIN_RS);

    let mut child = Command::new("rust-analyzer")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn rust-analyzer");

    let stdin = child.stdin.take().expect("take rust-analyzer stdin");
    let stdout = child.stdout.take().expect("take rust-analyzer stdout");
    let mut client = LspJsonRpcClient::new(stdin, stdout);

    client.initialize(root).expect("LSP initialize");
    client
        .send_did_open(&main_rs_path, "rust", LSP_MAIN_RS_BODY)
        .expect("LSP didOpen");

    // rust-analyzer needs time to analyze before responding to
    // documentSymbol. Poll via the client's collect path until we see
    // results — 30s mirrors the lower-level integration test.
    let symbol_count = poll_symbol_count_via_client(&mut client, &main_rs_path);
    assert!(
        symbol_count >= 4,
        "rust-analyzer should return at least 4 symbols (main, foo, bar, helper), got {}",
        symbol_count
    );

    // Persist symbols into lsp_symbols / mark lsp_indexed = 1.
    {
        let conn = db.lock().unwrap_or_else(|p| p.into_inner());
        let persist = client
            .collect_and_persist_file_symbols(&conn, &main_rs_path, LSP_MAIN_RS)
            .expect("collect_and_persist_file_symbols");
        assert!(
            persist.error.is_none(),
            "LSP documentSymbol should not error: {:?}",
            persist.error
        );
        assert!(
            persist.symbol_count >= 4,
            "expected at least 4 persisted symbols, got {}",
            persist.symbol_count
        );
    }

    // Attempt call-edge persistence. rust-analyzer's callHierarchy support
    // varies by version; on older builds this can return 0 edges without
    // erroring. The caller uses the returned count to gate edge-dependent
    // assertions.
    let edge_count = {
        let conn = db.lock().unwrap_or_else(|p| p.into_inner());
        match client.collect_and_persist_call_edges(&conn, &main_rs_path, LSP_MAIN_RS) {
            Ok(n) => n as i64,
            Err(e) => {
                println!(
                    "NOTE: collect_and_persist_call_edges failed (callHierarchy may be unsupported): {}",
                    e
                );
                0
            }
        }
    };

    let _ = client.shutdown();
    let _ = child.wait();

    edge_count
}

/// Poll rust-analyzer via `collect_file_symbols` until at least one symbol
/// surfaces or the 30s timeout is reached. Returns the final count.
fn poll_symbol_count_via_client(client: &mut LspJsonRpcClient, main_rs_path: &Path) -> usize {
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(30);
    let poll_interval = std::time::Duration::from_millis(500);
    let mut last = 0;
    while start.elapsed() < timeout {
        if let Ok(result) = client.collect_file_symbols(main_rs_path) {
            last = result.symbol_count;
            if last >= 4 {
                return last;
            }
        }
        std::thread::sleep(poll_interval);
    }
    last
}

/// End-to-end: real tree-sitter indexer + real `rust-analyzer` LSP session,
/// then call `search_symbol`, `get_callgraph`, and `get_blastradius` through
/// the MCP tool registry against a workspace whose `lsp_symbols` /
/// `lsp_call_edges` tables were populated by the production LSP path.
///
/// This is the MCP-tool-layer counterpart to
/// `swissarmyhammer-code-context/tests/integration_test.rs::test_lsp_call_edges_known_graph`
/// and `test_lsp_symbol_lookup_end_to_end`. The lower-level tests prove the
/// LSP-to-DB persistence path; this test proves the `ToolRegistry → code_context
/// tool → execute_search_symbol / execute_get_callgraph / execute_get_blastradius`
/// stack actually returns useful JSON when given real LSP-populated tables.
///
/// The test is gated on `rust-analyzer` being installed — it skips with a
/// `println!` and exits 0 if the binary is not on `$PATH`, matching the
/// pattern used by every other real-LSP test in the codebase.
///
/// Named `qwen_embedding_*` so the default nextest filter excludes it (the
/// test also drives the production indexer to satisfy `check_ts_readiness`,
/// which loads the qwen embedder).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn qwen_embedding_lsp_layered_e2e() {
    // -- Guard: skip cleanly if rust-analyzer is not installed ---------------
    if detect_rust_analyzer().is_none() {
        println!("SKIPPED: rust-analyzer not found in PATH");
        return;
    }

    // -- Set up isolated workspace with the call-graph fixture --------------
    let _env = IsolatedTestEnvironment::new().expect("create isolated test environment");
    let tmp = tempfile::TempDir::new().expect("create workspace tempdir");
    let root = tmp.path();
    write_lsp_sources(root);

    let ws = CodeContextWorkspace::open(root).expect("open workspace");
    let shared_db = ws.shared_db().expect("leader has shared db");

    // -- Drive the production TS indexer so check_ts_readiness passes -------
    // Every LSP-layered op gates on this — without ts_indexed=1 the ops
    // short-circuit to the "Index not ready" placeholder.
    index_discovered_files_async(
        root,
        Arc::clone(&shared_db),
        swissarmyhammer_code_context::noop_reporter(),
    )
    .await;

    // -- Drive the real rust-analyzer LSP pipeline --------------------------
    let edge_count = drive_lsp_persistence(root, &shared_db);

    // After persistence the lsp_symbols table must hold at least the four
    // expected symbols. Poll briefly to absorb any residual DB-write lag.
    let lsp_symbol_count = wait_for_lsp_symbol_rows(
        &shared_db,
        LSP_MAIN_RS,
        4,
        std::time::Duration::from_secs(5),
    );
    assert!(
        lsp_symbol_count >= 4,
        "expected at least 4 LSP symbols persisted for {}, got {}",
        LSP_MAIN_RS,
        lsp_symbol_count
    );

    // -- Wire up the MCP tool registry against the same workspace ----------
    let mut registry = ToolRegistry::new();
    register_code_context_tools(&mut registry);
    let context = make_context_with_dir(root);
    let tool = registry
        .get_tool("code_context")
        .expect("code_context tool");

    // -----------------------------------------------------------------------
    // search_symbol via MCP
    // -----------------------------------------------------------------------
    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("search symbol"));
    args.insert("query".to_string(), json!("helper"));

    let result = tool
        .execute(args, &context)
        .await
        .expect("search symbol dispatch should succeed");
    let body = extract_text(&result);
    assert!(
        !body.contains("Index not ready"),
        "search symbol must not return the readiness placeholder — \
         the TS indexer above just ran (got: {body})"
    );
    let parsed: serde_json::Value = serde_json::from_str(body).unwrap_or_else(|e| {
        panic!("search symbol result must be JSON, got error {e} for body: {body}")
    });
    let matches = parsed
        .as_array()
        .unwrap_or_else(|| panic!("search symbol result must be a JSON array, body was: {body}"));
    assert!(
        !matches.is_empty(),
        "search symbol for 'helper' on LSP-indexed fixture should return at least one match; \
         body was: {body}"
    );
    let names: Vec<&str> = matches
        .iter()
        .filter_map(|m| m.get("name").and_then(|v| v.as_str()))
        .collect();
    assert!(
        names.contains(&"helper"),
        "search symbol results must include the 'helper' symbol, got names: {:?} — body was: {body}",
        names
    );

    // -----------------------------------------------------------------------
    // get_callgraph via MCP
    // -----------------------------------------------------------------------
    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("get callgraph"));
    args.insert("symbol".to_string(), json!("main"));
    args.insert("direction".to_string(), json!("outbound"));
    args.insert("max_depth".to_string(), json!(3));

    let result = tool
        .execute(args, &context)
        .await
        .expect("get callgraph dispatch should succeed");
    let body = extract_text(&result);
    assert!(
        !body.contains("Index not ready"),
        "get callgraph must not return the readiness placeholder — \
         the TS indexer above just ran (got: {body})"
    );
    let parsed: serde_json::Value = serde_json::from_str(body).unwrap_or_else(|e| {
        panic!("get callgraph result must be JSON, got error {e} for body: {body}")
    });

    let root_node = parsed
        .get("root")
        .unwrap_or_else(|| panic!("callgraph result must have a `root` field, body was: {body}"));
    let root_name = root_node
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("root.name must be a string, body was: {body}"));
    assert_eq!(
        root_name, "main",
        "callgraph root.name should be 'main', body was: {body}"
    );

    // Edge-dependent assertions: only run if rust-analyzer actually wrote
    // call edges. On versions without callHierarchy support the BFS finds
    // no neighbors and the test should still exercise the dispatch path
    // without faking a false-positive pass.
    if edge_count > 0 {
        let edges = parsed
            .get("edges")
            .and_then(|v| v.as_array())
            .unwrap_or_else(|| {
                panic!("callgraph result must have an `edges` array, body was: {body}")
            });
        assert!(
            !edges.is_empty(),
            "rust-analyzer wrote {edge_count} call edges but get_callgraph BFS from 'main' \
             returned 0 edges — body was: {body}"
        );
        // The fixture is main -> foo, main -> bar (depth 1) plus foo -> helper,
        // bar -> helper (depth 2). At max_depth=3 we expect every callee name
        // to surface somewhere in the edges list.
        let callee_names: Vec<&str> = edges
            .iter()
            .filter_map(|e| {
                e.get("callee")
                    .and_then(|c| c.get("name"))
                    .and_then(|v| v.as_str())
            })
            .collect();
        for expected in ["foo", "bar", "helper"] {
            assert!(
                callee_names.contains(&expected),
                "callgraph from 'main' should reach '{expected}' at max_depth=3, \
                 got callees: {:?} — body was: {body}",
                callee_names
            );
        }
    } else {
        println!(
            "NOTE: rust-analyzer returned 0 LSP call edges; get_callgraph edge assertions skipped"
        );
    }

    // -----------------------------------------------------------------------
    // get_blastradius via MCP
    // -----------------------------------------------------------------------
    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("get blastradius"));
    args.insert("file_path".to_string(), json!(LSP_MAIN_RS));
    args.insert("symbol".to_string(), json!("helper"));
    args.insert("max_hops".to_string(), json!(3));

    let result = tool
        .execute(args, &context)
        .await
        .expect("get blastradius dispatch should succeed");
    let body = extract_text(&result);
    assert!(
        !body.contains("Index not ready"),
        "get blastradius must not return the readiness placeholder — \
         the TS indexer above just ran (got: {body})"
    );
    let parsed: serde_json::Value = serde_json::from_str(body).unwrap_or_else(|e| {
        panic!("get blastradius result must be JSON, got error {e} for body: {body}")
    });

    let roots = parsed
        .get("roots")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| {
            panic!("blastradius result must have a `roots` array, body was: {body}")
        });
    assert!(
        !roots.is_empty(),
        "blastradius for symbol 'helper' in {LSP_MAIN_RS} should resolve at least one root, \
         got 0 — body was: {body}"
    );

    if edge_count > 0 {
        // With LSP call edges in place the blast radius from `helper` must
        // include both `foo` and `bar` (direct inbound callers) and `main`
        // (transitive inbound caller). Hop entries carry per-hop affected
        // symbols; check that at least one of the expected callers appears
        // somewhere in the hops payload.
        let body_str = body.to_string();
        let mentions_foo = body_str.contains("\"foo\"");
        let mentions_bar = body_str.contains("\"bar\"");
        let mentions_main = body_str.contains("\"main\"");
        assert!(
            mentions_foo || mentions_bar || mentions_main,
            "blastradius from 'helper' with LSP edges present should mention at least one of \
             foo/bar/main as an affected caller, body was: {body}"
        );

        let total_affected = parsed
            .get("total_affected_symbols")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert!(
            total_affected > 0,
            "blastradius from 'helper' with {edge_count} LSP edges should report a positive \
             total_affected_symbols, body was: {body}"
        );
    } else {
        println!(
            "NOTE: rust-analyzer returned 0 LSP call edges; blastradius caller assertions skipped"
        );
    }

    // Keep the workspace handle alive until the end of the test so the
    // leader lock isn't released while tool calls are using a follower
    // view of the same DB on disk.
    drop(ws);
    drop(shared_db);
    drop(tmp);
    drop(_env);
}
