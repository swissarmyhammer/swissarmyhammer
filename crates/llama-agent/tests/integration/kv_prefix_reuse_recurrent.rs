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

use agent_client_protocol::schema::{NewSessionRequest, SessionId};
use agent_client_protocol_extras::{
    SessionForkRequest, SessionPinRequest, SessionStateStatusRequest,
};
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
#[ignore = "Heavy recurrent-MTP real-model proof. Loads the Qwen3.5-0.8B-MTP \
            model and runs multi-thousand-token prefill turns; on the shared \
            CI runner this hangs past the per-turn budget (recurrent KV-slot \
            contention under load — see the NoKvCacheSlot history), though it \
            passes locally (~462s). Run on demand with `--include-ignored`. \
            The selection/reuse logic is covered in CI by the model-free unit \
            tests in queue.rs; this is the on-demand real-FFI proof, also \
            validated live in calcutron-qwen."]
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

/// Read the prime session's draft byte count out of its prompt-boundary save
/// line (`cached T bytes of target + D bytes of draft state ... for session
/// <id>`), so a test can branch on whether the prime turn ACTUALLY ran MTP.
///
/// A prime that ran MTP saves `D > 0` draft bytes; a prime that did not run MTP
/// (no NextN head detected, or the draft context failed to build) saves `D ==
/// 0` and there is no draft for a fork to restore — so its first fork
/// legitimately cold-starts the draft and `skipping MTP this turn` fires once,
/// which is correct behaviour, not a regression. The zero-skip assertion is
/// therefore gated on this signal: `None` (no save line found at all) or
/// `Some(0)` means "the prime ran no MTP" and the strict zero-skip claim does
/// not apply.
fn prime_draft_bytes(logs: &str, prime_id: &str) -> Option<u64> {
    logs.lines()
        .find(|line| {
            line.contains("bytes of draft state at prompt boundary")
                && line.contains(&format!("for session {prime_id}"))
        })
        .and_then(|line| {
            // "... cached T bytes of target + D bytes of draft state ..."
            line.split("+ ")
                .nth(1)?
                .split(' ')
                .next()?
                .parse::<u64>()
                .ok()
        })
}

/// Real-model proof that an ACTUAL `session/fork` CHAIN reuses the parent's
/// full saved prefix at zero rollback on the **recurrent** (MTP) model — the
/// integration counterpart to the model-free unit test
/// `fork_chain_each_generation_reuses_own_donor_at_zero_rollback`
/// (`crates/llama-agent/src/queue.rs`).
///
/// The sibling test above proves CROSS-SESSION donor reuse (fresh sessions that
/// merely share a prefix). This test proves the FORK path specifically, and on
/// a chain: a fork's first prompt strictly EXTENDS its parent's saved tokens,
/// so the restore trims an empty range — zero rollback — even though the
/// divergent continuation is well over the 64-token recurrent window. The
/// existing `session_fork_real_model.rs` fork test runs on the plain-attention
/// model (`real_model_config`), where the window is `usize::MAX` and the
/// recurrent constraint never fires; this runs on `mtp_model_config` so the
/// finite window is the binding constraint, exactly as production Qwen.
///
/// Chain: prime parent → fork child off parent → fork grandchild off child.
/// Each link's first decode must log `streaming reusing N` for its OWN session
/// with N >= its donor's full saved prefix, and ZERO `KV trim … returned false`
/// fallbacks must fire across the whole chain.
///
/// MTP assertion (corrected, NON-over-strict per card): `skipping MTP this
/// turn` must NOT fire on the fork turns ONLY WHEN the prime turn actually ran
/// MTP (its prompt-boundary save carries `> 0` draft bytes). A prime that ran
/// no MTP saves no draft, so the first fork legitimately cold-starts the draft
/// — asserting zero skips in that case would be wrong.
#[tokio::test]
#[serial]
#[ignore = "Heavy recurrent-MTP real-model proof. Loads the Qwen3.5-0.8B-MTP \
            model and runs a multi-turn fork chain; on the shared CI runner \
            this hangs past the per-turn budget (recurrent KV-slot contention \
            under load — see the NoKvCacheSlot history), though it passes \
            locally (~462s). Run on demand with `--include-ignored`. The \
            fork-chain selection/reuse logic is covered in CI by the model-free \
            unit tests in queue.rs; this is the on-demand real-FFI proof, also \
            validated live in calcutron-qwen."]
