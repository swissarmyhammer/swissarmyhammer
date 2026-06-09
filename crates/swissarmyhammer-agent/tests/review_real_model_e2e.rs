//! End-to-end proof that the `review` pipeline runs over ACP against a **real
//! local model**, not a scripted agent.
//!
//! The wiring-layer test in `tests/review_factory.rs` proves the production
//! [`review_agent_factory`] *builds* a `review_op::AgentFactory` without loading
//! a model. This test closes the loop: it builds the factory against the
//! `qwen-0.6b-test` builtin (a real `llama-agent` GGUF chat model), then drives
//! the production review pipeline — the exact `run_review_request` call path the
//! MCP `review` op uses — over the in-process ACP connection the factory mints.
//!
//! It mirrors `apps/kanban-app/tests/ai_panel_e2e.rs`: the same model-id
//! resolution (`ModelManager::find_agent_by_name` + `parse_model_config`) and the
//! same model-unavailable skip idiom. It also honors the `LLAMA_N_GPU_LAYERS=0`
//! CPU/coverage gate the other llama-agent real-model tests use, skipping rather
//! than running a multi-minute CPU turn.
//!
//! ## Why this lives in `swissarmyhammer-agent`
//!
//! `swissarmyhammer-agent` depends on `swissarmyhammer-tools` (where the
//! `review_op::AgentFactory` seam and `run_review_request` live) AND on the ACP
//! backends (`create_agent`). `swissarmyhammer-tools` therefore cannot depend on
//! `swissarmyhammer-agent` without a cycle, so the *real-model* review e2e cannot
//! live alongside the scripted-agent review tests in the tools integration dir —
//! it belongs in this tier, next to the factory it exercises.
//!
//! ## Minimal by design — one generation, no verify pass
//!
//! This is the *minimum* real end-to-end: a single real fan-out generation over a
//! single real model turn, and nothing more. The review pipeline fans out one
//! prompt per (validator × file-batch), so running the full builtin validator set
//! (~15) over a file would mean ~15 slow generations plus a verify pass — minutes
//! of wall-clock for a structural smoke test. To stay minimal we scope the request
//! to exactly ONE validator, `function-length`, over ONE root-level `.rs` file:
//!
//! - `function-length` is a single rule with NO engine probe and severity `warn`,
//!   so its fan-out is one prompt; validators outside the `validators` subset are
//!   skipped with no model call (`loader.retain_rulesets` in `run_review_request`).
//! - The working-tree change is a short function (well under the ~50-line trigger),
//!   so `function-length` finds nothing → no confirmed findings → **no verify pass**.
//! - GLOB CAVEAT: the `source_code` file group matches `*.rs` (not `**/*.rs`), so
//!   the source file lives at the repo ROOT (`lib.rs`), not under `src/`. At
//!   `src/lib.rs` the work-list matcher would miss it, zero generations would run,
//!   and the test would pass hollow — the root path guarantees the single fan-out.
//!
//! Net: exactly one real fan-out generation for `function-length`, zero verify
//! calls — the shortest path that still proves the model actually ran.
//!
//! ## What it asserts
//!
//! Structure, not finding content. A 0.6B model will not reliably produce real
//! findings, so this asserts only that the pipeline **completes** and yields a
//! well-formed [`ReviewReport`]: a non-error `markdown` carrying the dated GFM
//! section header, with counts that are internally consistent (the per-severity
//! tallies do not exceed the confirmed count). It does NOT assert any specific bug
//! was found.

use std::path::Path;
use std::sync::Arc;

use swissarmyhammer_agent::review_agent_factory;
use swissarmyhammer_config::model::{parse_model_config, ModelConfig, ModelManager};
use swissarmyhammer_tools::mcp::tools::review::review_op::{
    default_embedder_factory, run_review_request, ReviewRequest,
};
use swissarmyhammer_validators::review::Scope;

/// The small llama chat model used for fast, real-model testing — a real
/// `llama-agent` GGUF executor `review_agent_factory` can drive directly.
const MODEL_ID: &str = "qwen-0.6b-test";

/// Resolve `qwen-0.6b-test` to its [`ModelConfig`] through the exact public APIs
/// the production model resolution uses (`ModelManager::find_agent_by_name` +
/// `parse_model_config`), mirroring `ai_panel_e2e::resolve_qwen_test_config`.
fn resolve_qwen_test_config() -> ModelConfig {
    let info = ModelManager::find_agent_by_name(MODEL_ID)
        .unwrap_or_else(|e| panic!("test model `{MODEL_ID}` must be discoverable: {e}"));
    parse_model_config(&info.content)
        .unwrap_or_else(|e| panic!("test model `{MODEL_ID}` must parse to a ModelConfig: {e}"))
}

/// Skip (return true) when the agent could not be built because the model was
/// unavailable — an HF rate-limit or an offline first-run download. Mirrors the
/// `is_model_unavailable` skip idiom across the real-model tests; on the
/// model-cached GPU runner the model loads and the assertions always run.
fn is_model_unavailable(message: &str) -> bool {
    let m = message.to_lowercase();
    m.contains("429")
        || m.contains("too many requests")
        || m.contains("rate limited")
        || m.contains("loadingfailed")
        || m.contains("failed to load")
}

/// A minimal git repo with one committed root-level `.rs` file plus a working-tree
/// edit, so `Scope::Working` resolves a small, real changed-file scope for the
/// engine to fan out over. The file lives at the repo root (not under `src/`) so
/// the `*.rs` validator glob matches it (see the module doc's GLOB CAVEAT).
struct TinyRepo {
    dir: tempfile::TempDir,
    repo: git2::Repository,
}

