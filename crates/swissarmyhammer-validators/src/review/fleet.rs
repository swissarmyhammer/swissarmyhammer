//! Engine stage 2 — the fan-out fleet.
//!
//! The shard is the validator; the **grain is the file**. This stage takes the
//! stage-1 [`WorkList`](crate::review::WorkList) and produces one agent task per
//! `(validator, file)` pair, submitting every task to the shared
//! [`AgentPool`](crate::validators::AgentPool). Each task reviews ONE file
//! against ONE validator's rules, armed with the engine-run probe evidence
//! stage 1 already gathered, and returns a `Vec<`[`Finding`]`>` tagged with the
//! validator (and, when the agent cites it, the rule).
//!
//! # Batching, not concurrency
//!
//! To bound the task count on a large diff, a handful of files are *packed* into
//! one task ([`FleetConfig::batch_size`]); the grain stays the file (each file is
//! rendered as its own self-contained block), the batch is just packing so a
//! 400-file diff does not mint 400 separate sessions. The batching applied is
//! logged via [`tracing`].
//!
//! **Parallelism is not controlled here.** Every task goes to the shared
//! [`AgentPool`], which owns the single concurrency control (worker count). This
//! stage only submits and collects; the pool queues and drains. A task that
//! errors or times out yields zero findings for its batch — logged, never a
//! panic — so one bad task never aborts the rest.
//!
//! # Primed prefix sessions + forks
//!
//! The first two prompt sections (change purpose + validator instructions) are
//! identical across every one of a validator's batches, and on a local model
//! they dominate the prompt (~15k tokens). So instead of re-decoding them per
//! task, each validator's fan-out runs as:
//!
//! 1. **Prime** — one session is prompted with [`render_validator_prefix`]
//!    (the shared sections, ending with an explicit "reply OK, files arrive
//!    next" handoff). The completed turn leaves the agent's saved state exactly
//!    at the boundary every batch continues from.
//! 2. **Confirm + pin** — the `session/state_status` extension confirms the
//!    state is actually saved ("never fork blind"), and `session/pin` protects
//!    it from cache eviction for the fan-out's duration.
//! 3. **Fork per batch** — each batch turn runs on a `session/fork` of the
//!    primed session and sends ONLY [`render_file_payload`], decoding strictly
//!    forward from the shared prefix. Warm reuse (and the reused token count)
//!    is logged per task.
//! 4. **Unpin** — the prefix pin is released once every batch has resolved.
//!    The pin is held by a [`SessionPinGuard`], so a fan-out future dropped
//!    mid-collect (cancelled review, caller timeout) still releases it.
//!
//! Any failure — the prime turn, the state confirmation, the pin, or an
//! individual fork — degrades that scope to today's monolithic prompt
//! ([`render_fleet_prompt`], one fresh session carrying everything) with a
//! logged warning: degraded but correct, never a lost task. The flow is
//! backend-agnostic; the extension contract lives in
//! [`agent_client_protocol_extras::session_fork`].
//!
//! # The prompt payload
//!
//! [`render_fleet_prompt`] assembles exactly the payload the task specifies,
//! reusing the structured data stage 1 produced (no new template engine):
//!
//! 1. **Change purpose** — [`WorkList::change_purpose`](crate::review::WorkList).
//! 2. **Validator instructions** — the mandate (the validator's `description`),
//!    each rule body verbatim, the severity default, and the output contract
//!    (every finding emits `rule` + `claim` + `evidence` + `suggestion`, matching
//!    the [`Finding`] type).
//! 3. **The file(s) under review** — for each file in the batch: its path, the
//!    structured semantic diff, the bounded source slice, and the probe results
//!    rendered as evidence blocks.
//!
//! Excluded by design: other validators' rules and any file outside the batch.
//! The split renders ([`render_validator_prefix`] = sections 1–2 + handoff,
//! [`render_file_payload`] = section 3) compose byte-identically into the
//! monolithic prompt, so the warm and degraded paths never drift.

use std::fmt::Write as _;

use crate::review::probes::render_probe_evidence;
use crate::review::scope::{FileWork, ValidatorWork, WorkList};
use crate::review::types::{parse_findings, Finding};
use crate::validators::{
    AgentPool, PoolError, RuleSet, SessionPinGuard, SessionTurn, SessionTurnResult, Severity,
    ValidatorLoader,
};
use agent_client_protocol_extras::SessionStateStatusResponse;

/// Default number of files packed into a single fan-out task.
///
/// Small enough that one task's prompt stays well inside an agent's context
/// window (the grain is still the file), large enough that a big diff does not
/// mint a separate session per file.
pub const DEFAULT_BATCH_SIZE: usize = 4;

/// Configuration for a fan-out run.
#[derive(Debug, Clone, Copy)]
pub struct FleetConfig {
    /// How many files to pack into one agent task. Clamped to at least 1.
    pub batch_size: usize,
}

impl Default for FleetConfig {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_BATCH_SIZE,
        }
    }
}