async fn fork_chain_reuses_full_parent_prefix_without_rollback_on_recurrent_model() {
    let capture = swissarmyhammer_common::test_utils::CaptureWriter::default();
    let installed = tracing_subscriber::fmt()
        .with_env_filter("warn,llama_agent::queue=info")
        .with_ansi(false)
        .with_writer(capture.clone())
        .try_init()
        .is_ok();
    if !installed {
        if std::env::var_os("NEXTEST").is_some() {
            panic!(
                "could not install the capturing tracing subscriber under nextest — \
                 the fork-chain prefix-reuse assertions cannot run, refusing to pass vacuously"
            );
        }
        warn!(
            "Skipping recurrent fork-chain test: a global tracing subscriber is already \
             installed (shared-process cargo test). Run under nextest to assert."
        );
        return;
    }

    // The recurrent (MTP) model is what makes the finite 64-token rollback
    // window apply — the whole point of running the fork chain here rather than
    // on the plain-attention model.
    let Some((server, _rx)) = build_real_model_server(mtp_model_config()).await else {
        return;
    };

    // --- 1. Prime the parent with a real turn (the shared validator prefix). ---
    let parent = server
        .new_session(NewSessionRequest::new(PathBuf::from("/tmp")))
        .await
        .expect("new_session")
        .session_id;
    prompt_turn(
        &server,
        &parent,
        &format!("{VALIDATOR_HEADER}{PRIME_TAIL}"),
        NO_HANG_BUDGET,
    )
    .await;

    // --- 2. Confirm the parent's state is saved (never fork blind), pin it. ---
    let status = server
        .session_state_status(SessionStateStatusRequest {
            session_id: parent.0.to_string(),
        })
        .await
        .expect("state_status");
    assert!(
        status.saved,
        "the prime turn must leave a saved KV snapshot for the parent"
    );
    let prefix_tokens = status
        .prompt_tokens
        .expect("saved state must report its prompt-token count");
    assert!(prefix_tokens > 0, "saved prefix must cover real tokens");
    let pinned = server
        .pin_session(SessionPinRequest {
            session_id: parent.0.to_string(),
            pinned: true,
        })
        .await
        .expect("pin");
    assert!(pinned.pinned, "the parent's saved state must be pinned");

    // --- 3. Fork a CHAIN: child off parent, then grandchild off child. Each
    // fork gets a different divergent continuation (> 64 tokens), so each link's
    // reuse must rest on a strict-prefix (rollback-0) restore of its OWN donor,
    // never an infeasible-rollback foreign donor. ---
    let child_fork = server
        .fork_session(SessionForkRequest {
            parent_session_id: parent.0.to_string(),
        })
        .await
        .expect("fork of a confirmed-saved parent must succeed");
    assert!(
        child_fork.state_attached,
        "the child fork must attach the parent's state"
    );
    assert_eq!(
        child_fork.prefix_tokens,
        Some(prefix_tokens),
        "the child fork must report the parent's full saved token count"
    );
    let child_id = child_fork.session_id.clone();
    prompt_turn(
        &server,
        &SessionId::new(child_id.clone()),
        &format!("{VALIDATOR_HEADER}{}", SIBLING_TAILS[0]),
        NO_HANG_BUDGET,
    )
    .await;

    // The grandchild forks off the CHILD (now carrying its own end-of-turn save
    // that strictly extends the parent's), proving reuse compounds down a chain.
    let grandchild_fork = server
        .fork_session(SessionForkRequest {
            parent_session_id: child_id.clone(),
        })
        .await
        .expect("fork of the child must succeed");
    assert!(
        grandchild_fork.state_attached,
        "the grandchild fork must attach the child's state"
    );
    let grandchild_prefix = grandchild_fork
        .prefix_tokens
        .expect("the grandchild fork must report the child's saved token count");
    assert!(
        grandchild_prefix >= prefix_tokens,
        "the grandchild inherits at least the parent's prefix (the child's save \
         strictly extends it): grandchild prefix {grandchild_prefix} >= parent {prefix_tokens}"
    );
    let grandchild_id = grandchild_fork.session_id.clone();
    prompt_turn(
        &server,
        &SessionId::new(grandchild_id.clone()),
        &format!("{VALIDATOR_HEADER}{}", SIBLING_TAILS[1]),
        NO_HANG_BUDGET,
    )
    .await;

    // --- 4. Worker-log assertions: each link reused its donor's full prefix. ---
    let logs = capture.contents();

    // (child reuses the parent's full prefix; grandchild reuses the child's.)
    for (fork_id, donor_floor) in [
        (&child_id, prefix_tokens),
        (&grandchild_id, grandchild_prefix),
    ] {
        let reused_tokens = logs
            .lines()
            .find(|line| {
                line.contains("streaming reusing")
                    && line.contains(&format!("for session {fork_id}"))
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
                    "fork {fork_id}'s first decode must log 'streaming reusing N cached \
                     tokens' (a warm strict-prefix restore, not a cold prefill). \
                     Captured logs:\n{logs}"
                )
            });
        assert!(
            reused_tokens >= donor_floor,
            "fork {fork_id}'s first decode reused only {reused_tokens} cached tokens — \
             a strict-prefix fork must reuse at least its donor's full saved prefix of \
             {donor_floor} tokens. Captured logs:\n{logs}"
        );
    }

    // The core acceptance criterion: a strict-prefix fork's empty-range trim
    // must succeed on the live recurrent context — the rollback fallback must
    // NEVER fire anywhere in the chain.
    assert!(
        !logs.contains("KV trim to common prefix returned false"),
        "a strict-prefix fork chain must never hit the hybrid-model rollback fallback \
         (the empty-range trim must succeed). Captured logs:\n{logs}"
    );

    // MTP retention (corrected, non-over-strict). Only assert zero MTP skips on
    // the fork turns when the PRIME turn actually ran MTP — i.e. its
    // prompt-boundary save carried draft bytes. A prime that ran no MTP saves no
    // draft, so the first fork legitimately cold-starts the draft and one skip
    // is correct, not a regression.
    let parent_id = parent.0.to_string();
    let prime_draft = prime_draft_bytes(&logs, &parent_id);
    let skipped_mtp_lines: Vec<&str> = logs
        .lines()
        .filter(|line| line.contains("skipping MTP this turn"))
        .collect();
    println!(
        "fork_chain_recurrent observation: prefix_tokens={prefix_tokens}, \
         prime_draft_bytes={prime_draft:?}, 'skipping MTP this turn' fired {} time(s).",
        skipped_mtp_lines.len()
    );
    for line in &skipped_mtp_lines {
        println!("  MTP-skip: {line}");
    }
    if matches!(prime_draft, Some(d) if d > 0) {
        assert!(
            skipped_mtp_lines.is_empty(),
            "the prime ran MTP (saved {prime_draft:?} draft bytes), so every fork in the \
             chain that reuses it must retain MTP: the prime's draft rides with the target \
             in the same donor and trims to the same zero-rollback offset, so \
             'skipping MTP this turn' must never fire. It fired {} time(s):\n{}\n\
             Captured logs:\n{logs}",
            skipped_mtp_lines.len(),
            skipped_mtp_lines.join("\n")
        );
    } else {
        // Prime ran no MTP — the strict zero-skip claim does not apply. The
        // target-side reuse + zero-rollback assertions above still hold and are
        // the real acceptance criteria; MTP retention is only asserted when
        // there is a prime draft to retain.
        println!(
            "fork_chain_recurrent: prime ran no MTP (draft bytes {prime_draft:?}); \
             skipping the strict zero-MTP-skip assertion (a cold draft start is correct here)."
        );
    }
}
