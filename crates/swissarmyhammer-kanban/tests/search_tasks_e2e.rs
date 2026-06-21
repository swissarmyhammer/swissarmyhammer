//! Real-pipeline end-to-end test for the `search tasks` op — the keystone
//! coverage that proves the whole kanban-search feature works through the REAL
//! path, not a fixture.
//!
//! Unlike the unit tests in `task::search` (which assert scoping, Doc shape, and
//! map-back WITHOUT a model), this test drives the production stack exactly as a
//! user would:
//!
//! 1. A real board built through the real kanban dispatch (`parse_input` +
//!    `execute_operation`) — ordinary tasks plus one whose title carries a
//!    distinctive identifier (`reticulate_splines`).
//! 2. The real `search tasks` op invoked via `execute_operation` (so it routes
//!    through the registered operation, not a hand-built `SearchTasks::execute`).
//! 3. The REAL `Embedder` (qwen-embedding) and REAL `EmbeddingCache` sidecar.
//!    NOTHING is raw-inserted — the op embeds every task and the query itself,
//!    and the cosine signal is genuine.
//!
//! It asserts five things end-to-end: a SEMANTIC paraphrase surfaces the
//! spline task (cosine contributes); a TYPO of the identifier ranks the
//! identifier task `matches[0]` on a non-zero lexical signal (a typo embeds
//! poorly, so the lexical signal must be what surfaces it); a DSL `filter`
//! scopes the corpus before ranking; the sidecar cache is created + reused
//! (timing-independent: the embedding row count is stable across a second call);
//! and a cold-start (sidecar deleted) transparently rebuilds it with the same
//! ranking — the cross-machine rebuild guarantee proven through the op.
//!
//! ## Gating
//!
//! This drives the real qwen-embedding model, so — mirroring the reference
//! `swissarmyhammer-tools/tests/integration/semantic_search_e2e.rs` — every test
//! name starts with `qwen_embedding_` and runs under `#[serial_test::serial]`.
//! The CI Test job runs `cargo nextest run` on the self-hosted GPU runner, where
//! these execute; the CPU-forced llama-agent coverage gate
//! (`.github/workflows/coverage.yml`) only instruments the `llama-agent` crate
//! and never compiles these. Run them explicitly with:
//!
//! ```text
//! cargo test -p swissarmyhammer-kanban --test search_tasks_e2e
//! ```

use rusqlite::Connection;
use serde_json::{json, Value};
use std::path::Path;
use swissarmyhammer_kanban::{
    board::InitBoard, dispatch::execute_operation, parse::parse_input, Execute, KanbanContext,
};
use tempfile::TempDir;

/// The distinctive identifier embedded in one task's title/description. Chosen
/// to be a real-looking code identifier that is NOT an English word, so a typo
/// of it (`reticulate_splne`) cannot be recovered semantically by the embedder
/// — only the lexical (BM25 / character-trigram) signal can surface it.
const SPLINE_TITLE: &str = "reticulate_splines refactor";

/// A misspelling of the rare `reticulate_splines` identifier. A typo embeds
/// poorly, so only the lexical (BM25 / character-trigram) signal can surface the
/// identifier task — this is the query used to prove lexical recovery, cache
/// reuse, and cold-start rebuild all rank the same task first.
const SPLINE_TYPO: &str = "reticulate_splne";

/// Open a fresh board under a temp dir, returning the temp guard (kept alive for
/// the test) and the context. The board dir is `<temp>/.kanban`, so the search
/// sidecar lands at `<temp>/.kanban/search-cache.sqlite3`.
async fn open_board() -> (TempDir, KanbanContext) {
    let temp = TempDir::new().expect("create temp dir");
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::open(&kanban_dir)
        .await
        .expect("KanbanContext::open must succeed");
    InitBoard::new("Search E2E")
        .execute(&ctx)
        .await
        .into_result()
        .expect("InitBoard must succeed");
    (temp, ctx)
}

/// Dispatch one operation through the canonical parse → execute pipeline, the
/// same path the MCP tool and CLI use. Returns the op's JSON result.
async fn dispatch(ctx: &KanbanContext, payload: Value) -> Value {
    let ops = parse_input(payload).expect("parse_input must succeed");
    assert_eq!(ops.len(), 1, "expected exactly one parsed operation");
    execute_operation(ctx, &ops[0])
        .await
        .expect("execute_operation must succeed")
}

/// Add a task through the real dispatch path. Tags are derived from `#tag`
/// patterns in the description, so a `#bug`-bearing description tags the task.
async fn add_task(ctx: &KanbanContext, title: &str, description: &str) {
    dispatch(
        ctx,
        json!({
            "op": "add task",
            "title": title,
            "description": description,
        }),
    )
    .await;
}

