//! The run's shared primed prefix: prime once, confirm the saved state, pin
//! it for the run, fork per validator, and classify how warm each fork was.
//!
//! The large content of a review run (the change purpose + every file's
//! rendered diff/source/probe evidence) is identical across validators, so it
//! is primed and pinned ONCE ([`prime_run_prefix`]) and every validator task
//! forks from the saved state. Any failure here degrades the fan-out to
//! monolithic prompts — correct, just cold, never a lost task.

use crate::review::scope::WorkList;
use crate::validators::{AgentPool, ForkAttachment, SessionPinGuard, SessionTurn};
use agent_client_protocol_extras::SessionStateStatusResponse;

use super::render_run_prime;

/// Prime the run's shared prompt prefix (change purpose + every file's
/// diff/source/probe evidence — no rule text) in a dedicated session, confirm
/// the agent saved restorable state for it ("never fork blind"), and acquire
/// the scoped pin guard that governs the run's pin lifecycle.
///
/// The prime turn is submitted with a born-pinned save intent
/// ([`AgentPool::submit_primed`] carries `pin_on_save` in `_meta`), so the
/// prefix is pinned **atomically at save time** — never an unpinned eviction
/// candidate, so a concurrent session's save cannot evict it before fan-out
/// forks from it. That is the structural close of the prime→pin eviction race.
///
/// The post-turn [`AgentPool::pin_session_scoped`] is therefore no longer the
/// load-bearing pin: it is an **idempotent re-pin / confirm** that (a) verifies
/// the state is still resident and (b) returns the [`SessionPinGuard`] whose
/// `release()`/`Drop` performs the matching unpin once the whole run (fan-out
/// AND verify) completes or the run future is dropped mid-flight. There is one
/// pin protocol — born-pinned at save, unpinned by the guard — not two competing
/// ones. A backend without a KV cache (claude) born-pins as a no-op and reports
/// `pinned: false`; forking still works, consistent with the pin=no-op contract.
///
/// Returns the guard for the primed session (carrying its id, the fork parent),
/// or `None` when any step failed — fan-out degrades to monolithic prompts
/// (correct, just cold), never a lost task.
pub(super) async fn prime_run_prefix(work: &WorkList, pool: &AgentPool) -> Option<SessionPinGuard> {
    const RUN: &str = "<run>";
    let prefix = render_run_prime(work);
    let turn = submit_prime(pool, RUN, prefix).await?;
    let status = confirm_saved_state(pool, RUN, &turn).await?;
    pin_prefix(pool, RUN, &turn, &status).await
}

/// Submit the born-pinned prime turn for a validator's shared prefix.
/// `None` (and a warn) on either a turn failure or a dropped result —
/// the caller degrades to monolithic prompts.
async fn submit_prime(pool: &AgentPool, name: &str, prefix: String) -> Option<SessionTurn> {
    match pool.submit_primed(prefix).await {
        Ok(Ok(turn)) => Some(turn),
        Ok(Err(err)) => {
            tracing::warn!(
                run = %name,
                error = %err,
                "prefix prime turn failed; falling back to monolithic prompts"
            );
            None
        }
        Err(_) => {
            tracing::warn!(
                run = %name,
                "prefix prime result was dropped; falling back to monolithic prompts"
            );
            None
        }
    }
}

/// Confirm the prime actually saved restorable state ("never fork blind").
/// `saved` is the contract's gate; a backend that tracks token counts must also
/// report a non-empty prefix. Backends without token counts (`prompt_tokens:
/// None`, e.g. the claude CLI) are still forkable per the contract. `None` (and
/// a warn) when the status check fails or the state is not restorable.
async fn confirm_saved_state(
    pool: &AgentPool,
    name: &str,
    turn: &SessionTurn,
) -> Option<SessionStateStatusResponse> {
    let status = match pool.session_state_status(&turn.session_id).await {
        Ok(status) => status,
        Err(err) => {
            tracing::warn!(
                run = %name,
                session = %turn.session_id,
                error = %err,
                "prefix state-status check failed; falling back to monolithic prompts"
            );
            return None;
        }
    };
    if !status.saved || status.prompt_tokens.is_some_and(|tokens| tokens == 0) {
        tracing::warn!(
            run = %name,
            session = %turn.session_id,
            saved = status.saved,
            prompt_tokens = ?status.prompt_tokens,
            "primed prefix session has no restorable state; falling back to monolithic prompts"
        );
        return None;
    }
    Some(status)
}

/// Acquire the scoped pin guard that governs the fan-out's pin lifecycle.
///
/// The prefix was already born pinned by the prime turn (the `_meta`
/// pin-on-save intent). This scoped call is therefore an idempotent
/// re-pin/confirm — it re-asserts the pin (a no-op when the state is already
/// born pinned) and, crucially, returns the guard that owns the matching unpin
/// for the fan-out's lifetime. A backend without pinning reports an effective
/// `pinned: false` and forking still works; only a pin ERROR (the state
/// vanished) degrades to monolithic prompts.
async fn pin_prefix(
    pool: &AgentPool,
    name: &str,
    turn: &SessionTurn,
    status: &SessionStateStatusResponse,
) -> Option<SessionPinGuard> {
    match pool.pin_session_scoped(&turn.session_id).await {
        Ok((pin, guard)) => {
            tracing::info!(
                run = %name,
                session = %turn.session_id,
                prefix_tokens = ?status.prompt_tokens,
                born_pinned = status.pinned,
                pinned = pin.pinned,
                "primed shared run prefix session (born pinned at save; pin confirmed)"
            );
            Some(guard)
        }
        Err(err) => {
            tracing::warn!(
                run = %name,
                session = %turn.session_id,
                error = %err,
                "failed to pin primed prefix state; falling back to monolithic prompts"
            );
            None
        }
    }
}

