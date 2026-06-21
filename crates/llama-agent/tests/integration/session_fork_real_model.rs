//! Real-model proof of the `session/fork` strict-prefix contract on the
//! hybrid Qwen test model.
//!
//! The whole point of forking (vs the cross-session LCP donor cache) is that a
//! fork's first prompt strictly EXTENDS the parent's saved prompt tokens, so
//! the restore trims an empty KV range — zero rollback. On hybrid
//! attention+recurrent models, a non-empty rollback past the recurrent-state
//! snapshot window fails silently (`clear_kv_cache_seq` → `Ok(false)`) and
//! falls back to a full cold prefill; a strict-prefix fork must never hit
//! that path.
//!
//! This test drives the production ACP surface end to end:
//!
//! 1. prime a parent session with a real prompt turn,
//! 2. CONFIRM its state is saved via `session/state_status` (token count
//!    visible — never fork blind) and pin it,
//! 3. fork it twice and prompt each fork with a different continuation,
//! 4. assert — from the queue worker's own logs — that each fork's first
//!    decode reused the parent's FULL saved token count ("streaming reusing N
//!    cached tokens") with zero "KV trim … returned false" fallbacks.
//!
//! The worker logs from another thread, so the global tracing subscriber is
//! installed with the shared in-memory `CaptureWriter` (same pattern as
//! `streaming_generation.rs`'s KV-reuse test).

use std::path::PathBuf;
use std::time::Duration;

use agent_client_protocol::schema::{NewSessionRequest, SessionId};
use agent_client_protocol_extras::{
    SessionForkRequest, SessionPinRequest, SessionStateStatusRequest,
};
use serial_test::serial;
use tracing::warn;

use crate::integration::real_model_helpers::{
    build_real_model_server, prompt_turn, real_model_config,
};

/// Per-prompt hang guard, matching the sibling agentic-loop tests.
const NO_HANG_BUDGET: Duration = Duration::from_secs(120);

/// Prime → confirm saved → pin → fork ×2 → both forks reuse the parent's full
/// saved prefix with zero KV-trim fallbacks.
#[tokio::test]
#[serial]
async fn forked_sessions_reuse_full_parent_prefix_without_rollback() {
    let capture = swissarmyhammer_common::test_utils::CaptureWriter::default();
    // Capture only the queue's INFO lines (the reuse/trim evidence) plus
    // warnings everywhere. The capture subscriber is GLOBAL: under
    // shared-process `cargo test` it observes every other test that runs
    // after this one, and formatting the full INFO firehose through the
    // capture mutex measurably slows the whole binary. The narrow filter
    // keeps exactly the lines the assertions read.
    let installed = tracing_subscriber::fmt()
        .with_env_filter("warn,llama_agent::queue=info")
        .with_ansi(false)
        .with_writer(capture.clone())
        .try_init()
        .is_ok();
    if !installed {
        // Same policy as `streaming_generation.rs`: under nextest
        // (process-per-test) installation always succeeds; under
        // shared-process `cargo test` another test may own the global
        // subscriber, and without capture the assertions cannot run.
        if std::env::var_os("NEXTEST").is_some() {
            panic!(
                "could not install the capturing tracing subscriber under nextest — \
                 the fork prefix-reuse assertions cannot run, refusing to pass vacuously"
            );
        }
        warn!(
            "Skipping fork real-model test: a global tracing subscriber is already \
             installed (shared-process cargo test). Run under nextest to assert."
        );
        return;
    }

    let Some((server, _rx)) = build_real_model_server(real_model_config()).await else {
        return;
    };

    // --- 1. Prime the parent with a real turn. ---
    let parent = server
        .new_session(NewSessionRequest::new(PathBuf::from("/tmp")))
        .await
        .expect("new_session")
        .session_id;
    prompt_turn(
        &server,
        &parent,
        "/no_think Remember the secret word is plum. Reply with exactly: ok",
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

    // --- 3. Fork twice; prompt each fork with a different continuation. ---
    let mut fork_ids = Vec::new();
    for continuation in [
        "/no_think What is the secret word? Reply with just the word.",
        "/no_think Reply with exactly: banana",
    ] {
        let fork = server
            .fork_session(SessionForkRequest {
                parent_session_id: parent.0.to_string(),
            })
            .await
            .expect("fork of a confirmed-saved parent must succeed");
        assert!(fork.state_attached, "fork must attach the parent's state");
        assert_eq!(
            fork.prefix_tokens,
            Some(prefix_tokens),
            "fork must report the parent's full saved token count"
        );

        let fork_session = SessionId::new(fork.session_id.clone());
        prompt_turn(&server, &fork_session, continuation, NO_HANG_BUDGET).await;
        fork_ids.push(fork.session_id);
    }

    // --- 4. Worker-log assertions: full-prefix reuse, zero trim fallbacks. ---
    let logs = capture.contents();

    // Each fork's first decode must reuse AT LEAST the parent's full saved
    // token count. Fork 1's donor is the parent's entry (exactly
    // `prefix_tokens`); a later fork may legitimately reuse MORE — the donor
    // scan picks the deepest prefix, and an earlier fork's own boundary save
    // extends the parent's (e.g. shared `[parent history + "<|im_start|>user"]`
    // out to where the continuations diverge). Both are strict-prefix
    // restores with zero rollback; what must never happen is reuse below the
    // parent's saved count (a degraded/cold restore).
    for fork_id in &fork_ids {
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
                    "fork {fork_id}'s first decode must log 'streaming reusing N \
                     cached tokens' (a warm strict-prefix restore, not a cold \
                     prefill). Captured logs:\n{logs}"
                )
            });
        assert!(
            reused_tokens >= prefix_tokens,
            "fork {fork_id}'s first decode reused only {reused_tokens} cached \
             tokens — it must reuse at least the parent's full saved prefix of \
             {prefix_tokens} tokens. Captured logs:\n{logs}"
        );
    }

    assert!(
        !logs.contains("KV trim to common prefix returned false"),
        "a strict-prefix fork must never hit the hybrid-model rollback \
         fallback (trim of an empty range must succeed). Captured logs:\n{logs}"
    );
}