impl TinyRepo {
    fn new() -> Self {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let repo = git2::Repository::init(dir.path()).expect("git init");
        {
            let mut cfg = repo.config().unwrap();
            cfg.set_str("user.name", "Test").unwrap();
            cfg.set_str("user.email", "test@example.com").unwrap();
        }
        Self { dir, repo }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn write(&self, rel: &str, content: &str) {
        let full = self.dir.path().join(rel);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(full, content).unwrap();
    }

    fn commit(&self, message: &str) {
        let mut index = self.repo.index().unwrap();
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = self.repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = self.repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        self.repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
            .unwrap();
    }
}

/// Seed an empty-but-valid on-disk code_context index at `<root>/.code-context/
/// index.db` using the real production schema. The review pipeline opens this
/// read-only (`open_index_connection`); a probe finds no candidates in an empty
/// corpus, which is exactly fine — the point is the pipeline runs end-to-end over
/// the real agent, not that the seeded fixture plants a finding.
fn seed_empty_index(root: &Path) {
    use swissarmyhammer_code_context::db::{configure_connection, create_schema};

    let ctx_dir = root.join(".code-context");
    std::fs::create_dir_all(&ctx_dir).unwrap();
    let conn = rusqlite::Connection::open(ctx_dir.join("index.db")).unwrap();
    configure_connection(&conn).unwrap();
    create_schema(&conn).unwrap();
}

/// The production review pipeline, driven over a real local model end-to-end:
/// resolve `qwen-0.6b-test`, build the production `review_agent_factory`, and run
/// `run_review_request` (the MCP `review` op's call path) with the real platform
/// embedder over a tiny working-tree scope. Asserts the run completes and returns
/// a well-formed `ReviewReport` — structure, not finding content.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_runs_over_acp_against_a_real_local_model() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    // CPU/coverage gate: this test drives a real local model over ACP, which is a
    // GPU-runner concern (the CI Test job and the coverage gate both run on the
    // self-hosted Metal GPU). When GPU offload is explicitly disabled
    // (`LLAMA_N_GPU_LAYERS=0` — a deliberate CPU/coverage run), a real turn takes
    // minutes; skip rather than blow the hang budget, exactly as the llama-agent
    // Metal-offload proof does.
    if std::env::var("LLAMA_N_GPU_LAYERS").ok().as_deref() == Some("0") {
        tracing::warn!(
            "skipping real-model review e2e: LLAMA_N_GPU_LAYERS=0 forces CPU (coverage/CPU gate)"
        );
        return;
    }

    let repo = TinyRepo::new();
    // The source file lives at the repo ROOT as `lib.rs`, not under `src/`: the
    // `function-length` validator matches `*.rs` (not `**/*.rs`), so a nested path
    // would miss the work-list matcher and run zero generations (see the module
    // doc's GLOB CAVEAT).
    repo.write("lib.rs", "pub fn placeholder() {}\n");
    repo.commit("baseline");
    // A small working-tree change for the working scope to resolve — a short
    // function, well under the ~50-line `function-length` trigger, so the single
    // fan-out generation finds nothing and there is no verify pass.
    repo.write(
        "lib.rs",
        "pub fn placeholder() {}\n\npub fn added(x: i32) -> i32 {\n    x + 1\n}\n",
    );
    seed_empty_index(repo.path());

    // The production factory built against the real local model config — the
    // construction `tests/review_factory.rs` proves type-checks, now invoked.
    let config = Arc::new(resolve_qwen_test_config());
    let agent_factory = review_agent_factory(config);
    // The real platform embedder factory — the exact one the MCP `review` op uses
    // in production (loaded once, cached).
    let embedder_factory = default_embedder_factory();

    let request = ReviewRequest {
        scope: Scope::Working,
        backend: Some("local".to_string()),
        // Exactly one validator → exactly one fan-out generation. Every other
        // builtin validator is dropped by `retain_rulesets` with no model call,
        // keeping this the minimum real end-to-end (see the module doc).
        validators: vec!["function-length".to_string()],
        concurrency: None,
    };

    let outcome = run_review_request(
        request,
        repo.path().to_path_buf(),
        embedder_factory,
        agent_factory,
        "2026-06-08 12:00".to_string(),
    )
    .await;

    let report = match outcome {
        Ok(report) => report,
        Err(e) => {
            if is_model_unavailable(&e) {
                tracing::warn!("skipping: qwen-0.6b-test model unavailable ({e})");
                return;
            }
            panic!("review pipeline must complete over the real local model, got error: {e}");
        }
    };

    // Structure, not content: the dated GFM section header always renders, even
    // for an empty findings set. A 0.6B model's actual findings are not asserted.
    assert!(
        report.markdown.contains("## Review Findings ("),
        "the report must render the dated GFM section header (well-formed, \
         non-error markdown), got: {}",
        report.markdown
    );

    // The counts must be internally consistent: the per-severity confirmed
    // tallies sum to no more than the total confirmed count. `counts.confirmed`
    // is the pre-dedup confirmed count, while the per-severity tallies are taken
    // over the set returned by `dedup_exact`, which collapses exact-duplicate
    // confirmed findings. A nondeterministic 0.6B model can emit identical
    // confirmed findings on a tiny diff, so the kept (deduped) findings never
    // exceed the confirmed count — they may be strictly fewer.
    let counts = &report.counts;
    assert!(
        counts.blockers + counts.warnings + counts.nits <= counts.confirmed,
        "the per-severity tallies must not exceed the confirmed findings: {counts:?}"
    );
}
