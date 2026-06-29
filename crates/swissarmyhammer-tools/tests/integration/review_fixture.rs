//! Shared fixtures for the local multi-agent review integration tests.
//!
//! This module holds the reusable plumbing that drives the **registered
//! production `review` tool** without a live model: a **real temp git repo**
//! with a planted diff, a **real on-disk code_context index** (production
//! schema), a **scripted/playback ACP agent**, and a **mock embedder**. It is
//! shared by every review integration binary so the agent + index + diff
//! plumbing is written once:
//!
//! - [`review_e2e`](super::review_e2e) — the behavioral acceptance tests
//!   (scope → fan-out → guard → verify → synthesize through the tool).
//! - the global-subscriber observability test — proves the engine's `tracing`
//!   lines reach a process-global subscriber installed the way `sah serve`
//!   installs it (`set_global_default`), not just a thread-local scoped one.
//!
//! Because this is a `#[path]`-included support module compiled into more than
//! one test binary, some items are unused in any single binary; `dead_code` is
//! allowed here rather than per-item.
#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;

use agent_client_protocol::DynConnectTo;
use serde_json::json;
// The ONE shared review test seam, consumed via the validators crate's
// `test-support` feature instead of per-file copies: the scripted ACP agent
// harness, the throwaway git repo, the on-disk index builder + row seeders, and
// the shared embedding dimension. Re-exported (`pub use`) so the sibling review
// test binaries that `#[path]`-include this module keep importing `TestRepo` /
// `DIM` from `review_fixture` exactly as before.
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{ToolContext, ToolRegistry};
use swissarmyhammer_tools::mcp::tools::review::review_op::{
    AgentFactory, AgentHandle, EmbedderFactory,
};
use swissarmyhammer_tools::mcp::tools::review::ReviewTool;
pub use swissarmyhammer_validators::review::test_support::{
    on_disk_index_conn, seed_call_edge, seed_chunk, seed_symbol, ScriptedAdapter, ScriptedAgent,
    ScriptedReply, TestRepo, DIM,
};
use tokio::sync::broadcast;

// ---------------------------------------------------------------------------
// The planted source.
//
// Every file is `.rs`, so it matches all the builtin source-code validators
// (`duplication`, `reuse`, `data-driven`, `dead-code`, `no-secrets`, `rust`). The
// defects are spread across four files (≤ the default fan-out batch size of 4) so
// each validator's files fit one batch — one fan-out task per validator, so the
// scripted agent fires each validator response exactly once.
// ---------------------------------------------------------------------------

pub const FILE_PAYMENTS: &str = "src/payments.rs";
pub const FILE_REUSE: &str = "src/util_reuse.rs";
pub const FILE_ORPHAN: &str = "src/orphan.rs";
pub const FILE_LIVE: &str = "src/live.rs";

/// An existing indexed file whose function the duplicate (item 1) copies. It is
/// only in the index, never in the diff.
pub const FILE_EXISTING: &str = "src/existing_total.rs";
/// An existing indexed util the reuse helper (item 2) reimplements.
pub const FILE_UTIL: &str = "src/shared_util.rs";

/// The function body item 1 copy-pastes verbatim from `existing_total`. Long
/// enough to clear the index `min_chunk_bytes` (100).
fn duplicated_total_body() -> String {
    "pub fn sum_amounts(input: &[f64]) -> f64 {\n    let mut total = 0.0;\n    for value in input {\n        total += value * value;\n    }\n    total / input.len() as f64\n}".to_string()
}

/// The reuse helper body item 2 reimplements (semantically the shared util).
fn reimplemented_util_body() -> String {
    "pub fn my_mean_squared(values: &[f64]) -> f64 {\n    let mut acc = 0.0;\n    for v in values {\n        acc += v * v;\n    }\n    acc / values.len() as f64\n}".to_string()
}

