//! Engine stage 2 — the fan-out fleet.
//!
//! The shard is the **validator**: this stage takes the stage-1
//! [`WorkList`](crate::review::WorkList) and produces one agent task per
//! validator, submitting every task to the shared
//! [`AgentPool`](crate::validators::AgentPool). Each task reviews the change —
//! every file under review, with the engine-run probe evidence stage 1 already
//! gathered — against ONE validator's full ruleset, and returns a
//! `Vec<`[`Finding`]`>` tagged with the validator (and, when the agent cites it,
//! the rule).
//!
//! **Parallelism is not controlled here.** Every task goes to the shared
//! [`AgentPool`], which owns the single concurrency control (worker count). This
//! stage only submits and collects; the pool queues and drains. A task that
//! errors or times out yields zero findings for its validator — logged, never a
//! panic — so one bad task never aborts the rest.
//!
//! # One shared prime, fork per validator
//!
//! The large content of a review run — the change description and the full
//! diffs/sources of every file under review — is identical across every
//! validator, and on a local model it dominates the prompt. So instead of
//! re-decoding it per task, the whole run shares ONE primed prefix and fans every
//! validator out as a fork of it:
//!
//! 1. **Prime once per run** — one session is prompted with [`render_run_prime`]
//!    (the change purpose + every file's diff/source/probe evidence, ending with
//!    an explicit "reply OK, the rules arrive next" handoff). The completed turn
//!    leaves the agent's saved state exactly at the boundary every validator fork
//!    continues from. There is no validator-specific text in the prime.
//! 2. **Confirm + pin** — the `session/state_status` extension confirms the
//!    state is actually saved ("never fork blind"), and `session/pin` protects
//!    it from cache eviction for the run's duration (fan-out AND verify).
//! 3. **Fork per validator** — each validator turn runs on a `session/fork`
//!    of the primed session and sends ONLY [`render_validator_suffix`] (that
//!    validator's instructions — its full ruleset + output contract), decoding
//!    strictly forward from the shared prefix. Each suffix is non-empty by
//!    construction (always at least the rule bodies + the contract). Warm reuse
//!    (and the reused token count) is logged per task.
//! 4. **Unpin** — the prefix pin is released by [`run_review`](crate::review::run_review)
//!    once both fan-out and verify have drained. The pin is held by a
//!    [`SessionPinGuard`], so a future dropped mid-run (cancelled review, caller
//!    timeout) still releases it.
//!
//! Any failure — the prime turn, the state confirmation, the pin, or an
//! individual fork — degrades that task to a self-contained monolithic prompt
//! ([`render_fleet_prompt`], one fresh session carrying everything for the
//! validator) with a logged warning: degraded but correct, never a lost task.
//! The flow is backend-agnostic; the extension contract lives in
//! [`agent_client_protocol_extras::session_fork`].
//!
//! # The prompt payload
//!
//! The split renders compose byte-identically into the monolithic per-validator
//! prompt, so the warm and degraded paths never drift. The pieces, reusing the
//! structured data stage 1 produced (no template engine):
//!
//! - [`render_run_prime`] (primed once): the **change purpose** plus, for every
//!   distinct file under review, its path, the structured semantic diff, the
//!   bounded source slice, and the probe results rendered as evidence blocks —
//!   then the prime handoff. No validator text.
//! - [`render_validator_suffix`] (forked per validator): the **validator
//!   instructions** — the mandate (the validator's `description`), the paths of
//!   the validator's files in scope, every rule body verbatim, the
//!   default, and the output contract (every finding emits `rule` + `claim` +
//!   `evidence` + `suggestion`, matching the [`Finding`] type).
//! - [`render_fleet_prompt`] (degraded fallback): the change purpose, the
//!   validator's own files, and the validator suffix, in one fresh-session prompt.

use std::fmt::Write as _;

use crate::review::probes::render_probe_evidence;
use crate::review::scope::{FileWork, ValidatorWork, WorkList};
use crate::review::types::{parse_findings, Finding};
use crate::validators::{
    AgentPool, ForkAttachment, PoolError, RuleSet, SessionPinGuard, SessionTurn, SessionTurnResult,
    ValidatorLoader,
};
use agent_client_protocol::schema::SessionId;
use agent_client_protocol_extras::SessionStateStatusResponse;

/// The default review `batch_size` in **bytes** (128 KiB).
///
/// Cramming every changed file's full source into one shared prime overflows the
/// review model's context on a large diff (every fan-out validator then fails
/// uniformly), and even when it fits it dilutes attention. So a run is split into
/// byte-budgeted batches and each batch fans out independently. This budget is a
/// deliberate, tunable knob — not derived from the model's context window.
///
/// It is sized to clear the largest single source file in a typical change
/// (~95 KB) so an ordinary commit reviews in one or a few batches instead of
/// tripping the oversize-file error, while a genuinely large multi-file diff
/// still splits across batches. (32 KiB — the previous default — was smaller
/// than many real source files, so default reviews of normal commits errored.)
pub const DEFAULT_BATCH_SIZE: usize = 128 * 1024;

/// Configuration for a fan-out run.
///
/// The fan-out grain is the validator and the change's files live in the run's
/// shared prime. [`batch_size`](FleetConfig::batch_size) bounds how much inlined
/// file content one prime may carry: [`run_review`](crate::review::run_review)
/// uses it to split the work-list into batches
/// ([`batch_work_list`](crate::review::scope::batch_work_list)) and fan each batch
/// out independently, so a large diff no longer overflows the prime.
#[derive(Debug, Clone, Copy)]
pub struct FleetConfig {
    /// The maximum inlined file content, in bytes, one batch's shared prime may
    /// carry. Whole files are packed greedily up to this budget; a single file
    /// larger than it is a hard error (never split, never sliced).
    pub batch_size: usize,
}

impl Default for FleetConfig {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_BATCH_SIZE,
        }
    }
}

/// The result of a fan-out run: the merged findings plus the task tally.
///
/// A task that errors, is dropped, or returns unparseable content still
/// degrades to zero findings (one bad task never aborts the rest), but unlike
/// the findings — which simply omit it — the tally records that it was both
/// `attempted` and `failed`. A review where most tasks fail therefore renders an
/// empty findings set with a non-zero `failed` count, which is exactly what
/// distinguishes a wedged run from a genuinely clean diff.
#[derive(Default)]
pub struct FleetOutcome {
    /// The merged, validator-tagged findings from every task that succeeded.
    pub findings: Vec<Finding>,
    /// How many validator tasks were submitted. Read through
    /// [`attempted`](Self::attempted); private so the tally can evolve without a
    /// field-level API commitment.
    attempted: usize,
    /// How many of those tasks failed (errored, were dropped, or did not parse)
    /// and so degraded to zero findings. Read through [`failed`](Self::failed);
    /// private for the same reason as [`attempted`](Self::attempted).
    failed: usize,
    /// The run's shared primed-prefix pin guard, when priming succeeded.
    ///
    /// The change + diffs are primed ONCE per run and forked per validator here;
    /// the same prime is then reused by the verify stage. So the pin must outlive
    /// fan-out — it is handed back for [`run_review`](crate::review::run_review)
    /// to keep alive across verify and release at the end. `None` when priming
    /// failed (every task ran the monolithic fallback) so there is nothing to
    /// release.
    pub prime: Option<SessionPinGuard>,
}

impl FleetOutcome {
    /// How many validator tasks were submitted in this run.
    pub fn attempted(&self) -> usize {
        self.attempted
    }

    /// How many submitted tasks failed (errored, were dropped, or did not parse)
    /// and so degraded to zero findings.
    pub fn failed(&self) -> usize {
        self.failed
    }
}

impl std::fmt::Debug for FleetOutcome {
    /// Hand-rolled because [`SessionPinGuard`] is not `Debug`; the guard is
    /// summarized as the boolean "is a prime held" rather than its contents.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FleetOutcome")
            .field("findings", &self.findings)
            .field("attempted", &self.attempted)
            .field("failed", &self.failed)
            .field("primed", &self.prime.is_some())
            .finish()
    }
}

/// Fan a [`WorkList`] out across the shared [`AgentPool`] and collect the merged,
/// validator-tagged findings.
///
/// The run's large shared content — the change description and every file's
/// diff/source/probe evidence — is primed ONCE into a single session
/// ([`render_run_prime`]). Then one task is built per validator: each forks the
/// shared prime and sends only that validator's instructions
/// ([`render_validator_suffix`] — its full ruleset + output contract), decoding
/// strictly forward from the cached prefix. As each task returns, its response is
/// parsed by [`parse_findings`] and every finding is tagged with the validator. A
/// task that errors or returns unparseable content contributes zero findings for
/// its validator and is logged — never a panic.
///
/// `loader` is the same fully-loaded [`ValidatorLoader`] stage 1 matched against,
/// reused here as the authoritative source of each validator's mandate and rule
/// bodies (the [`WorkList`] carries only the per-file work and the rule *names*).
/// A validator in the work-list with no matching RuleSet in the loader is logged
/// and skipped rather than rendered with empty instructions.
///
/// `work` is one already content-budgeted batch: the size policy
/// ([`FleetConfig::batch_size`]) is applied upstream by
/// [`run_review`](crate::review::run_review), which splits the work-list into
/// batches ([`batch_work_list`](crate::review::scope::batch_work_list)) and calls
/// `run_fleet` once per batch. So `run_fleet` itself takes no config — it just
/// fans the batch it is given out across the pool.
///
/// The returned findings are ordered by validator (work-list order). Alongside
/// them, the returned [`FleetOutcome`] carries the task tally — how many tasks
/// were attempted and how many failed — so a saturated run (most tasks rejected)
/// is distinguishable from a genuinely clean diff rather than both rendering an
/// empty findings set, plus the shared prime's pin guard ([`FleetOutcome::prime`])
/// so the caller can reuse the prime for verify and release the pin once the
/// whole run drains.
pub async fn run_fleet(
    work: &WorkList,
    loader: &ValidatorLoader,
    pool: &AgentPool,
) -> FleetOutcome {
    // Plan the fan-out BEFORE priming so an empty plan (no matching ruleset)
    // skips the prime entirely — an empty run never prompts the agent.
    let plan = plan_fan_out(work, loader);
    if plan.is_empty() {
        return FleetOutcome::default();
    }

    // Prime the run's shared prefix (change + all diffs) ONCE, then submit one
    // fork (or monolithic fallback) per planned validator and collect them all.
    // `None` from priming → every task degrades to a self-contained monolithic
    // prompt.
    let prime = prime_run_prefix(work, pool).await;
    let pending = submit_fan_out(plan, work, pool, &prime);
    let (findings, attempted, failed) = collect_fan_out(pending, work, pool).await;

    FleetOutcome {
        findings,
        attempted,
        failed,
        prime,
    }
}

/// Plan the fan-out: one [`ValidatorTask`] per validator the `loader` knows,
/// in work-list order. A validator with no matching RuleSet in the loader is
/// logged and skipped (never rendered with empty instructions); each planned
/// validator's rule names are logged so the fan-out shows exactly what ran.
fn plan_fan_out<'w>(work: &'w WorkList, loader: &'w ValidatorLoader) -> Vec<ValidatorTask<'w>> {
    let mut plan: Vec<ValidatorTask<'w>> = Vec::new();
    for validator in &work.validators {
        let Some(ruleset) = loader.get_ruleset(&validator.validator_name) else {
            tracing::warn!(
                validator = %validator.validator_name,
                "fleet fan-out: no RuleSet for validator in loader; skipping it"
            );
            continue;
        };
        let rule_names: Vec<&str> = ruleset.rules.iter().map(|r| r.name.as_str()).collect();
        tracing::info!(
            validator = %validator.validator_name,
            files = validator.files.len(),
            rules = ?rule_names,
            "fleet fan-out: forking one task per validator against the shared prime"
        );
        plan.push(ValidatorTask { validator, ruleset });
    }
    plan
}

/// Submit every planned validator task to the pool, returning the in-flight
/// receivers paired with their context.
///
/// The fan-out grain is the validator: the files live in the shared prime, so a
/// validator's task carries only its instructions (its full ruleset) as the fork
/// suffix. With a live `prime` each task forks the shared prefix and sends just
/// the suffix; without one (priming failed) each task degrades to a
/// self-contained monolithic prompt on a fresh session.
fn submit_fan_out<'w>(
    plan: Vec<ValidatorTask<'w>>,
    work: &WorkList,
    pool: &AgentPool,
    prime: &Option<SessionPinGuard>,
) -> Vec<PendingValidator<'w>> {
    plan.into_iter()
        .map(|task| {
            tracing::debug!(
                validator = %task.validator.validator_name,
                warm = prime.is_some(),
                "fleet fan-out: submitting validator task"
            );
            let suffix = render_validator_suffix(task.validator, task.ruleset);
            let rx = match prime {
                Some(guard) => Submitted::Forked(pool.submit_forked(guard.session_id(), suffix)),
                None => Submitted::Monolithic(pool.submit(render_fleet_prompt(
                    &work.change_purpose,
                    task.validator,
                    task.ruleset,
                ))),
            };
            PendingValidator { task, rx }
        })
        .collect()
}

/// Collect every submitted validator task in submission order, returning the
/// merged findings plus the `(attempted, failed)` tally.
///
/// Each receiver resolves independently while the pool drains in parallel up to
/// its worker count. A task that errors, is dropped, or returns unparseable
/// content contributes zero findings and bumps `failed` — one bad task never
/// aborts the rest. Awaiting here (rather than in a detached task) is what keeps
/// the run's shared-prime pin released on cancellation: dropping the `run_fleet`
/// future drops this collect mid-await.
async fn collect_fan_out(
    pending: Vec<PendingValidator<'_>>,
    work: &WorkList,
    pool: &AgentPool,
) -> (Vec<Finding>, usize, usize) {
    let attempted = pending.len();
    let mut findings: Vec<Finding> = Vec::new();
    let mut failed = 0usize;
    for pending in pending {
        let name = pending.task.validator.validator_name.as_str();
        let files: Vec<String> = pending
            .task
            .validator
            .files
            .iter()
            .map(|f| f.path.clone())
            .collect();
        let collected = match pending.rx {
            Submitted::Monolithic(rx) => collect_task(rx.await, name, &files),
            Submitted::Forked(rx) => {
                collect_forked_task(
                    rx.await,
                    &work.change_purpose,
                    pending.task.validator,
                    pending.task.ruleset,
                    &files,
                    pool,
                )
                .await
            }
        };
        match collected {
            Ok(parsed) => findings.extend(parsed),
            Err(()) => failed += 1,
        }
    }
    (findings, attempted, failed)
}

