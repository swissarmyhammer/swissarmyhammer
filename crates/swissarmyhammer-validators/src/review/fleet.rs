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

/// The default review `batch_size` in **bytes** (256 KiB).
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
/// still splits across batches. (32 KiB, then 128 KiB — the previous defaults —
/// were smaller than many real source files, so default reviews of normal
/// commits errored.)
pub const DEFAULT_BATCH_SIZE: usize = 256 * 1024;

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
    /// larger than it is a hard error (never split, never sliced). Read through
    /// [`batch_size`](Self::batch_size); private so the config can evolve
    /// without a field-level API commitment.
    batch_size: usize,
}

impl FleetConfig {
    /// Build a config with an explicit batch budget (bytes of inlined file
    /// content per batch). [`FleetConfig::default`] uses [`DEFAULT_BATCH_SIZE`].
    pub fn new(batch_size: usize) -> Self {
        Self { batch_size }
    }

    /// The maximum inlined file content, in bytes, one batch's shared prime may
    /// carry.
    pub fn batch_size(&self) -> usize {
        self.batch_size
    }
}

impl Default for FleetConfig {
    fn default() -> Self {
        Self::new(DEFAULT_BATCH_SIZE)
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
    /// Read through [`findings`](Self::findings) or moved out via
    /// [`into_parts`](Self::into_parts); private so the outcome can evolve
    /// without a field-level API commitment.
    findings: Vec<Finding>,
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
    /// release. Read through [`prime`](Self::prime) or moved out via
    /// [`into_parts`](Self::into_parts); private for the same reason as
    /// [`findings`](Self::findings).
    prime: Option<SessionPinGuard>,
}

impl FleetOutcome {
    /// The merged, validator-tagged findings from every task that succeeded.
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// The run's shared primed-prefix pin guard, when priming succeeded.
    /// `None` means every task ran the monolithic fallback, so there is no pin
    /// to reuse or release.
    pub fn prime(&self) -> Option<&SessionPinGuard> {
        self.prime.as_ref()
    }