/// The dead orphan body (item 4) — a new function nothing calls.
fn dead_orphan_body() -> String {
    "pub fn orphan_never_called(input: &[f64]) -> f64 {\n    let mut s = 0.0;\n    for x in input {\n        s += x;\n    }\n    s\n}".to_string()
}

/// The "claimed dead but actually called" body (item 7).
fn live_called_body() -> String {
    "pub fn claimed_dead_but_called(input: &[f64]) -> f64 {\n    let mut s = 1.0;\n    for x in input {\n        s *= x;\n    }\n    s\n}".to_string()
}

/// The whole `src/payments.rs` after-content: hosts the duplicate (item 1), the
/// hardcoded if-chain (item 3), the planted secret (item 5), and the
/// correct-but-looks-buggy code (item 6).
fn payments_after() -> String {
    format!(
        "//! Payment helpers.\nuse std::fmt;\n\n\
         // item 5: a planted secret committed to code.\n\
         const STRIPE_KEY: &str = \"sk_live_PLACEHOLDER_not_a_real_key_for_tests\";\n\n\
         {dup}\n\n\
         // item 3: a hardcoded if-chain over a known set that should be a table.\n\
         pub fn fee_for_tier(tier: &str) -> f64 {{\n\
         \x20   if tier == \"bronze\" {{\n        0.03\n    }} else if tier == \"silver\" {{\n        0.02\n    }} else if tier == \"gold\" {{\n        0.01\n    }} else {{\n        0.05\n    }}\n}}\n\n\
         // item 6: looks like an off-by-one but is correct (inclusive range is intended).\n\
         pub fn last_index(len: usize) -> usize {{\n    len.saturating_sub(1)\n}}\n",
        dup = duplicated_total_body(),
    )
}

/// Write the planted working-tree diff into `repo`: a committed baseline, then the
/// four changed `.rs` files added on top.
pub fn plant_diff(repo: &TestRepo) {
    repo.write("src/lib.rs", "pub fn placeholder() {}\n");
    repo.commit("baseline");

    repo.write(FILE_PAYMENTS, &payments_after());
    repo.write(
        FILE_REUSE,
        &format!(
            "//! A util that reinvents the shared one.\n\n{}\n",
            reimplemented_util_body()
        ),
    );
    repo.write(
        FILE_ORPHAN,
        &format!("//! An orphan module.\n\n{}\n", dead_orphan_body()),
    );
    repo.write(
        FILE_LIVE,
        &format!(
            "//! A function the guard will find a caller for.\n\n{}\n",
            live_called_body()
        ),
    );
}

// ---------------------------------------------------------------------------
// On-disk code_context index (production schema, deterministic rows).
//
// Built at `<repo>/.code-context/index.db` — exactly the path the production
// review tool opens read-only — using the real `create_schema`, then seeded so
// the engine-run probes hit deterministically:
//   - `duplicates` finds `existing_total.rs` for the payments copy (item 1),
//   - `similar` finds `shared_util.rs` for the reuse helper (item 2),
//   - `callers` finds an inbound caller for `claimed_dead_but_called` (item 7),
//     and finds NONE for `orphan_never_called` (item 4).
// ---------------------------------------------------------------------------

/// The constant vector the [`model_embedding::mock::MockEmbedder`] returns for
/// every text. Chunks seeded with this vector are maximally similar to any query.
fn mock_vec() -> Vec<f32> {
    vec![0.1_f32; DIM]
}