impl FleetConfig {
    /// The effective, clamped batch size (never zero).
    fn effective_batch_size(&self) -> usize {
        self.batch_size.max(1)
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
#[derive(Debug, Default)]
pub struct FleetOutcome {
    /// The merged, validator-tagged findings from every task that succeeded.
    pub findings: Vec<Finding>,
    /// How many `(validator, batch-of-files)` tasks were submitted.
    pub attempted: usize,
    /// How many of those tasks failed (errored, were dropped, or did not parse)
    /// and so degraded to zero findings.
    pub failed: usize,
}

/// Fan a [`WorkList`] out across the shared [`AgentPool`] and collect the merged,
/// validator-tagged findings.
///
/// One task is built per `(validator, batch-of-files)`: every validator's files
/// are packed into batches of [`FleetConfig::batch_size`], each batch rendered
/// into one prompt by [`render_fleet_prompt`] and submitted to `pool`. As each
/// task returns, its response is parsed by [`parse_findings`] and every finding
/// is tagged with the validator. A task that errors or returns unparseable
/// content contributes zero findings for its batch and is logged — never a panic.
///
/// `loader` is the same fully-loaded [`ValidatorLoader`] stage 1 matched against,
/// reused here as the authoritative source of each validator's mandate and rule
/// bodies (the [`WorkList`] carries only the per-file work and the rule *names*).
/// A validator in the work-list with no matching RuleSet in the loader is logged
/// and skipped rather than rendered with empty instructions.
///
/// The returned findings are ordered by validator (work-list order), then by the
/// order the pool delivered each batch. Alongside them, the returned
/// [`FleetOutcome`] carries the task tally — how many tasks were attempted and
/// how many failed — so a saturated run (most tasks rejected) is distinguishable
/// from a genuinely clean diff rather than both rendering an empty findings set.
pub async fn run_fleet(
    work: &WorkList,
    loader: &ValidatorLoader,
    pool: &AgentPool,
    config: FleetConfig,
) -> FleetOutcome {
    let batch_size = config.effective_batch_size();

    // One run per validator: prime its shared prefix once, then fork a payload
    // turn per batch (see `run_validator_fleet`). The runs are driven
    // concurrently so one validator's prime never serializes another
    // validator's batches; the pool still owns the only real concurrency
    // control (its worker count).
    let mut runs = Vec::new();
    for validator in &work.validators {
        let Some(ruleset) = loader.get_ruleset(&validator.validator_name) else {
            tracing::warn!(
                validator = %validator.validator_name,
                "fleet fan-out: no RuleSet for validator in loader; skipping its files"
            );
            continue;
        };
        let total_batches = batch_count(validator.files.len(), batch_size);
        // The rule names being applied come from the loader's RuleSet (the
        // authoritative source), so the log shows exactly which validator×rules
        // ran — not just the validator name.
        let rule_names: Vec<&str> = ruleset.rules.iter().map(|r| r.name.as_str()).collect();
        tracing::info!(
            validator = %validator.validator_name,
            files = validator.files.len(),
            batch_size,
            batches = total_batches,
            rules = ?rule_names,
            "fleet fan-out: batching files into agent tasks"
        );
        runs.push(run_validator_fleet(
            &work.change_purpose,
            validator,
            ruleset,
            pool,
            batch_size,
        ));
    }

    let mut outcome = FleetOutcome::default();
    for run in futures::future::join_all(runs).await {
        outcome.findings.extend(run.findings);
        outcome.attempted += run.attempted;
        outcome.failed += run.failed;
    }
    outcome
}

/// One validator's slice of the [`FleetOutcome`] tally.
struct ValidatorRun {
    findings: Vec<Finding>,
    attempted: usize,
    failed: usize,
}

/// How one batch task was submitted: a payload-only prompt on a fork of the
/// validator's primed prefix session (the warm path), or the full monolithic
/// prompt on a fresh session (the degraded path).
enum Submitted {
    Forked(tokio::sync::oneshot::Receiver<SessionTurnResult>),
    Monolithic(tokio::sync::oneshot::Receiver<crate::validators::PromptResult>),
}

/// Fan one validator's files out: prime the shared prefix session once, fork a
/// payload-only turn per batch, collect, and unpin.
///
/// When priming (or its saved-state confirmation/pin) fails, every batch runs
/// as today's monolithic fresh-session prompt instead — degraded but correct.
/// A batch whose fork fails falls back to a monolithic prompt individually.
/// The prefix session's pin is released after every batch task has resolved,
/// including failures — and, because the prime returns a [`SessionPinGuard`],
/// even when this future is dropped mid-collect — so the pinned entry never
/// outlives the fan-out.
async fn run_validator_fleet(
    change_purpose: &str,
    validator: &ValidatorWork,
    ruleset: &RuleSet,
    pool: &AgentPool,
    batch_size: usize,
) -> ValidatorRun {
    let name = validator.validator_name.as_str();

    // Prime the shared prefix once; `None` → degraded to monolithic prompts.
    let prefix_session = prime_validator_prefix(change_purpose, validator, ruleset, pool).await;

    struct Pending<'w> {
        files: Vec<String>,
        batch: &'w [FileWork],
        rx: Submitted,
    }

    let mut pending: Vec<Pending<'_>> = Vec::new();
    for batch in validator.files.chunks(batch_size) {
        let files: Vec<String> = batch.iter().map(|f| f.path.clone()).collect();
        tracing::debug!(
            validator = %name,
            files = ?files,
            warm = prefix_session.is_some(),
            "fleet fan-out: submitting validator×files task"
        );
        let rx = match &prefix_session {
            Some(guard) => Submitted::Forked(
                pool.submit_forked(guard.session_id(), render_file_payload(batch)),
            ),
            None => Submitted::Monolithic(pool.submit(render_fleet_prompt(
                change_purpose,
                validator,
                ruleset,
                batch,
            ))),
        };
        pending.push(Pending { files, batch, rx });
    }

    // Collect every batch task in submission order; each receiver resolves
    // independently while the pool drains in parallel up to its worker count.
    let attempted = pending.len();
    let mut findings: Vec<Finding> = Vec::new();
    let mut failed = 0usize;
    for task in pending {
        let collected = match task.rx {
            Submitted::Monolithic(rx) => collect_task(rx.await, name, &task.files),
            Submitted::Forked(rx) => {
                collect_forked_task(
                    rx.await,
                    change_purpose,
                    validator,
                    ruleset,
                    task.batch,
                    &task.files,
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

    // Release the prefix pin now that every batch task has resolved (success
    // or failure), so the pinned entry never outlives this validator's fan-out.
    if let Some(guard) = prefix_session {
        unpin_prefix_session(guard, name).await;
    }

    ValidatorRun {
        findings,
        attempted,
        failed,
    }
}

/// Prime one validator's shared prompt prefix in a dedicated session, confirm
/// the agent saved restorable state for it ("never fork blind"), and acquire
/// the scoped pin guard that governs the fan-out's pin lifecycle.
///
/// The prime turn is submitted with a born-pinned save intent
/// ([`AgentPool::submit_primed`] carries `pin_on_save` in `_meta`), so the
/// prefix is pinned **atomically at save time** — never an unpinned eviction
/// candidate, so a concurrent session's save cannot evict it before the fan-out
/// forks from it. That is the structural close of the prime→pin eviction race.
///
/// The post-turn [`AgentPool::pin_session_scoped`] is therefore no longer the
/// load-bearing pin: it is an **idempotent re-pin / confirm** that (a) verifies
/// the state is still resident and (b) returns the [`SessionPinGuard`] whose
/// `release()`/`Drop` performs the matching unpin when the validator's batches
/// complete (or the fan-out future is dropped mid-flight). There is one pin
/// protocol — born-pinned at save, unpinned by the guard — not two competing
/// ones. A backend without a KV cache (claude) born-pins as a no-op and reports
/// `pinned: false`; forking still works, consistent with the pin=no-op
/// contract.
///
/// Returns the guard for the primed session (carrying its id, the fork parent),
/// or `None` when any step failed — the caller degrades to monolithic prompts
/// (correct, just cold), never a lost task.
async fn prime_validator_prefix(
    change_purpose: &str,
    validator: &ValidatorWork,
    ruleset: &RuleSet,
    pool: &AgentPool,
) -> Option<SessionPinGuard> {
    let name = validator.validator_name.as_str();
    let prefix = render_validator_prefix(change_purpose, validator, ruleset);
    let turn = submit_prime(pool, name, prefix).await?;
    let status = confirm_saved_state(pool, name, &turn).await?;
    pin_prefix(pool, name, &turn, &status).await
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
                validator = %name,
                session = %turn.session_id,
                prefix_tokens = ?status.prompt_tokens,
                born_pinned = status.pinned,
                pinned = pin.pinned,
                "primed validator prefix session (born pinned at save; pin confirmed)"
            );
            Some(guard)
        }
        Err(err) => {
            tracing::warn!(
                validator = %name,
                session = %turn.session_id,
                error = %err,
                "failed to pin primed prefix state; falling back to monolithic prompts"
            );
            None
        }
    }
}

/// Release a primed prefix session's pin once its validator's batches have all
/// resolved, so the pinned cache entry does not outlive the fan-out. A failed
/// unpin is logged, never fatal — the entry falls back to normal eviction.
/// (Cancellation is covered separately: a fan-out future dropped before
/// reaching this point releases the pin from the guard's `Drop`.)
async fn unpin_prefix_session(guard: SessionPinGuard, validator: &str) {
    let session = guard.session_id().to_string();
    match guard.release().await {
        Ok(_) => tracing::debug!(
            validator = %validator,
            session = %session,
            "unpinned validator prefix session"
        ),
        Err(err) => tracing::warn!(
            validator = %validator,
            session = %session,
            error = %err,
            "failed to unpin validator prefix session"
        ),
    }
}

/// Resolve one forked batch task's delivered result into tagged findings.
///
/// A delivered turn is parsed exactly like the monolithic path, after logging
/// whether the fork was warm (parent state attached — with the reused token
/// count, so a run's prefill savings are measurable from the log) or degraded
/// (history cloned, cold prefill). A turn whose FORK failed falls back to the
/// monolithic fresh-session prompt for the batch — degraded but correct, never
/// a lost task. Any other failure degrades to zero findings like
/// [`collect_task`].
async fn collect_forked_task(
    delivered: Result<SessionTurnResult, tokio::sync::oneshot::error::RecvError>,
    change_purpose: &str,
    validator: &ValidatorWork,
    ruleset: &RuleSet,
    batch: &[FileWork],
    files: &[String],
    pool: &AgentPool,
) -> Result<Vec<Finding>, ()> {
    let name = validator.validator_name.as_str();
    match delivered {
        Ok(Ok(turn)) => {
            match turn.fork {
                Some(fork) if fork.state_attached => tracing::info!(
                    validator = %name,
                    files = ?files,
                    session = %turn.session_id,
                    reused_tokens = ?fork.prefix_tokens,
                    "fleet task ran on a warm fork of the primed prefix session"
                ),
                _ => tracing::warn!(
                    validator = %name,
                    files = ?files,
                    session = %turn.session_id,
                    "fleet task fork was degraded (no parent state attached); proceeding cold"
                ),
            }
            parse_task_response(&turn.content, name, files)
        }
        Ok(Err(PoolError::ForkFailed {
            parent_session_id,
            message,
        })) => {
            tracing::warn!(
                validator = %name,
                files = ?files,
                parent = %parent_session_id,
                error = %message,
                "fleet task fork failed; falling back to a monolithic fresh-session prompt"
            );
            let prompt = render_fleet_prompt(change_purpose, validator, ruleset, batch);
            collect_task(pool.submit(prompt).await, name, files)
        }
        Ok(Err(err)) => {
            tracing::warn!(
                validator = %name,
                files = ?files,
                error = %err,
                "fleet task failed; yielding zero findings for this batch"
            );
            Err(())
        }
        Err(_) => {
            tracing::warn!(
                validator = %name,
                files = ?files,
                "fleet task result was dropped before delivery; yielding zero findings"
            );
            Err(())
        }
    }
}

/// Resolve one task's delivered result into tagged findings.
///
/// Returns `Ok(findings)` for a task that delivered a parseable response (the
/// findings may legitimately be empty), and `Err(())` for any failure — a task
/// error, a dropped channel, or a response that did not parse. A failure is
/// logged and degrades the batch to zero findings (one bad task never aborts the
/// rest); the `Err` lets the caller tally it as failed rather than silently
/// conflating it with a clean batch.
fn collect_task(
    delivered: Result<crate::validators::PromptResult, tokio::sync::oneshot::error::RecvError>,
    validator: &str,
    files: &[String],
) -> Result<Vec<Finding>, ()> {
    let response = match delivered {
        Ok(Ok(response)) => response,
        Ok(Err(err)) => {
            tracing::warn!(
                validator = %validator,
                files = ?files,
                error = %err,
                "fleet task failed; yielding zero findings for this batch"
            );
            return Err(());
        }
        Err(_) => {
            tracing::warn!(
                validator = %validator,
                files = ?files,
                "fleet task result was dropped before delivery; yielding zero findings"
            );
            return Err(());
        }
    };

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

/// How many batches `file_count` files split into at `batch_size` per batch.
fn batch_count(file_count: usize, batch_size: usize) -> usize {
    file_count.div_ceil(batch_size.max(1))
}

/// Render the fan-out prompt for one `(validator, batch-of-files)` task — the
/// monolithic fallback shape (one fresh session, everything in one prompt).
///
/// The payload is assembled directly from the structured stage-1 data — there is
/// no template engine. The three sections are, in order: the change purpose, the
/// validator's instructions (mandate + rule bodies + severity default + the
/// output contract), and one self-contained block per file in the batch (path +
/// semantic diff + bounded source slice + probe evidence).
///
/// The warm path splits the same bytes across two turns instead:
/// [`render_validator_prefix`] (the first two sections, primed once per
/// validator) and [`render_file_payload`] (the third, sent on each fork). This
/// function composes from the identical pieces, so the two paths never drift.
///
/// `validator` is the work-list entry (its name and the file work); `ruleset` is
/// the same validator's loaded [`RuleSet`], the authoritative source of the
/// mandate (its description) and the verbatim rule bodies.
pub fn render_fleet_prompt(
    change_purpose: &str,
    validator: &ValidatorWork,
    ruleset: &RuleSet,
    files: &[FileWork],
) -> String {
    let mut out = render_shared_sections(change_purpose, validator, ruleset);
    out.push_str(&render_file_payload(files));
    out
}

/// The sentence the prime turn ends with: an explicit completed-turn handoff so
/// the parent session's end-of-turn KV snapshot lands exactly at the boundary
/// every fork's payload prompt continues from.
///
/// Crate-visible so the scripted test agent (`review::test_support`) recognizes
/// prime turns by this exact constant rather than a re-typed fragment — the
/// handoff wording changes in exactly one place.
pub(crate) const PRIME_HANDOFF: &str =
    "Reply with exactly OK. The files to review arrive in the next message.\n";

/// Render the shared per-validator prompt prefix the prime turn decodes once:
/// the change purpose + the validator's instructions (mandate, rule bodies,
/// severity default, output contract), ending with [`PRIME_HANDOFF`].
///
/// The render is a pure function of its inputs — byte-stable across calls — so
/// every batch fork of the primed session shares the exact prefix bytes the
/// parent decoded, and the fork's first decode reuses the full saved state.
pub fn render_validator_prefix(
    change_purpose: &str,
    validator: &ValidatorWork,
    ruleset: &RuleSet,
) -> String {
    let mut out = render_shared_sections(change_purpose, validator, ruleset);
    out.push_str(PRIME_HANDOFF);
    out
}

/// Render the per-batch payload a forked session is prompted with: ONLY the
/// file blocks (path + semantic diff + bounded source slice + probe evidence).
/// The rules and contract are already in the fork's inherited prefix.
pub fn render_file_payload(files: &[FileWork]) -> String {
    let mut out = String::new();
    out.push_str("# Files under review\n\n");
    for file in files {
        render_file_block(&mut out, file);
    }
    out
}

/// Render the sections every prompt shape shares: the change purpose followed by
/// the validator's instructions. Both the monolithic prompt and the primed
/// prefix are built on these exact bytes.
fn render_shared_sections(
    change_purpose: &str,
    validator: &ValidatorWork,
    ruleset: &RuleSet,
) -> String {
    let mut out = String::new();
    out.push_str("# Change purpose\n\n");
    out.push_str(change_purpose.trim());
    out.push_str("\n\n");
    render_validator_instructions(&mut out, validator, ruleset);
    out
}

/// Append the validator-instructions section: mandate, rule bodies, severity
/// default, and the finding output contract.
fn render_validator_instructions(out: &mut String, validator: &ValidatorWork, ruleset: &RuleSet) {
    let _ = writeln!(out, "# Validator: {}\n", validator.validator_name);
    out.push_str("## Mandate\n\n");
    out.push_str(ruleset.description().trim());
    out.push_str("\n\n");

    out.push_str("## Rules\n\n");
    for rule in &ruleset.rules {
        let _ = writeln!(out, "### Rule: {}\n", rule.name);
        out.push_str(rule.body.trim());
        out.push_str("\n\n");
    }

    let _ = writeln!(
        out,
        "## Default severity\n\nUnless a rule states otherwise, findings default to severity `{}`.\n",
        severity_default(validator.severity)
    );

    out.push_str(OUTPUT_CONTRACT);
    out.push('\n');
}

/// The validator's default severity as the `blocker`/`warning`/`nit` word the
/// [`Finding`] severity field uses, so the contract speaks the agent's output
/// vocabulary rather than the loader's internal `info`/`warn`/`error`.
fn severity_default(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "blocker",
        Severity::Warn => "warning",
        Severity::Info => "nit",
    }
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

The changed files under review are already provided in full below — their \
COMPLETE current contents are inlined, so do NOT `read_file` (or `glob`/`grep`) \
the changed files; you already have them. `read_file`/`glob`/`grep` remain \
available, but only for OTHER files: cross-file duplication evidence, a changed \
symbol's callers, or a type defined elsewhere. Reach for them only when a \
finding genuinely depends on a file that is not already inlined here.

## Output contract

Once you have reviewed the inlined files (reading other files only if needed), \
reply with your findings as a JSON array, written directly as the plain text of \
your reply — the reply is parsed as JSON. The findings reply itself must be \
plain JSON text, never a tool call: a tool call is not a valid way to report \
findings.

Each finding is one object with these fields:

- `file`: the path of the file the finding is about.
- `line`: the 1-based line number the finding points at.
- `rule`: which rule of this validator fired.
- `severity`: one of `blocker`, `warning`, `nit`.
- `claim`: what is wrong AND why it matters — one concern per finding.
- `evidence`: the proof the issue is real — cite the injected probe result \
(e.g. \"per `duplicates`: 0.94 at `bar.rs:88`\") or a `file:line` citation.
- `suggestion`: the fix.

Report only real issues. If you find none, emit an empty array `[]`.
";

/// Append one file's review block: path, the full current source (or the bounded
/// fallback for an oversized file), the semantic diff of what changed, and the
/// probe results rendered as evidence.
///
/// The changed file is handed to the model **in full** when it fits the inline
/// budget ([`FileWork::inlined_full`]), framed explicitly as the complete current
/// contents the model does NOT need to re-read — the read-round-trips that
/// dominated review wall-clock came from the model re-reading a file it was only
/// given a partial slice of. An oversized file falls back to the bounded slice
/// (which already carries a `read_file` note from the scope stage) and is framed
/// as a partial view.
fn render_file_block(out: &mut String, file: &FileWork) {
    let _ = writeln!(out, "## File: {}\n", file.path);

    if file.inlined_full {
        out.push_str(
            "### Full current contents\n\n\
             This is the COMPLETE current source of the file. You do not need to read this \
             file — it is provided here in full. Review it directly.\n\n",
        );
    } else {
        out.push_str(
            "### Source slice (partial — file too large to inline in full)\n\n\
             This is a BOUNDED slice of an oversized file, not its complete contents. \
             Use `read_file` on this path to see the remainder before reasoning about it.\n\n",
        );
    }
    out.push_str("```\n");
    out.push_str(file.source_slice.trim_end());
    out.push_str("\n```\n\n");

    out.push_str("### What changed (semantic diff)\n\n");
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

    // ---- fixtures --------------------------------------------------------

    /// A RuleSet whose mandate (description) and rule bodies are distinctive so
    /// the rendered prompt can be asserted against them verbatim.
    fn ruleset(name: &str, mandate: &str, rules: &[(&str, &str)]) -> RuleSet {
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
                severity: Severity::Warn,
                timeout: 30,
                once: false,
            },
            rules: rules
                .iter()
                .map(|(rname, body)| Rule {
                    name: rname.to_string(),
                    description: format!("{rname} description"),
                    body: body.to_string(),
                    severity: None,
                    timeout: None,
                })
                .collect(),
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
            inlined_full: true,
            probe_results: vec![ProbeResult {
                name: "duplicates".to_string(),
                kind: ProbeKind::Fact,
                target: path.to_string(),
                rows: vec![ProbeRow {
                    file_path: dup_at.to_string(),
                    symbol: Some(symbol.to_string()),
                    line: Some(88),
                    similarity: Some(0.94),
                    detail: None,
                }],
            }],
        }
    }

    fn validator_work(name: &str, files: Vec<FileWork>) -> ValidatorWork {
        ValidatorWork {
            validator_name: name.to_string(),
            severity: Severity::Warn,
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
    fn forking_agent(script: Vec<(String, ScriptedReply)>) -> Arc<ScriptedAgent> {
        ScriptedAgent::with_config(
            script,
            ScriptedAgentConfig {
                fork_mode: ForkMode::Supported,
                ..ScriptedAgentConfig::default()
            },
        )
    }

    /// A degraded fork-capable scripted agent in the given [`ForkMode`].
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

    // ---- renderer tests (pure) -------------------------------------------

    #[test]
    fn prompt_contains_change_purpose_mandate_rules_and_output_contract() {
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

        let prompt = render_fleet_prompt("PURPOSE: scaffolding the parser.", &vw, &rs, &vw.files);

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
        // Output contract: the four load-bearing finding fields.
        assert!(prompt.contains("`rule`"), "{prompt}");
        assert!(prompt.contains("`claim`"), "{prompt}");
        assert!(prompt.contains("`evidence`"), "{prompt}");
        assert!(prompt.contains("`suggestion`"), "{prompt}");
        // Severity default rendered from the validator severity (warn → warning).
        assert!(prompt.contains("severity `warning`"), "{prompt}");
    }

    #[test]
    fn prompt_renders_the_files_probe_evidence_and_excludes_other_files() {
        let rs = ruleset("deduplicate", "mandate", &[("r", "rule body")]);
        let vw = validator_work(
            "deduplicate",
            vec![
                file_work("src/a.rs", "alpha", "src/dup_of_a.rs"),
                file_work("src/b.rs", "beta", "src/dup_of_b.rs"),
            ],
        );

        // Render a batch of JUST the first file.
        let prompt = render_fleet_prompt("purpose", &vw, &rs, &vw.files[..1]);

        // This file's path, symbol, slice, and probe evidence are present.
        assert!(prompt.contains("src/a.rs"), "{prompt}");
        assert!(prompt.contains("alpha"), "{prompt}");
        assert!(prompt.contains("// slice for src/a.rs"), "{prompt}");
        assert!(
            prompt.contains("probe `duplicates`"),
            "probe evidence must be rendered: {prompt}"
        );
        assert!(prompt.contains("src/dup_of_a.rs:88"), "{prompt}");
        assert!(prompt.contains("@ 0.94"), "{prompt}");

        // The OTHER file's content is excluded from this task's prompt.
        assert!(
            !prompt.contains("src/b.rs"),
            "other file must be excluded: {prompt}"
        );
        assert!(
            !prompt.contains("beta"),
            "other file's symbol must be excluded: {prompt}"
        );
        assert!(!prompt.contains("src/dup_of_b.rs"), "{prompt}");
    }

    #[test]
    fn prefix_and_payload_split_is_byte_stable_and_composes_the_monolithic_prompt() {
        let rs = ruleset(
            "deduplicate",
            "DEDUP_MANDATE: never copy-paste logic.",
            &[("no-copy-paste", "RULE_BODY: extract shared helpers.")],
        );
        let vw = validator_work(
            "deduplicate",
            vec![file_work("src/a.rs", "alpha", "src/x.rs")],
        );

        // Byte-stable: two renders of the same inputs are identical, so every
        // fork shares the exact prefix the prime turn decoded.
        let prefix = render_validator_prefix("purpose", &vw, &rs);
        assert_eq!(
            prefix,
            render_validator_prefix("purpose", &vw, &rs),
            "the prefix render must be byte-stable across calls"
        );
        let payload = render_file_payload(&vw.files);
        assert_eq!(payload, render_file_payload(&vw.files));

        // The prefix carries the shared sections and ends with the prime
        // handoff — a completed turn whose KV lands where continuations begin.
        assert!(prefix.contains("DEDUP_MANDATE"), "{prefix}");
        assert!(prefix.contains("RULE_BODY"), "{prefix}");
        assert!(prefix.contains("## Output contract"), "{prefix}");
        assert!(
            prefix.ends_with(PRIME_HANDOFF),
            "the prefix must end with the prime handoff: {prefix}"
        );
        assert!(
            !prefix.contains("# Files under review"),
            "the prefix must not carry any file payload: {prefix}"
        );
        // The cached/primed prefix must not carry the per-file source — that
        // (now full-file) content lives only in the forked payload, so the
        // shared prefix bytes (and their cache) are unaffected by file size.
        assert!(
            !prefix.contains("// slice for src/a.rs") && !prefix.contains("fn alpha"),
            "the prefix must not carry the file's source contents: {prefix}"
        );

        // The payload carries ONLY the file blocks — no rules, no contract.
        assert!(payload.starts_with("# Files under review"), "{payload}");
        assert!(payload.contains("## File: src/a.rs"), "{payload}");
        assert!(!payload.contains("## Mandate"), "{payload}");
        assert!(!payload.contains("## Output contract"), "{payload}");

        // The monolithic fallback prompt is exactly the shared sections (the
        // prefix minus the handoff) followed by the payload.
        let monolithic = render_fleet_prompt("purpose", &vw, &rs, &vw.files);
        let shared = prefix
            .strip_suffix(PRIME_HANDOFF)
            .expect("prefix ends with the handoff");
        assert_eq!(
            monolithic,
            format!("{shared}{payload}"),
            "monolithic prompt must compose from the same shared sections + payload"
        );
    }

    /// A small (fully-inlined) changed file's payload carries the file's
    /// COMPLETE current contents in a clearly-labeled fenced block plus explicit
    /// "you do NOT need to read this file" framing — so the model stops
    /// re-reading the changed file it was already handed.
    #[test]
    fn full_inline_payload_carries_complete_source_and_no_reread_framing() {
        // A FileWork whose source_slice is the WHOLE file (inlined_full = true),
        // including a marker line the old bounded slice would have trimmed.
        let mut file = file_work("src/a.rs", "alpha", "src/x.rs");
        file.source_slice =
            "use std::fmt;\n// distant_marker_kept_in_full\npub fn alpha() {}".to_string();
        file.inlined_full = true;

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
    }

    /// An oversized (fallback) changed file's payload carries the bounded slice
    /// and the read-the-rest note carried through from the scope stage, and does
    /// NOT claim to be the complete file.
    #[test]
    fn fallback_payload_keeps_the_read_for_the_rest_note() {
        let mut file = file_work("src/big.rs", "huge", "src/x.rs");
        file.source_slice =
            "// bounded slice\npub fn huge() {}\n\n// NOTE: this file is too large to inline in full; \
the slice above is bounded. Use `read_file` on this path to see the remainder before reasoning about it."
                .to_string();
        file.inlined_full = false;

        let payload = render_file_payload(std::slice::from_ref(&file));

        assert!(
            payload.contains("read_file"),
            "the fallback payload must direct the model to read_file for the rest: {payload}"
        );
        assert!(
            !payload.to_lowercase().contains("do not need to read"),
            "an oversized file must NOT be framed as fully provided: {payload}"
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

    #[test]
    fn severity_default_maps_to_finding_vocabulary() {
        assert_eq!(severity_default(Severity::Error), "blocker");
        assert_eq!(severity_default(Severity::Warn), "warning");
        assert_eq!(severity_default(Severity::Info), "nit");
    }

    // ---- batching tests (pure) -------------------------------------------

    #[test]
    fn batch_count_packs_files_into_bounded_batches() {
        assert_eq!(batch_count(0, 4), 0);
        assert_eq!(batch_count(1, 4), 1);
        assert_eq!(batch_count(4, 4), 1);
        assert_eq!(batch_count(5, 4), 2);
        assert_eq!(batch_count(8, 4), 2);
        assert_eq!(batch_count(9, 4), 3);
        // A zero batch size is clamped to 1 (one task per file), never a panic.
        assert_eq!(batch_count(3, 0), 3);
    }

    // ---- orchestrator tests (scripted mock agent) ------------------------

    #[tokio::test]
    async fn fan_out_two_validators_two_files_submits_at_most_four_tasks() {
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

        // Script: a finding for val-a on src/a.rs, a finding for val-b on
        // src/b.rs, empty for the rest (matched by validator + file in prompt).
        let agent = forking_agent(vec![
            (
                "# Validator: val-a".to_string() + "\n\n## Mandate",
                ScriptedReply::Text(findings_json("src/a.rs", 42, "ra", "warning", "dup in a")),
            ),
            (
                "# Validator: val-b".to_string() + "\n\n## Mandate",
                ScriptedReply::Text(findings_json("src/b.rs", 42, "rb", "warning", "dup in b")),
            ),
        ]);
        let agent_probe = Arc::clone(&agent);

        // batch_size=1 → file-grain: 2 validators × 2 files = 4 tasks (plus one
        // prefix prime per validator).
        let findings = with_pool(agent, PoolConfig::remote(4), move |pool| async move {
            run_fleet(&work, &loader, &pool, FleetConfig { batch_size: 1 })
                .await
                .findings
        })
        .await;

        let seen = agent_probe.seen_prompts();
        let payloads = seen
            .iter()
            .filter(|p| p.starts_with("# Files under review"))
            .count();
        assert_eq!(
            payloads, 4,
            "2 validators × 2 files at batch_size 1 = 4 payload tasks: {seen:#?}"
        );
        let primes = seen.iter().filter(|p| p.contains(PRIME_HANDOFF)).count();
        assert_eq!(primes, 2, "one prefix prime per validator: {seen:#?}");

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

    #[tokio::test]
    async fn many_small_files_collapse_into_fewer_batched_tasks() {
        let rs = ruleset("val", "mandate", &[("r", "body")]);
        let loader = loader_with(vec![rs]);

        // 10 files for one validator.
        let files: Vec<FileWork> = (0..10)
            .map(|i| file_work(&format!("src/f{i}.rs"), &format!("sym{i}"), "src/x.rs"))
            .collect();
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work("val", files)],
        };

        let agent = forking_agent(vec![]);
        let agent_probe = Arc::clone(&agent);

        // batch_size=4 → 10 files collapse into ceil(10/4) = 3 tasks.
        let _findings = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
            run_fleet(&work, &loader, &pool, FleetConfig { batch_size: 4 }).await
        })
        .await;

        let seen = agent_probe.seen_prompts();
        let payloads: Vec<&String> = seen
            .iter()
            .filter(|p| p.starts_with("# Files under review"))
            .collect();
        assert_eq!(
            payloads.len(),
            3,
            "10 small files at batch_size 4 collapse into 3 tasks, not 10: {seen:#?}"
        );
        // Each batched task carries multiple files (the grain stays the file:
        // each is its own block, the batch just packs them).
        let file_blocks = payloads[0].matches("## File: ").count();
        assert_eq!(file_blocks, 4, "the first batch packs 4 file blocks");
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn batching_applied_is_logged() {
        let rs = ruleset("val", "mandate", &[("r", "body")]);
        let loader = loader_with(vec![rs]);

        let files: Vec<FileWork> = (0..5)
            .map(|i| file_work(&format!("src/f{i}.rs"), &format!("sym{i}"), "src/x.rs"))
            .collect();
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work("val", files)],
        };

        let agent = forking_agent(vec![]);
        let _findings = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
            run_fleet(&work, &loader, &pool, FleetConfig { batch_size: 2 }).await
        })
        .await;

        // The fan-out logs the batching it applied: 5 files at batch_size 2 → 3
        // batches, attributed to the validator.
        assert!(logs_contain(
            "fleet fan-out: batching files into agent tasks"
        ));
        assert!(logs_contain("batches=3"));
        assert!(logs_contain("batch_size=2"));
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
            run_fleet(&work, &loader, &pool, FleetConfig::default()).await
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
    async fn prefix_is_primed_once_per_validator_and_batches_fork_payload_only() {
        let rs = ruleset(
            "val",
            "MANDATE_MARKER mandate",
            &[("r", "RULE_MARKER body")],
        );
        let loader = loader_with(vec![rs]);

        let files: Vec<FileWork> = (0..4)
            .map(|i| file_work(&format!("src/f{i}.rs"), &format!("sym{i}"), "src/x.rs"))
            .collect();
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work("val", files)],
        };