    /// Consume the outcome into its movable halves: the merged findings and the
    /// prime pin guard.
    ///
    /// The verify stage takes the findings by value ([`build_candidates`] in
    /// `synthesize.rs`) and the caller keeps the prime alive across verify,
    /// releasing it via [`unpin_prefix_session`] once the batch drains. Read the
    /// task tally ([`attempted`](Self::attempted) / [`failed`](Self::failed))
    /// before consuming.
    pub fn into_parts(self) -> (Vec<Finding>, Option<SessionPinGuard>) {
        (self.findings, self.prime)
    }

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

/// One progress event from the fan-out fleet.
///
/// The fan-out unit is one task per validator covering that validator's files,
/// so the finest real evaluation grain is the **(validator, file) pair**. The
/// fleet emits [`Planned`](ReviewProgressEvent::Planned) once per batch after
/// planning, one [`PairStarted`](ReviewProgressEvent::PairStarted) per pair at
/// submission, and one [`PairDone`](ReviewProgressEvent::PairDone) per pair as
/// each task resolves — including failed and monolithic-fallback tasks, so a
/// consumer counting `PairDone` events always reaches the planned total.
///
/// The type is deliberately MCP-free: the engine emits plain events on a tokio
/// mpsc channel and the MCP tool boundary maps them to `notifications/progress`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewProgressEvent {
    /// The review embedder is downloading its model before the scope stage can
    /// run. Emitted (via the embedder's download observer) on the FIRST review
    /// run of a process — the process-global embedder cache means only the cold
    /// run downloads — BEFORE any [`FileScoped`](ReviewProgressEvent::FileScoped)
    /// or [`Planned`](ReviewProgressEvent::Planned) event, so the pre-scope
    /// model-download window is no longer silent (an MCP client's tool-call
    /// inactivity timeout would otherwise fire during it). Advisory: it advances
    /// no pair counter. `file` is the full, untruncated filename; the byte
    /// counts are the running and final sizes reported by the hub.
    DownloadingModel {
        /// The full, untruncated filename being downloaded.
        file: String,
        /// Bytes of `file` received so far.
        downloaded_bytes: u64,
        /// Total size of `file` in bytes as reported by the hub.
        total_bytes: u64,
    },
    /// One file entered the scope stage (semantic diff + probes). Emitted per
    /// resolved file by [`scope_review`](crate::review::scope_review) BEFORE
    /// any fleet work, so a progress consumer sees its first event within
    /// seconds of the call starting — the scope stage alone can outlast a
    /// client's silence timeout, and these events (plus the consumer's
    /// keep-alive) are what carry it through. Advisory only: it advances no
    /// counter.
    FileScoped {
        /// The full path of the file being scoped — never truncated.
        file: String,
    },
    /// One batch's fan-out plan is ready: `total_pairs` (validator, file)
    /// pairs will be reviewed. Emitted once per non-empty batch; a consumer
    /// sums the totals across batches.
    Planned {
        /// How many (validator, file) pairs this batch plans to review.
        total_pairs: usize,
    },
    /// One (validator, file) pair was submitted to the pool.
    PairStarted {
        /// The validator (RuleSet) name the file is reviewed against.
        validator: String,
        /// The full path of the file under review — never truncated.
        file: String,
    },
    /// One (validator, file) pair resolved — successfully, degraded to the
    /// monolithic fallback, or failed. Always emitted, so progress reaches
    /// the planned total even when tasks fail.
    PairDone {
        /// The validator (RuleSet) name the file was reviewed against.
        validator: String,
        /// The full path of the reviewed file — never truncated.
        file: String,
    },
    /// One validator task's parsed findings, emitted as that task resolves in
    /// [`collect_fan_out`]. Unlike the pair-count ticks this carries CONTENT: a
    /// client can start resolving these findings while the run is still going.
    ///
    /// The findings are already validator-tagged (via
    /// [`parse_task_response`](crate::review::types)). An empty vec means the
    /// validator came back clean; a failed task emits no `Findings` event at all
    /// (the [`PairDone`](ReviewProgressEvent::PairDone) accounting still fires).
    /// It advances no pair counter — it is content, not progress, so the MCP
    /// boundary routes it to `notifications/message`, not `notifications/progress`.
    Findings {
        /// The validator (RuleSet) the findings were produced against.
        validator: String,
        /// Every finding the task parsed, in emission order — never truncated.
        findings: Vec<Finding>,
    },
    /// One candidate finding's verdict, emitted as it resolves in the verify
    /// stage — a deterministic guard refutation or an adversarial agent verdict.
    /// Like [`Findings`](ReviewProgressEvent::Findings) this carries CONTENT: a
    /// client learns each confirmed/refuted decision the moment the verify task
    /// resolves, not at the end. It advances no pair counter and routes to
    /// `notifications/message`.
    Verdict {
        /// The candidate finding the verdict decided.
        finding: Finding,
        /// Whether the finding was confirmed (`true`) or refuted (`false`).
        confirmed: bool,
        /// One sentence — what confirmed or refuted the finding.
        reason: String,
    },
}

/// The sender half review progress events are emitted on.
///
/// Threaded as an `Option` from [`run_review_over_agent`] down to [`run_fleet`]:
/// `None` emits nothing (the pre-progress behavior). `UnboundedSender::send` is
/// synchronous, so emission works inside the tool's nested current-thread
/// runtime without blocking the fan-out.
///
/// [`run_review_over_agent`]: crate::review::run_review_over_agent
pub type ReviewProgressSender = tokio::sync::mpsc::UnboundedSender<ReviewProgressEvent>;

/// Send `event` when a progress channel is wired. A `None` sender or a closed
/// receiver is a no-op — progress is advisory, never load-bearing.
///
/// `pub(crate)` so the scope stage ([`scope_review`](crate::review::scope_review))
/// emits through the same helper instead of a copy.
pub(crate) fn emit_progress(progress: Option<&ReviewProgressSender>, event: ReviewProgressEvent) {
    if let Some(tx) = progress {
        let _ = tx.send(event);
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
    progress: Option<&ReviewProgressSender>,
) -> FleetOutcome {
    // Plan the fan-out BEFORE priming so an empty plan (no matching ruleset)
    // skips the prime entirely — an empty run never prompts the agent.
    let plan = plan_fan_out(work, loader);
    if plan.is_empty() {
        return FleetOutcome::default();
    }

    // Announce the batch's plan before any agent work so a progress consumer
    // knows the pair total up front (it sums totals across batches).
    emit_progress(
        progress,
        ReviewProgressEvent::Planned {
            total_pairs: plan.iter().map(|task| task.validator.files().len()).sum(),
        },
    );

    // Prime the run's shared prefix (change + all diffs) ONCE, then submit one
    // fork (or monolithic fallback) per planned validator and collect them all.
    // `None` from priming → every task degrades to a self-contained monolithic
    // prompt.
    let prime = prime_run_prefix(work, pool).await;
    let pending = submit_fan_out(plan, work, pool, &prime, progress);
    let (findings, attempted, failed) = collect_fan_out(pending, work, pool, progress).await;

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
    for validator in work.validators() {
        let Some(ruleset) = loader.get_ruleset(validator.validator_name()) else {
            tracing::warn!(
                validator = %validator.validator_name(),
                "fleet fan-out: no RuleSet for validator in loader; skipping it"
            );
            continue;
        };
        let rule_names: Vec<&str> = ruleset.rules.iter().map(|r| r.name.as_str()).collect();
        tracing::info!(
            validator = %validator.validator_name(),
            files = validator.files().len(),
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
    progress: Option<&ReviewProgressSender>,
) -> Vec<PendingValidator<'w>> {
    plan.into_iter()
        .map(|task| {
            tracing::debug!(
                validator = %task.validator.validator_name(),
                warm = prime.is_some(),
                "fleet fan-out: submitting validator task"
            );
            // One PairStarted per (validator, file): the task covers every one
            // of the validator's files, so each pair is announced at submission.
            for file in task.validator.files() {
                emit_progress(
                    progress,
                    ReviewProgressEvent::PairStarted {
                        validator: task.validator.validator_name().to_string(),
                        file: file.path().to_string(),
                    },
                );
            }
            let suffix = render_validator_suffix(task.validator, task.ruleset);
            let rx = match prime {
                Some(guard) => Submitted::Forked(pool.submit_forked(guard.session_id(), suffix)),
                None => Submitted::Monolithic(pool.submit(render_fleet_prompt(
                    work.change_purpose(),
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
    progress: Option<&ReviewProgressSender>,
) -> (Vec<Finding>, usize, usize) {
    let attempted = pending.len();
    let mut findings: Vec<Finding> = Vec::new();
    let mut failed = 0usize;
    for pending in pending {
        let name = pending.task.validator.validator_name();
        let files: Vec<String> = pending
            .task
            .validator
            .files()
            .iter()
            .map(|f| f.path().to_string())
            .collect();
        let collected = match pending.rx {
            Submitted::Monolithic(rx) => collect_task(rx.await, name, &files),
            Submitted::Forked(rx) => {
                collect_forked_task(
                    rx.await,
                    work.change_purpose(),
                    pending.task.validator,
                    pending.task.ruleset,
                    &files,
                    pool,
                )
                .await
            }
        };
        match collected {
            Ok(parsed) => {
                // Stream this task's parsed, validator-tagged findings as CONTENT
                // the moment the task resolves — a client can start resolving
                // them while the rest of the fleet is still running. An empty vec
                // is a clean validator; a failed task takes the `Err` arm below
                // and emits no `Findings` event.
                emit_progress(
                    progress,
                    ReviewProgressEvent::Findings {
                        validator: name.to_string(),
                        findings: parsed.clone(),
                    },
                );
                findings.extend(parsed);
            }
            Err(()) => failed += 1,
        }
        // One PairDone per (validator, file) regardless of how the task
        // resolved — success, monolithic fallback, or failure — so a consumer
        // counting PairDone always reaches the planned total.
        for file in &files {
            emit_progress(
                progress,
                ReviewProgressEvent::PairDone {
                    validator: name.to_string(),
                    file: file.clone(),
                },
            );
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
    let name = validator.validator_name();
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
    let name = validator.validator_name();
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

/// The header that opens both fan-out prompt renderings — the monolithic
/// fallback ([`render_fleet_prompt`]) and the shared run prime
/// ([`render_run_prime`]) — so the two prompt shapes stay byte-identical on
/// this section and a wording change lands in one place.
const CHANGE_PURPOSE_HEADER: &str = "# Change purpose\n\n";

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
    out.push_str(CHANGE_PURPOSE_HEADER);
    out.push_str(change_purpose.trim());
    out.push_str("\n\n");
    out.push_str(&render_file_payload(validator.files()));
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
    out.push_str(CHANGE_PURPOSE_HEADER);
    out.push_str(work.change_purpose().trim());
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
    let _ = writeln!(out, "{VALIDATOR_HEADER}{}\n", validator.validator_name());
    out.push_str(MANDATE_HEADER);
    out.push_str(ruleset.description().trim());
    out.push_str("\n\n");

    render_validator_guidance(&mut out, ruleset.manifest_body());

    render_focus_files(&mut out, validator.files());

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
        let _ = writeln!(out, "- `{}`", file.path());
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
    let _ = writeln!(out, "## File: {}\n", file.path());

    out.push_str(
        "### Full current contents\n\n\
         This is the COMPLETE current source of the file. You do not need to read this \
         file — it is provided here in full. Review it directly. This whole file is the \
         review boundary: report every place a rule fires anywhere in it, including \
         pre-existing instances that sit outside the change described below.\n\n",
    );
    out.push_str("```\n");
    out.push_str(file.source_slice().trim_end());
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
    render_probe_evidence(out, file.probe_results(), false);
}

/// Append the structured semantic diff for a file as a list of changed entities.
fn render_semantic_diff(out: &mut String, file: &FileWork) {
    if file.semantic_diff().is_empty() {
        out.push_str("_No structured entity changes._\n\n");
        return;
    }
    for change in file.semantic_diff() {
        let _ = writeln!(
            out,
            "- {} {} `{}`",
            change.change_type, change.entity_type, change.entity_name
        );
    }
    out.push('\n');
}

#[cfg(test)]
mod tests;
