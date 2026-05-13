//! End-to-end semantic search test — the reference pattern for code-context op coverage.
//!
//! Every MCP tool that advertises a capability (search, lookup, analysis) needs at
//! least one test in this style: drive the real production indexer over real files,
//! then call the user-facing op and assert on the result. Fixture-only tests that
//! raw-SQL-insert pre-computed data prove math, not features.
//!
//! See card A (01KREPJATDWF2JFBMR2S6ZHZBR) for the audit that ensures other ops
//! have equivalent coverage.
//!
//! ## Pattern: real indexer → real query → real result
//!
//! 1. Create a `tempfile::TempDir` and write small Rust files with distinct
//!    semantic meaning.
//! 2. Open a real [`CodeContextWorkspace`] so `startup_cleanup` populates
//!    `indexed_files` with the new files (marked dirty).
//! 3. Call [`index_discovered_files_async`] — the production indexer used by
//!    the MCP server bootstrap and the file watcher. It loads
//!    `Embedder::default()` (qwen-embedding) and writes real embedding blobs.
//! 4. Verify `ts_chunks.embedding IS NOT NULL` for the produced chunks AND
//!    `indexed_files.embedded = 1` for each file.
//! 5. Call `execute_search_code` via the registered MCP tool path with a
//!    semantically related query and assert the matching file ranks first.
//!
//! ## Why this test guards against past bugs
//!
//! Before card 2 (real embedding writes) and card 3 (gate removed),
//! `execute_search_code` would either return an empty result set (no embeddings
//! were ever written) or the `"Index not ready"` placeholder string. Either
//! failure mode would make at least one assertion below blow up. The two old
//! fixture-only tests of `search_code` still pass on trunk because they
//! raw-SQL-insert pre-computed embeddings — exactly the gap this test closes.
//!
//! ## Embedding model dependency
//!
//! This test calls `Embedder::default()` which resolves to `qwen-embedding`
//! (~600 MB GGUF on Linux / Apple Neural Engine `.mlpackage` on macOS arm64).
//! Like `swissarmyhammer-embedding/tests/integration_test.rs::qwen_embedding_*`,
//! the test name starts with `qwen_embedding_` so the nextest `default-filter`
//! in `.config/nextest.toml` excludes it from the standard suite. Run it
//! explicitly with the `embedding-models` profile and `--ignore-default-filter`
//! so the inherited `not test(/qwen_embedding/)` filter does not skip it:
//!
//! ```text
//! cargo nextest run --profile embedding-models --ignore-default-filter \
//!     -p swissarmyhammer-tools --test tools_tests \
//!     qwen_embedding_semantic_search_e2e
//! ```

use std::path::Path;
use std::sync::Arc;

use serde_json::json;
use swissarmyhammer_code_context::{CodeContextWorkspace, SharedDb};
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_config::ModelConfig;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{ToolContext, ToolRegistry};
use swissarmyhammer_tools::mcp::tools::code_context::{
    index_discovered_files_async, register_code_context_tools,
};
use tokio::sync::Mutex as TokioMutex;

/// File names used inside the temp workspace. Each file has a distinct
/// semantic theme so an embedding-driven query can pick the right one.
const FILE_AUTH: &str = "src/auth.rs";
const FILE_PARSER: &str = "src/parser.rs";
const FILE_MATH: &str = "src/math.rs";

/// Build a [`ToolContext`] rooted at `dir` so MCP operations resolve the
/// workspace under that directory rather than CWD.
fn make_context_with_dir(dir: &Path) -> ToolContext {
    let git_ops = Arc::new(TokioMutex::new(None));
    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(ModelConfig::default());
    let mut ctx = ToolContext::new(tool_handlers, git_ops, agent_config);
    ctx.working_dir = Some(dir.to_path_buf());
    ctx
}