/// Populate the board with a small, semantically-distinct corpus: several
/// ordinary tasks, two of them tagged `#bug`, plus the distinctive-identifier
/// spline task. Returns nothing — the board is the state under test.
async fn populate_corpus(ctx: &KanbanContext) {
    // The distinctive-identifier task. Its prose is genuinely about spline
    // interpolation so the SEMANTIC paraphrase query can surface it via cosine,
    // while the rare `reticulate_splines` token gives the typo query a lexical
    // anchor.
    add_task(
        ctx,
        SPLINE_TITLE,
        "Rework the reticulate_splines module that interpolates smooth spline \
         curves through control points, tidying the surface subdivision math.",
    )
    .await;

    // Two #bug-tagged tasks (the in-scope set for the filter assertion).
    add_task(
        ctx,
        "Fix login authentication failure",
        "Users cannot sign in after a password reset. This is a #bug in the auth flow.",
    )
    .await;
    add_task(
        ctx,
        "Crash when exporting empty report",
        "Exporting a report with zero rows panics the renderer — a #bug to triage.",
    )
    .await;

    // Ordinary, untagged tasks — semantically far from splines and not #bug.
    add_task(
        ctx,
        "Write onboarding documentation",
        "Draft the getting-started guide for new contributors.",
    )
    .await;
    add_task(
        ctx,
        "Upgrade the build toolchain",
        "Bump the compiler and CI image to the latest stable release.",
    )
    .await;
    add_task(
        ctx,
        "Add a dark mode theme",
        "Provide a dark color palette toggle in the settings panel.",
    )
    .await;
}

/// Run `search tasks` through the registered op via dispatch, returning the
/// `matches` array (the op returns `{ count, tasks }`; we expose `tasks` as
/// `matches` for assertion clarity).
async fn search(ctx: &KanbanContext, query: &str, filter: Option<&str>) -> Vec<Value> {
    let mut payload = json!({
        "op": "search tasks",
        "query": query,
    });
    if let Some(f) = filter {
        payload["filter"] = json!(f);
    }
    let result = dispatch(ctx, payload).await;
    result["tasks"]
        .as_array()
        .unwrap_or_else(|| panic!("search tasks must return a `tasks` array, got: {result}"))
        .clone()
}

/// The title of a result row.
fn title_of(row: &Value) -> &str {
    row["title"].as_str().unwrap_or("")
}

/// Count rows in the sidecar's `embeddings` table. Opens the SQLite file
/// directly (read path) so the assertion is independent of the op's internals.
fn embedding_row_count(cache_path: &Path) -> i64 {
    let conn = Connection::open(cache_path).expect("open sidecar");
    conn.query_row("SELECT COUNT(*) FROM embeddings", [], |r| r.get(0))
        .expect("count embeddings")
}

