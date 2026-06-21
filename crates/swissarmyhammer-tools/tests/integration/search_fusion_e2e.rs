//! End-to-end fusion proof — the keystone test for hybrid `search code`.
//!
//! Where [`semantic_search_e2e`](super::semantic_search_e2e) proves the cosine
//! signal is real (a semantic query ranks the right file first), this test
//! proves the *fusion* is real: a query that pure cosine ranks BELOW #1 is
//! lifted to #1 once the lexical (BM25 / character-trigram on `symbol_path`)
//! signals are mixed in.
//!
//! ## The differential proof
//!
//! Predicting the embedding model's absolute cosine ranking is impossible, so
//! we do not try to engineer files into a particular cosine order. Instead we
//! prove fusion *changed the outcome* by flipping the internal
//! [`SearchCodeOptions`] weights on the SAME indexed DB and the SAME query
//! embedding:
//!
//! * `{ w_bm25: 0.0, w_trigram: 0.0, w_cosine: 1.0 }` (cosine-only) — the
//!   query's prose semantically describes the SEMANTIC DECOY (`geometry.rs`), so
//!   in embedding space the decoy out-ranks the target and the target is NOT
//!   `matches[0]`.
//! * `{ w_bm25: 1.0, w_trigram: 1.0, w_cosine: 1.0 }` (default fusion) — the
//!   character-trigram signal fires on the distinctive `reticulate_splines`
//!   identifier in the chunk's high-weight `symbol_path` field (the typo
//!   `reticulate_splne` shares almost all of its 3-grams with it), lifting the
//!   target to `matches[0]`.
//!
//! ## Why the query defeats cosine but trigram rescues it
//!
//! The query (see [`TYPO_QUERY`]) is built from two parts that pull in opposite
//! directions:
//!
//! * **Prose** (`subdivide the surface mesh and rasterize ...`) drawn from the
//!   decoy's DOC COMMENT — semantically near "spline/mesh rendering", so it
//!   pulls the query EMBEDDING toward `geometry.rs`. Cosine-only therefore ranks
//!   the decoy above the terse target. Crucially these words appear only in the
//!   decoy's PROSE, never in its `symbol_path` (`tessellate_curve_patches`), so
//!   they give the decoy NO lexical (trigram-on-symbol_path) edge.
//! * **Typo identifier** (`reticulate_splne`, dropping the `i`) — not a real
//!   token, so the embedder cannot recover the target from it semantically, but
//!   it shares the long common prefix `reticulate_spl` with the target's
//!   `symbol_path`, giving a near-maximal character-trigram (Dice) overlap on
//!   the short, high-weight `symbol_path` field.
//!
//! So cosine ranks the decoy first; the trigram on the target's distinctive
//! identifier flips the fused ranking back to the target. The empirically
//! measured signals on the real model: cosine-only puts `geometry.rs` (~0.63)
//! above the target `render.rs` (~0.57); default fusion puts the target first
//! with bm25 ~4.7 / trigram ~2.0 on its symbol_path. This is precisely the
//! fusion contract for code search.
//!
//! ## Gating
//!
//! Like the reference e2e, this drives the REAL indexer + REAL embedder and is
//! gated under `#[serial_test::serial(cwd)]`. Its name starts with
//! `qwen_embedding_` so the nextest `default-filter` excludes it from the
//! standard suite; run it explicitly with the `embedding-models` profile and
//! `--ignore-default-filter` (see [`semantic_search_e2e`] for the exact
//! invocation).

use std::path::Path;
use std::sync::Arc;

use serde_json::json;
use swissarmyhammer_code_context::{
    search_code, CodeContextWorkspace, SearchCodeOptions, SharedDb,
};
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_config::ModelConfig;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{ToolContext, ToolRegistry};
use swissarmyhammer_tools::mcp::tools::code_context::{
    index_discovered_files_async, register_code_context_tools,
};
use tokio::sync::Mutex as TokioMutex;