        // The scripted response is keyed on a file in the FIRST batch; the fork
        // inherits the prefix, so the script context is prefix + payload.
        let agent = forking_agent(vec![(
            "## File: src/f0.rs".to_string(),
            ScriptedReply::Text(findings_json(
                "src/f0.rs",
                42,
                "r",
                "warning",
                "warm finding",
            )),
        )]);
        let agent_probe = Arc::clone(&agent);

        // 4 files at batch_size 2 → 1 prime + 2 forked payload tasks.
        let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
            run_fleet(&work, &loader, &pool, FleetConfig { batch_size: 2 }).await
        })
        .await;

        let seen = agent_probe.seen_prompts();
        let primes: Vec<&String> = seen.iter().filter(|p| p.contains(PRIME_HANDOFF)).collect();
        assert_eq!(
            primes.len(),
            1,
            "the shared prefix is primed exactly once per validator: {seen:#?}"
        );
        assert!(
            primes[0].contains("MANDATE_MARKER") && primes[0].contains("RULE_MARKER"),
            "the prime carries the validator's rules: {}",
            primes[0]
        );

        let payloads: Vec<&String> = seen
            .iter()
            .filter(|p| p.starts_with("# Files under review"))
            .collect();
        assert_eq!(
            payloads.len(),
            2,
            "each batch forks the primed session and sends ONLY the payload: {seen:#?}"
        );
        assert!(
            payloads
                .iter()
                .all(|p| !p.contains("MANDATE_MARKER") && !p.contains("RULE_MARKER")),
            "payload prompts must not re-send the rules: {payloads:#?}"
        );
        assert_eq!(agent_probe.fork_count(), 2, "one fork per batch");

        assert_eq!(outcome.attempted, 2);
        assert_eq!(outcome.failed, 0);
        assert_eq!(outcome.findings.len(), 1, "{:#?}", outcome.findings);
        assert_eq!(outcome.findings[0].claim, "warm finding");
        assert_eq!(outcome.findings[0].validator, "val");

        // The prefix state was pinned for the fan-out and unpinned at the end.
        assert_eq!(
            agent_probe.pin_calls(),
            vec![("sess-0".to_string(), true), ("sess-0".to_string(), false)],
            "pin for the fan-out, unpin when the validator's batches complete"
        );

        // Observability: each fork task logs the warm reuse and token count.
        assert!(logs_contain("fleet task ran on a warm fork"));
        assert!(logs_contain(&format!(
            "reused_tokens=Some({MOCK_PREFIX_TOKENS})"
        )));
        assert!(logs_contain("primed validator prefix session"));
    }

    /// The primed prefix is born pinned through the PRODUCTION prime path:
    /// `prime_validator_prefix` → `submit_primed` → the prompt's `_meta`
    /// pin-on-save intent → the agent saving its prefix pinned atomically at
    /// turn completion — BEFORE any separate `session/pin` confirm runs. This is
    /// the end-to-end (scripted agent, no real model) assertion for the
    /// structural close of the prime→pin eviction race: the prefix is never an
    /// unpinned eviction candidate, independent of any post-turn pin.
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
            run_fleet(&work, &loader, &pool, FleetConfig { batch_size: 2 }).await
        })
        .await;

        // The prefix session the validator primed (`sess-0`) was born pinned by
        // the prime turn's `_meta` intent — recorded at turn completion, before
        // the post-turn `session/pin` confirm. Forked batch sessions are NOT
        // born pinned (they save their own cold state unpinned).
        assert_eq!(
            agent_probe.born_pinned_sessions(),
            vec!["sess-0".to_string()],
            "the primed prefix must be born pinned through the production prime path, \
             and only the prefix (not the forked batch sessions)"
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

        // Every `session/fork` is rejected; the batch tasks must fall back to
        // fresh-session monolithic prompts and still deliver their findings.
        let agent = agent_with_fork_mode(
            vec![(
                "## File: src/a.rs".to_string(),
                ScriptedReply::Text(findings_json(
                    "src/a.rs",
                    42,
                    "r",
                    "warning",
                    "found despite fork failure",
                )),
            )],
            ForkMode::RejectFork,
        );
        let agent_probe = Arc::clone(&agent);

        let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
            run_fleet(&work, &loader, &pool, FleetConfig { batch_size: 1 }).await
        })
        .await;

        assert_eq!(outcome.attempted, 2);
        assert_eq!(outcome.failed, 0, "a failed fork is never a lost task");
        assert_eq!(outcome.findings.len(), 1);
        assert_eq!(outcome.findings[0].claim, "found despite fork failure");

        // The fallback prompts are the full monolithic shape (rules + files).
        let seen = agent_probe.seen_prompts();
        let monolithic = seen
            .iter()
            .filter(|p| p.contains("## Mandate") && p.contains("# Files under review"))
            .count();
        assert_eq!(
            monolithic, 2,
            "each batch fell back to a monolithic prompt: {seen:#?}"
        );
        assert!(logs_contain("falling back to a monolithic"));

        // The prime succeeded, so it was pinned and is still unpinned at the end.
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
        // its state can never be confirmed, so the whole validator degrades to
        // monolithic prompts — never a lost task.
        let agent = agent_with_fork_mode(
            vec![(
                "## File: src/b.rs".to_string(),
                ScriptedReply::Text(findings_json(
                    "src/b.rs",
                    42,
                    "r",
                    "warning",
                    "found without forks",
                )),
            )],
            ForkMode::Unsupported,
        );
        let agent_probe = Arc::clone(&agent);

        let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
            run_fleet(&work, &loader, &pool, FleetConfig { batch_size: 1 }).await
        })
        .await;

        assert_eq!(outcome.attempted, 2);
        assert_eq!(outcome.failed, 0);
        assert_eq!(outcome.findings.len(), 1);
        assert_eq!(outcome.findings[0].claim, "found without forks");

        let seen = agent_probe.seen_prompts();
        let monolithic = seen
            .iter()
            .filter(|p| p.contains("## Mandate") && p.contains("# Files under review"))
            .count();
        assert_eq!(monolithic, 2, "{seen:#?}");
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
            vec![(
                "## File: src/a.rs".to_string(),
                ScriptedReply::Text(findings_json(
                    "src/a.rs",
                    42,
                    "r",
                    "warning",
                    "cold but correct",
                )),
            )],
            ForkMode::DegradedAttach,
        );

        let outcome = with_pool(agent, PoolConfig::local(), move |pool| async move {
            run_fleet(&work, &loader, &pool, FleetConfig { batch_size: 1 }).await
        })
        .await;

        assert_eq!(outcome.attempted, 1);
        assert_eq!(outcome.failed, 0);
        assert_eq!(outcome.findings.len(), 1);
        assert_eq!(outcome.findings[0].claim, "cold but correct");
        assert!(logs_contain("fleet task fork was degraded"));
    }

    #[tokio::test]
    async fn prefix_session_is_unpinned_even_when_a_batch_task_errors() {
        let rs = ruleset("val", "mandate", &[("r", "body")]);
        let loader = loader_with(vec![rs]);
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work(
                "val",
                vec![
                    file_work("src/good.rs", "good", "src/x.rs"),
                    file_work("src/bad.rs", "bad", "src/y.rs"),
                ],
            )],
        };

        // One forked batch task errors; the unpin must still happen.
        let agent = forking_agent(vec![(
            "## File: src/bad.rs".to_string(),
            ScriptedReply::Error,
        )]);
        let agent_probe = Arc::clone(&agent);

        let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
            run_fleet(&work, &loader, &pool, FleetConfig { batch_size: 1 }).await
        })
        .await;

        assert_eq!(outcome.attempted, 2);
        assert_eq!(outcome.failed, 1, "the erroring fork task is a failed task");
        assert_eq!(
            agent_probe.pin_calls(),
            vec![("sess-0".to_string(), true), ("sess-0".to_string(), false)],
            "the prefix pin is released even when a batch task errors"
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

    /// Cancellation-safety regression: a fan-out future dropped mid-collect
    /// (review cancelled, caller timeout) must STILL release the prefix pin —
    /// a pinned session is exempt from cache eviction, so a leaked pin
    /// outlives the review until process restart.
    #[tokio::test]
    async fn prefix_pin_is_released_when_the_fanout_future_is_dropped_mid_collect() {
        let rs = ruleset("val", "mandate", &[("r", "body")]);
        let loader = loader_with(vec![rs]);
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work(
                "val",
                vec![file_work("src/a.rs", "alpha", "src/x.rs")],
            )],
        };

        // The payload turn wedges forever, holding the fan-out mid-collect.
        let agent = forking_agent(vec![(
            "# Files under review".to_string(),
            ScriptedReply::Stall,
        )]);
        let agent_probe = Arc::clone(&agent);

        with_pool(agent, PoolConfig::remote(2), move |pool| async move {
            let fanout = tokio::spawn(async move {
                run_fleet(&work, &loader, &pool, FleetConfig { batch_size: 1 }).await
            });

            // Wait until the prefix is pinned and the wedged payload fork is
            // in flight — the fan-out is now mid-collect.
            wait_for("the prefix pin and the wedged payload fork", || {
                agent_probe
                    .pin_calls()
                    .contains(&("sess-0".to_string(), true))
                    && agent_probe
                        .seen_prompts()
                        .iter()
                        .any(|p| p.starts_with("# Files under review"))
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
        let rs = ruleset("val", "mandate", &[("r", "body")]);
        let loader = loader_with(vec![rs]);

        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work(
                "val",
                vec![
                    file_work("src/good.rs", "good", "src/x.rs"),
                    file_work("src/bad.rs", "bad", "src/y.rs"),
                ],
            )],
        };

        // The task whose prompt mentions src/bad.rs errors; the good one returns
        // a finding.
        let agent = forking_agent(vec![
            ("## File: src/bad.rs".to_string(), ScriptedReply::Error),
            (
                "## File: src/good.rs".to_string(),
                ScriptedReply::Text(findings_json(
                    "src/good.rs",
                    42,
                    "r",
                    "warning",
                    "real issue",
                )),
            ),
        ]);

        let outcome = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
            run_fleet(&work, &loader, &pool, FleetConfig { batch_size: 1 }).await
        })
        .await;

        // The erroring task contributed nothing; the good one still returned.
        assert_eq!(
            outcome.findings.len(),
            1,
            "the failing task degrades to zero findings"
        );
        assert_eq!(outcome.findings[0].claim, "real issue");
        assert_eq!(outcome.findings[0].validator, "val");
        // The tally records both tasks attempted and exactly the one that failed.
        assert_eq!(
            outcome.attempted, 2,
            "two (validator, file) tasks attempted"
        );
        assert_eq!(outcome.failed, 1, "the erroring task is counted as failed");
    }

    #[tokio::test]
    async fn all_tasks_failing_yields_zero_findings_and_a_full_failure_tally() {
        let rs = ruleset("val", "mandate", &[("r", "body")]);
        let loader = loader_with(vec![rs]);

        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work(
                "val",
                vec![
                    file_work("src/a.rs", "a", "src/x.rs"),
                    file_work("src/b.rs", "b", "src/y.rs"),
                    file_work("src/c.rs", "c", "src/z.rs"),
                ],
            )],
        };

        // Every (validator, file) task errors.
        let agent = forking_agent(vec![("## File:".to_string(), ScriptedReply::Error)]);

        let outcome = with_pool(agent, PoolConfig::remote(3), move |pool| async move {
            run_fleet(&work, &loader, &pool, FleetConfig { batch_size: 1 }).await
        })
        .await;

        assert!(
            outcome.findings.is_empty(),
            "every task failed, so there are no findings"
        );
        assert_eq!(outcome.attempted, 3, "three tasks attempted");
        assert_eq!(outcome.failed, 3, "all three failed");
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
            run_fleet(&work, &loader, &pool, FleetConfig::default()).await
        })
        .await;

        assert!(
            outcome.findings.is_empty(),
            "an unknown validator yields no findings"
        );
        assert_eq!(
            outcome.attempted, 0,
            "no task is attempted for a validator missing from the loader"
        );
        assert_eq!(outcome.failed, 0);
        assert!(
            agent_probe.seen_prompts().is_empty(),
            "no task is submitted for a validator missing from the loader"
        );
    }
}
