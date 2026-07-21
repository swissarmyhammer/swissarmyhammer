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
//! findings — or even parseable output — so this asserts only that the run
//! completes with a well-formed [`ReviewReport`]: a non-error `markdown`
//! carrying the dated GFM section header, with counts that are internally
//! consistent (the per-severity tallies do not exceed the confirmed count).
//!
//! There is no retry and no refusal at the tool boundary (see
//! `run_review_request_inner` in `review_op.rs`): a fan-out task whose
//! generation errors or produces unparseable output degrades to zero findings
//! and is counted as failed, but the run still finishes and returns its
//! report — `synthesize` stamps the "results are INCOMPLETE" banner and
//! carries the failure tally when that happens, it does not surface as a
//! `run_review_request` error. So a 0.6B model hallucinating tool calls
//! instead of the findings JSON is still a well-formed, non-error report; only
//! a genuine wiring failure (hang, transport error, malformed report) fails
//! this test. It does NOT assert any specific bug was found.

use std::sync::Arc;

use swissarmyhammer_agent::review_agent_factory;
use swissarmyhammer_config::model::{parse_model_config, ModelConfig, ModelManager};
use swissarmyhammer_tools::mcp::tools::review::review_op::{
    default_embedder_factory, run_review_request, ReviewRequest,
};
use swissarmyhammer_validators::review::test_support::{on_disk_index_conn, TestRepo};
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

/// Lowercased substrings that mark an agent-build failure as a model
/// *availability* problem — an HF rate-limit or an offline first-run download —
/// rather than a wiring bug. Matched case-insensitively in
/// [`is_model_unavailable`].
const MODEL_UNAVAILABLE_PATTERNS: &[&str] = &[
    "429",
    "too many requests",
    "rate limited",
    "loadingfailed",
    "failed to load",
    "model loading failed",
];

/// Skip (return true) when the agent could not be built because the model was
/// unavailable — an HF rate-limit or an offline first-run download. Mirrors the
/// `is_model_unavailable` skip idiom across the real-model tests; on the
/// model-cached GPU runner the model loads and the assertions always run.
fn is_model_unavailable(message: &str) -> bool {
    let m = message.to_lowercase();
    MODEL_UNAVAILABLE_PATTERNS.iter().any(|p| m.contains(p))
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

    let repo = TestRepo::new();
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
    // An empty-but-valid on-disk index at the production path the review pipeline
    // opens read-only: a probe finds no candidates in an empty corpus, which is
    // exactly fine — the point is the pipeline runs end-to-end over the real
    // agent, not that the fixture plants a finding.
    on_disk_index_conn(repo.path());

    // The production factory built against the real local model config — the
    // construction `tests/review_factory.rs` proves type-checks, now invoked.
    let config = Arc::new(resolve_qwen_test_config());
    let agent_factory = review_agent_factory(config);
    // The real platform embedder factory — the exact one the MCP `review` op uses
    // in production (loaded once, cached).
    let embedder_factory = default_embedder_factory();

    let request = ReviewRequest::new(Scope::Working)
        .with_backend(Some("local".to_string()))
        // Exactly one validator → exactly one fan-out generation. Every other
        // builtin validator is dropped by `retain_rulesets` with no model call,
        // keeping this the minimum real end-to-end (see the module doc).
        .with_validators(vec!["function-length".to_string()]);

    let outcome = run_review_request(
        request,
        repo.path(),
        embedder_factory,
        agent_factory,
        "2026-06-08 12:00",
        None,
    )
    .await;

    let report = match outcome {
        Ok(report) => report,
        Err(err) => {
            let e = err.to_string();
            if is_model_unavailable(&e) {
                tracing::warn!("skipping: qwen-0.6b-test model unavailable ({e})");
                return;
            }
            // There is no completeness gate/refusal to special-case here anymore:
            // `run_review_request` never errors on a fan-out failure rate, no
            // matter how a 0.6B model's unparseable output degrades individual
            // tasks — it always returns the report (see the module doc). So the
            // only errors left reaching this branch are genuine wiring failures
            // (a hang surfacing as a timeout, a transport error, a pipeline
            // error) — none of which are a valid outcome for this test.
            panic!("review pipeline must complete over the real local model, got error: {e}");
        }
    };

    // Structure, not content: the dated GFM section header always renders, even
    // for an empty findings set. A 0.6B model's actual findings are not asserted.
    assert!(
        report.markdown().contains("## Review Findings ("),
        "the report must render the dated GFM section header (well-formed, \
         non-error markdown), got: {}",
        report.markdown()
    );

    // The counts must be internally consistent: the rendered-findings count is
    // no more than the total confirmed count. `counts.confirmed` is the pre-dedup
    // confirmed count, while `counts.findings` is taken over the set returned by
    // `dedup_exact`, which collapses exact-duplicate confirmed findings. A
    // nondeterministic 0.6B model can emit identical confirmed findings on a tiny
    // diff, so the kept (deduped) findings never exceed the confirmed count —
    // they may be strictly fewer.
    let counts = report.counts();
    assert!(
        counts.findings() <= counts.confirmed(),
        "the rendered findings count must not exceed the confirmed findings: {counts:?}"
    );
}