/// The file carrying the distinctively-named target identifier. Its body is
/// deliberately TERSE and generic so the chunk embeds to a weak (non-distinctive)
/// point — the identifier is the only thing tying it to the query, and that tie
/// is lexical (trigram), not semantic (cosine).
const FILE_TARGET: &str = "src/render.rs";
/// The semantic decoy: a function whose prose describes the SAME concept as the
/// target ("reticulating splines for the render pass") but using SYNONYMS so it
/// shares NO distinctive trigram with the typo query, while embedding to a point
/// at least as close to the query as the terse target. This is what lets
/// cosine-only out-rank the target.
const FILE_DECOY: &str = "src/geometry.rs";
/// Ordinary decoy files with fluent, real code.
const FILE_AUTH: &str = "src/auth.rs";
const FILE_PARSER: &str = "src/parser.rs";
const FILE_MATH: &str = "src/math.rs";

/// The distinctively-named target identifier the test queries for.
const TARGET_SYMBOL: &str = "reticulate_splines";
/// The search query. It carries a TYPO of [`TARGET_SYMBOL`] (the `i` of
/// `splines` dropped: `reticulate_splne`) so the long common prefix
/// `reticulate_spl` gives it near-maximal character-trigram (Dice) overlap with
/// the target's `symbol_path` — the lexical hook that rescues the rank.
///
/// The leading prose (`subdivide the surface mesh and rasterize ...`) is drawn
/// from the SEMANTIC decoy's DOC COMMENT, not its identifier — those words pull
/// the query's EMBEDDING toward `geometry.rs` (so cosine-only ranks the decoy
/// above the target) WITHOUT giving the decoy a lexical edge: none of
/// `subdivide`/`surface`/`mesh`/`rasterize`/`geometry`/`render` appear in the
/// decoy's `symbol_path` (`tessellate_curve_patches`), so the high-weight
/// trigram field stays clean. Only the target's `symbol_path`
/// (`reticulate_splines`) trigram-matches the typo, so default fusion flips the
/// target back to #1. The typo is not a real token, so the embedder cannot
/// recover the target from it semantically — that is why cosine-only genuinely
/// misses.
const TYPO_QUERY: &str =
    "subdivide the surface mesh and rasterize the geometry for the render pass reticulate_splne";

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

/// Write the target file plus several ordinary decoy files into `root`.
///
/// The target file defines `fn reticulate_splines()`; the decoys are fluent,
/// real code (auth / JSON parsing / arithmetic) whose bodies embed to
/// well-behaved points that can sit closer to a noisy typo query than the
/// target does — which is exactly what makes cosine-only misrank the target.
fn write_corpus(root: &Path) {
    std::fs::create_dir_all(root.join("src")).expect("create src/");

    // The target: distinctive identifier, deliberately TERSE/generic body. No
    // descriptive prose, so the chunk's embedding is bland — cosine has little
    // semantic signal to latch onto and the typo query does NOT land closest to
    // it. The identifier `reticulate_splines` is the only hook, and it is a
    // LEXICAL (trigram) hook.
    std::fs::write(
        root.join(FILE_TARGET),
        r#"pub fn reticulate_splines(n: usize) -> usize {
    let mut t = 0;
    for i in 0..n {
        t += i + 1;
    }
    t
}
"#,
    )
    .expect("write render.rs");

    // The semantic decoy: rich prose describing the SAME concept as the target,
    // but with SYNONYMS (`tessellate the curve patches for the draw pass ...
    // rasterize the surface mesh`) so it shares no distinctive trigram with the
    // typo query `reticulate_splne`. Its embedding sits at least as close to the
    // query as the terse target's, so cosine-only ranks it at or above the
    // target — defeating cosine. Its symbol_path (`tessellate_curve_patches`)
    // has no trigram overlap with the query, so default fusion cannot rescue it.
    std::fs::write(
        root.join(FILE_DECOY),
        r#"//! Geometry preparation for the renderer.

/// Tessellate the curve patches for the active draw pass so the surface mesh is
/// subdivided and ready to rasterize. Walks each control polygon, refines the
/// curved segments into triangle strips, and emits the tessellated geometry for
/// the rendering pipeline to draw.
pub fn tessellate_curve_patches(control_points: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut mesh = Vec::new();
    for window in control_points.windows(2) {
        let (ax, ay) = window[0];
        let (bx, by) = window[1];
        for step in 0..=4 {
            let t = step as f32 / 4.0;
            mesh.push((ax + (bx - ax) * t, ay + (by - ay) * t));
        }
    }
    mesh
}
"#,
    )
    .expect("write geometry.rs");

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
    format!("hash::{}", plain.len())
}

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