/// Write three small Rust source files into `root` with semantically distinct
/// content, used by every assertion below.
fn write_distinct_sources(root: &Path) {
    std::fs::create_dir_all(root.join("src")).expect("create src/");

    std::fs::write(
        root.join(FILE_AUTH),
        r#"//! Authentication helpers.

/// Verify a user's identity by checking the supplied credentials against
/// the stored password hash. Returns `true` on a successful login.
pub fn verify_user_credentials(username: &str, password: &str, expected_hash: &str) -> bool {
    let computed = hash_password(password);
    constant_time_eq(&computed, expected_hash) && !username.is_empty()
}

/// Hash a plaintext password into the canonical storage form.
pub fn hash_password(plain: &str) -> String {
    // Toy hash — real code would use argon2 or scrypt.
    format!("hash::{}", plain.len())
}

/// Constant-time string comparison so login does not leak timing information
/// about partial matches.
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}
"#,
    )
    .expect("write auth.rs");

    std::fs::write(
        root.join(FILE_PARSER),
        r#"//! JSON parsing helpers.

/// Parse a JSON object out of the given byte slice.
pub fn parse_json_object(input: &[u8]) -> Result<serde_json::Value, String> {
    serde_json::from_slice(input).map_err(|e| e.to_string())
}

/// Pretty-print a JSON value with two-space indentation.
pub fn pretty_print_json(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}
"#,
    )
    .expect("write parser.rs");

    std::fs::write(
        root.join(FILE_MATH),
        r#"//! Arithmetic utilities.

/// Sum two integers and return the result.
pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

/// Multiply two integers and return the product.
pub fn multiply(a: i64, b: i64) -> i64 {
    a * b
}

/// Compute the greatest common divisor via Euclid's algorithm.
pub fn gcd(mut a: i64, mut b: i64) -> i64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a.abs()
}
"#,
    )
    .expect("write math.rs");
}

/// Count `ts_chunks` rows with a non-NULL embedding blob.
fn count_embedded_chunks(db: &SharedDb) -> i64 {
    let conn = db.lock().unwrap_or_else(|p| p.into_inner());
    conn.query_row(
        "SELECT COUNT(*) FROM ts_chunks WHERE embedding IS NOT NULL",
        [],
        |r| r.get(0),
    )
    .expect("count embedded chunks")
}