/// Seed the on-disk index used by the production review tool path. The index
/// builder and the per-table row seeders are the shared review test seam; this
/// fixture only supplies the planted scenario rows.
pub fn seed_on_disk_index(root: &Path) {
    let conn = on_disk_index_conn(root);

    // item 1: the duplicate's chunk in the changed file + the same block in an
    // existing indexed file → `find_duplicates` on payments.rs hits existing.
    let total = duplicated_total_body();
    let dup_emb = vec![1.0_f32, 0.0, 0.0, 0.0];
    seed_chunk(&conn, FILE_PAYMENTS, "sum_amounts", &total, &dup_emb);
    seed_chunk(&conn, FILE_EXISTING, "compute_total", &total, &dup_emb);

    // item 2: an existing shared util with the mock embedder's query vector, so
    // `search code` ranks it as a reuse candidate for the reimplemented helper.
    seed_chunk(
        &conn,
        FILE_UTIL,
        "mean_squared_error",
        &reimplemented_util_body(),
        &mock_vec(),
    );

    // item 7: `claimed_dead_but_called` HAS an inbound caller → `callers` fact has
    // rows → the guard refutes the "dead" claim deterministically.
    seed_symbol(&conn, "callee-live", "claimed_dead_but_called", FILE_LIVE);
    seed_symbol(&conn, "caller-live", "invoke_claimed", "src/caller.rs");
    seed_call_edge(
        &conn,
        "caller-live",
        "callee-live",
        "src/caller.rs",
        FILE_LIVE,
    );

    // item 4: `orphan_never_called` is intentionally absent from lsp_symbols /
    // lsp_call_edges → `callers` returns no rows → a real dead-code signal.
}

// ---------------------------------------------------------------------------
// The scripted ACP agent.
//
// Maps each prompt onto a response by matching ALL of a set of substrings, so a
// response can require both a validator header and a specific file (fan-out) or a
// specific claim (verify). The guard-vs-agent distinction (items 6 vs 7) is then
// asserted from the report's confirmed/refuted counts and the absence of each
// refuted claim — both observable through the production tool's response.
//
// It is built on the ONE shared [`ScriptedAgent`] harness, driven through its
// multi-needle [`ScriptedAgent::with_script`] form; this fixture only supplies
// the script (the planted-defect (validator, file, claim) → response map).
// ---------------------------------------------------------------------------

/// One scripted entry in the shared harness's multi-needle form: every needle
/// must be present for `response` to fire.
type Rule = (Vec<String>, ScriptedReply);

/// A fan-out rule: fire `findings` when the prompt is the fan-out task for
/// `validator` AND mentions `file` (so batching can never double-fire it).
fn fanout(validator: &str, file: &str, findings: &str) -> Rule {
    (
        vec![
            format!("# Validator: {validator}"),
            format!("## File: {file}"),
        ],
        ScriptedReply::Text(findings.to_string()),
    )
}

/// A verify rule: fire `verdict` when the adversarial prompt carries `claim`.
fn verify(claim: &str, verdict: &str) -> Rule {
    (
        vec!["# Adversarial verification".to_string(), claim.to_string()],
        ScriptedReply::Text(verdict.to_string()),
    )
}

/// A findings JSON array (fenced) with one finding. `validator` is overwritten by
/// the engine, but must be present to deserialize.
fn finding(file: &str, line: u32, severity: &str, claim: &str) -> String {
    format!(
        "```json\n[{{\"file\":\"{file}\",\"line\":{line},\"validator\":\"agent-tagged\",\
         \"rule\":\"r\",\"severity\":\"{severity}\",\"claim\":\"{claim}\",\
         \"evidence\":\"per probe evidence\",\"suggestion\":\"fix it\"}}]\n```"
    )
}

/// Two findings in one array (for a validator that flags two files in one batch).
fn two_findings(a: (&str, u32, &str, &str), b: (&str, u32, &str, &str)) -> String {
    let obj = |file: &str, line: u32, severity: &str, claim: &str| {
        format!(
            "{{\"file\":\"{file}\",\"line\":{line},\"validator\":\"agent-tagged\",\
             \"rule\":\"r\",\"severity\":\"{severity}\",\"claim\":\"{claim}\",\
             \"evidence\":\"per probe evidence\",\"suggestion\":\"fix it\"}}"
        )
    };
    format!(
        "```json\n[{},{}]\n```",
        obj(a.0, a.1, a.2, a.3),
        obj(b.0, b.1, b.2, b.3),
    )
}

