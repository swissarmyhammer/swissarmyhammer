//! Real-model proof that a primed, pinned validator prefix is reused as a
//! cross-session donor by sibling review turns on a **recurrent** model, with
//! ZERO rollback fallback.
//!
//! This is the integration counterpart to the pure-selector unit test
//! `find_best_prefix_match_prefers_zero_rollback_prime_under_recurrent_window`
//! (`crates/llama-agent/src/queue.rs`): that test proves the donor *scan* picks
//! the zero-rollback pinned prime over an infeasible-rollback sibling; this test
//! proves the real FFI trim (`clear_kv_cache_seq`) of that pinned prime actually
//! succeeds against a live recurrent context — i.e. the
//! `KV trim to common prefix returned false … invalidating cache` WARN
//! (`queue.rs`) never fires, and the `streaming reusing N cached tokens` INFO
//! (`queue.rs`) does fire on each sibling turn.
//!
//! Why a recurrent model: the rollback window
//! (`crate::agent::recurrent_rollback_window`) is finite (= `N_RS_SEQ`, 64) only
//! for hybrid attention+recurrent identifiers (`qwen3.5`/`qwen3.6`). On the
//! plain-attention Qwen3-0.6B test model the window is `usize::MAX` and every
//! rollback is feasible, so the recurrent constraint this card exists to prove
//! is never exercised. The MTP test model (`unsloth/Qwen3.5-0.8B-MTP-GGUF`) IS
//! recurrent, so `RequestQueue::new` derives the finite 64-token window and the
//! donor scan must reject any donor whose trim-to-LCP rollback exceeds it.
//!
//! Scenario (mirrors the selector unit test on real tokens):
//!
//! 1. Prime a session with a long validator-style header, confirm its state is
//!    saved (`session/state_status`), and pin it (`session/pin`) so it is a
//!    stable cross-session donor that cannot be evicted mid-run.
//! 2. Run >= 2 sibling turns on **fresh** sessions whose prompts share that
//!    exact header and then diverge with a tail well over 64 tokens. Each
//!    sibling's longest common prefix with the pinned prime is the shared
//!    header; its rollback against the prime is only the prime's tiny
//!    generation-prompt suffix (well within the 64-token window), so the prime
//!    is a feasible, preferred donor. The sibling's own save, by contrast, would
//!    require a > 64-token rollback for the *next* sibling — the exact donor the
//!    finite window must reject.
//! 3. Assert — from the queue worker's own logs — that each sibling's first
//!    decode logged `streaming reusing N cached tokens` (a warm cross-session
//!    restore, not a cold prefill) with N == the prime's shared-prefix length,
//!    and that ZERO `KV trim … returned false` WARN lines fired across the run.
//! 4. Assert that `skipping MTP this turn` fired ZERO times on the sibling
//!    turns (card evhk399): the prime's draft KV is snapshotted at the prompt
//!    boundary and packaged in the SAME donor entry as the target, so the draft
//!    trims to the same tiny-rollback offset as the target and MTP is retained
//!    "for free" on every sibling turn that reuses the pinned prime.
//!
//! The worker logs from another thread, so the global tracing subscriber is
//! installed with the shared in-memory `CaptureWriter`, exactly like
//! `session_fork_real_model.rs`.

use std::path::PathBuf;
use std::time::Duration;

use agent_client_protocol::schema::NewSessionRequest;
use agent_client_protocol_extras::{SessionPinRequest, SessionStateStatusRequest};
use serial_test::serial;
use tracing::warn;

use crate::integration::real_model_helpers::{
    build_real_model_server, mtp_model_config, prompt_turn,
};

/// Per-prompt hang guard, matching the sibling real-model tests.
const NO_HANG_BUDGET: Duration = Duration::from_secs(180);

/// A long validator-style header shared verbatim by the prime and every
/// sibling. Long enough that the shared cross-session prefix is substantial
/// (many tokens), so a warm reuse is clearly visible vs a cold prefill.
const VALIDATOR_HEADER: &str = "\
/no_think You are a strict code reviewer enforcing the project rules below. \
Rule 1: every public function must have a doc comment. \
Rule 2: never use unwrap() outside of tests. \
Rule 3: errors must be propagated with the ? operator, never swallowed. \
Rule 4: no println! or eprintln! in library code; use the tracing crate. \
Rule 5: prefer iterators over index loops. \
Rule 6: keep functions under fifty lines and avoid deep nesting. \
Rule 7: derive Debug on all public types. \
Rule 8: match the project formatting and naming conventions exactly. \
Review the following file and reply with exactly: ok\n\nFILE:\n";