/// Whether `matches[0]`'s `file_path` ends with the target file name.
fn top_is_target(matches: &[serde_json::Value]) -> bool {
    matches
        .first()
        .and_then(|m| m.get("file_path"))
        .and_then(|v| v.as_str())
        .map(|p| p.ends_with("render.rs"))
        .unwrap_or(false)
}

/// Rank of the target file within `matches`, or `None` if absent.
fn target_rank(matches: &[swissarmyhammer_code_context::SearchCodeMatch]) -> Option<usize> {
    matches
        .iter()
        .position(|m| m.file_path.ends_with("render.rs"))
}

/// End-to-end: a typo query that pure cosine ranks below #1 is lifted to #1 by
/// default fusion, on the SAME indexed DB and SAME query embedding.
///
/// This is the test that proves the hybrid search is doing real fusion work —
/// not just an empty-cosine fallback and not just cosine ranking dressed up.
/// The cosine signal is genuinely present (`ts_chunks.embedding IS NOT NULL`,
/// `indexed_files.embedded = 1`), yet cosine alone misranks the typo; mixing in
/// the lexical signals fixes it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(cwd)]
async fn qwen_embedding_search_fusion_rescues_typo_e2e() {
    let _env = IsolatedTestEnvironment::new().expect("create isolated test environment");

    let tmp = tempfile::TempDir::new().expect("create workspace tempdir");
    let root = tmp.path().to_path_buf();

    // 1. Write the target file plus ordinary decoys.
    write_corpus(&root);

    // 2. Open the workspace (runs startup_cleanup -> tracks files dirty).
    let ws = CodeContextWorkspace::open(&root).expect("open workspace");
    let shared_db = ws.shared_db().expect("leader has shared db");

    // 3. Drive the REAL production indexer: real chunking + real embeddings.
    index_discovered_files_async(
        &root,
        Arc::clone(&shared_db),
        swissarmyhammer_code_context::noop_reporter(),
    )
    .await;

    // 4. The cosine signal is genuinely present — embeddings were written and
    //    every file is embedded. Fusion (not an empty-cosine fallback) is what
    //    the differential below exercises.
    let embedded_count = count_embedded_chunks(&shared_db);
    assert!(
        embedded_count > 0,
        "expected the real indexer to write non-NULL embedding blobs, got {embedded_count}"
    );
    for relative in [FILE_TARGET, FILE_DECOY, FILE_AUTH, FILE_PARSER, FILE_MATH] {
        let flags = read_index_flags(&shared_db, relative);
        assert_eq!(
            flags,
            Some((1, 1)),
            "expected {relative} ts_indexed=1 embedded=1 after indexing, got {flags:?}"
        );
    }

    // 5. Embed the TYPO query ONCE with the real embedder.
    use swissarmyhammer_embedding::{Embedder, TextEmbedder};
    let embedder = Embedder::default().await.expect("create embedder");
    embedder.load().await.expect("load embedding model");
    let embed_result = embedder
        .embed_text(TYPO_QUERY)
        .await
        .expect("embed typo query");
    let query_embedding: Vec<f32> = embed_result.embedding().to_vec();

    // 6. THE DIFFERENTIAL — same DB, same query embedding, weights flipped.
    //
    //    Cosine-only: the query's prose pulls the embedding toward the semantic
    //    decoy (geometry.rs), so the target is NOT matches[0].
    let cosine_only = SearchCodeOptions {
        w_bm25: 0.0,
        w_trigram: 0.0,
        w_cosine: 1.0,
        ..Default::default()
    };
    let cosine_result = {
        let conn = shared_db.lock().unwrap_or_else(|p| p.into_inner());
        search_code(&conn, TYPO_QUERY, &query_embedding, &cosine_only).expect("cosine-only search")
    };
    let cosine_rank = target_rank(&cosine_result.matches);
    assert_ne!(
        cosine_rank,
        Some(0),
        "cosine-only must NOT rank the target ('{TARGET_SYMBOL}') first for the typo query \
         '{TYPO_QUERY}'; if it does, the differential is unprovable on this corpus. \
         Ranking was: {:?}",
        cosine_result
            .matches
            .iter()
            .map(|m| (m.file_path.as_str(), m.score))
            .collect::<Vec<_>>()
    );

    //    Default fusion: trigram on the high-weight `symbol_path` fires for the
    //    near-identical identifier, lifting the target to matches[0].
    let fusion = SearchCodeOptions {
        w_bm25: 1.0,
        w_trigram: 1.0,
        w_cosine: 1.0,
        ..Default::default()
    };
    let fusion_result = {
        let conn = shared_db.lock().unwrap_or_else(|p| p.into_inner());
        search_code(&conn, TYPO_QUERY, &query_embedding, &fusion).expect("fusion search")
    };
    assert_eq!(
        target_rank(&fusion_result.matches),
        Some(0),
        "default fusion must rank the target ('{TARGET_SYMBOL}') first for the typo query \
         '{TYPO_QUERY}'. Ranking was: {:?}",
        fusion_result
            .matches
            .iter()
            .map(|m| (m.file_path.as_str(), m.score))
            .collect::<Vec<_>>()
    );

    // 7. ONE `search code` MCP dispatch with the same query — assert the wire
    //    shape: target at matches[0], score present, lexical signal non-zero.
    let mut registry = ToolRegistry::new();
    register_code_context_tools(&mut registry);
    let context = make_context_with_dir(&root);
    let tool = registry
        .get_tool("code_context")
        .expect("code_context tool");

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("search code"));
    args.insert("query".to_string(), json!(TYPO_QUERY));
    args.insert("top_k".to_string(), json!(5));

    let result = tool
        .execute(args, &context)
        .await
        .expect("search code dispatch should succeed");
    let body = extract_text(&result);

    let parsed: serde_json::Value = serde_json::from_str(body)
        .unwrap_or_else(|e| panic!("search code result must be JSON, got error {e}: {body}"));
    let matches = parsed
        .get("matches")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("result must have a `matches` array, body was: {body}"));

    assert!(
        top_is_target(matches),
        "MCP `search code` for the typo query '{TYPO_QUERY}' must rank the target first \
         (matches: {:?})",
        matches
            .iter()
            .map(|m| (
                m.get("file_path").and_then(|v| v.as_str()).unwrap_or("?"),
                m.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0)
            ))
            .collect::<Vec<_>>()
    );

    let top = &matches[0];
    assert!(
        top.get("score").and_then(|v| v.as_f64()).is_some(),
        "matches[0] must carry a numeric `score`, got: {top}"
    );
    let signals = top
        .get("signals")
        .unwrap_or_else(|| panic!("matches[0] must carry a `signals` object, got: {top}"));
    let bm25 = signals.get("bm25").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let trigram = signals
        .get("trigram")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    assert!(
        bm25 > 0.0 || trigram > 0.0,
        "the lexical signal must have fired for the typo query (bm25={bm25}, trigram={trigram}); \
         that is what rescues the rank. signals were: {signals}"
    );
}