/// One planned validator task: the work-list/ruleset context needed to render its
/// prompt, attribute its findings, and (on fork failure) re-render the monolithic
/// fallback.
struct ValidatorTask<'w> {
    validator: &'w ValidatorWork,
    ruleset: &'w RuleSet,
}

/// A submitted [`ValidatorTask`]: its context plus the in-flight receiver.
struct PendingValidator<'w> {
    task: ValidatorTask<'w>,
    rx: Submitted,
}

/// How one validator task was submitted: a suffix-only prompt on a fork of the
/// run's primed prefix session (the warm path), or the full monolithic prompt on
/// a fresh session (the degraded path).
enum Submitted {
    Forked(tokio::sync::oneshot::Receiver<SessionTurnResult>),
    Monolithic(tokio::sync::oneshot::Receiver<crate::validators::PromptResult>),
}

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
async fn prime_run_prefix(work: &WorkList, pool: &AgentPool) -> Option<SessionPinGuard> {
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
                validator = %name,
                error = %err,
                "prefix prime turn failed; falling back to monolithic prompts"
            );
            None
        }
        Err(_) => {
            tracing::warn!(
                validator = %name,
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
                validator = %name,
                session = %turn.session_id,
                error = %err,
                "prefix state-status check failed; falling back to monolithic prompts"
            );
            return None;
        }
    };
    if !status.saved || status.prompt_tokens.is_some_and(|tokens| tokens == 0) {
        tracing::warn!(
            validator = %name,
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
                scope = %name,
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
                scope = %name,
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
/// The native KV (llama/qwen) backend reports reuse as a fork attaching the
/// parent's saved generation state with a prefix token count
/// ([`ForkAttachment::prefix_tokens`]); the claude backend's fork attaches no
/// token counts and instead reports Anthropic prompt-cache reads/writes on the
/// turn's [`SessionTurn::cache_usage`]. This enum unifies both so warm vs cold
/// reuse is observable on either backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixReuse {
    /// A native KV fork attached the parent's saved state, reusing
    /// `reused_tokens` prompt tokens (the llama/qwen warm path).
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
///    (the llama/qwen path, whose `fork.prefix_tokens` is authoritative).
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

/// Resolve one forked validator task's delivered result into tagged findings.
///
/// A delivered turn is parsed exactly like the monolithic path, after logging
/// whether the fork was warm (parent state attached — with the reused token
/// count, so a run's prefill savings are measurable from the log) or degraded
/// (history cloned, cold prefill). A turn whose FORK failed falls back to the
/// monolithic fresh-session prompt for the validator — degraded but correct,
/// never a lost task. Any other failure degrades to zero findings like
/// [`collect_task`].
async fn collect_forked_task(
    delivered: Result<SessionTurnResult, tokio::sync::oneshot::error::RecvError>,
    change_purpose: &str,
    validator: &ValidatorWork,
    ruleset: &RuleSet,
    files: &[String],
    pool: &AgentPool,
) -> Result<Vec<Finding>, ()> {
    let name = validator.validator_name.as_str();
    match delivered {
        Ok(Ok(turn)) => handle_fork_success(turn, name, files, pool).await,
        Ok(Err(PoolError::ForkFailed {
            parent_session_id,
            message,
        })) => {
            handle_fork_failed(
                parent_session_id,
                message,
                change_purpose,
                validator,
                ruleset,
                files,
                pool,
            )
            .await
        }
        Ok(Err(err)) => handle_task_failure(
            name,
            files,
            Some(&err),
            "fleet task failed; yielding zero findings for this validator",
        ),
        Err(_) => handle_task_failure(
            name,
            files,
            None,
            "fleet task result was dropped before delivery; yielding zero findings",
        ),
    }
}

/// The warm/degraded fork-success arm of [`collect_forked_task`]: log the prefix
/// reuse, parse the delivered turn exactly like the monolithic path, then drive
/// the session forward with [`sweep_until_dry`] to recover under-reported
/// instances before returning.
///
/// A turn whose fork ran cold (no warm prefix reuse) is logged as degraded but
/// still parsed — correctness never depends on the cache hit. Returns `Err(())`
/// only when the response does not parse (propagated from [`parse_task_response`]).
async fn handle_fork_success(
    turn: SessionTurn,
    name: &str,
    files: &[String],
    pool: &AgentPool,
) -> Result<Vec<Finding>, ()> {
    let reuse = classify_reuse(turn.fork, turn.cache_usage);
    tracing::info!(
        validator = %name,
        files = ?files,
        session = %turn.session_id,
        reuse = reuse.label(),
        reused_tokens = ?reuse.reused_tokens(),
        cache_read_input_tokens = ?reuse.cache_read(),
        cache_creation_input_tokens = ?reuse.cache_created(),
        "fleet task prefix reuse"
    );
    if matches!(reuse, PrefixReuse::Cold) {
        tracing::warn!(
            validator = %name,
            files = ?files,
            session = %turn.session_id,
            "fleet task fork was degraded (no warm prefix reuse); proceeding cold"
        );
    }
    let findings = parse_task_response(&turn.content, name, files)?;
    Ok(sweep_until_dry(pool, &turn.session_id, name, files, findings).await)
}

/// The fork-failed arm of [`collect_forked_task`]: the `session/fork` call failed,
/// so the validator never ran on the primed prefix. Fall back to a monolithic
/// fresh-session prompt for the validator — degraded (cold, no shared prime) but
/// correct; a fork failure must never lose a task.
async fn handle_fork_failed(
    parent_session_id: String,
    message: String,
    change_purpose: &str,
    validator: &ValidatorWork,
    ruleset: &RuleSet,
    files: &[String],
    pool: &AgentPool,
) -> Result<Vec<Finding>, ()> {
    let name = validator.validator_name.as_str();
    tracing::warn!(
        validator = %name,
        files = ?files,
        parent = %parent_session_id,
        error = %message,
        "fleet task fork failed; falling back to a monolithic fresh-session prompt"
    );
    let prompt = render_fleet_prompt(change_purpose, validator, ruleset);
    collect_task(pool.submit(prompt).await, name, files)
}

/// The failure arm of the task collectors ([`collect_forked_task`] /
/// [`collect_task`]): a task that failed for any reason other than a fork
/// failure — a pool error (idle/ceiling abandonment, an extension failure, or an
/// agent error) or a dropped result channel. Logged with `message` (and the
/// `error` field when the failure carried one — a [`PoolError`], absent for a
/// dropped delivery) and degraded to zero findings — one bad task never aborts
/// the rest — returning `Err(())` so the caller tallies it as failed rather than
/// conflating it with a clean validator.
fn handle_task_failure(
    name: &str,
    files: &[String],
    error: Option<&PoolError>,
    message: &str,
) -> Result<Vec<Finding>, ()> {
    tracing::warn!(
        validator = %name,
        files = ?files,
        error = error.map(tracing::field::display),
        "{message}"
    );
    Err(())
}

/// Drive a validator's review session forward with a repeated "any more?"
/// follow-up until it goes dry, merging every additional finding into `findings`.
///
/// Per-pass recall is low: a small model anchors on the salient match and
/// under-reports the other instances of a rule on its first pass, even under the
/// whole-file [`OUTPUT_CONTRACT`]. More admonishment text does not beat the
/// anchoring; the fix is structural. After the first pass returned `findings`,
/// this tacks [`FOLLOWUP_PROMPT`] onto the SAME accumulating session — "you've
/// listed these; report any ADDITIONAL violations you have not already named" —
/// and repeats that nudge, terminating when the model itself answers with an
/// empty array (it is the authority on "found them all").
///
/// The loop is **forward-driving, not a re-fork of the first pass**. Each turn
/// forks the session that produced the PRIOR follow-up answer
/// ([`SessionTurn::session_id`]), so the model's own accumulated answers are in
/// context and "additional" means additional-to-everything-said-so-far. Were it
/// to re-fork the first-pass session every iteration, each nudge would only see
/// the first-pass findings and re-report them — it would oscillate, never go dry.
///
/// Termination is the empty turn OR the [`MAX_FOLLOWUP_SWEEPS`] runaway cap;
/// both are logged. It only ever ADDS: an empty first pass spends zero follow-up
/// turns, and a follow-up that fork-fails, errors, returns nothing, or does not
/// parse ends the loop while keeping every finding gathered so far. Downstream
/// [`dedup_exact`] collapses any exact repeat, so a model that re-lists something
/// is harmless rather than a convergence breaker.
///
/// [`dedup_exact`]: crate::review::synthesize
async fn sweep_until_dry(
    pool: &AgentPool,
    parent_session: &SessionId,
    validator: &str,
    files: &[String],
    findings: Vec<Finding>,
) -> Vec<Finding> {
    // Nothing reported → nothing to be incomplete about; do not spend a turn.
    if findings.is_empty() {
        return findings;
    }
    let mut merged = findings;
    // The session each follow-up turn forks from: the first pass, then the
    // session that delivered the previous follow-up answer. Driving this forward
    // is what makes "additional, not already listed" well-defined.
    let mut session = parent_session.clone();
    for sweep in 1..=MAX_FOLLOWUP_SWEEPS {
        let delivered = pool
            .submit_forked(&session, FOLLOWUP_PROMPT.to_string())
            .await;
        let Ok(Ok(turn)) = delivered else {
            tracing::debug!(
                validator = %validator,
                files = ?files,
                sweep,
                "fleet follow-up sweep unavailable; ending the loop with the findings gathered so far"
            );
            return merged;
        };
        let Ok(additional) = parse_task_response(&turn.content, validator, files) else {
            return merged;
        };
        if additional.is_empty() {
            tracing::info!(
                validator = %validator,
                files = ?files,
                sweep,
                "fleet follow-up sweep went dry; the model reports no further instances"
            );
            return merged;
        }
        tracing::info!(
            validator = %validator,
            files = ?files,
            sweep,
            added = additional.len(),
            "fleet follow-up sweep recovered further instances on the first review"
        );
        merged.extend(additional);
        // Drive the SAME session forward: the next nudge forks the session that
        // just answered, so it sees its own accumulated findings — never a
        // re-fork of the first pass, which would re-report and never go dry.
        session = turn.session_id;
    }
    tracing::info!(
        validator = %validator,
        files = ?files,
        cap = MAX_FOLLOWUP_SWEEPS,
        "fleet follow-up sweep hit the runaway cap without going dry; keeping the gathered findings"
    );
    merged
}

/// Resolve one task's delivered result into tagged findings.
///
/// Returns `Ok(findings)` for a task that delivered a parseable response (the
/// findings may legitimately be empty), and `Err(())` for any failure — a task
/// error, a dropped channel, or a response that did not parse. A failure is
/// logged and degrades the validator to zero findings (one bad task never aborts
/// the rest); the `Err` lets the caller tally it as failed rather than silently
/// conflating it with a clean validator. `files` are the validator's files the
/// failure is attributed to in the log.
fn collect_task(
    delivered: Result<crate::validators::PromptResult, tokio::sync::oneshot::error::RecvError>,
    validator: &str,
    files: &[String],
) -> Result<Vec<Finding>, ()> {
    let response = match delivered {
        Ok(Ok(response)) => response,
        Ok(Err(err)) => {
            return handle_task_failure(
                validator,
                files,
                Some(&err),
                "fleet task failed; yielding zero findings for this validator",
            )
        }
        Err(_) => {
            return handle_task_failure(
                validator,
                files,
                None,
                "fleet task result was dropped before delivery; yielding zero findings",
            )
        }
    };

    // A monolithic task runs on a fresh session (no fork), so any reuse is
    // hosted-cache only; classify with `fork = None` and log it so the cold
    // fallback path also reports cache usage.
    let reuse = classify_reuse(None, response.cache_usage);
    tracing::info!(
        validator = %validator,
        files = ?files,
        reuse = reuse.label(),
        cache_read_input_tokens = ?reuse.cache_read(),
        cache_creation_input_tokens = ?reuse.cache_created(),
        "fleet monolithic task prefix reuse"
    );
    parse_task_response(&response.content, validator, files)
}

/// Parse one task's response text into validator-tagged findings, degrading an
/// unparseable response to a logged failure — shared by the monolithic and
/// forked collection paths so both parse identically.
fn parse_task_response(
    content: &str,
    validator: &str,
    files: &[String],
) -> Result<Vec<Finding>, ()> {
    match parse_findings(content) {
        Ok(parsed) => Ok(tag_findings(parsed, validator)),
        Err(err) => {
            tracing::warn!(
                validator = %validator,
                files = ?files,
                error = %err,
                "fleet task response did not parse into findings; yielding zero findings"
            );
            Err(())
        }
    }
}

/// Tag every finding with its source `validator` name, overriding whatever the
/// agent emitted so the validator attribution is always authoritative.
fn tag_findings(mut findings: Vec<Finding>, validator: &str) -> Vec<Finding> {
    for finding in &mut findings {
        finding.validator = validator.to_string();
    }
    findings
}

/// Render the fan-out prompt for one validator task — the monolithic fallback
/// shape (one fresh session, everything for the validator in one prompt).
///
/// Self-contained and scoped exactly as the old per-validator prompt was: the
/// change purpose, that validator's own files (path + semantic diff + bounded
/// source slice + probe evidence), and the validator's instructions (mandate +
/// every rule body + output contract). It is the cold fallback
/// when priming or this validator's fork fails — correct, just not warm.
///
/// The warm path splits the run's large shared content into the run prime
/// ([`render_run_prime`], every file, primed once) and per-validator forks
/// ([`render_validator_suffix`], one full ruleset each). The fallback re-renders
/// both halves for the validator in one prompt, so a degraded task is
/// byte-for-byte the same review of the validator against its files — only the
/// session reuse differs.
///
/// `validator` is the work-list entry (its name and the file work); `ruleset` is
/// the same validator's loaded [`RuleSet`], the authoritative source of the
/// mandate (its description) and the verbatim rule bodies.
pub fn render_fleet_prompt(
    change_purpose: &str,
    validator: &ValidatorWork,
    ruleset: &RuleSet,
) -> String {
    let mut out = String::new();
    out.push_str("# Change purpose\n\n");
    out.push_str(change_purpose.trim());
    out.push_str("\n\n");
    out.push_str(&render_file_payload(&validator.files));
    out.push_str(&render_validator_suffix(validator, ruleset));
    out
}

/// The sentence the prime turn ends with: an explicit completed-turn handoff so
/// the parent session's end-of-turn KV snapshot lands exactly at the boundary
/// every fork's validator prompt continues from.
///
/// Crate-visible so the scripted test agent (`review::test_support`) recognizes
/// prime turns by this exact constant rather than a re-typed fragment — the
/// handoff wording changes in exactly one place.
pub(crate) const PRIME_HANDOFF: &str =
    "Reply with exactly OK. The rules to review against arrive in the next message.\n";

/// Render the run's shared primed prefix the prime turn decodes ONCE per review
/// run: the change purpose + every distinct file under review (path + semantic
/// diff + bounded source slice + probe evidence), ending with [`PRIME_HANDOFF`].
///
/// This is the large content shared across every validator — the diffs are primed
/// and cached once, never re-sent per validator. It carries NO validator-specific
/// text; the validator's rules arrive on each fork as [`render_validator_suffix`].
/// Files are de-duplicated by path (a file matched by several validators is
/// inlined once), so the prime stays a single rendering of the whole change.
///
/// The render is a pure function of its inputs — byte-stable across calls — so
/// every validator fork of the primed session shares the exact prefix bytes the
/// parent decoded, and the fork's first decode reuses the full saved state.
pub fn render_run_prime(work: &WorkList) -> String {
    let mut out = String::new();
    out.push_str("# Change purpose\n\n");
    out.push_str(work.change_purpose.trim());
    out.push_str("\n\n");
    let distinct: Vec<FileWork> = work.distinct_files().cloned().collect();
    out.push_str(&render_file_payload(&distinct));
    out.push_str(PRIME_HANDOFF);
    out
}

/// The line that opens every per-validator suffix: `# Validator: ` immediately
/// followed by the validator name. The single source of truth shared by
/// [`render_validator_suffix`] and the tests that key scripts/assertions on the
/// header, so a format change lands in one place.
pub(crate) const VALIDATOR_HEADER: &str = "# Validator: ";

/// The mandate section header that follows the validator line in every suffix.
/// Shared by [`render_validator_suffix`] and the header-keyed tests so the
/// format stays synchronized in one place.
pub(crate) const MANDATE_HEADER: &str = "## Mandate\n\n";

/// Render the per-validator suffix a forked session is prompted with: the
/// validator header, mandate, the files this validator must focus on, every one
/// of the validator's rule bodies, and the output contract.
/// The files' contents are already in the fork's inherited prime; only their
/// paths are named here so the validator stays scoped to its matched files (not
/// every file in the prime), without re-sending any diff.
///
/// Always non-empty: it carries at least the rule bodies and the output contract,
/// so a fork turn never degenerates to a full reprocess (`lcp == new_len`).
pub fn render_validator_suffix(validator: &ValidatorWork, ruleset: &RuleSet) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{VALIDATOR_HEADER}{}\n", validator.validator_name);
    out.push_str(MANDATE_HEADER);
    out.push_str(ruleset.description().trim());
    out.push_str("\n\n");

    render_validator_guidance(&mut out, ruleset.manifest_body());

    render_focus_files(&mut out, &validator.files);

    out.push_str("## Rules\n\n");
    for rule in &ruleset.rules {
        let _ = writeln!(out, "### Rule: {}\n", rule.name);
        out.push_str(rule.body.trim());
        out.push_str("\n\n");
    }

    out.push_str(OUTPUT_CONTRACT);
    out.push('\n');
    out
}

/// Append the validator's VALIDATOR.md prose body as a validator-level guidance
/// block, emitted between the [`MANDATE_HEADER`] (the description) and `## Rules`
/// so it is shared by every rule in the validator's fan-out.
///
/// This is authored validator-WIDE direction — intent, scope, and blanket
/// exclusions that apply across all of a validator's rules (e.g. "this validator
/// does not apply to test code"). An empty body emits nothing, keeping the render
/// byte-identical for validators that carry no body (the fork-prefix reuse
/// contract depends on this render being a pure function of its inputs).
fn render_validator_guidance(out: &mut String, body: &str) {
    let body = body.trim();
    if body.is_empty() {
        return;
    }
    out.push_str("## Guidance\n\n");
    out.push_str(body);
    out.push_str("\n\n");
}

/// Append the "files in scope for this validator" list: the paths of the
/// validator's matched files. The contents are in the inherited prime; this just
/// scopes the validator to those files so it does not flag files another
/// validator matched.
fn render_focus_files(out: &mut String, files: &[FileWork]) {
    out.push_str(
        "## Files in scope\n\nApply the rules below to the WHOLE current contents of each \
         file listed here — their complete current source is already provided above. Review \
         every line of these files, not only the lines the change touched: a rule that fires \
         anywhere in one of these files is in scope and must be reported now.\n\n",
    );
    for file in files {
        let _ = writeln!(out, "- `{}`", file.path);
    }
    out.push('\n');
}

/// Render the file payload — one self-contained block per file (path + semantic
/// diff + bounded source slice + probe evidence). Used by the run prime (every
/// distinct file) and the monolithic fallback (one validator's files).
pub fn render_file_payload(files: &[FileWork]) -> String {
    let mut out = String::new();
    out.push_str("# Files under review\n\n");
    for file in files {
        render_file_block(&mut out, file);
    }
    out
}

/// The finding output contract, shared verbatim by every fan-out prompt.
///
/// It instructs the agent to emit a JSON array of findings, each carrying the
/// four load-bearing fields the [`Finding`] type and the verify stage require:
/// `rule`, `claim` (what + why it matters), `evidence` (a cited probe proof), and
/// `suggestion` (the fix).
///
/// The contract is explicit that the reply must be the JSON array as plain
/// message text and that tools must NOT be called: review sessions still
/// advertise the agent's intrinsic tools, and without this instruction small
/// models deliver their findings as a hallucinated tool call (e.g. invoking the
/// validator name as a tool), leaving the parsed message empty and failing the
/// task.
const OUTPUT_CONTRACT: &str = "\
## Reading files

The changed files under review are already provided in full — their \
COMPLETE current contents are inlined above, so do NOT `read_file` (or `glob`/`grep`) \
the changed files; you already have them. `read_file`/`glob`/`grep` remain \
available, but only for OTHER files: cross-file duplication evidence, a changed \
symbol's callers, or a type defined elsewhere. Reach for them only when a \
finding genuinely depends on a file that is not already inlined here.

## Review scope

The review boundary is the WHOLE current file, not the changed lines. Each changed \
file is inlined above in full; review every line of it. Pre-existing instances of a \
rule — ones that were already there before this change, anywhere in a changed file — \
are in scope and must be reported now, in this same pass, alongside instances in the \
changed region. The \"What changed\" semantic diff is orientation only: it tells you \
what this change touched, it is NOT the review boundary and NOT where to limit your \
search. Do not treat the diff as the review boundary.

## Output contract

Once you have reviewed the inlined files in full (reading other files only if needed), \
reply with your findings as a JSON array, written directly as the plain text of \
your reply — the reply is parsed as JSON. The findings reply itself must be \
plain JSON text, never a tool call: a tool call is not a valid way to report \
findings.

Each finding is one object with these fields:

- `file`: the path of the file the finding is about.
- `line`: the 1-based line number the finding points at.
- `rule`: which rule of this validator fired.
- `claim`: what is wrong AND why it matters — one concern per finding.
- `evidence`: the proof the issue is real — cite the injected probe result \
(e.g. \"per `duplicates`: 0.94 at `bar.rs:88`\") or a `file:line` citation.
- `suggestion`: the fix.

Report every occurrence of every rule that fires, in this single pass — across the \
WHOLE file, not just the changed lines: when a rule matches on several lines, emit a \
separate finding for each match — one finding per `file:line`. Do not stop at the \
first match and do not collapse repeated matches into one finding; list them ALL, \
including pre-existing instances that sit outside the changed region, so the whole \
file can be fixed in one go rather than re-reviewed match by match.

Report only real issues. If you find none, emit an empty array `[]`.
";

/// The hard cap on follow-up sweep turns [`sweep_until_dry`] drives after a
/// validator's first pass, before it gives up on the model going dry on its own.
///
/// The loop normally terminates when the model itself answers a follow-up with
/// an empty array; this cap is only the runaway backstop for a model that never
/// says "none left". Set small (a few turns): each turn is a cheap warm-session
/// delta, but the recall gain falls off fast, and the next `/finish` round plus
/// downstream [`dedup_exact`] still backstop anything not recovered here.
///
/// [`dedup_exact`]: crate::review::synthesize
const MAX_FOLLOWUP_SWEEPS: u32 = 4;

/// The follow-up "any more?" prompt [`sweep_until_dry`] tacks onto a validator's
/// review session, repeated each sweep until the model goes dry.
///
/// Small models under-report instances of a rule on the first pass even under the
/// whole-file [`OUTPUT_CONTRACT`] — they anchor on the salient match, and more
/// admonishment text does not beat the anchoring. So instead of one re-ask, the
/// session is driven forward conversationally: each turn runs on the session that
/// already holds the model's OWN accumulated answers, so "additional, not already
/// listed" is well-defined and the loop can actually go dry. The same prompt is
/// re-sent every sweep — its meaning shifts because the context (the prior
/// answers) grows under it.
///
/// It must NOT contain [`PRIME_HANDOFF`] (so the turn is treated as a real review
/// turn, not a prime), and its `## Completeness re-scan` header is the stable
/// marker the fan-out logs and tests key on.
const FOLLOWUP_PROMPT: &str = "\
## Completeness re-scan

You just reported your findings for these files. Before we finish, scan the SAME \
files again — their full current contents are already provided above — and report \
any ADDITIONAL violations of the same rules that you have NOT already named: \
pre-existing matches outside the changed region, or further lines the same rule \
fires on. This is a within-file completeness sweep of the whole file, not a new \
review.

Reply with ONLY the additional, not-already-listed findings, as a JSON array in \
the exact same object shape as before (`file`, `line`, `rule`, `claim`, \
`evidence`, `suggestion`), written directly as the plain text of your reply — \
never a tool call. If you have now named every instance and none remain, reply \
with an empty array `[]`.
";

/// Append one file's review block: path, the full current source, the semantic
/// diff of what changed, and the probe results rendered as evidence.
///
/// The changed file is always handed to the model **in full** — framed explicitly
/// as the complete current contents the model does NOT need to re-read, because
/// the read-round-trips that dominated review wall-clock came from the model
/// re-reading a file it was only given a partial slice of. A file too large for
/// the review `batch_size` never reaches here as a partial view: the scope stage
/// rejects it with a hard error rather than trimming it to a slice.
fn render_file_block(out: &mut String, file: &FileWork) {
    let _ = writeln!(out, "## File: {}\n", file.path);

    out.push_str(
        "### Full current contents\n\n\
         This is the COMPLETE current source of the file. You do not need to read this \
         file — it is provided here in full. Review it directly. This whole file is the \
         review boundary: report every place a rule fires anywhere in it, including \
         pre-existing instances that sit outside the change described below.\n\n",
    );
    out.push_str("```\n");
    out.push_str(file.source_slice.trim_end());
    out.push_str("\n```\n\n");

    out.push_str(
        "### What changed (semantic diff — orientation only, NOT the review boundary)\n\n",
    );
    out.push_str(
        "The entities below are what this change touched, to orient you. They are context, \
         not the review scope: do NOT limit findings to these lines. Review the whole file \
         above and report every instance of every rule, changed or pre-existing.\n\n",
    );
    render_semantic_diff(out, file);

    out.push_str("### Probe evidence\n\n");
    render_probe_evidence(out, &file.probe_results, false);
}

/// Append the structured semantic diff for a file as a list of changed entities.
fn render_semantic_diff(out: &mut String, file: &FileWork) {
    if file.semantic_diff.is_empty() {
        out.push_str("_No structured entity changes._\n\n");
        return;
    }
    for change in &file.semantic_diff {
        let _ = writeln!(
            out,
            "- {} {} `{}`",
            change.change_type, change.entity_type, change.entity_name
        );
    }
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;
    use std::sync::Arc;

    use serde_json::json;
    use swissarmyhammer_sem::model::change::{ChangeType, SemanticChange};

    use crate::review::probes::{ProbeKind, ProbeResult, ProbeRow};
    use crate::review::scope::WorkList;
    use crate::review::test_support::{
        findings_json, with_pool, ForkMode, ScriptedAgent, ScriptedAgentConfig, ScriptedReply,
        MOCK_PREFIX_TOKENS,
    };
    use crate::validators::types::{
        Rule, RuleSet, RuleSetManifest, RuleSetMetadata, ValidatorMatch,
    };
    use crate::validators::{PoolConfig, ValidatorLoader, ValidatorSource};
    use claude_agent::protocol_translator::CacheUsage;

    // ---- fixtures --------------------------------------------------------

    /// The 1-based source line every scripted finding fixture points at. The
    /// exact value is immaterial to these tests (none assert on the line); naming
    /// it keeps the fixtures from sprinkling an unexplained literal.
    const TEST_FINDING_LINE: u32 = 42;

    /// The 1-based line the shared `file_work` fixture's `duplicates` probe row
    /// cites. Like [`TEST_FINDING_LINE`] the exact value is immaterial; naming it
    /// keeps the hidden fixture constant out of the probe row and its assertions.
    const TEST_PROBE_LINE: u32 = 88;

    /// The similarity score the shared `file_work` fixture's `duplicates` probe
    /// row reports. Like [`TEST_PROBE_LINE`] the exact value is immaterial; naming
    /// it keeps the score out of the fixture, the agent-output helper, and the
    /// rendered-prompt assertion so all three stay locked to one number. Rendered
    /// with `{:.2}` (matching the production probe formatting) wherever it appears
    /// as text.
    const TEST_SIMILARITY: f32 = 0.94;

    /// A RuleSet whose mandate (description) and rule bodies are distinctive so
    /// the rendered prompt can be asserted against them verbatim. Carries no
    /// VALIDATOR.md body — use [`ruleset_with_body`] when the body matters.
    fn ruleset(name: &str, mandate: &str, rules: &[(&str, &str)]) -> RuleSet {
        ruleset_with_body(name, mandate, "", rules)
    }

    /// Like [`ruleset`] but with a distinctive VALIDATOR.md prose `body` so the
    /// rendered prompt can be asserted against the validator-wide guidance block.
    fn ruleset_with_body(
        name: &str,
        mandate: &str,
        body: &str,
        rules: &[(&str, &str)],
    ) -> RuleSet {
        RuleSet {
            manifest: RuleSetManifest {
                name: name.to_string(),
                description: mandate.to_string(),
                metadata: RuleSetMetadata {
                    version: "1.0.0".to_string(),
                },
                match_criteria: Some(ValidatorMatch {
                    tools: vec![],
                    files: vec!["*.rs".to_string()],
                }),
                trigger_matcher: None,
                tags: vec![],
                probes: vec![],
                timeout: 30,
                once: false,
            },
            rules: rules
                .iter()
                .map(|(rname, body)| Rule {
                    name: rname.to_string(),
                    description: format!("{rname} description"),
                    body: body.to_string(),
                    timeout: None,
                })
                .collect(),
            manifest_body: body.to_string(),
            source: ValidatorSource::Builtin,
            base_path: PathBuf::from("/test"),
        }
    }

    /// A loader carrying the given rulesets, matched by name in `run_fleet`.
    fn loader_with(rulesets: Vec<RuleSet>) -> ValidatorLoader {
        let mut loader = ValidatorLoader::new();
        for rs in rulesets {
            loader.add_builtin_ruleset(rs);
        }
        loader
    }

    /// A `FileWork` carrying a distinctive added entity, a source slice tagged
    /// with the path, and one `duplicates` probe row.
    fn file_work(path: &str, symbol: &str, dup_at: &str) -> FileWork {
        FileWork {
            path: path.to_string(),
            semantic_diff: vec![SemanticChange {
                id: format!("{path}:{symbol}"),
                entity_id: symbol.to_string(),
                change_type: ChangeType::Added,
                entity_type: "function".to_string(),
                entity_name: symbol.to_string(),
                file_path: path.to_string(),
                old_file_path: None,
                before_content: None,
                after_content: Some(format!("fn {symbol}() {{}}")),
                commit_sha: None,
                author: None,
                timestamp: None,
                structural_change: None,
            }],
            changed_symbols: vec![symbol.to_string()],
            source_slice: format!("// slice for {path}\nfn {symbol}() {{}}"),
            probe_results: vec![ProbeResult {
                name: "duplicates".to_string(),
                kind: ProbeKind::Fact,
                target: path.to_string(),
                rows: vec![ProbeRow {
                    file_path: dup_at.to_string(),
                    symbol: Some(symbol.to_string()),
                    line: Some(TEST_PROBE_LINE),
                    similarity: Some(TEST_SIMILARITY),
                    detail: None,
                }],
            }],
        }
    }

    fn validator_work(name: &str, files: Vec<FileWork>) -> ValidatorWork {
        ValidatorWork {
            validator_name: name.to_string(),
            rules: vec![format!("{name}-rule")],
            probes: vec!["duplicates".to_string()],
            files,
        }
    }

    // ---- scripted mock agent (shared harness) ------------------------------
    //
    // The scripted ACP agent lives in `crate::review::test_support` — one
    // implementation shared with verify.rs, drive.rs, and the pool tests.
    // Fleet tests run it with the fork extension `Supported` unless a test
    // selects a degraded `ForkMode` explicitly.

    /// A fork-capable scripted agent — the default fleet backend under test.
    /// The [`ForkMode::Supported`] special case of [`agent_with_fork_mode`].
    fn forking_agent(script: Vec<(String, ScriptedReply)>) -> Arc<ScriptedAgent> {
        agent_with_fork_mode(script, ForkMode::Supported)
    }

    /// A scripted agent in the given [`ForkMode`] (the default fleet config
    /// otherwise).
    fn agent_with_fork_mode(
        script: Vec<(String, ScriptedReply)>,
        fork_mode: ForkMode,
    ) -> Arc<ScriptedAgent> {
        ScriptedAgent::with_config(
            script,
            ScriptedAgentConfig {
                fork_mode,
                ..ScriptedAgentConfig::default()
            },
        )
    }

    /// The stable header [`FOLLOWUP_PROMPT`] carries — only a follow-up sweep
    /// turn sends it, so a script entry keyed on it matches a sweep fork's
    /// context and never the first-pass prompt.
    const RESCAN_NEEDLE: &str = "## Completeness re-scan";

    /// Broadcast-channel capacity for a rebind's notification stream. A small
    /// buffer is plenty here: these single-prompt rebinds emit one reply each,
    /// well under capacity, so the subscriber never lags chunks away.
    const BROADCAST_BUFFER_SIZE: usize = 8;

    /// A scripted follow-up reply that finds nothing further, going dry on the
    /// first sweep. Every warm fork now drives at least one follow-up sweep after
    /// its first pass; a test asserting unchanged first-pass behavior scripts the
    /// first sweep to add nothing so the loop terminates immediately. Keyed on
    /// [`RESCAN_NEEDLE`] and ordered FIRST so it wins on the sweep fork's context
    /// (which also inherits the first-pass needles).
    fn rescan_finds_nothing() -> (String, ScriptedReply) {
        (
            RESCAN_NEEDLE.to_string(),
            ScriptedReply::Text("[]".to_string()),
        )
    }

    /// Two independent rebinds of one base agent must NOT share a
    /// [`ScriptedReply::Sequence`] queue — each rebind is a "fresh agent", so
    /// consuming the sequence on one must leave the other's untouched.
    ///
    /// `rebind_broadcast` deep-clones the script, so each rebind gets its own
    /// queue and a prompt matching the sequence needle yields the SAME first delta
    /// on both. With a shallow `Arc` share (the pre-fix bug), the first rebind's
    /// prompt would pop the queue and the second would see the drained tail — a
    /// silent cross-rebind test-isolation leak.
    #[tokio::test]
    async fn rebinds_do_not_share_sequence_state() {
        const NEEDLE: &str = "consume the sequence";
        let base = forking_agent(vec![(
            NEEDLE.to_string(),
            ScriptedReply::sequence(["first-delta".to_string(), "second-delta".to_string()]),
        )]);

        // Each rebind submits one prompt matching the sequence needle and reads
        // back which delta it served.
        async fn first_served(base: &Arc<ScriptedAgent>) -> String {
            let (tx, _) = tokio::sync::broadcast::channel(BROADCAST_BUFFER_SIZE);
            // Bridge onto the live connection too, so the pool's connection-side
            // collector (the stream `with_pool` wires up) sees the reply.
            let rebind = ScriptedAgent::rebind_broadcast(base, tx, true);
            with_pool(rebind, PoolConfig::remote(1), |pool| async move {
                let result = pool
                    .submit(format!("please {NEEDLE} now"))
                    .await
                    .expect("result")
                    .expect("ok");
                result.content
            })
            .await
        }

        let one = first_served(&base).await;
        let two = first_served(&base).await;
        assert_eq!(
            one, two,
            "each rebind has its own sequence queue, so both serve the first delta; \
             a shared queue would drain across rebinds and they would diverge"
        );
        assert!(
            one.contains("first-delta"),
            "a fresh rebind serves the sequence's first delta, got: {one}"
        );
    }

    /// A findings array of N objects as an agent emits it, fenced in prose — the
    /// multi-instance shape `findings_json` (a single finding) does not cover.
    /// Each tuple is `(file, line, rule, claim)`.
    fn findings_array_json(items: &[(&str, u32, &str, &str)]) -> String {
        // Built through `serde_json` so any `"`/`\` in a field is escaped
        // correctly — a raw `format!` template would corrupt the JSON.
        let objects: Vec<serde_json::Value> = items
            .iter()
            .map(|(file, line, rule, claim)| {
                json!({
                    "file": file,
                    "line": line,
                    "validator": "ignored-by-agent",
                    "rule": rule,
                    "claim": claim,
                    "evidence": format!("per `duplicates`: {TEST_SIMILARITY:.2}"),
                    "suggestion": "extract a helper",
                })
            })
            .collect();
        let array = json!(objects);
        format!("Here are my findings:\n\n```json\n{array}\n```\n")
    }

    #[test]
    fn findings_array_json_escapes_embedded_quotes() {
        // A claim carrying a double quote must round-trip through valid JSON,
        // proving the helper escapes rather than concatenates raw text.
        let claim = r#"the literal "7" is a magic number"#;
        let fenced = findings_array_json(&[("src/a.rs", TEST_FINDING_LINE, "no-magic", claim)]);
        let body = fenced
            .split("```json")
            .nth(1)
            .and_then(|s| s.split("```").next())
            .expect("fenced JSON block")
            .trim();
        let parsed: serde_json::Value =
            serde_json::from_str(body).expect("findings_array_json is valid JSON");
        assert_eq!(parsed[0]["claim"], json!(claim));
        assert_eq!(parsed[0]["file"], json!("src/a.rs"));
    }

    /// Run the fleet and then release its shared-prime pin, exactly as
    /// `run_review` drives the prime lifecycle (fan-out primes once, the caller
    /// unpins when the run drains). The returned outcome has its `prime` cleared
    /// so the orchestrator tests can assert the full pin→unpin cycle while the
    /// pool/connection is still live.
    async fn run_fleet_and_unpin(
        work: &WorkList,
        loader: &ValidatorLoader,
        pool: &AgentPool,
    ) -> FleetOutcome {
        let outcome = run_fleet(work, loader, pool).await;
        if let Some(guard) = outcome.prime {
            unpin_prefix_session(guard).await;
        }
        FleetOutcome {
            prime: None,
            ..outcome
        }
    }

    // ---- config tests ----------------------------------------------------

    #[test]
    fn default_batch_size_is_128_kib() {
        // The default budget clears the largest single source file in a typical
        // change (~95 KB) so an ordinary commit reviews without tripping the
        // oversize-file error; only genuinely huge multi-file diffs still split.
        assert_eq!(DEFAULT_BATCH_SIZE, 128 * 1024);
        assert_eq!(DEFAULT_BATCH_SIZE, 131072);
        assert_eq!(FleetConfig::default().batch_size, DEFAULT_BATCH_SIZE);
    }

    // ---- renderer tests (pure) -------------------------------------------

    #[test]
    fn monolithic_prompt_contains_change_purpose_mandate_rules_and_output_contract() {
        let rs = ruleset(
            "deduplicate",
            "DEDUP_MANDATE: never copy-paste logic.",
            &[(
                "no-copy-paste",
                "RULE_BODY: extract shared helpers verbatim.",
            )],
        );
        let vw = validator_work(
            "deduplicate",
            vec![file_work("src/a.rs", "alpha", "src/x.rs")],
        );

        // The monolithic fallback for one validator: change purpose + the
        // validator's files + the validator's instructions (its full ruleset),
        // all in one self-contained prompt.
        let prompt = render_fleet_prompt("PURPOSE: scaffolding the parser.", &vw, &rs);

        assert!(
            prompt.contains("PURPOSE: scaffolding the parser."),
            "{prompt}"
        );
        assert!(
            prompt.contains("DEDUP_MANDATE: never copy-paste logic."),
            "{prompt}"
        );
        assert!(
            prompt.contains("RULE_BODY: extract shared helpers verbatim."),
            "rule body must appear verbatim: {prompt}"
        );
        // The validator's file is inlined (the cold fallback is self-contained).
        assert!(prompt.contains("## File: src/a.rs"), "{prompt}");
        assert!(prompt.contains("// slice for src/a.rs"), "{prompt}");
        // Output contract: the four load-bearing finding fields.
        assert!(prompt.contains("`rule`"), "{prompt}");
        assert!(prompt.contains("`claim`"), "{prompt}");
        assert!(prompt.contains("`evidence`"), "{prompt}");
        assert!(prompt.contains("`suggestion`"), "{prompt}");
        // Binary pass/fail: the contract carries no severity field at all.
        assert!(!prompt.contains("`severity`"), "{prompt}");
    }

    #[test]
    fn monolithic_prompt_renders_all_of_the_validators_rules() {
        // A multi-rule validator: the per-validator monolithic prompt carries
        // EVERY one of the validator's rules.
        let rs = ruleset(
            "deduplicate",
            "mandate",
            &[
                ("no-copy-paste", "FIRST_RULE_BODY"),
                ("prefer-reuse", "SECOND_RULE_BODY"),
            ],
        );
        let vw = validator_work(
            "deduplicate",
            vec![file_work("src/a.rs", "alpha", "src/dup_of_a.rs")],
        );

        let prompt = render_fleet_prompt("purpose", &vw, &rs);

        assert!(
            prompt.contains("FIRST_RULE_BODY"),
            "the validator's first rule body must appear: {prompt}"
        );
        assert!(
            prompt.contains("SECOND_RULE_BODY"),
            "the validator's second rule body must also appear: {prompt}"
        );
        // The validator's file, slice, and probe evidence are present.
        assert!(prompt.contains("// slice for src/a.rs"), "{prompt}");
        assert!(
            prompt.contains("probe `duplicates`"),
            "probe evidence must be rendered: {prompt}"
        );
        assert!(
            prompt.contains(&format!("src/dup_of_a.rs:{TEST_PROBE_LINE}")),
            "{prompt}"
        );
        assert!(
            prompt.contains(&format!("@ {TEST_SIMILARITY:.2}")),
            "{prompt}"
        );
    }

    /// The run prime carries the change + every diff and NOT any validator text;
    /// the per-validator suffix carries that validator's full ruleset and NOT any
    /// file content. Both renders are byte-stable so every fork shares the exact
    /// primed prefix.
    #[test]
    fn run_prime_holds_change_and_diffs_only_and_validator_suffix_holds_the_full_ruleset() {
        let rs = ruleset(
            "deduplicate",
            "DEDUP_MANDATE: never copy-paste logic.",
            &[
                ("no-copy-paste", "RULE_BODY: extract shared helpers."),
                ("prefer-reuse", "OTHER_RULE_BODY: reuse first."),
            ],
        );
        let vw = validator_work(
            "deduplicate",
            vec![file_work("src/a.rs", "alpha", "src/x.rs")],
        );
        let work = WorkList {
            change_purpose: "PURPOSE: scaffolding the parser.".to_string(),
            validators: vec![vw.clone()],
        };

        // Byte-stable: two renders of the same inputs are identical, so every
        // validator fork shares the exact prefix the prime turn decoded.
        let prime = render_run_prime(&work);
        assert_eq!(
            prime,
            render_run_prime(&work),
            "the run prime render must be byte-stable across calls"
        );
        let suffix = render_validator_suffix(&vw, &rs);
        assert_eq!(suffix, render_validator_suffix(&vw, &rs));

        // The PRIME carries the change purpose and the file diff/source, ending
        // with the handoff — and carries NO validator text or contract.
        assert!(
            prime.contains("PURPOSE: scaffolding the parser."),
            "{prime}"
        );
        assert!(prime.contains("# Files under review"), "{prime}");
        assert!(prime.contains("## File: src/a.rs"), "{prime}");
        assert!(prime.contains("// slice for src/a.rs"), "{prime}");
        assert!(prime.contains("probe `duplicates`"), "{prime}");
        assert!(
            prime.ends_with(PRIME_HANDOFF),
            "the prime must end with the prime handoff: {prime}"
        );
        assert!(
            !prime.contains("DEDUP_MANDATE")
                && !prime.contains("RULE_BODY")
                && !prime.contains("## Output contract"),
            "the prime must carry NO validator text or contract: {prime}"
        );

        // The SUFFIX carries the validator + mandate + EVERY rule + contract,
        // and NOT the file's source contents (those live in the prime).
        assert!(
            suffix.contains(&format!("{VALIDATOR_HEADER}deduplicate")),
            "{suffix}"
        );
        assert!(suffix.contains("DEDUP_MANDATE"), "{suffix}");
        assert!(
            suffix.contains("RULE_BODY") && suffix.contains("OTHER_RULE_BODY"),
            "the suffix must carry ALL of the validator's rules: {suffix}"
        );
        assert!(suffix.contains("## Output contract"), "{suffix}");
        // The suffix names the focus file (path only) but never re-sends its
        // source — the cached prime already has it.
        assert!(
            suffix.contains("`src/a.rs`"),
            "the suffix must name the focus file path: {suffix}"
        );
        assert!(
            !suffix.contains("// slice for src/a.rs"),
            "the suffix must NOT re-send the file's source: {suffix}"
        );
        // Non-empty by construction — a fork turn never degenerates to a full
        // reprocess.
        assert!(
            !suffix.is_empty(),
            "the per-validator suffix must be non-empty"
        );

        // The monolithic fallback for the validator is self-contained: change +
        // validator's files + the validator suffix (path-scoped, contract, all
        // rules).
        let monolithic = render_fleet_prompt(&work.change_purpose, &vw, &rs);
        assert!(
            monolithic.contains("PURPOSE: scaffolding the parser."),
            "{monolithic}"
        );
        assert!(monolithic.contains("## File: src/a.rs"), "{monolithic}");
        assert!(monolithic.contains("// slice for src/a.rs"), "{monolithic}");
        assert!(monolithic.contains("RULE_BODY"), "{monolithic}");
        assert!(monolithic.contains("OTHER_RULE_BODY"), "{monolithic}");
        assert!(monolithic.ends_with(&suffix), "{monolithic}");
    }

    /// The validator's VALIDATOR.md prose body is folded into the per-validator
    /// suffix as a validator-wide guidance block, positioned AFTER the mandate
    /// (description) and BEFORE the rules so it is shared by every rule.
    #[test]
    fn validator_suffix_emits_the_manifest_body_after_mandate_before_rules() {
        let rs = ruleset_with_body(
            "duplication",
            "DEDUP_MANDATE: never copy-paste logic.",
            "This validator does not apply to test code.",
            &[("no-copy-paste", "RULE_BODY: extract shared helpers.")],
        );
        let vw = validator_work(
            "duplication",
            vec![file_work("src/a.rs", "alpha", "src/x.rs")],
        );

        let suffix = render_validator_suffix(&vw, &rs);

        // The body line appears verbatim in the suffix.
        assert!(
            suffix.contains("does not apply to test code"),
            "the validator body guidance must appear in the suffix: {suffix}"
        );

        // Ordering: mandate < body guidance < rules.
        let mandate_at = suffix
            .find("DEDUP_MANDATE")
            .expect("mandate must be present");
        let body_at = suffix
            .find("does not apply to test code")
            .expect("body must be present");
        let rules_at = suffix.find("## Rules").expect("rules header must be present");
        assert!(
            mandate_at < body_at,
            "the body must come AFTER the mandate: {suffix}"
        );
        assert!(
            body_at < rules_at,
            "the body must come BEFORE the rules: {suffix}"
        );
    }

    /// A validator with no VALIDATOR.md body emits no guidance block — the suffix
    /// is unchanged for body-less validators (the fork-prefix reuse contract
    /// depends on the render being a pure function of its inputs).
    #[test]
    fn validator_suffix_omits_guidance_when_body_is_empty() {
        let rs = ruleset(
            "duplication",
            "mandate",
            &[("no-copy-paste", "RULE_BODY")],
        );
        let vw = validator_work("duplication", vec![file_work("src/a.rs", "alpha", "src/x.rs")]);

        let suffix = render_validator_suffix(&vw, &rs);
        assert!(
            !suffix.contains("## Guidance"),
            "a body-less validator must emit no guidance block: {suffix}"
        );
    }

    /// The monolithic fallback shares the same `render_validator_suffix`, so the
    /// validator body guidance reaches the degraded path too.
    #[test]
    fn monolithic_prompt_contains_the_manifest_body_guidance() {
        let rs = ruleset_with_body(
            "duplication",
            "mandate",
            "This validator does not apply to test code.",
            &[("no-copy-paste", "RULE_BODY")],
        );
        let vw = validator_work(
            "duplication",
            vec![file_work("src/a.rs", "alpha", "src/x.rs")],
        );

        let prompt = render_fleet_prompt("purpose", &vw, &rs);
        assert!(
            prompt.contains("does not apply to test code"),
            "the validator body guidance must reach the monolithic fallback: {prompt}"
        );
    }

    /// The run prime de-duplicates files matched by several validators: a file
    /// in two validators' work appears ONCE in the cached prefix.
    #[test]
    fn run_prime_dedups_files_shared_across_validators() {
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![
                validator_work("val-a", vec![file_work("src/shared.rs", "s", "src/x.rs")]),
                validator_work("val-b", vec![file_work("src/shared.rs", "s", "src/x.rs")]),
            ],
        };

        let prime = render_run_prime(&work);
        assert_eq!(
            prime.matches("## File: src/shared.rs").count(),
            1,
            "a file matched by two validators is inlined once in the prime: {prime}"
        );
    }

    /// A small (fully-inlined) changed file's payload carries the file's
    /// COMPLETE current contents in a clearly-labeled fenced block plus explicit
    /// "you do NOT need to read this file" framing — so the model stops
    /// re-reading the changed file it was already handed.
    #[test]
    fn full_inline_payload_carries_complete_source_and_no_reread_framing() {
        // A FileWork whose source_slice is the WHOLE file, including a marker line
        // the old bounded slice would have trimmed.
        let mut file = file_work("src/a.rs", "alpha", "src/x.rs");
        file.source_slice =
            "use std::fmt;\n// distant_marker_kept_in_full\npub fn alpha() {}".to_string();

        let payload = render_file_payload(std::slice::from_ref(&file));

        // The complete source — including the distant marker — is present.
        assert!(
            payload.contains("// distant_marker_kept_in_full"),
            "full inline must carry every line of the file: {payload}"
        );
        // Explicit framing that the file is the complete contents and need not
        // be read.
        assert!(
            payload.to_lowercase().contains("full")
                && payload.to_lowercase().contains("do not need to read"),
            "the block must frame the source as the full file the model need not read: {payload}"
        );
        // The whole inlined file is the review boundary; the "What changed"
        // semantic diff is orientation only, NOT the review boundary — so the
        // model reviews every line, not just the changed region.
        let lower = payload.to_lowercase();
        assert!(
            lower.contains("whole file") || lower.contains("every line"),
            "the block must name the whole file as the review boundary: {payload}"
        );
        assert!(
            lower.contains("orientation only"),
            "the diff section must be framed as orientation only: {payload}"
        );
        assert!(
            lower.contains("not the review boundary"),
            "the diff section must be framed as NOT the review boundary: {payload}"
        );
    }

    /// The output contract scopes intrinsic reads to OTHER files (cross-file
    /// duplication, callers, type defs), not the changed files already inlined in
    /// full — while still leaving the tools advertised.
    #[test]
    fn output_contract_scopes_reads_to_other_files() {
        assert!(
            OUTPUT_CONTRACT.contains("other files"),
            "the contract must scope reads to other (cross-file) files: {OUTPUT_CONTRACT}"
        );
        // The changed files are provided in full — the contract says so.
        assert!(
            OUTPUT_CONTRACT.to_lowercase().contains("already provided")
                || OUTPUT_CONTRACT.to_lowercase().contains("provided in full"),
            "the contract must state the changed files are provided in full: {OUTPUT_CONTRACT}"
        );
    }

    /// The contract must demand reporting EVERY occurrence of every rule that
    /// fires in a single pass — one finding per `file:line`, never stopping at the
    /// first match. Bail-fast (find-one → fix → re-review) is the re-review token
    /// storm this contract exists to prevent.
    #[test]
    fn output_contract_demands_every_occurrence_with_no_bail_fast() {
        let lower = OUTPUT_CONTRACT.to_lowercase();
        assert!(
            lower.contains("every occurrence of every rule"),
            "the contract must demand every occurrence of every rule: {OUTPUT_CONTRACT}"
        );
        assert!(
            lower.contains("do not stop at the first"),
            "the contract must forbid stopping at the first match: {OUTPUT_CONTRACT}"
        );
        assert!(
            OUTPUT_CONTRACT.contains("one finding per `file:line`"),
            "the contract must require one finding per file:line: {OUTPUT_CONTRACT}"
        );
    }

    /// The contract must name the WHOLE current file as the review boundary and
    /// demote the semantic diff to orientation only — so a small model does not
    /// anchor on the changed region and under-report pre-existing instances
    /// elsewhere in the file (the finding-dribble this card kills).
    #[test]
    fn output_contract_names_the_whole_file_as_the_review_boundary_not_the_diff() {
        let lower = OUTPUT_CONTRACT.to_lowercase();
        assert!(
            OUTPUT_CONTRACT.contains("## Review scope"),
            "the contract must carry an explicit review-scope section: {OUTPUT_CONTRACT}"
        );
        assert!(
            lower.contains("whole current file"),
            "the contract must name the whole current file as the review boundary: \
             {OUTPUT_CONTRACT}"
        );
        assert!(
            lower.contains("pre-existing instances"),
            "the contract must put pre-existing instances in scope: {OUTPUT_CONTRACT}"
        );
        assert!(
            lower.contains("orientation only"),
            "the contract must frame the semantic diff as orientation only: {OUTPUT_CONTRACT}"
        );
        assert!(
            lower.contains("not the review boundary"),
            "the contract must state the diff is NOT the review boundary: {OUTPUT_CONTRACT}"
        );
    }

    // ---- orchestrator tests (scripted mock agent) ------------------------

    #[tokio::test]
    async fn fan_out_two_validators_two_files_submits_one_prime_and_one_fork_per_validator() {
        // Two validators over the same two files. Under the new grain — fork per
        // VALIDATOR, files in the shared prime — the run primes ONCE and forks ONE
        // task per validator: 2 validators = 2 forks, regardless of how many files
        // each validator reviews or how many rules it carries.
        let rs_a = ruleset("val-a", "mandate a", &[("ra", "body a")]);
        let rs_b = ruleset("val-b", "mandate b", &[("rb", "body b")]);
        let loader = loader_with(vec![rs_a, rs_b]);

        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![
                validator_work(
                    "val-a",
                    vec![
                        file_work("src/a.rs", "alpha", "src/x.rs"),
                        file_work("src/b.rs", "beta", "src/y.rs"),
                    ],
                ),
                validator_work(
                    "val-b",
                    vec![
                        file_work("src/a.rs", "alpha", "src/x.rs"),
                        file_work("src/b.rs", "beta", "src/y.rs"),
                    ],
                ),
            ],
        };

        // Script: a finding for each validator. The fork inherits the shared
        // prime (all files) and appends the validator suffix carrying the
        // validator header, so we key on that header.
        let agent = forking_agent(vec![
            // Each validator's first pass is exhaustive, so its completeness
            // re-scan finds nothing more — this test asserts the first-pass
            // fan-out shape (one prime + one fork per validator + one re-scan).
            rescan_finds_nothing(),
            (
                format!("{VALIDATOR_HEADER}val-a\n\n{MANDATE_HEADER}"),
                ScriptedReply::Text(findings_json(
                    "src/a.rs",
                    TEST_FINDING_LINE,
                    "ra",
                    "dup in a",
                )),
            ),
            (
                format!("{VALIDATOR_HEADER}val-b\n\n{MANDATE_HEADER}"),
                ScriptedReply::Text(findings_json(
                    "src/b.rs",
                    TEST_FINDING_LINE,
                    "rb",
                    "dup in b",
                )),
            ),
        ]);
        let agent_probe = Arc::clone(&agent);

        let findings = with_pool(agent, PoolConfig::remote(4), move |pool| async move {
            run_fleet(&work, &loader, &pool).await.findings
        })
        .await;

        let seen = agent_probe.seen_prompts();
        // Exactly ONE shared prime for the whole run (not one per validator).
        let primes = seen.iter().filter(|p| p.contains(PRIME_HANDOFF)).count();
        assert_eq!(
            primes, 1,
            "the run primes the shared prefix exactly once: {seen:#?}"
        );
        // One forked validator task per validator: 2 validators = 2 forks.
        let validator_tasks = seen
            .iter()
            .filter(|p| p.starts_with("# Validator:"))
            .count();
        assert_eq!(
            validator_tasks, 2,
            "one forked task per validator: {seen:#?}"
        );
        // Two validator forks PLUS one completeness re-scan fork each (the
        // re-scan inherits the validator session) = four forks total.
        assert_eq!(
            agent_probe.fork_count(),
            4,
            "one validator fork plus one completeness re-scan fork per validator"
        );

        // Every finding is tagged with its validator (overriding the agent's
        // self-reported `ignored-by-agent`), and the rule tag survives.
        let a = findings
            .iter()
            .find(|f| f.claim == "dup in a")
            .expect("val-a finding");
        assert_eq!(a.validator, "val-a");
        assert_eq!(a.rule.as_deref(), Some("ra"));
        let b = findings
            .iter()
            .find(|f| f.claim == "dup in b")
            .expect("val-b finding");
        assert_eq!(b.validator, "val-b");
        assert_eq!(b.rule.as_deref(), Some("rb"));
        assert!(
            findings.iter().all(|f| f.validator != "ignored-by-agent"),
            "the agent's self-reported validator must be overridden"
        );
    }

    /// A file containing several instances of ONE rule, touched by a single
    /// commit, must yield ALL of them on the FIRST review pass — the whole-file
    /// sweep, not a dribble of one-instance-per-re-review. Driven end-to-end
    /// through `run_fleet` with a scripted agent that reports every instance.
    #[tokio::test]
    async fn one_rule_with_many_instances_reports_them_all_on_the_first_pass() {
        let rs = ruleset(
            "magic-numbers",
            "no unexplained numeric literals",
            &[("no-magic", "name your constants")],
        );
        let loader = loader_with(vec![rs]);
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work(
                "magic-numbers",
                vec![file_work("src/a.rs", "alpha", "src/x.rs")],
            )],
        };

        // The agent reports several instances of the one rule across the whole
        // file in a single reply; its completeness re-scan then finds nothing
        // more. Each instance sits on its own line derived from TEST_FINDING_LINE
        // so the findings are distinct file:line instances, not a shared-line
        // collapse — the exact lines are immaterial.
        let instances = [
            ("src/a.rs", TEST_FINDING_LINE, "no-magic", "magic number 7"),
            (
                "src/a.rs",
                TEST_FINDING_LINE + 1,
                "no-magic",
                "magic number 13",
            ),
            (
                "src/a.rs",
                TEST_FINDING_LINE + 2,
                "no-magic",
                "magic number 99",
            ),
            (
                "src/a.rs",
                TEST_FINDING_LINE + 3,
                "no-magic",
                "magic number 256",
            ),
        ];
        let first_pass = findings_array_json(&instances);
        let agent = forking_agent(vec![
            rescan_finds_nothing(),
            (
                format!("{VALIDATOR_HEADER}magic-numbers\n\n{MANDATE_HEADER}"),
                ScriptedReply::Text(first_pass),
            ),
        ]);

        let findings = with_pool(agent, PoolConfig::remote(4), move |pool| async move {
            run_fleet(&work, &loader, &pool).await.findings
        })
        .await;

        let magic: Vec<_> = findings
            .iter()
            .filter(|f| f.rule.as_deref() == Some("no-magic"))
            .collect();
        assert_eq!(
            magic.len(),
            instances.len(),
            "all instances of the one rule must report on the first pass, \
             not dribble one per round: {findings:#?}"
        );
        assert!(
            magic.iter().all(|f| f.validator == "magic-numbers"),
            "every instance is tagged with its validator: {findings:#?}"
        );
    }

    /// A magic-numbers single-validator `WorkList` over one file — the shared
    /// setup for the follow-up-sweep tests, which all drive the loop on one
    /// validator and assert on what it surfaced and how many sweeps it took.
    fn magic_numbers_work() -> (ValidatorLoader, WorkList) {
        let rs = ruleset(
            "magic-numbers",
            "no unexplained numeric literals",
            &[("no-magic", "name your constants")],
        );
        let loader = loader_with(vec![rs]);
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work(
                "magic-numbers",
                vec![file_work("src/a.rs", "alpha", "src/x.rs")],
            )],
        };
        (loader, work)
    }

    /// The first-pass script entry: keyed on the validator header so it answers
    /// the first review turn (never a follow-up sweep, which carries the sweep
    /// header instead) with `findings`.
    fn first_pass_entry(findings: String) -> (String, ScriptedReply) {
        (
            "# Validator: magic-numbers".to_string() + "\n\n## Mandate",
            ScriptedReply::Text(findings),
        )
    }

    /// The sessions each follow-up sweep turn ran on, in order — the prompts
    /// carrying the sweep header, mapped to their session. The loop drives the
    /// session forward, so these must be a chain of DISTINCT sessions (one fresh
    /// fork per sweep), never the same session re-forked.
    fn sweep_sessions(probe: &ScriptedAgent) -> Vec<String> {
        probe
            .prompted_sessions()
            .into_iter()
            .zip(probe.seen_prompts())
            .filter(|(_, prompt)| prompt.contains(RESCAN_NEEDLE))
            .map(|(session, _)| session)
            .collect()
    }

    /// Lever 2 (a) — the follow-up sweep keeps going while turns return findings
    /// and STOPS when a turn goes dry (`[]`). The first pass under-reports one
    /// instance; sweep 1 recovers one more, sweep 2 one more, sweep 3 is empty
    /// and ends the loop. All four findings merge on the first review, distinct.
    #[tokio::test]
    async fn followup_sweep_continues_while_findings_arrive_and_stops_when_dry() {
        let (loader, work) = magic_numbers_work();

        let first_pass =
            findings_array_json(&[("src/a.rs", TEST_FINDING_LINE, "no-magic", "magic number 7")]);
        // ONE script entry keyed on the sweep header answers EVERY sweep, with a
        // different delta each turn — findings, findings, then dry. A constant
        // prompt is re-sent each sweep, so this sequence is the only way to script
        // the model converging across the loop.
        let sweep_deltas = ScriptedReply::sequence([
            findings_array_json(&[(
                "src/a.rs",
                TEST_FINDING_LINE + 1,
                "no-magic",
                "magic number 13",
            )]),
            findings_array_json(&[(
                "src/a.rs",
                TEST_FINDING_LINE + 2,
                "no-magic",
                "magic number 99",
            )]),
            "[]".to_string(),
        ]);
        let agent = forking_agent(vec![
            (RESCAN_NEEDLE.to_string(), sweep_deltas),
            first_pass_entry(first_pass),
        ]);
        let probe = Arc::clone(&agent);

        let findings = with_pool(agent, PoolConfig::remote(4), move |pool| async move {
            run_fleet(&work, &loader, &pool).await.findings
        })
        .await;

        // First pass (1) + sweep 1 (1) + sweep 2 (1) = 3 findings; sweep 3 is dry.
        assert_eq!(
            findings.len(),
            3,
            "every instance recovered across the sweeps must merge: {findings:#?}"
        );
        let lines: std::collections::BTreeSet<u32> = findings.iter().map(|f| f.line).collect();
        assert_eq!(
            lines.len(),
            3,
            "the merged findings are distinct file:line instances, not re-reports: {findings:#?}"
        );
        assert!(
            findings
                .iter()
                .all(|f| f.validator == "magic-numbers" && f.rule.as_deref() == Some("no-magic")),
            "merged findings keep their validator and rule tags: {findings:#?}"
        );

        // Three sweep turns fired: two that returned findings plus the dry one
        // that stopped the loop — well under the runaway cap.
        let sessions = sweep_sessions(&probe);
        assert_eq!(
            sessions.len(),
            3,
            "the loop runs sweeps until one goes dry, then stops: {sessions:#?}"
        );
    }

    /// Lever 2 (c) — the loop drives the SAME accumulating session forward, not a
    /// re-fork of the first pass. Each sweep forks the session that delivered the
    /// PRIOR sweep's answer, so the sweeps run on a chain of distinct sessions and
    /// the model's own earlier answers are in context — the structural reason it
    /// converges instead of oscillating.
    #[tokio::test]
    async fn followup_sweep_drives_the_session_forward_not_reforking_the_first_pass() {
        let (loader, work) = magic_numbers_work();

        let first_pass =
            findings_array_json(&[("src/a.rs", TEST_FINDING_LINE, "no-magic", "magic number 7")]);
        let sweep_deltas = ScriptedReply::sequence([
            findings_array_json(&[(
                "src/a.rs",
                TEST_FINDING_LINE + 1,
                "no-magic",
                "magic number 13",
            )]),
            "[]".to_string(),
        ]);
        let agent = forking_agent(vec![
            (RESCAN_NEEDLE.to_string(), sweep_deltas),
            first_pass_entry(first_pass),
        ]);
        let probe = Arc::clone(&agent);

        with_pool(agent, PoolConfig::remote(4), move |pool| async move {
            run_fleet(&work, &loader, &pool).await;
        })
        .await;

        let sessions = sweep_sessions(&probe);
        assert_eq!(
            sessions.len(),
            2,
            "two sweeps fired (one with findings, one dry): {sessions:#?}"
        );
        let distinct: std::collections::BTreeSet<&String> = sessions.iter().collect();
        assert_eq!(
            distinct.len(),
            sessions.len(),
            "each sweep runs on a fresh fork of the prior sweep's session — a forward chain, \
             never the same first-pass session re-forked: {sessions:#?}"
        );

        // The load-bearing proof: the SECOND sweep ran on a session forked from
        // the FIRST sweep's session, so its accumulated context already carries
        // the first sweep's nudge — the sweep header appears TWICE. Re-forking the
        // first pass each time would leave it appearing only once, the model would
        // never see its own prior answer, and the loop could not converge.
        let last_sweep_history = probe
            .session_history(sessions.last().unwrap())
            .expect("the last sweep's session ran");
        let header_occurrences = last_sweep_history.matches(RESCAN_NEEDLE).count();
        assert_eq!(
            header_occurrences, 2,
            "the second sweep continues the first sweep's session (forward chain), so its context \
             holds the nudge twice — not a re-fork of the first pass: {last_sweep_history}"
        );
    }

    /// Lever 2 (b) — the runaway cap. A model that never goes dry (every sweep
    /// returns the same finding) is bounded: the loop stops after exactly
    /// [`MAX_FOLLOWUP_SWEEPS`] sweeps rather than looping forever. The re-reported
    /// duplicates are harmless — downstream `dedup_exact` collapses them.
    #[tokio::test]
    async fn followup_sweep_stops_at_the_cap_when_never_dry() {
        let (loader, work) = magic_numbers_work();

        let first_pass =
            findings_array_json(&[("src/a.rs", TEST_FINDING_LINE, "no-magic", "magic number 7")]);
        // Every sweep returns a (non-empty) finding, so the model never says
        // "none left" — only the cap can terminate the loop.
        let never_dry = findings_array_json(&[(
            "src/a.rs",
            TEST_FINDING_LINE + 1,
            "no-magic",
            "magic number 13",
        )]);
        let agent = forking_agent(vec![
            (RESCAN_NEEDLE.to_string(), ScriptedReply::Text(never_dry)),
            first_pass_entry(first_pass),
        ]);
        let probe = Arc::clone(&agent);

        with_pool(agent, PoolConfig::remote(4), move |pool| async move {
            run_fleet(&work, &loader, &pool).await;
        })
        .await;

        let sessions = sweep_sessions(&probe);
        assert_eq!(
            sessions.len() as u32,
            MAX_FOLLOWUP_SWEEPS,
            "a never-dry model is bounded at the runaway cap, not looped forever: {sessions:#?}"
        );
    }

    /// Lever 2 (d) — an empty first pass spends ZERO follow-up turns. A clean
    /// validator has nothing to be incomplete about, so the loop is skipped
    /// entirely: one validator fork, no sweeps.
    #[tokio::test]
    async fn empty_first_pass_spends_no_followup_sweeps() {
        let (loader, work) = magic_numbers_work();

        // The first pass finds nothing; the sweep header still has a (would-be)
        // entry so a stray sweep would be observable — it must not fire.
        let agent = forking_agent(vec![
            (RESCAN_NEEDLE.to_string(), ScriptedReply::Text("[]".to_string())),
            first_pass_entry("[]".to_string()),
        ]);
        let probe = Arc::clone(&agent);

        let findings = with_pool(agent, PoolConfig::remote(4), move |pool| async move {
            run_fleet(&work, &loader, &pool).await.findings
        })
        .await;

        assert!(
            findings.is_empty(),
            "a clean validator reports nothing: {findings:#?}"
        );
        let sessions = sweep_sessions(&probe);
        assert!(
            sessions.is_empty(),
            "an empty first pass must not spend any follow-up sweep turn: {sessions:#?}"
        );
        assert_eq!(
            probe.fork_count(),
            1,
            "exactly one validator fork and no sweep fork on a clean validator"
        );
    }

    #[tokio::test]
    async fn multi_rule_validator_forks_one_task_carrying_all_rules_against_one_prime() {
        // One validator with three rules over ten files. The files all live in
        // the single shared prime; the fan-out is per VALIDATOR, so this mints
        // exactly one prime + ONE validator fork carrying ALL THREE rules — never
        // per-rule, per-file, or per-batch.
        let rs = ruleset(
            "val",
            "mandate",
            &[
                ("r1", "RULE1_MARKER body 1"),
                ("r2", "RULE2_MARKER body 2"),
                ("r3", "RULE3_MARKER body 3"),
            ],
        );
        let loader = loader_with(vec![rs]);

        let files: Vec<FileWork> = (0..10)
            .map(|i| file_work(&format!("src/f{i}.rs"), &format!("sym{i}"), "src/x.rs"))
            .collect();
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work("val", files)],
        };

        let agent = forking_agent(vec![]);
        let agent_probe = Arc::clone(&agent);

        let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
            run_fleet(&work, &loader, &pool).await
        })
        .await;

        let seen = agent_probe.seen_prompts();
        let primes = seen.iter().filter(|p| p.contains(PRIME_HANDOFF)).count();
        assert_eq!(primes, 1, "one shared prime for the whole run: {seen:#?}");
        let validator_tasks = seen
            .iter()
            .filter(|p| p.starts_with("# Validator:"))
            .count();
        assert_eq!(
            validator_tasks, 1,
            "one validator → one forked validator task (not three rule tasks, not ten file tasks): {seen:#?}"
        );
        assert_eq!(outcome.attempted(), 1, "one validator task attempted");

        // The single prime carries ALL ten files' diffs; the validator fork
        // carries every rule of the validator (no file content re-sent).
        let prime = seen
            .iter()
            .find(|p| p.contains(PRIME_HANDOFF))
            .expect("the run prime");
        assert_eq!(
            prime.matches("## File: ").count(),
            10,
            "the shared prime inlines every file once: {prime}"
        );
        let validator_suffix = seen
            .iter()
            .find(|p| p.starts_with("# Validator:"))
            .expect("a validator fork");
        assert!(
            validator_suffix.contains("RULE1_MARKER")
                && validator_suffix.contains("RULE2_MARKER")
                && validator_suffix.contains("RULE3_MARKER"),
            "the validator fork must carry ALL of its rules: {validator_suffix}"
        );
        assert!(
            !validator_suffix.contains("## File: "),
            "a validator fork must NOT re-send file content (it is in the prime): {validator_suffix}"
        );
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn fan_out_logs_the_rule_names_being_applied_per_validator() {
        // A validator with two distinctively-named rules; the fan-out log must
        // name the rules being applied (sourced from the loader's RuleSet) so the
        // logs show exactly which validator×rules ran.
        let rs = ruleset(
            "deduplicate",
            "mandate",
            &[("no-copy-paste", "body a"), ("prefer-reuse", "body b")],
        );
        let loader = loader_with(vec![rs]);

        let files: Vec<FileWork> = vec![file_work("src/a.rs", "alpha", "src/x.rs")];
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work("deduplicate", files)],
        };

        let agent = forking_agent(vec![]);
        let _findings = with_pool(agent, PoolConfig::remote(1), move |pool| async move {
            run_fleet(&work, &loader, &pool).await
        })
        .await;

        // The batching log carries the rule names from the loader's RuleSet as a
        // structured field (the exact bracketed list only this log emits — the
        // rendered prompt spells rules as `### Rule: ...` prose, not this shape).
        assert!(logs_contain("rules=[\"no-copy-paste\", \"prefer-reuse\"]"));
    }

    // ---- primed-prefix + fork orchestration tests -------------------------

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn prefix_is_primed_once_per_run_and_validators_fork_suffix_only() {
        // One validator, two rules, over four files. The new grain: the change +
        // every file diff is primed ONCE for the whole run, and each VALIDATOR
        // forks it sending only its validator suffix (its full ruleset). So: 1
        // prime + 1 validator fork carrying BOTH rules, never one fork per rule
        // and never one fork per file/batch.
        let rs = ruleset(
            "val",
            "MANDATE_MARKER mandate",
            &[("r1", "RULE1_MARKER body"), ("r2", "RULE2_MARKER body")],
        );
        let loader = loader_with(vec![rs]);

        let files: Vec<FileWork> = (0..4)
            .map(|i| file_work(&format!("src/f{i}.rs"), &format!("sym{i}"), "src/x.rs"))
            .collect();
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work("val", files)],
        };

        // The validator's fork emits a finding. The fork inherits the shared
        // prime (all files) and appends the validator suffix (which carries the
        // mandate marker), so we key on that marker.
        let agent = forking_agent(vec![
            // The first pass is exhaustive; its completeness re-scan finds
            // nothing more, so this test asserts the unchanged one-fork-per-
            // validator prime shape (plus the bounded re-scan fork).
            rescan_finds_nothing(),
            (
                "MANDATE_MARKER".to_string(),
                ScriptedReply::Text(findings_json(
                    "src/f0.rs",
                    TEST_FINDING_LINE,
                    "r1",
                    "warm finding",
                )),
            ),
        ]);
        let agent_probe = Arc::clone(&agent);

        // Drive the prime lifecycle the way `run_review` does: run the fleet,
        // then release the returned shared-prime guard once the run drains.
        let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
            let outcome = run_fleet(&work, &loader, &pool).await;
            if let Some(guard) = outcome.prime {
                unpin_prefix_session(guard).await;
            }
            FleetOutcome {
                prime: None,
                ..outcome
            }
        })
        .await;

        let seen = agent_probe.seen_prompts();
        let primes: Vec<&String> = seen.iter().filter(|p| p.contains(PRIME_HANDOFF)).collect();
        assert_eq!(
            primes.len(),
            1,
            "the shared prefix is primed exactly once per RUN: {seen:#?}"
        );
        // The prime carries the change + every file diff, and NO validator text.
        assert!(
            primes[0].contains("# Files under review") && primes[0].contains("## File: src/f0.rs"),
            "the prime carries the diffs: {}",
            primes[0]
        );
        assert!(
            !primes[0].contains("MANDATE_MARKER")
                && !primes[0].contains("RULE1_MARKER")
                && !primes[0].contains("RULE2_MARKER"),
            "the prime must NOT carry any validator text: {}",
            primes[0]
        );

        // One forked task per validator, carrying ONLY its validator suffix (the
        // validator/mandate/full-ruleset/contract) and never re-sending file
        // content.
        let validator_tasks: Vec<&String> = seen
            .iter()
            .filter(|p| p.starts_with("# Validator:"))
            .collect();
        assert_eq!(
            validator_tasks.len(),
            1,
            "the validator forks the primed session and sends ONLY its validator suffix: {seen:#?}"
        );
        assert!(
            validator_tasks.iter().all(|p| !p.contains("## File: ")),
            "validator forks must not re-send the file diffs: {validator_tasks:#?}"
        );
        // The single validator fork carries BOTH of the validator's rules.
        assert!(validator_tasks[0].contains("RULE1_MARKER"));
        assert!(validator_tasks[0].contains("RULE2_MARKER"));
        // One validator fork plus its one bounded completeness re-scan fork.
        assert_eq!(
            agent_probe.fork_count(),
            2,
            "one validator fork plus one completeness re-scan fork"
        );

        assert_eq!(outcome.attempted(), 1);
        assert_eq!(outcome.failed(), 0);
        assert_eq!(outcome.findings.len(), 1, "{:#?}", outcome.findings);
        assert_eq!(outcome.findings[0].claim, "warm finding");
        assert_eq!(outcome.findings[0].validator, "val");

        // The shared prime was pinned for the run and unpinned when it drained.
        assert_eq!(
            agent_probe.pin_calls(),
            vec![("sess-0".to_string(), true), ("sess-0".to_string(), false)],
            "pin the shared prime for the run, unpin when it drains"
        );

        // Observability: each fork task logs the warm reuse and token count,
        // classified as a warm KV fork (the native llama/qwen path).
        assert!(logs_contain("fleet task prefix reuse"));
        assert!(logs_contain("reuse=\"warm KV fork\""));
        assert!(logs_contain(&format!(
            "reused_tokens=Some({MOCK_PREFIX_TOKENS})"
        )));
        assert!(logs_contain("primed shared run prefix session"));
    }

    /// The shared run prime is born pinned through the PRODUCTION prime path:
    /// `prime_run_prefix` → `submit_primed` → the prompt's `_meta` pin-on-save
    /// intent → the agent saving its prefix pinned atomically at turn completion
    /// — BEFORE any separate `session/pin` confirm runs. This is the end-to-end
    /// (scripted agent, no real model) assertion for the structural close of the
    /// prime→pin eviction race: the prefix is never an unpinned eviction
    /// candidate, independent of any post-turn pin.
    #[tokio::test]
    async fn primed_prefix_is_born_pinned_through_the_production_path() {
        let rs = ruleset("val", "mandate", &[("r", "body")]);
        let loader = loader_with(vec![rs]);
        let files: Vec<FileWork> = (0..2)
            .map(|i| file_work(&format!("src/f{i}.rs"), &format!("sym{i}"), "src/x.rs"))
            .collect();
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work("val", files)],
        };

        let agent = forking_agent(vec![]);
        let agent_probe = Arc::clone(&agent);

        with_pool(agent, PoolConfig::remote(2), move |pool| async move {
            run_fleet_and_unpin(&work, &loader, &pool).await
        })
        .await;

        // The shared prime session (`sess-0`) was born pinned by the prime turn's
        // `_meta` intent — recorded at turn completion, before the post-turn
        // `session/pin` confirm. Forked validator sessions are NOT born pinned
        // (they save their own cold state unpinned).
        assert_eq!(
            agent_probe.born_pinned_sessions(),
            vec!["sess-0".to_string()],
            "the run prime must be born pinned through the production prime path, \
             and only the prime (not the forked validator sessions)"
        );
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn fork_failure_falls_back_to_monolithic_without_losing_tasks() {
        let rs = ruleset("val", "mandate", &[("r", "body")]);
        let loader = loader_with(vec![rs]);
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work(
                "val",
                vec![
                    file_work("src/a.rs", "alpha", "src/x.rs"),
                    file_work("src/b.rs", "beta", "src/y.rs"),
                ],
            )],
        };

        // Every `session/fork` is rejected; the validator task must fall back to
        // a fresh-session monolithic prompt and still deliver its findings.
        let agent = agent_with_fork_mode(
            vec![(
                "## File: src/a.rs".to_string(),
                ScriptedReply::Text(findings_json(
                    "src/a.rs",
                    TEST_FINDING_LINE,
                    "r",
                    "found despite fork failure",
                )),
            )],
            ForkMode::RejectFork,
        );
        let agent_probe = Arc::clone(&agent);

        let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
            run_fleet_and_unpin(&work, &loader, &pool).await
        })
        .await;

        assert_eq!(outcome.attempted(), 1, "one validator task");
        assert_eq!(outcome.failed(), 0, "a failed fork is never a lost task");
        assert_eq!(outcome.findings.len(), 1);
        assert_eq!(outcome.findings[0].claim, "found despite fork failure");

        // The fallback prompt is the full monolithic shape (rules + files).
        let seen = agent_probe.seen_prompts();
        let monolithic = seen
            .iter()
            .filter(|p| p.contains(MANDATE_HEADER) && p.contains("# Files under review"))
            .count();
        assert_eq!(
            monolithic, 1,
            "the validator fell back to a monolithic prompt: {seen:#?}"
        );
        assert!(logs_contain("falling back to a monolithic"));

        // The prime succeeded, so it was pinned and is unpinned when the run drains.
        assert_eq!(
            agent_probe.pin_calls(),
            vec![("sess-0".to_string(), true), ("sess-0".to_string(), false)],
        );
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn unsupported_fork_extension_degrades_to_monolithic_prompts() {
        let rs = ruleset("val", "mandate", &[("r", "body")]);
        let loader = loader_with(vec![rs]);
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work(
                "val",
                vec![
                    file_work("src/a.rs", "alpha", "src/x.rs"),
                    file_work("src/b.rs", "beta", "src/y.rs"),
                ],
            )],
        };

        // The backend implements NO extension methods: the prime turn runs but
        // its state can never be confirmed, so the whole run degrades to
        // monolithic per-validator prompts — never a lost task.
        let agent = agent_with_fork_mode(
            vec![(
                "## File: src/b.rs".to_string(),
                ScriptedReply::Text(findings_json(
                    "src/b.rs",
                    TEST_FINDING_LINE,
                    "r",
                    "found without forks",
                )),
            )],
            ForkMode::Unsupported,
        );
        let agent_probe = Arc::clone(&agent);

        let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
            run_fleet_and_unpin(&work, &loader, &pool).await
        })
        .await;

        assert_eq!(outcome.attempted(), 1, "one validator task");
        assert_eq!(outcome.failed(), 0);
        assert_eq!(outcome.findings.len(), 1);
        assert_eq!(outcome.findings[0].claim, "found without forks");

        let seen = agent_probe.seen_prompts();
        let monolithic = seen
            .iter()
            .filter(|p| p.contains("## Mandate") && p.contains("# Files under review"))
            .count();
        assert_eq!(monolithic, 1, "{seen:#?}");
        assert_eq!(
            agent_probe.fork_count(),
            0,
            "no forks on an unsupported backend"
        );
        assert!(
            agent_probe.pin_calls().is_empty(),
            "nothing is pinned when state confirmation fails"
        );
        assert!(logs_contain("falling back to monolithic prompts"));
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn degraded_fork_runs_cold_but_still_parses_findings() {
        let rs = ruleset("val", "mandate", &[("r", "body")]);
        let loader = loader_with(vec![rs]);
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work(
                "val",
                vec![file_work("src/a.rs", "alpha", "src/x.rs")],
            )],
        };

        // Forks succeed but attach no parent state — the task proceeds on the
        // forked session (history is intact, just cold) and is logged.
        let agent = agent_with_fork_mode(
            vec![
                rescan_finds_nothing(),
                (
                    "## File: src/a.rs".to_string(),
                    ScriptedReply::Text(findings_json(
                        "src/a.rs",
                        TEST_FINDING_LINE,
                        "r",
                        "cold but correct",
                    )),
                ),
            ],
            ForkMode::DegradedAttach,
        );

        let outcome = with_pool(agent, PoolConfig::local(), move |pool| async move {
            run_fleet_and_unpin(&work, &loader, &pool).await
        })
        .await;

        assert_eq!(outcome.attempted(), 1);
        assert_eq!(outcome.failed(), 0);
        assert_eq!(outcome.findings.len(), 1);
        assert_eq!(outcome.findings[0].claim, "cold but correct");
        assert!(logs_contain("fleet task fork was degraded"));
    }

    /// The claude backend shape: a fork that attaches no native KV state
    /// (`fork.prefix_tokens == None`) but whose turn reports Anthropic
    /// prompt-cache reads. The forked task must resolve through the real
    /// `collect_forked_task` path without error AND log the warm-cache reuse
    /// (`classify_reuse` → `WarmCache`), so warm/cold is observable on claude
    /// even though the native KV reuse log is blind.
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn forked_task_with_claude_cache_usage_logs_warm_cache() {
        let rs = ruleset("val", "mandate", &[("r", "body")]);
        let loader = loader_with(vec![rs]);
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work(
                "val",
                vec![file_work("src/a.rs", "alpha", "src/x.rs")],
            )],
        };

        // Forks succeed but attach no native parent state (claude shape:
        // `prefix_tokens == None`); the turn's `_meta` reports a warm cache read,
        // which is what makes the reuse observable on claude.
        let agent = ScriptedAgent::with_config(
            vec![
                rescan_finds_nothing(),
                (
                    "## File: src/a.rs".to_string(),
                    ScriptedReply::Text(findings_json(
                        "src/a.rs",
                        TEST_FINDING_LINE,
                        "r",
                        "warm on claude",
                    )),
                ),
            ],
            ScriptedAgentConfig {
                fork_mode: ForkMode::DegradedAttach,
                cache_usage: Some(CacheUsage {
                    cache_read_input_tokens: Some(2048),
                    cache_creation_input_tokens: Some(16),
                    input_tokens: Some(2064),
                    output_tokens: Some(40),
                }),
                ..ScriptedAgentConfig::default()
            },
        );

        let outcome = with_pool(agent, PoolConfig::local(), move |pool| async move {
            run_fleet_and_unpin(&work, &loader, &pool).await
        })
        .await;

        assert_eq!(outcome.attempted(), 1);
        assert_eq!(
            outcome.failed(), 0,
            "the forked task resolved through collect_forked_task without error"
        );
        assert_eq!(outcome.findings.len(), 1);
        assert_eq!(outcome.findings[0].claim, "warm on claude");
        assert!(
            logs_contain("warm prompt cache"),
            "the warm-cache reuse must be logged so claude reuse is observable"
        );
    }

    #[tokio::test]
    async fn prefix_session_is_unpinned_even_when_a_validator_task_errors() {
        // Two validators; the second's fork errors. The shared-prime pin must
        // still be released once the run drains, regardless of a failed validator
        // task.
        let rs_ok = ruleset("val-ok", "mandate ok", &[("ok-rule", "OK_BODY")]);
        let rs_bad = ruleset("val-bad", "mandate bad", &[("bad-rule", "BAD_BODY")]);
        let loader = loader_with(vec![rs_ok, rs_bad]);
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![
                validator_work("val-ok", vec![file_work("src/a.rs", "alpha", "src/x.rs")]),
                validator_work("val-bad", vec![file_work("src/b.rs", "beta", "src/y.rs")]),
            ],
        };

        // The `val-bad` fork carries the `bad-rule` body and errors; the `val-ok`
        // one is empty. One forked validator task errors → the unpin must still
        // happen.
        let agent = forking_agent(vec![("BAD_BODY".to_string(), ScriptedReply::Error)]);
        let agent_probe = Arc::clone(&agent);

        let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
            run_fleet_and_unpin(&work, &loader, &pool).await
        })
        .await;

        assert_eq!(outcome.attempted(), 2, "two validator tasks");
        assert_eq!(
            outcome.failed(), 1,
            "the erroring validator task is a failed task"
        );
        assert_eq!(
            agent_probe.pin_calls(),
            vec![("sess-0".to_string(), true), ("sess-0".to_string(), false)],
            "the prefix pin is released even when a validator task errors"
        );
    }

    /// Poll `condition` every [`POLL_INTERVAL`] until it holds, panicking after
    /// [`POLL_TIMEOUT`]. The retry count is derived from the two so the wait
    /// budget is expressed once, not as a product of two coupled literals.
    async fn wait_for(what: &str, condition: impl Fn() -> bool) {
        const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(10);
        const POLL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
        let attempts = POLL_TIMEOUT.as_millis() / POLL_INTERVAL.as_millis();
        for _ in 0..attempts {
            if condition() {
                return;
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
        panic!("timed out waiting for {what}");
    }

    /// Cancellation-safety regression: a run future dropped mid-collect
    /// (review cancelled, caller timeout) must STILL release the prefix pin —
    /// a pinned session is exempt from cache eviction, so a leaked pin
    /// outlives the review until process restart.
    #[tokio::test]
    async fn prefix_pin_is_released_when_the_fanout_future_is_dropped_mid_collect() {
        let rs = ruleset("val", "mandate", &[("r", "WEDGE_BODY")]);
        let loader = loader_with(vec![rs]);
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work(
                "val",
                vec![file_work("src/a.rs", "alpha", "src/x.rs")],
            )],
        };

        // The validator fork turn wedges forever (its suffix carries the rule
        // body), holding the fan-out mid-collect AFTER the prime has been pinned.
        let agent = forking_agent(vec![("WEDGE_BODY".to_string(), ScriptedReply::Stall)]);
        let agent_probe = Arc::clone(&agent);

        with_pool(agent, PoolConfig::remote(2), move |pool| async move {
            let fanout = tokio::spawn(async move { run_fleet(&work, &loader, &pool).await });

            // Wait until the prefix is pinned and the wedged validator fork is in
            // flight — the run is now mid-collect.
            wait_for("the prefix pin and the wedged validator fork", || {
                agent_probe
                    .pin_calls()
                    .contains(&("sess-0".to_string(), true))
                    && agent_probe
                        .seen_prompts()
                        .iter()
                        .any(|p| p.starts_with("# Validator:"))
            })
            .await;

            // Cancel the review: drop the fan-out future mid-collect.
            fanout.abort();
            let _ = fanout.await;

            // The pin must still be released — the cancelled fan-out cannot
            // leak the pinned prefix session.
            wait_for("the cancelled fan-out to release the prefix pin", || {
                agent_probe
                    .pin_calls()
                    .contains(&("sess-0".to_string(), false))
            })
            .await;
        })
        .await;
    }

    #[tokio::test]
    async fn one_failing_task_yields_zero_findings_without_aborting_the_rest() {
        // Two validators: the `val-bad` fork errors, the `val-good` fork finds an
        // issue. One bad validator task never aborts the rest.
        let rs_good = ruleset("val-good", "mandate good", &[("good-rule", "GOOD_BODY")]);
        let rs_bad = ruleset("val-bad", "mandate bad", &[("bad-rule", "BAD_BODY")]);
        let loader = loader_with(vec![rs_good, rs_bad]);

        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![
                validator_work("val-good", vec![file_work("src/a.rs", "alpha", "src/x.rs")]),
                validator_work("val-bad", vec![file_work("src/b.rs", "beta", "src/y.rs")]),
            ],
        };

        // The fork carrying `BAD_BODY` errors; the `GOOD_BODY` one returns a
        // finding. Both keys appear only in their own validator's suffix.
        let agent = forking_agent(vec![
            // The good validator's first pass is exhaustive; its completeness
            // re-scan finds nothing more, so the surviving count is unchanged.
            rescan_finds_nothing(),
            ("BAD_BODY".to_string(), ScriptedReply::Error),
            (
                "GOOD_BODY".to_string(),
                ScriptedReply::Text(findings_json(
                    "src/a.rs",
                    TEST_FINDING_LINE,
                    "good-rule",
                    "real issue",
                )),
            ),
        ]);

        let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
            run_fleet_and_unpin(&work, &loader, &pool).await
        })
        .await;

        // The erroring task contributed nothing; the good one still returned.
        assert_eq!(
            outcome.findings.len(),
            1,
            "the failing task degrades to zero findings"
        );
        assert_eq!(outcome.findings[0].claim, "real issue");
        assert_eq!(outcome.findings[0].validator, "val-good");
        // The tally records both tasks attempted and exactly the one that failed.
        assert_eq!(outcome.attempted(), 2, "two validator tasks attempted");
        assert_eq!(outcome.failed(), 1, "the erroring task is counted as failed");
    }

    #[tokio::test]
    async fn all_tasks_failing_yields_zero_findings_and_a_full_failure_tally() {
        // Three validators; every validator fork errors.
        let loader = loader_with(vec![
            ruleset("val-a", "mandate a", &[("r1", "body 1")]),
            ruleset("val-b", "mandate b", &[("r2", "body 2")]),
            ruleset("val-c", "mandate c", &[("r3", "body 3")]),
        ]);

        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![
                validator_work("val-a", vec![file_work("src/a.rs", "a", "src/x.rs")]),
                validator_work("val-b", vec![file_work("src/b.rs", "b", "src/y.rs")]),
                validator_work("val-c", vec![file_work("src/c.rs", "c", "src/z.rs")]),
            ],
        };

        // Every validator fork errors (every validator suffix carries the
        // validator header).
        let agent = forking_agent(vec![("# Validator:".to_string(), ScriptedReply::Error)]);

        let outcome = with_pool(agent, PoolConfig::remote(3), move |pool| async move {
            run_fleet_and_unpin(&work, &loader, &pool).await
        })
        .await;

        assert!(
            outcome.findings.is_empty(),
            "every task failed, so there are no findings"
        );
        assert_eq!(outcome.attempted(), 3, "three validator tasks attempted");
        assert_eq!(outcome.failed(), 3, "all three failed");
    }

    #[tokio::test]
    async fn validator_missing_from_loader_is_skipped_not_panicked() {
        // The work-list names a validator the loader does not know.
        let loader = loader_with(vec![ruleset("known", "mandate", &[("r", "body")])]);
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work(
                "unknown",
                vec![file_work("src/a.rs", "alpha", "src/x.rs")],
            )],
        };

        let agent = forking_agent(vec![]);
        let agent_probe = Arc::clone(&agent);

        let outcome = with_pool(agent, PoolConfig::remote(1), move |pool| async move {
            run_fleet(&work, &loader, &pool).await
        })
        .await;

        assert!(
            outcome.findings.is_empty(),
            "an unknown validator yields no findings"
        );
        assert_eq!(
            outcome.attempted(), 0,
            "no task is attempted for a validator missing from the loader"
        );
        assert_eq!(outcome.failed(), 0);
        assert!(
            agent_probe.seen_prompts().is_empty(),
            "no task is submitted for a validator missing from the loader"
        );
    }

    // ---- classify_reuse --------------------------------------------------

    /// A native KV fork that attached its parent's saved state with a token
    /// count classifies as `WarmKv` carrying that count — the llama/qwen path.
    #[test]
    fn test_classify_reuse_kv_fork_is_warm_kv() {
        let fork = Some(ForkAttachment {
            state_attached: true,
            prefix_tokens: Some(MOCK_PREFIX_TOKENS),
        });
        assert_eq!(
            classify_reuse(fork, None),
            PrefixReuse::WarmKv {
                reused_tokens: MOCK_PREFIX_TOKENS
            }
        );
    }

    /// A claude turn with `cache_read_input_tokens > 0` classifies as
    /// `WarmCache` carrying the read/created split — even though the fork
    /// attached no native KV token count (the production blind spot this task
    /// closes).
    #[test]
    fn test_classify_reuse_claude_cache_read_is_warm_cache() {
        let usage = Some(CacheUsage {
            cache_read_input_tokens: Some(900),
            cache_creation_input_tokens: Some(100),
            input_tokens: Some(1000),
            output_tokens: Some(20),
        });
        assert_eq!(
            classify_reuse(None, usage),
            PrefixReuse::WarmCache {
                read: 900,
                created: 100
            }
        );
    }

    /// A claude turn that only wrote the cache (`cache_creation_input_tokens >
    /// 0`, no reads) is a cold prefill — `Cold` (no warm reuse to report).
    #[test]
    fn test_classify_reuse_claude_cold_write_is_cold() {
        let usage = Some(CacheUsage {
            cache_read_input_tokens: Some(0),
            cache_creation_input_tokens: Some(1000),
            input_tokens: Some(1000),
            output_tokens: Some(20),
        });
        assert_eq!(classify_reuse(None, usage), PrefixReuse::Cold);
    }

    /// No fork and no usage is unknown/cold.
    #[test]
    fn test_classify_reuse_empty_is_cold() {
        assert_eq!(classify_reuse(None, None), PrefixReuse::Cold);
    }
}