/// The prime's own short file tail. Kept tiny on purpose: the prime's SAVED
/// prefix is `[header + this tail + the generation-prompt suffix]`, and a
/// sibling diverges from the prime at this tail. So the prime's trim-to-LCP
/// rollback is `len(this tail + gen suffix)` — which must stay well under the
/// 64-token recurrent window for the prime to be a feasible donor. A short,
/// self-contained file also keeps the prime a single terminal turn (it replies
/// `ok` without tool-calling), so its save is not extended by an agentic loop.
const PRIME_TAIL: &str = "fn ok() {}";

/// Two divergent file tails, each well over 64 tokens, so a sibling's own saved
/// state would require a > 64-token rollback to serve the next sibling — the
/// infeasible-recurrent-rollback donor the finite window must reject in favour
/// of the zero-/tiny-rollback pinned prime.
const SIBLING_TAILS: [&str; 2] = [
    "fn alpha_one() { let a = 1; let b = 2; let c = a + b; let d = c * 3; \
     let e = d - 4; let f = e / 5; let g = f + 6; let h = g * 7; let i = h - 8; \
     let j = i + 9; let k = j * 10; let l = k - 11; let m = l + 12; let n = m * 13; \
     let o = n - 14; let p = o + 15; let q = p * 16; let r = q - 17; let s = r + 18; \
     println!(\"{}\", s); }",
    "fn beta_two() { let z = 99; let y = 98; let x = z - y; let w = x * 30; \
     let v = w + 40; let u = v / 5; let t = u - 60; let s = t * 70; let r = s + 80; \
     let q = r - 90; let p = q * 11; let o = p + 22; let n = o * 33; let m = n - 44; \
     let l = m + 55; let k = l * 66; let j = k - 77; let i = j + 88; \
     eprintln!(\"{}\", i); }",
];