/// Release the run's shared primed-prefix pin once the whole run (fan-out AND
/// verify) has drained, so the pinned cache entry does not outlive the run. A
/// failed unpin is logged, never fatal — the entry falls back to normal
/// eviction. (Cancellation is covered separately: a run future dropped before
/// reaching this point releases the pin from the guard's `Drop`.)
pub async fn unpin_prefix_session(guard: SessionPinGuard) {
    let session = guard.session_id().to_string();
    match guard.release().await {
        Ok(_) => tracing::debug!(
            session = %session,
            "unpinned shared run prefix session"
        ),
        Err(err) => tracing::warn!(
            session = %session,
            error = %err,
            "failed to unpin shared run prefix session"
        ),
    }
}

/// How a turn reused the shared file-context prefix, classified from the two
/// reuse signals the two backends report.
///
/// A backend with a native KV cache reports reuse as a fork attaching the
/// parent's saved generation state with a prefix token count
/// ([`ForkAttachment::prefix_tokens`]); the claude backend's fork attaches no
/// token counts and instead reports Anthropic prompt-cache reads/writes on the
/// turn's [`SessionTurn::cache_usage`]. This enum unifies both so warm vs cold
/// reuse is observable on either backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixReuse {
    /// A native KV fork attached the parent's saved state, reusing
    /// `reused_tokens` prompt tokens (the native-KV warm path).
    WarmKv {
        /// Prompt tokens the attached parent state covered.
        reused_tokens: u64,
    },
    /// The Anthropic prompt cache served the prefix warm: `read` tokens came
    /// from a cache read, `created` tokens were (re)written this turn.
    WarmCache {
        /// Tokens served from the warm prompt cache (`cache_read_input_tokens`).
        read: u64,
        /// Tokens written to the prompt cache this turn
        /// (`cache_creation_input_tokens`).
        created: u64,
    },
    /// No warm reuse observed: a cold prefill (cache write only, or native
    /// degraded fork), or no reuse signal at all.
    Cold,
}

/// Classify how a turn reused the primed prefix, from the fork attachment and
/// the turn's prompt-cache usage. Pure so the warm/cold decision is unit-tested
/// without asserting on log strings.
///
/// Precedence:
/// 1. A native KV fork with a prefix token count → [`PrefixReuse::WarmKv`]
///    (the native-KV path, whose `fork.prefix_tokens` is authoritative).
/// 2. Otherwise a claude turn reporting `cache_read_input_tokens > 0` →
///    [`PrefixReuse::WarmCache`] (the hosted prefix cache served it warm).
/// 3. Otherwise [`PrefixReuse::Cold`] — a cold write (`cache_creation_input_tokens
///    > 0` with no reads), a degraded fork, or no reuse signal at all.
pub fn classify_reuse(
    fork: Option<ForkAttachment>,
    usage: Option<claude_agent::protocol_translator::CacheUsage>,
) -> PrefixReuse {
    if let Some(reused_tokens) = fork.and_then(|f| f.prefix_tokens) {
        return PrefixReuse::WarmKv { reused_tokens };
    }
    if let Some(usage) = usage {
        let read = usage.cache_read_input_tokens.unwrap_or(0);
        if read > 0 {
            return PrefixReuse::WarmCache {
                read,
                created: usage.cache_creation_input_tokens.unwrap_or(0),
            };
        }
    }
    PrefixReuse::Cold
}

impl PrefixReuse {
    /// A short human label for the reuse outcome, for log messages.
    pub fn label(&self) -> &'static str {
        match self {
            PrefixReuse::WarmKv { .. } => "warm KV fork",
            PrefixReuse::WarmCache { .. } => "warm prompt cache",
            PrefixReuse::Cold => "cold (no reuse)",
        }
    }

    /// The native KV reused token count, when this is a [`PrefixReuse::WarmKv`].
    pub fn reused_tokens(&self) -> Option<u64> {
        match self {
            PrefixReuse::WarmKv { reused_tokens } => Some(*reused_tokens),
            _ => None,
        }
    }

    /// The Anthropic prompt-cache read token count, when this is a
    /// [`PrefixReuse::WarmCache`].
    pub fn cache_read(&self) -> Option<u64> {
        match self {
            PrefixReuse::WarmCache { read, .. } => Some(*read),
            _ => None,
        }
    }

    /// The Anthropic prompt-cache created (cold write) token count, when this is
    /// a [`PrefixReuse::WarmCache`].
    pub fn cache_created(&self) -> Option<u64> {
        match self {
            PrefixReuse::WarmCache { created, .. } => Some(*created),
            _ => None,
        }
    }
}