/// Read `(ts_indexed, embedded)` flags for a file from `indexed_files`.
fn read_index_flags(db: &SharedDb, file_path: &str) -> Option<(i64, i64)> {
    let conn = db.lock().unwrap_or_else(|p| p.into_inner());
    conn.query_row(
        "SELECT ts_indexed, embedded FROM indexed_files WHERE file_path = ?",
        rusqlite::params![file_path],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .ok()
}

/// Extract the text content of the first item of a `CallToolResult`.
fn extract_text(result: &rmcp::model::CallToolResult) -> &str {
    match &result.content[0].raw {
        rmcp::model::RawContent::Text(t) => &t.text,
        _ => panic!("Expected text content"),
    }
}

/// End-to-end: index a real workspace with the production indexer, run a
/// semantic query through the MCP tool path, and assert the right file is
/// the top match.
///
/// This is the test that would have caught both bugs in this project:
///
/// * The missing-embeddings bug (every `ts_chunks` row had `embedding IS NULL`).
///   The assertion `count_embedded_chunks(&db) > 0` and the per-file
///   `embedded=1` check fail without card 2 (real embedding writes).
/// * The over-strict readiness gate (`execute_search_code` returned the
///   `"Index not ready"` placeholder on a fresh workspace). The
///   `SearchCodeResult` deserialisation fails without card 3 (gate removed).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn qwen_embedding_semantic_search_e2e() {
    // IsolatedTestEnvironment chdirs into a private temp directory and
    // restores CWD on drop. We must keep `_env` alive for the lifetime of
    // the test so the guarded HOME/CWD overrides remain in effect.
    let _env = IsolatedTestEnvironment::new().expect("create isolated test environment");

    // The workspace must be a separate directory so it is not polluted by
    // anything `IsolatedTestEnvironment` writes into the env temp dir.
    let tmp = tempfile::TempDir::new().expect("create workspace tempdir");
    let root = tmp.path().to_path_buf();

    // 1. Write the three distinct source files.
    write_distinct_sources(&root);

    // 2. Open the workspace. `CodeContextWorkspace::open` runs
    //    `startup_cleanup` automatically, which inserts the three files into
    //    `indexed_files` marked dirty (ts_indexed=0).
    let ws = CodeContextWorkspace::open(&root).expect("open workspace");
    let shared_db = ws.shared_db().expect("leader has shared db");

    // Sanity: the three files we wrote should appear in `indexed_files`,
    // each with ts_indexed=0 and embedded=0.
    for relative in [FILE_AUTH, FILE_PARSER, FILE_MATH] {
        let flags = read_index_flags(&shared_db, relative);
        assert_eq!(
            flags,
            Some((0, 0)),
            "expected {relative} to be tracked as dirty pre-index, got {flags:?}"
        );
    }

    // 3. Drive the REAL production indexer end-to-end. This loads
    //    `Embedder::default()` (qwen-embedding) and embeds every chunk.
    index_discovered_files_async(&root, Arc::clone(&shared_db)).await;

    // 4. The whole point: embeddings exist in the DB. Card 2 makes this true;
    //    on trunk before card 2 landed, this was always 0.
    let embedded_count = count_embedded_chunks(&shared_db);
    assert!(
        embedded_count > 0,
        "expected the real indexer to write at least one non-NULL embedding blob, \
         got {embedded_count} — this is the bug card 2 fixed"
    );

    // 5. Every file should now be ts_indexed=1 AND embedded=1.
    for relative in [FILE_AUTH, FILE_PARSER, FILE_MATH] {
        let flags = read_index_flags(&shared_db, relative);
        assert_eq!(
            flags,
            Some((1, 1)),
            "expected {relative} to have ts_indexed=1 embedded=1 after indexing, got {flags:?}"
        );
    }

    // 6. Run a semantic query through the MCP tool path. This loads the
    //    embedder again to embed the query text, then ranks chunks by cosine
    //    similarity. The `auth.rs` chunk that talks about verifying user
    //    identity should be the top match.
    let mut registry = ToolRegistry::new();
    register_code_context_tools(&mut registry);
    let context = make_context_with_dir(&root);
    let tool = registry
        .get_tool("code_context")
        .expect("code_context tool");

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("search code"));
    args.insert("query".to_string(), json!("verify user identity"));
    args.insert("top_k".to_string(), json!(3));
    // Lower the default min_similarity (0.7) — Qwen3 cosine scores between
    // short query strings and ~10-line chunks are typically below 0.7 even
    // when semantically aligned. We still rely on RANK ordering to prove the
    // embedding signal is real; the absolute floor is just there to keep the
    // result set non-empty.
    args.insert("min_similarity".to_string(), json!(0.0));

    let result = tool
        .execute(args, &context)
        .await
        .expect("search code dispatch should succeed");

    let body = extract_text(&result);
    assert!(
        !body.contains("Index not ready"),
        "search code must not return the readiness placeholder — \
         card 3 removed this gate (got: {body})"
    );

    // `SearchCodeResult` only derives `Serialize` (it's a one-way response
    // type). Inspecting the response shape via `serde_json::Value` matches
    // the pattern already in use by the inner unit test for this op at
    // `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs`
    // (`test_search_code_returns_result_with_progress_when_not_embedded`).
    let parsed: serde_json::Value = serde_json::from_str(body).unwrap_or_else(|e| {
        panic!(
            "search code result must be JSON-encoded SearchCodeResult, \
             got error {e} for body: {body}"
        )
    });

    let matches = parsed
        .get("matches")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| {
            panic!("result must have a `matches` array, body was: {body}");
        });

    assert!(
        !matches.is_empty(),
        "real indexer + real query should return at least one match, got 0 — \
         body was: {body}"
    );

    // The semantic claim: `auth.rs` should rank #1 for "verify user identity",
    // proving the embedding signal is real (not just substring fallback).
    let top_file_path = matches[0]
        .get("file_path")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("top match must have a string `file_path`, got: {body}"));
    assert!(
        top_file_path.ends_with("auth.rs"),
        "expected the top match for 'verify user identity' to be auth.rs, got {top_file_path} \
         (all matches: {:?})",
        matches
            .iter()
            .map(|m| (
                m.get("file_path").and_then(|v| v.as_str()).unwrap_or("?"),
                m.get("similarity").and_then(|v| v.as_f64()).unwrap_or(0.0)
            ))
            .collect::<Vec<_>>()
    );

    // Every file is embedded, so `progress` must be null per the
    // `compute_indexing_progress` contract.
    let progress = parsed
        .get("progress")
        .expect("result must have a `progress` field (may be null)");
    assert!(
        progress.is_null(),
        "progress must be null when every tracked file is embedded, got {progress:?}"
    );
}