/// A confirming verify verdict.
fn confirm() -> String {
    "```json\n{\"confirmed\": true, \"reason\": \"substantiated by the evidence\"}\n```".to_string()
}

/// A refuting verify verdict (the agent disproves the red herring).
fn refute() -> String {
    "```json\n{\"confirmed\": false, \"reason\": \"the code is correct; the claim is disproven\"}\n```".to_string()
}

// ---- unique claim strings (keys shared by fan-out emission and verify) -------

pub const CLAIM_DUP: &str = "copy-pasted sum_amounts duplicates compute_total";
pub const CLAIM_REUSE: &str = "my_mean_squared reimplements the shared mean_squared_error util";
pub const CLAIM_DATA: &str = "fee_for_tier hardcodes a tier if-chain that should be a table";
pub const CLAIM_DEAD_ORPHAN: &str = "orphan_never_called has no inbound callers and is dead";
pub const CLAIM_SECRET: &str = "STRIPE_KEY is a hardcoded live secret";
pub const CLAIM_RED_HERRING: &str = "last_index looks like an off-by-one bug";
pub const CLAIM_GUARD_HERRING: &str = "claimed_dead_but_called appears to be dead code";
pub const CLAIM_RUST_IDIOM: &str =
    "fee_for_tier returns a bare f64 where a typed Money would be safer";

/// The full scripted agent for the planted diff: each validator's fan-out emits
/// its planted finding(s); each confirmable claim confirms on verify; the two red
/// herrings are refuted (item 6 by the agent here, item 7 by the guard — no verify
/// rule, the guard intercepts it first).
pub fn planted_agent() -> Arc<ScriptedAgent> {
    use swissarmyhammer_validators::review::test_support::ScriptedAgentConfig;
    ScriptedAgent::with_script(
        vec![
            // ---- fan-out: one rule per (validator, file) -------------------------
            fanout(
                "duplication",
                FILE_PAYMENTS,
                &finding(FILE_PAYMENTS, 8, "blocker", CLAIM_DUP),
            ),
            fanout(
                "reuse",
                FILE_REUSE,
                &finding(FILE_REUSE, 3, "warning", CLAIM_REUSE),
            ),
            fanout(
                "data-driven",
                FILE_PAYMENTS,
                &finding(FILE_PAYMENTS, 16, "warning", CLAIM_DATA),
            ),
            fanout(
                "no-secrets",
                FILE_PAYMENTS,
                &finding(FILE_PAYMENTS, 5, "blocker", CLAIM_SECRET),
            ),
            // dead-code flags BOTH the real orphan (item 4) and the red-herring it
            // wrongly believes is dead (item 7). One fan-out task, two findings.
            fanout(
                "dead-code",
                FILE_ORPHAN,
                &two_findings(
                    (FILE_ORPHAN, 3, "blocker", CLAIM_DEAD_ORPHAN),
                    (FILE_LIVE, 3, "blocker", CLAIM_GUARD_HERRING),
                ),
            ),
            // rust flags a real idiom (item 8) and a correct-but-looks-buggy red
            // herring (item 6) that the verifier will refute.
            fanout(
                "rust",
                FILE_PAYMENTS,
                &two_findings(
                    (FILE_PAYMENTS, 16, "warning", CLAIM_RUST_IDIOM),
                    (FILE_PAYMENTS, 22, "warning", CLAIM_RED_HERRING),
                ),
            ),
            // ---- verify: confirm the real findings, refute the agent red herring --
            verify(CLAIM_DUP, &confirm()),
            verify(CLAIM_REUSE, &confirm()),
            verify(CLAIM_DATA, &confirm()),
            verify(CLAIM_SECRET, &confirm()),
            verify(CLAIM_DEAD_ORPHAN, &confirm()),
            verify(CLAIM_RUST_IDIOM, &confirm()),
            // item 6: looks buggy, is correct → the adversarial verifier refutes it.
            verify(CLAIM_RED_HERRING, &refute()),
            // item 7 deliberately has NO verify rule: the guard refutes it first, so a
            // verify prompt for it must never be generated.
        ],
        ScriptedAgentConfig {
            // No rule matched: an empty findings array for fan-out, which also
            // parses as "no verdict" for verify → refute by default.
            default_response: "```json\n[]\n```".to_string(),
            ..Default::default()
        },
    )
}