/// End-to-end over the real pipeline: build a real board, run real searches via
/// the registered op against the real embedder + cache, and assert semantic
/// ranking, lexical typo recovery, filter scoping, cache reuse, and cold-start
/// rebuild.
///
/// All assertions live in one test because they share one expensive resource:
/// the process-lifetime embedder loads once, the corpus embeds once, and the
/// cache-reuse / cold-start assertions build directly on the populated sidecar.
/// Splitting them would reload the model and re-embed per test for no added
/// coverage.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial]
async fn qwen_embedding_search_tasks_real_pipeline_e2e() {
    let (temp, ctx) = open_board().await;
    let cache_path = ctx.search_cache_path();
    populate_corpus(&ctx).await;

    // --- 1. SEMANTIC query: a paraphrase with NO shared identifier surfaces
    //        the spline task, proving the cosine signal contributes. ----------
    let semantic = search(&ctx, "clean up the spline interpolation code", None).await;
    assert!(
        !semantic.is_empty(),
        "semantic query must return at least one ranked task"
    );
    let semantic_titles: Vec<&str> = semantic.iter().map(title_of).collect();
    let spline_rank = semantic_titles
        .iter()
        .position(|t| *t == SPLINE_TITLE)
        .unwrap_or_else(|| {
            panic!("spline task must appear in semantic results, got: {semantic_titles:?}")
        });
    assert!(
        spline_rank < 3,
        "the spline task should rank highly (top 3) for the paraphrase \
         'clean up the spline interpolation code' — got rank {spline_rank} in {semantic_titles:?}"
    );
    // The cosine signal must be a live contributor (non-zero) on that hit.
    assert!(
        semantic[spline_rank]["signals"]["cosine"]
            .as_f64()
            .unwrap_or(0.0)
            > 0.0,
        "cosine signal must be non-zero for the spline hit: {:?}",
        semantic[spline_rank]
    );

    // --- 2. TYPO/exact-identifier query: a misspelling of the rare identifier
    //        ranks the identifier task #1 on a non-zero LEXICAL signal. --------
    let typo = search(&ctx, SPLINE_TYPO, None).await;
    assert!(
        !typo.is_empty(),
        "typo identifier query must return at least one ranked task"
    );
    assert_eq!(
        title_of(&typo[0]),
        SPLINE_TITLE,
        "the typo 'reticulate_splne' must rank the identifier task first, got: {:?}",
        typo.iter().map(title_of).collect::<Vec<_>>()
    );
    let bm25 = typo[0]["signals"]["bm25"].as_f64().unwrap_or(0.0);
    let trigram = typo[0]["signals"]["trigram"].as_f64().unwrap_or(0.0);
    assert!(
        bm25 > 0.0 || trigram > 0.0,
        "a lexical signal (bm25 or trigram) must be non-zero for the typo hit — \
         a typo embeds poorly, so the lexical signal is what surfaces it: \
         bm25={bm25}, trigram={trigram}"
    );

    // --- 3. DSL filter scoping: `#bug` restricts the corpus BEFORE ranking, so
    //        only the two #bug tasks can appear. ------------------------------
    let bug_hits = search(&ctx, "problem failure crash", Some("#bug")).await;
    assert!(
        !bug_hits.is_empty(),
        "the #bug filter must still return ranked in-scope tasks"
    );
    let bug_titles: Vec<&str> = bug_hits.iter().map(title_of).collect();
    assert_eq!(
        bug_hits.len(),
        2,
        "exactly the two #bug tasks are in scope, got: {bug_titles:?}"
    );
    assert!(
        bug_titles.contains(&"Fix login authentication failure")
            && bug_titles.contains(&"Crash when exporting empty report"),
        "the #bug results must be exactly the two tagged tasks, got: {bug_titles:?}"
    );
    assert!(
        !bug_titles.contains(&SPLINE_TITLE),
        "out-of-scope tasks (the untagged spline task) must be excluded before ranking"
    );

    // --- 4. Cache create + reuse: the op created and populated the sidecar; a
    //        repeated search reuses it (no re-embed). Timing-independent: the
    //        embedding row count is stable across the second call. ------------
    assert!(
        cache_path.exists(),
        "the op must have created the sidecar at {}",
        cache_path.display()
    );
    let rows_after_first = embedding_row_count(&cache_path);
    assert!(
        rows_after_first > 0,
        "the sidecar must hold cached vectors after the first search, got {rows_after_first}"
    );
    // A second identical search reuses every cached vector — no row churn.
    let reuse = search(&ctx, SPLINE_TYPO, None).await;
    assert_eq!(
        title_of(&reuse[0]),
        SPLINE_TITLE,
        "the reused-cache search must still rank the identifier task first"
    );
    let rows_after_second = embedding_row_count(&cache_path);
    assert_eq!(
        rows_after_first, rows_after_second,
        "a second search over unchanged tasks must reuse cached vectors — \
         the embedding row count must not change ({rows_after_first} -> {rows_after_second})"
    );

    // --- 5. Cold-start rebuild: delete the sidecar (+ WAL/SHM) — simulating a
    //        fresh clone where the gitignored cache is absent — and re-run. The
    //        op must transparently recreate the sidecar AND reproduce the
    //        ranking. ------------------------------------------------------------
    delete_sidecar(&cache_path);
    assert!(
        !cache_path.exists(),
        "sidecar must be gone before the cold-start search"
    );

    let cold = search(&ctx, SPLINE_TYPO, None).await;
    assert_eq!(
        title_of(&cold[0]),
        SPLINE_TITLE,
        "after a cold-start the typo query must rank the identifier task first again, got: {:?}",
        cold.iter().map(title_of).collect::<Vec<_>>()
    );
    assert!(
        cache_path.exists(),
        "the cold-start search must transparently recreate the sidecar at {}",
        cache_path.display()
    );
    let rows_after_cold = embedding_row_count(&cache_path);
    assert_eq!(
        rows_after_cold, rows_after_first,
        "the rebuilt sidecar must re-cache the same number of task vectors \
         ({rows_after_first} originally -> {rows_after_cold} rebuilt)"
    );

    // The cold-start ranking must match the pre-deletion ranking exactly, proving
    // the rebuild is transparent (same corpus, same order).
    let cold_titles: Vec<&str> = cold.iter().map(title_of).collect();
    let reuse_titles: Vec<&str> = reuse.iter().map(title_of).collect();
    assert_eq!(
        cold_titles, reuse_titles,
        "cold-start ranking must match the warm-cache ranking for the same query"
    );

    drop(temp);
}

/// Delete the SQLite sidecar and its WAL/SHM companions, simulating a fresh
/// clone on another machine where the gitignored cache never existed.
fn delete_sidecar(cache_path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let p = if suffix.is_empty() {
            cache_path.to_path_buf()
        } else {
            let mut name = cache_path.as_os_str().to_os_string();
            name.push(suffix);
            std::path::PathBuf::from(name)
        };
        if p.exists() {
            std::fs::remove_file(&p).unwrap_or_else(|e| {
                panic!("failed to delete sidecar companion {}: {e}", p.display())
            });
        }
    }
}