/// Prime + pin a validator prefix, run two sibling turns on fresh sessions that
/// share that prefix, and prove from the worker logs that each sibling reused
/// the pinned prime as a cross-session donor with zero rollback fallback.
#[tokio::test]
#[serial]
async fn sibling_turns_reuse_pinned_prefix_without_rollback_on_recurrent_model() {
    let capture = swissarmyhammer_common::test_utils::CaptureWriter::default();
    // Capture the queue's INFO lines (the reuse evidence) plus warnings
    // everywhere. The capture subscriber is GLOBAL — the narrow filter keeps
    // exactly the lines the assertions read and avoids formatting the full INFO
    // firehose through the capture mutex (same policy as the fork test).
    let installed = tracing_subscriber::fmt()
        .with_env_filter("warn,llama_agent::queue=info")
        .with_ansi(false)
        .with_writer(capture.clone())
        .try_init()
        .is_ok();
    if !installed {
        // Same policy as the fork test: under nextest (process-per-test)
        // installation always succeeds; under shared-process `cargo test`
        // another test may own the global subscriber, and without capture the
        // assertions cannot run.
        if std::env::var_os("NEXTEST").is_some() {
            panic!(
                "could not install the capturing tracing subscriber under nextest — \
                 the sibling prefix-reuse assertions cannot run, refusing to pass vacuously"
            );
        }
        warn!(
            "Skipping recurrent prefix-reuse test: a global tracing subscriber is already \
             installed (shared-process cargo test). Run under nextest to assert."
        );
        return;
    }

    // The recurrent (MTP) model is what makes the finite 64-token rollback
    // window apply; on the plain-attention model the window is unbounded and the
    // constraint under test never fires.
    let Some((server, _rx)) = build_real_model_server(mtp_model_config()).await else {
        return;
    };

    // --- 1. Prime the validator prefix on its own session. ---
    let prime = server
        .new_session(NewSessionRequest::new(PathBuf::from("/tmp")))
        .await
        .expect("new_session")
        .session_id;
    prompt_turn(
        &server,
        &prime,
        &format!("{VALIDATOR_HEADER}{PRIME_TAIL}"),
        NO_HANG_BUDGET,
    )
    .await;

    // --- 2. Confirm the prime's state is saved (never reuse blind), pin it. ---
    let status = server
        .session_state_status(SessionStateStatusRequest {
            session_id: prime.0.to_string(),
        })
        .await
        .expect("state_status");
    assert!(
        status.saved,
        "the prime turn must leave a saved KV snapshot for the donor"
    );
    let prefix_tokens = status
        .prompt_tokens
        .expect("saved state must report its prompt-token count");
    assert!(prefix_tokens > 0, "saved prefix must cover real tokens");

    let pinned = server
        .pin_session(SessionPinRequest {
            session_id: prime.0.to_string(),
            pinned: true,
        })
        .await
        .expect("pin");
    assert!(pinned.pinned, "the prime's saved state must be pinned");

    // --- 3. Run >= 2 sibling turns on FRESH sessions sharing the prefix. ---
    let mut sibling_ids = Vec::new();
    for tail in SIBLING_TAILS {
        let sibling = server
            .new_session(NewSessionRequest::new(PathBuf::from("/tmp")))
            .await
            .expect("new_session for sibling")
            .session_id;
        // The sibling's prompt is the SAME validator header plus a long,
        // divergent file tail (> 64 tokens), so its longest common prefix with
        // the pinned prime is the shared header.
        let prompt = format!("{VALIDATOR_HEADER}{tail}");
        prompt_turn(&server, &sibling, &prompt, NO_HANG_BUDGET).await;
        sibling_ids.push(sibling.0.to_string());
    }

    // --- 4. Worker-log assertions: cross-session reuse, zero trim fallback. ---
    let logs = capture.contents();

    let prime_id = prime.0.to_string();
    for sibling_id in &sibling_ids {
        // Each sibling must select the PINNED PRIME as its cross-session
        // donor — not a fresh cold prefill and not another sibling. The donor
        // line names the source and target sessions and reports the donor's
        // pinned flag, so this pins the proof to the prime specifically.
        assert!(
            logs.lines().any(|line| {
                line.contains("as prefix donor")
                    && line.contains(&format!("from session {prime_id}"))
                    && line.contains(&format!("for session {sibling_id}"))
                    && line.contains("donor_pinned=true")
            }),
            "sibling {sibling_id} must reuse the PINNED prime ({prime_id}) as its \
             cross-session prefix donor. Captured logs:\n{logs}"
        );

        // Each sibling's first decode must log a warm cross-session reuse for
        // its own session id (a cross-session restore, not a cold prefill).
        let reused_tokens = logs
            .lines()
            .find(|line| {
                line.contains("streaming reusing")
                    && line.contains(&format!("for session {sibling_id}"))
            })
            .and_then(|line| {
                line.split("streaming reusing ")
                    .nth(1)?
                    .split(' ')
                    .next()?
                    .parse::<u64>()
                    .ok()
            })
            .unwrap_or_else(|| {
                panic!(
                    "sibling {sibling_id}'s first decode must log 'streaming reusing N \
                     cached tokens' (a warm cross-session restore of the pinned prime, \
                     not a cold prefill). Captured logs:\n{logs}"
                )
            });

        // The reuse N is the shared validator header — the longest common
        // prefix of the prime's saved prompt (`header + PRIME_TAIL + gen
        // suffix`) and the sibling's prompt (`header + long tail + gen
        // suffix`), which diverge right after the header. So N is the prime's
        // full saved length minus its short tail+suffix. Assert the reuse
        // covers the header bulk (all but a small, bounded tail), so a
        // degraded/cold restore (reuse near 0, or the prime rejected as an
        // infeasible-rollback donor) fails loudly while the legitimate small
        // divergence passes. The prime's tail is `PRIME_TAIL` (~a handful of
        // tokens) plus the generation-prompt suffix; bound it generously.
        const MAX_PRIME_TAIL_TOKENS: u64 = 32;
        let floor = prefix_tokens.saturating_sub(MAX_PRIME_TAIL_TOKENS);
        assert!(
            reused_tokens >= floor,
            "sibling {sibling_id} reused only {reused_tokens} cached tokens — it must \
             reuse the pinned prime's shared validator prefix (>= {floor} of the prime's \
             {prefix_tokens} saved tokens, allowing for the prime's short divergent tail). \
             A reuse far below this is a degraded/cold restore, or the prime was rejected \
             as an infeasible-rollback donor. Captured logs:\n{logs}"
        );

        // Positive MTP guard (card evhk399): the `skipping MTP this turn` WARN
        // can only fire when the draft path actually runs, which is gated on
        // `use_mtp = detect_nextn_predict_layers(model) > 0`. Without this
        // guard, a regression that silently disables MTP (lost NextN head,
        // failed draft-context build) would make the skip line impossible and
        // the zero-skip assertion below pass VACUOUSLY while the target-side
        // reuse assertions above (independent of MTP) still hold. Pin that the
        // sibling actually restored the pinned prime's draft KV — the
        // `apply_draft_kv_state` success INFO names this sibling's session id —
        // so the zero-skip claim means "MTP ran and was retained," not "MTP
        // never ran." This line is captured by the `llama_agent::queue=info`
        // filter.
        assert!(
            logs.lines().any(|line| {
                line.contains("MTP: restored")
                    && line.contains("bytes of draft KV state")
                    && line.contains(&format!("for session {sibling_id}"))
            }),
            "sibling {sibling_id} must run MTP and restore the pinned prime's draft KV state \
             (a 'MTP: restored N bytes of draft KV state for session {sibling_id}' INFO). \
             Without it the zero-skip assertion below would pass vacuously — MTP may not have \
             run at all. Captured logs:\n{logs}"
        );
    }

    // The core acceptance criterion: the recurrent rollback fallback must NEVER
    // fire. A pinned, tiny-rollback prime donor's trim must succeed on the live
    // recurrent context.
    assert!(
        !logs.contains("KV trim to common prefix returned false"),
        "a sibling reusing the pinned prime as a tiny-rollback donor must never hit the \
         recurrent-model rollback fallback (the trim must succeed). Captured logs:\n{logs}"
    );

    // MTP draft-KV reuse on the prime donor (card evhk399). The prime turn ran
    // MTP and saved its draft KV at the PROMPT boundary, packaged in the SAME
    // cache entry as the target bytes (`save_prompt_boundary_state` snapshots
    // `draft_ctx.state_seq_get_data(0)` via the `on_prefill_complete` hook in
    // `generation/mtp/streaming.rs`). When a sibling selects that pinned prime as
    // its donor, the draft bytes travel WITH the target in one `PrefixMatch`
    // (`find_best_prefix_match`), and `apply_draft_kv_state` trims the draft to
    // the SAME offset (= target LCP) the target was trimmed to. Because the
    // prime's draft snapshot ends at the prompt boundary, the draft's rollback
    // distance == `donor_len - lcp` == the target's rollback (the prime's short
    // divergent tail, well under the 64-token recurrent window). So if the
    // target trim succeeds — and the prior assertion proves it does — the draft
    // trim to the identical offset succeeds too, and MTP is retained "for free":
    // `skipping MTP this turn` must NOT fire on any sibling turn.
    //
    // Printed to stdout (not `tracing`, which the capture subscriber swallows
    // into `logs`) so the count is visible under `--nocapture`.
    let skipped_mtp_lines: Vec<&str> = logs
        .lines()
        .filter(|line| line.contains("skipping MTP this turn"))
        .collect();
    println!(
        "kv_prefix_reuse_recurrent observation: prefix_tokens={prefix_tokens}, siblings={}, \
         'skipping MTP this turn' fired {} time(s) on this run.",
        sibling_ids.len(),
        skipped_mtp_lines.len()
    );
    for line in &skipped_mtp_lines {
        println!("  MTP-skip: {line}");
    }
    assert!(
        skipped_mtp_lines.is_empty(),
        "MTP must be retained on every sibling turn that reuses the pinned prime: the prime's \
         draft KV was snapshotted at the prompt boundary and rides with the target in the same \
         donor, so the draft trims to the same tiny-rollback offset as the target and \
         'skipping MTP this turn' must never fire. It fired {} time(s):\n{}\nCaptured logs:\n{logs}",
        skipped_mtp_lines.len(),
        skipped_mtp_lines.join("\n")
    );
}