/// Adapt a scripted agent into an [`AgentFactory`], opening a fresh
/// `broadcast(256)` notification channel per connection so each minted agent is
/// shaped like a real `AcpAgentHandle`: it streams onto a backend broadcast (the
/// handle's `notification_rx`) AND bridges the same notification onto the live
/// connection. Both come for free from the shared harness's per-connection
/// broadcast rebind.
pub fn scripted_factory(agent: Arc<ScriptedAgent>) -> AgentFactory {
    Arc::new(move || {
        let agent = Arc::clone(&agent);
        Box::pin(async move {
            let (notify_tx, notification_rx) = broadcast::channel(256);
            let agent = ScriptedAgent::rebind_broadcast(&agent, notify_tx, true);
            let dyn_agent = DynConnectTo::new(ScriptedAdapter::new(agent));
            Ok(AgentHandle {
                agent: dyn_agent,
                notification_rx,
            })
        })
    })
}

/// An [`EmbedderFactory`] yielding the deterministic mock embedder (no model load).
pub fn mock_embedder_factory() -> EmbedderFactory {
    Arc::new(|| {
        Box::pin(async {
            Ok(Arc::new(model_embedding::mock::MockEmbedder::new(DIM))
                as Arc<dyn model_embedding::TextEmbedder>)
        })
    })
}

// The throwaway git repo fixture (`TestRepo`) is the SHARED review test seam from
// `swissarmyhammer_validators::review::test_support`, re-exported above rather
// than re-declared here.

// ---------------------------------------------------------------------------
// harness: register the tool, run an op, parse the report.
// ---------------------------------------------------------------------------

/// Build a [`ToolContext`] rooted at `dir`.
pub async fn context_at(dir: &Path) -> ToolContext {
    let git_ops = Arc::new(tokio::sync::Mutex::new(None));
    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(swissarmyhammer_config::ModelConfig::default());
    let mut ctx = ToolContext::new(tool_handlers, git_ops, agent_config);
    ctx.working_dir = Some(dir.to_path_buf());
    ctx
}

/// Extract the JSON text body of a tool result.
pub fn extract_text(result: &rmcp::model::CallToolResult) -> String {
    match &result.content[0].raw {
        rmcp::model::RawContent::Text(t) => t.text.clone(),
        _ => panic!("expected text content"),
    }
}

/// Run one review op through the registered production tool and return the parsed
/// `{markdown, counts}` response.
pub async fn run_review_op(
    repo: &TestRepo,
    args: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut registry = ToolRegistry::new();
    registry.register(
        ReviewTool::new()
            .with_agent_factory(scripted_factory(planted_agent()))
            .with_embedder_factory(mock_embedder_factory()),
    );
    let tool = registry.get_tool("review").expect("review tool registered");
    let context = context_at(repo.path()).await;
    let result = tool
        .execute(args, &context)
        .await
        .expect("review op dispatch");
    serde_json::from_str(&extract_text(&result)).expect("review response is JSON")
}

/// Args for `review working`, forcing the local single-worker backend so the run
/// is deterministic.
pub fn working_args() -> serde_json::Map<String, serde_json::Value> {
    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("review working"));
    args.insert("backend".to_string(), json!("local"));
    args
}

/// Whether the rendered markdown contains a confirmed finding whose claim matches.
pub fn report_has_claim(markdown: &str, claim_fragment: &str) -> bool {
    markdown.contains(claim_fragment)
}
