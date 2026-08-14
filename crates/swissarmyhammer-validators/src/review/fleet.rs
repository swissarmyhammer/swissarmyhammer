//! Engine stage 2 — the fan-out fleet.
//!
//! The shard is the **validator**: this stage takes the stage-1
//! [`WorkList`](crate::review::WorkList) and produces one agent task per
//! validator, submitting every task to the shared
//! [`AgentPool`](crate::validators::AgentPool). Each task reviews the change —
//! every file under review, with the engine-run probe evidence stage 1 already
//! gathered — against ONE validator's prompt rules (tool rules are executed
//! separately by the tool runner, never by an LLM), and returns a
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
//!    validator's instructions — its prompt rules + output contract), decoding
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
//!   the validator's files in scope, every prompt rule body verbatim (tool
//!   rules are executed separately by the tool runner), and the output
//!   contract (every finding emits `rule` + `claim` +
//!   `evidence` + `suggestion`, matching the [`Finding`] type).
//! - [`render_fleet_prompt`] (degraded fallback): the change purpose, the
//!   validator's own files, and the validator suffix, in one fresh-session prompt.

use std::collections::{BTreeMap, BTreeSet};

use crate::review::scope::{
    BatchBudget, BatchBytes, FileCapBytes, FileWork, ProbeNames, ReviewSubject, RuleNames,
    ValidatorWork, WorkList,
};
use crate::review::tool_rules::ToolSuppression;
use crate::review::types::{parse_findings_repaired, Finding};
use crate::validators::types::Rule;
use crate::validators::{
    AgentPool, PoolError, RuleSet, SessionPinGuard, SessionTurn, SessionTurnResult, ValidatorLoader,
};
use agent_client_protocol::schema::SessionId;

mod prime;
mod render;

use prime::prime_run_prefix;
pub use prime::{classify_reuse, unpin_prefix_session, PrefixReuse};
pub use render::{
    prompt_framing, prompt_framing_bytes, render_file_payload, render_fleet_prompt,
    render_run_prime, render_shared_probe_evidence, render_validator_suffix,
    rendered_file_block_bytes, PromptFraming,
};
pub(crate) use render::{render_numbered_lines, SourceView};

/// The agent prompt cap every review prompt must fit inside, in **bytes**.
///
/// Read from [`claude_agent`], never re-declared here. That is the whole point:
/// the batch budget and the cap the agent rejects a prompt against used to be
/// separate numbers in separate crates, so the batcher packed roughly 4x what
/// the agent would accept and every fat batch came back as a bare
/// `invalid_params` with no message (see `^6jsxjbc`). One declaration means the
/// two cannot silently disagree again.
///
/// A prompt is rejected against the `max_prompt_length` of the agent actually
/// serving the run. ACP exposes no prompt-length capability to query, so this
/// compile-time constant is the single source both sides read:
/// `claude_agent::AgentConfig`'s default is this value, and
/// `swissarmyhammer_agent` sets its Claude config from the same constant.
///
/// Re-exported from [`crate::validators::AGENT_PROMPT_CAP`], where the
/// [`AgentPool`] that enforces it lives.
pub use crate::validators::AGENT_PROMPT_CAP;

/// The default review `batch_size` in **bytes** — the agent's prompt cap.
///
/// Cramming every changed file's full content into one prompt overflows the
/// review model on a large diff (every fan-out validator then fails uniformly),
/// so a run is split into budgeted batches that each fan out independently. The
/// budget is not an independent knob: it is [`AGENT_PROMPT_CAP`], from which
/// [`FleetConfig::file_payload_budget`] subtracts the run's measured framing to
/// get what is left for file blocks.
///
/// # The budget measures the RENDERED prompt
///
/// [`batch_work_list`](crate::review::scope::batch_work_list) is handed a cost
/// function and budgets on what it returns. The fleet passes
/// [`rendered_file_block_bytes`], which renders the file through the very
/// renderer the prompt uses, so the measured number and the sent number are the
/// same bytes.
///
/// Nothing derived from raw source bytes can work here. [`render_file_block`]
/// emits each line as `{line:>6} | {sha:8} {mark} | {text}` — about 16 fixed
/// bytes PER LINE — so expansion is a function of line COUNT, not of byte
/// count: a file of 4-byte lines renders at ~5x, a file of 200-byte lines at
/// ~1.08x. A fixed multiplier is therefore most wrong for exactly the files
/// most likely to overflow. And source is not even the dominant term: a
/// production `duplication` prompt measured 14.9 MB, of which 14.3 MB was probe
/// evidence and 0.1 MB was source. Rendering is the only honest measure.
pub const DEFAULT_BATCH_SIZE: usize = AGENT_PROMPT_CAP;

/// How many equal shares of [`AGENT_PROMPT_CAP`] a prompt is split into when
/// deciding how much of it ONE file block may occupy.
///
/// Two: one share for the file, one for the framing the same prompt must also
/// carry (the change purpose, the shared probe evidence, and the validator's
/// whole ruleset). A file that cannot leave the framing its half is too large
/// to review in one prompt however small the rest of the change is.
const PROMPT_SHARES_PER_FILE_BLOCK: usize = 2;

/// The largest RENDERED block one file may contribute to a review prompt, in
/// bytes — one share of [`AGENT_PROMPT_CAP`].
///
/// This is a **constant**, and that is the whole point (^tsram0q). The over-cap
/// verdict it decides tells the author to split the file, so it must be a
/// property of the file alone. Measuring it against what the run's framing left
/// over made it a property of the CHANGE: splitting a file grew the change, a
/// bigger change rendered more framing, and the smaller remainder put more
/// files over cap on the next run. Each round made the next one worse.
///
/// [`FleetConfig::file_block_cap`] applies it, taking the stricter of this and
/// the caller's `batch_size`.
pub const MAX_FILE_BLOCK_BYTES: usize = AGENT_PROMPT_CAP / PROMPT_SHARES_PER_FILE_BLOCK;

/// The largest RENDERED framing a batch's prompt may carry, in bytes — the
/// share of [`AGENT_PROMPT_CAP`] left over once one file block has taken its own
/// ([`MAX_FILE_BLOCK_BYTES`]).
///
/// This is the bound that keeps a packed batch sendable. A batch's prompt is
/// framing plus file blocks, and the packer bounds the blocks two different
/// ways:
///
/// - a multi-file batch by [`FleetConfig::file_payload_budget`], which is the
///   cap MINUS the framing, so that batch fits whatever the framing is;
/// - a batch holding one file too big for that budget by the constant
///   [`MAX_FILE_BLOCK_BYTES`], which never reads the framing (^tsram0q, and it
///   must not — an over-cap verdict has to be a property of the file alone).
///
/// Only the second case can overflow, and it cannot once the framing stays
/// inside this bound. It measured 469950 bytes on a real 15-file change of this
/// repo — 90% of the whole cap — so a file at the per-file cap made a prompt
/// `AgentPool::enqueue` refused outright (^x8z9hgf).
pub const MAX_FRAMING_BYTES: usize = AGENT_PROMPT_CAP - MAX_FILE_BLOCK_BYTES;

/// How many equal shares of [`MAX_FRAMING_BYTES`] the framing is split into when
/// deciding how much of it the shared probe evidence may occupy.
///
/// Two: one share for the evidence, one for the authored framing the same prompt
/// must also carry (the change purpose and the largest validator's mandate,
/// guidance, rule bodies, and output contract).
const FRAMING_SHARES_PER_EVIDENCE_BLOCK: usize = 2;

/// The largest RENDERED shared-evidence section a prompt may carry, in bytes —
/// one share of [`MAX_FRAMING_BYTES`].
///
/// The `<changed-set>` `duplicates` rows this section renders are PAIRWISE, so
/// they grow with the square of the changed entity count: on a real 15-file
/// change of this repo they rendered about 452 KB, 96% of the whole framing,
/// leaving almost nothing of the prompt cap for the files under review
/// (^x8z9hgf). The authored share is generous by comparison — the largest
/// builtin validator's suffix measured 21 KB against this many bytes — so the
/// split is not close on either side.
///
/// [`render_shared_probe_evidence`] applies it and names how many rows it
/// dropped.
pub const MAX_SHARED_EVIDENCE_BYTES: usize = MAX_FRAMING_BYTES / FRAMING_SHARES_PER_EVIDENCE_BLOCK;

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
    /// The maximum RENDERED file content, in bytes, one batch's prompts may
    /// carry. Whole files are packed greedily up to this budget (never split,
    /// never sliced), and it is also the caller's ceiling on
    /// [`file_block_cap`](Self::file_block_cap) — the constant a
    /// (validator, file) pair must satisfy to be reviewed at all. Always
    /// `<= `[`AGENT_PROMPT_CAP`] — [`new`](Self::new) clamps it. Read through
    /// [`batch_size`](Self::batch_size); private so the config can evolve
    /// without a field-level API commitment.
    batch_size: usize,
}

impl FleetConfig {
    /// Build a config with an explicit batch budget (bytes of rendered file
    /// content per batch), clamped to [`AGENT_PROMPT_CAP`].
    ///
    /// The clamp is load-bearing: `batch_size` is a caller-supplied `review`
    /// modifier, so without it a caller could ask for a budget the agent
    /// rejects outright. [`FleetConfig::default`] uses [`DEFAULT_BATCH_SIZE`].
    pub fn new(batch_size: usize) -> Self {
        Self {
            batch_size: batch_size.min(AGENT_PROMPT_CAP),
        }
    }

    /// The maximum rendered file content, in bytes, one batch's prompts may
    /// carry, before the run's framing is subtracted.
    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    /// The bytes of rendered file blocks one batch may carry, given the run's
    /// `framing` bytes ([`prompt_framing_bytes`]).
    ///
    /// The cap applies to the WHOLE prompt, and a prompt is framing plus file
    /// blocks, so the packer's budget is the cap minus the framing. Saturates
    /// at zero: framing that alone exceeds the cap leaves no room for any file,
    /// and every file then takes a batch of its own rather than the budget
    /// underflowing into an enormous number.
    ///
    /// This sizes BATCHES only. The over-cap verdict is
    /// [`file_block_cap`](Self::file_block_cap), which never reads the framing.
    pub fn file_payload_budget(&self, framing: usize) -> usize {
        self.batch_size
            .min(AGENT_PROMPT_CAP.saturating_sub(framing))
    }

    /// The largest rendered block one file may contribute, in bytes — the
    /// stricter of [`MAX_FILE_BLOCK_BYTES`] and the caller's `batch_size`.
    ///
    /// A file over this is reported as a
    /// [`SkippedFile`](crate::review::scope::SkippedFile) gap. Both inputs are
    /// constants for a run, so the verdict is a pure function of the file: the
    /// same content is judged the same way on every run, and a file that
    /// satisfied the cap can only go over it by growing.
    pub fn file_block_cap(&self) -> usize {
        self.batch_size.min(MAX_FILE_BLOCK_BYTES)
    }

    /// The [`BatchBudget`] the packer runs on, given the run's `framing` bytes.
    ///
    /// Pairs the constant [`file_block_cap`](Self::file_block_cap) (the
    /// over-cap verdict) with the framing-sensitive
    /// [`file_payload_budget`](Self::file_payload_budget) (where batch
    /// boundaries fall). Building both here is what keeps the two numbers from
    /// collapsing back into one at a call site.
    pub fn batch_budget(&self, framing: usize) -> BatchBudget {
        BatchBudget::new(
            FileCapBytes(self.file_block_cap()),
            BatchBytes(self.file_payload_budget(framing)),
        )
    }
}

impl Default for FleetConfig {
    fn default() -> Self {
        Self::new(DEFAULT_BATCH_SIZE)
    }
}

/// The result of a fan-out run: the merged findings plus the task tally.
///
/// A task that errors, is dropped, or answers unreadably twice still degrades to
/// zero findings (one bad task never aborts the rest), but unlike
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
    /// How many of those tasks failed (errored, were dropped, or answered
    /// unreadably even on their one re-ask) and so degraded to zero findings.
    /// Read through [`failed`](Self::failed);
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

    /// How many submitted tasks failed (errored, were dropped, or answered
    /// unreadably even on their one re-ask) and so degraded to zero findings.
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
/// ([`render_validator_suffix`] — its prompt rules + output contract), decoding
/// strictly forward from the cached prefix. As each task returns, its reply is
/// parsed by [`parse_findings`](crate::review::types::parse_findings) — plus a
/// cheap bracket repair — and every finding is tagged with the validator. A
/// reply no parse can read earns the task one re-ask before it is declared
/// failed; a task that errors, or whose second reply is unreadable too,
/// contributes zero findings for its validator and is logged — never a panic.
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
    suppressed: &ToolSuppression,
    progress: Option<&ReviewProgressSender>,
) -> FleetOutcome {
    // Plan the fan-out BEFORE priming so an empty plan (no matching ruleset,
    // or every rule covered by a healthy tool rule) skips the prime entirely —
    // an empty run never prompts the agent.
    let plan = plan_fan_out(work, loader, suppressed);
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

/// Plan the fan-out: one [`ValidatorTask`] per validator (and per group of its
/// files with the same suppressed-rule set), in work-list order. A validator
/// with no matching RuleSet in the loader is logged and skipped (never
/// rendered with empty instructions); each planned task's rule names are
/// logged so the fan-out shows exactly what ran.
///
/// Tool rules never enter a task's prompt — an LLM must not read them. On top
/// of that, `suppressed` names the prompt rules a healthy tool rule supersedes
/// per `(validator, file)`: the validator's files are grouped by their
/// suppressed set, each group becomes its own task carrying only the prompt
/// rules that still apply, and a group with no prompt rule left submits no LLM
/// task at all (its review already happened in the tool runner). With no tool
/// rules and no suppression the plan is exactly the old one-task-per-validator
/// shape.
fn plan_fan_out(
    work: &WorkList,
    loader: &ValidatorLoader,
    suppressed: &ToolSuppression,
) -> Vec<ValidatorTask> {
    let mut plan: Vec<ValidatorTask> = Vec::new();
    for validator in work.validators() {
        let Some(ruleset) = loader.get_ruleset(validator.validator_name()) else {
            tracing::warn!(
                validator = %validator.validator_name(),
                "fleet fan-out: no RuleSet for validator in loader; skipping it"
            );
            continue;
        };
        for (skip_set, files) in group_files_by_suppression(validator, suppressed) {
            let rules = prompt_rules_for(ruleset, &skip_set);
            if rules.is_empty() && !ruleset.rules.is_empty() {
                tracing::info!(
                    validator = %validator.validator_name(),
                    files = files.len(),
                    "fleet fan-out: every rule for these files is a tool rule or superseded \
                     by a healthy one; no LLM task"
                );
                continue;
            }
            let rule_names: Vec<&str> = rules.iter().map(|r| r.name.as_str()).collect();
            tracing::info!(
                validator = %validator.validator_name(),
                files = files.len(),
                rules = ?rule_names,
                "fleet fan-out: forking one task per validator against the shared prime"
            );
            let task_validator = ValidatorWork::new(
                validator.validator_name(),
                RuleNames::new(rules.iter().map(|rule| rule.name.clone())),
                ProbeNames::new(validator.probes().iter().cloned()),
                files.into_iter().cloned(),
            )
            .with_shared_probe_results(validator.shared_probe_results().iter().cloned());
            let task_ruleset = RuleSet {
                rules,
                ..ruleset.clone()
            };
            plan.push(ValidatorTask {
                validator: task_validator,
                ruleset: task_ruleset,
                roster: ruleset.rules.iter().map(|rule| rule.name.clone()).collect(),
            });
        }
    }
    plan
}

/// The rules the fleet puts in front of an agent for one group of files.
///
/// Two kinds never reach an agent. A tool rule does not, because its script IS
/// the review. Nor does a prompt rule a healthy tool rule superseded for these
/// files, which is what `suppressed_rules` names — the set
/// [`ToolSuppression::suppressed_rules`] returns for the pair.
///
/// What is left is the whole of an LLM's reading list for those files, so a
/// rule absent from it is a rule no agent will ever see. That is what makes a
/// tool rule's "no LLM reads this" claim a thing a test can check rather than
/// a thing a rule file asserts.
pub fn prompt_rules_for(ruleset: &RuleSet, suppressed_rules: &BTreeSet<String>) -> Vec<Rule> {
    ruleset
        .rules
        .iter()
        .filter(|rule| !rule.is_tool_rule() && !suppressed_rules.contains(&rule.name))
        .cloned()
        .collect()
}

/// Group a validator's files by their suppressed prompt-rule set, preserving
/// the validator's file order inside each group.
///
/// The common case — no suppression — yields exactly one group with an empty
/// key, so the plan stays one task per validator.
fn group_files_by_suppression<'v>(
    validator: &'v ValidatorWork,
    suppressed: &ToolSuppression,
) -> Vec<(BTreeSet<String>, Vec<&'v FileWork>)> {
    let mut groups: BTreeMap<BTreeSet<String>, Vec<&FileWork>> = BTreeMap::new();
    for file in validator.files() {
        let key = suppressed.suppressed_rules(validator.validator_name(), file.path());
        groups.entry(key).or_default().push(file);
    }
    groups.into_iter().collect()
}

/// Submit every planned validator task to the pool, returning the in-flight
/// receivers paired with their context.
///
/// The fan-out grain is the validator: the files live in the shared prime, so a
/// validator's task carries only its instructions (its prompt rules) as the fork
/// suffix. With a live `prime` each task forks the shared prefix and sends just
/// the suffix; without one (priming failed) each task degrades to a
/// self-contained monolithic prompt on a fresh session.
fn submit_fan_out(
    plan: Vec<ValidatorTask>,
    work: &WorkList,
    pool: &AgentPool,
    prime: &Option<SessionPinGuard>,
    progress: Option<&ReviewProgressSender>,
) -> Vec<PendingValidator> {
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
            let suffix = render_validator_suffix(&task.validator, &task.ruleset, work.subject());
            let rx = match prime {
                Some(guard) => Submitted::Forked(pool.submit_forked(guard.session_id(), suffix)),
                None => Submitted::Monolithic(pool.submit(render_fleet_prompt(
                    work.change_purpose(),
                    &task.validator,
                    &task.ruleset,
                    work.subject(),
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
/// its worker count. A task that errors, is dropped, or answers unreadably even
/// on its one re-ask contributes zero findings and bumps `failed` — one bad task
/// never aborts the rest. Awaiting here (rather than in a detached task) keeps
/// the run's shared-prime pin released on cancellation: dropping the `run_fleet`
/// future drops this collect mid-await.
async fn collect_fan_out(
    pending: Vec<PendingValidator>,
    work: &WorkList,
    pool: &AgentPool,
    progress: Option<&ReviewProgressSender>,
) -> (Vec<Finding>, usize, usize) {
    let attempted = pending.len();
    let mut findings: Vec<Finding> = Vec::new();
    let mut failed = 0usize;
    for PendingValidator { task, rx } in pending {
        let files: Vec<String> = task
            .validator
            .files()
            .iter()
            .map(|f| f.path().to_string())
            .collect();
        let ctx = TaskContext {
            change_purpose: work.change_purpose(),
            validator: &task.validator,
            ruleset: &task.ruleset,
            roster: &task.roster,
            files: &files,
            subject: work.subject(),
        };
        let name = ctx.name();
        let collected = match rx {
            Submitted::Monolithic(rx) => collect_monolithic_task(rx.await, &ctx, pool).await,
            Submitted::Forked(rx) => collect_forked_task(rx.await, &ctx, pool).await,
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
            // Both failure kinds land here: the re-ask ladder is already spent
            // by the time a task reports one, so the tally is the same.
            Err(_) => failed += 1,
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
///
/// Owned rather than borrowed: [`plan_fan_out`] filters tool rules (and
/// suppressed prompt rules) out of the ruleset and may narrow the validator to
/// a file group, so each task carries its own filtered copies.
struct ValidatorTask {
    validator: ValidatorWork,
    ruleset: RuleSet,
    /// Every rule name the validator's loaded set carries, tool and superseded
    /// rules included — the roster [`resolve_rule`] pins a cited name to. Wider
    /// than `ruleset.rules` on purpose: a rule a healthy tool rule superseded
    /// left the shard but is still a real rule document a reader can open.
    roster: Vec<String>,
}

/// A submitted [`ValidatorTask`]: its context plus the in-flight receiver.
struct PendingValidator {
    task: ValidatorTask,
    rx: Submitted,
}

/// How one validator task was submitted: a suffix-only prompt on a fork of the
/// run's primed prefix session (the warm path), or the full monolithic prompt on
/// a fresh session (the degraded path).
enum Submitted {
    Forked(tokio::sync::oneshot::Receiver<SessionTurnResult>),
    Monolithic(tokio::sync::oneshot::Receiver<crate::validators::PromptResult>),
}

/// Everything one validator task's collectors need: the name a finding is
/// tagged with, the files a failure is attributed to, and the material to
/// re-render the task's monolithic prompt.
struct TaskContext<'a> {
    /// The run's change purpose — the first section of a monolithic prompt.
    change_purpose: &'a str,
    /// The validator the task reviews for, narrowed to its file group.
    validator: &'a ValidatorWork,
    /// The task's rule set, already filtered of tool and suppressed rules.
    ruleset: &'a RuleSet,
    /// Every rule name the validator's loaded set carries — see
    /// [`ValidatorTask::roster`].
    roster: &'a [String],
    /// The validator's files, as every log line and progress event names them.
    files: &'a [String],
    /// What the run REVIEWS — carried so the monolithic fallback and the
    /// follow-up sweep state the same boundary the forked prompt did.
    subject: ReviewSubject,
}

impl TaskContext<'_> {
    /// The validator's name — what every finding is tagged with and every log
    /// line names.
    fn name(&self) -> &str {
        self.validator.validator_name()
    }

    /// Render this task's self-contained monolithic prompt: the fresh-session
    /// prompt used when there is no prime to fork from, and again when a re-ask
    /// needs a second sample with no session to continue.
    fn monolithic_prompt(&self) -> String {
        render_fleet_prompt(
            self.change_purpose,
            self.validator,
            self.ruleset,
            self.subject,
        )
    }
}

/// Why one collected validator task yielded no findings.
///
/// The two are not the same accident, and only one of them is worth asking
/// again about: a reply that arrived and could not be read is a bad sample from
/// the model, while a task that never delivered a reply would fail the same way
/// a second time.
enum TaskFailure {
    /// No usable reply was delivered — a pool error, or a dropped result
    /// channel.
    NotDelivered,
    /// A reply arrived and no parse could read it, repair included.
    Unparseable,
}

/// The log message for a validator task whose turn came back as a pool error.
const TASK_ERRORED: &str = "fleet task failed; yielding zero findings for this validator";

/// The log message for a validator task whose result channel was dropped before
/// the turn was delivered.
const TASK_DROPPED: &str = "fleet task result was dropped before delivery; yielding zero findings";

/// The log message for a re-ask that never came back, so the task's unreadable
/// first reply is final.
const REASK_UNDELIVERED: &str =
    "fleet re-ask was not delivered; the task's unreadable reply stands";

/// The one re-ask a validator task gets when its reply cannot be read.
///
/// One malformed reply used to throw away a whole validator's review: the task
/// yielded zero findings, the tally counted it failed, and the report ended
/// INCOMPLETE with a full re-run as the only recovery. Asking the model to
/// restate what it already found is far cheaper than that, and on the forked
/// path it runs on a fork of the very session that answered, so the findings are
/// still in context. The instruction is deliberately about the SHAPE of the
/// reply, not about reviewing again — the review already happened.
const REASK_PROMPT: &str = "\
Your last reply could not be read as JSON. Reply again with the SAME findings as \
a single JSON array and nothing else: no prose, no code fence, no tool call. An \
empty array is the correct reply when you found nothing.";

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
    ctx: &TaskContext<'_>,
    pool: &AgentPool,
) -> Result<Vec<Finding>, TaskFailure> {
    match delivered {
        Ok(Ok(turn)) => handle_fork_success(turn, ctx, pool).await,
        Ok(Err(PoolError::ForkFailed {
            parent_session_id,
            message,
        })) => handle_fork_failed(parent_session_id, message, ctx, pool).await,
        Ok(Err(err)) => Err(note_task_failure(ctx, Some(&err), TASK_ERRORED)),
        Err(_) => Err(note_task_failure(ctx, None, TASK_DROPPED)),
    }
}

/// The warm/degraded fork-success arm of [`collect_forked_task`]: log the prefix
/// reuse, parse the delivered turn exactly like the monolithic path, then drive
/// the session forward with [`sweep_until_dry`] to recover under-reported
/// instances before returning.
///
/// A turn whose fork ran cold (no warm prefix reuse) is logged as degraded but
/// still parsed — correctness never depends on the cache hit. A reply no parse
/// can read earns one [`reask_forked_task`] before the task fails; the sweep
/// then drives whichever session actually answered.
async fn handle_fork_success(
    turn: SessionTurn,
    ctx: &TaskContext<'_>,
    pool: &AgentPool,
) -> Result<Vec<Finding>, TaskFailure> {
    log_prefix_reuse(&turn, ctx);
    let (findings, answered_on) = match parse_task_response(&turn.content, ctx) {
        Some(findings) => (findings, turn.session_id),
        None => reask_forked_task(pool, &turn.session_id, ctx).await?,
    };
    Ok(sweep_until_dry(pool, &answered_on, ctx, findings).await)
}

/// Log how warm a forked turn's prefix reuse was, warning when it ran cold, so a
/// run's prefill savings are measurable from the log.
fn log_prefix_reuse(turn: &SessionTurn, ctx: &TaskContext<'_>) {
    let reuse = classify_reuse(turn.fork, turn.cache_usage);
    tracing::info!(
        validator = %ctx.name(),
        files = ?ctx.files,
        session = %turn.session_id,
        reuse = reuse.label(),
        reused_tokens = ?reuse.reused_tokens(),
        cache_read_input_tokens = ?reuse.cache_read(),
        cache_creation_input_tokens = ?reuse.cache_created(),
        "fleet task prefix reuse"
    );
    if matches!(reuse, PrefixReuse::Cold) {
        tracing::warn!(
            validator = %ctx.name(),
            files = ?ctx.files,
            session = %turn.session_id,
            "fleet task fork was degraded (no warm prefix reuse); proceeding cold"
        );
    }
}

/// Re-ask one forked validator task whose reply could not be read, exactly once.
///
/// The re-ask forks the session that produced the unreadable reply, so the model
/// restates findings it has already made rather than reviewing the files again —
/// the cheap rung, and the warm one. Returns the findings with the session that
/// answered, so [`sweep_until_dry`] keeps driving that same conversation
/// forward. A second unreadable reply, and a re-ask that never comes back, both
/// fail the task.
async fn reask_forked_task(
    pool: &AgentPool,
    session: &SessionId,
    ctx: &TaskContext<'_>,
) -> Result<(Vec<Finding>, SessionId), TaskFailure> {
    tracing::warn!(
        validator = %ctx.name(),
        files = ?ctx.files,
        session = %session,
        "fleet task reply could not be read; re-asking the validator once on a fork of its session"
    );
    let turn = match pool.submit_forked(session, REASK_PROMPT.to_string()).await {
        Ok(Ok(turn)) => turn,
        Ok(Err(err)) => return Err(note_task_failure(ctx, Some(&err), REASK_UNDELIVERED)),
        Err(_) => return Err(note_task_failure(ctx, None, REASK_UNDELIVERED)),
    };
    let findings = parse_task_response(&turn.content, ctx).ok_or(TaskFailure::Unparseable)?;
    Ok((findings, turn.session_id))
}

/// The fork-failed arm of [`collect_forked_task`]: the `session/fork` call failed,
/// so the validator never ran on the primed prefix. Fall back to a monolithic
/// fresh-session prompt for the validator — degraded (cold, no shared prime) but
/// correct; a fork failure must never lose a task.
async fn handle_fork_failed(
    parent_session_id: String,
    message: String,
    ctx: &TaskContext<'_>,
    pool: &AgentPool,
) -> Result<Vec<Finding>, TaskFailure> {
    tracing::warn!(
        validator = %ctx.name(),
        files = ?ctx.files,
        parent = %parent_session_id,
        error = %message,
        "fleet task fork failed; falling back to a monolithic fresh-session prompt"
    );
    collect_monolithic_task(pool.submit(ctx.monolithic_prompt()).await, ctx, pool).await
}

/// The failure arm of the task collectors ([`collect_forked_task`] /
/// [`collect_task`]): a task that failed for any reason other than a fork
/// failure — a pool error (idle/ceiling abandonment, an extension failure, or an
/// agent error) or a dropped result channel. Logged with `message` (and the
/// `error` field when the failure carried one — a [`PoolError`], absent for a
/// dropped delivery) and degraded to zero findings — one bad task never aborts
/// the rest — returning [`TaskFailure::NotDelivered`] so the caller tallies it
/// as failed rather than conflating it with a clean validator, and so no re-ask
/// is spent on a reply that never arrived.
fn note_task_failure(
    ctx: &TaskContext<'_>,
    error: Option<&PoolError>,
    message: &str,
) -> TaskFailure {
    tracing::warn!(
        validator = %ctx.name(),
        files = ?ctx.files,
        error = error.map(tracing::field::display),
        "{message}"
    );
    TaskFailure::NotDelivered
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
    ctx: &TaskContext<'_>,
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
            .submit_forked(&session, followup_prompt(ctx.subject).to_string())
            .await;
        let Ok(Ok(turn)) = delivered else {
            tracing::debug!(
                validator = %ctx.name(),
                files = ?ctx.files,
                sweep,
                "fleet follow-up sweep unavailable; ending the loop with the findings gathered so far"
            );
            return merged;
        };
        let Some(additional) = parse_task_response(&turn.content, ctx) else {
            return merged;
        };
        if additional.is_empty() {
            tracing::info!(
                validator = %ctx.name(),
                files = ?ctx.files,
                sweep,
                "fleet follow-up sweep went dry; the model reports no further instances"
            );
            return merged;
        }
        tracing::info!(
            validator = %ctx.name(),
            files = ?ctx.files,
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
        validator = %ctx.name(),
        files = ?ctx.files,
        cap = MAX_FOLLOWUP_SWEEPS,
        "fleet follow-up sweep hit the runaway cap without going dry; keeping the gathered findings"
    );
    merged
}

/// Collect a monolithic validator task, re-asking it once when its reply cannot
/// be read.
///
/// A monolithic task runs on a fresh session with nothing to fork from, so the
/// re-ask re-renders and re-submits the SAME prompt: a second sample on a fresh
/// session is the only re-ask a run without a live prime can take. Only a
/// delivered reply that no parse could read is re-asked — a pool error or a
/// dropped channel is a delivery failure a second ask would only repeat.
async fn collect_monolithic_task(
    delivered: Result<crate::validators::PromptResult, tokio::sync::oneshot::error::RecvError>,
    ctx: &TaskContext<'_>,
    pool: &AgentPool,
) -> Result<Vec<Finding>, TaskFailure> {
    match collect_task(delivered, ctx) {
        Err(TaskFailure::Unparseable) => {
            tracing::warn!(
                validator = %ctx.name(),
                files = ?ctx.files,
                "fleet task reply could not be read; re-asking the validator once on a fresh session"
            );
            collect_task(pool.submit(ctx.monolithic_prompt()).await, ctx)
        }
        collected => collected,
    }
}

/// Resolve one task's delivered result into tagged findings.
///
/// Returns `Ok(findings)` for a task that delivered a readable reply (the
/// findings may legitimately be empty), and a [`TaskFailure`] naming what went
/// wrong otherwise — a task error, a dropped channel, or a reply no parse could
/// read. A failure is logged and degrades the validator to zero findings (one
/// bad task never aborts the rest); the `Err` lets the caller tally it as failed
/// rather than silently conflating it with a clean validator, and decide whether
/// the failure is worth a re-ask.
fn collect_task(
    delivered: Result<crate::validators::PromptResult, tokio::sync::oneshot::error::RecvError>,
    ctx: &TaskContext<'_>,
) -> Result<Vec<Finding>, TaskFailure> {
    let response = match delivered {
        Ok(Ok(response)) => response,
        Ok(Err(err)) => return Err(note_task_failure(ctx, Some(&err), TASK_ERRORED)),
        Err(_) => return Err(note_task_failure(ctx, None, TASK_DROPPED)),
    };

    // A monolithic task runs on a fresh session (no fork), so any reuse is
    // hosted-cache only; classify with `fork = None` and log it so the cold
    // fallback path also reports cache usage.
    let reuse = classify_reuse(None, response.cache_usage);
    tracing::info!(
        validator = %ctx.name(),
        files = ?ctx.files,
        reuse = reuse.label(),
        cache_read_input_tokens = ?reuse.cache_read(),
        cache_creation_input_tokens = ?reuse.cache_created(),
        "fleet monolithic task prefix reuse"
    );
    parse_task_response(&response.content, ctx).ok_or(TaskFailure::Unparseable)
}

/// Parse one task's reply text into validator-tagged findings, degrading a reply
/// no parse can read to a logged `None` — shared by the monolithic and forked
/// collection paths so both read a reply identically.
///
/// The parse is [`parse_findings_repaired`]: the strict parse first, then the
/// cheap bracket repair, which is why a repaired reply is a plain success and
/// costs nothing further. `None` therefore means no parse could read the reply
/// at all, which is exactly what earns it a re-ask.
fn parse_task_response(content: &str, ctx: &TaskContext<'_>) -> Option<Vec<Finding>> {
    match parse_findings_repaired(content) {
        Ok(parsed) => Some(tag_findings(
            parsed,
            ctx.name(),
            &ctx.ruleset.rules,
            ctx.roster,
        )),
        Err(err) => {
            tracing::warn!(
                validator = %ctx.name(),
                files = ?ctx.files,
                error = %err,
                "fleet task response did not parse into findings"
            );
            None
        }
    }
}

/// Tag every finding with its authoritative `validator`/`rule` attribution,
/// overriding whatever the agent emitted.
///
/// Neither half of the attribution is the agent's to decide. The validator is
/// the shard's, flatly. The rule is whatever [`resolve_rule`] can pin the
/// agent's citation to in `roster` — the validator's whole loaded rule list —
/// so a report never names a rule the roster does not carry. `shown` is the
/// shard's own prompt-rule list, the closed set the agent actually read.
fn tag_findings(
    mut findings: Vec<Finding>,
    validator: &str,
    shown: &[Rule],
    roster: &[String],
) -> Vec<Finding> {
    for finding in &mut findings {
        finding.validator = validator.to_string();
        let resolved = resolve_rule(finding.rule.as_deref(), shown, roster);
        if resolved.is_none() {
            tracing::warn!(
                validator = %validator,
                cited = ?finding.rule,
                file = %finding.file,
                "fleet: no roster rule matches the finding's cited rule; reporting it unattributed"
            );
        }
        finding.rule = resolved;
    }
    findings
}

/// Pin the rule name an agent cited to a rule the validator really carries.
///
/// An implementer acting on a finding has to open the rule that produced it, so
/// the name in the report must be a roster name — never the spelling a model
/// happened to invent. The rungs, each reached only when the one above returns
/// nothing:
///
/// 1. `cited` already names a roster rule.
/// 2. `cited` names one after [`normalized_rule_name`] flattens case and
///    separators (a model writes "Magic Numbers" for `magic-numbers`).
/// 3. Exactly one roster name wraps, or is wrapped by, the normalized `cited`
///    (a model writes "no-magic-numbers" for `magic-numbers`). Several
///    candidates means the citation does not identify a rule, so nothing is
///    returned rather than one picked at random.
/// 4. `shown` held exactly one rule, so that rule is the only one the agent
///    could have fired — certainty, not inference.
///
/// `None` means the finding cannot be attributed to one rule; it is still
/// reported, marked [`UNATTRIBUTED_RULE`](crate::review::types::UNATTRIBUTED_RULE).
/// Dropping a real defect over missing metadata would be worse: the whole batch
/// once degraded to zero findings for exactly that kind of strictness.
fn resolve_rule(cited: Option<&str>, shown: &[Rule], roster: &[String]) -> Option<String> {
    let sole_shown = || match shown {
        [only] => Some(only.name.clone()),
        _ => None,
    };
    let Some(cited) = cited.map(str::trim).filter(|cited| !cited.is_empty()) else {
        return sole_shown();
    };

    if let Some(exact) = roster.iter().find(|name| name.as_str() == cited) {
        return Some(exact.clone());
    }

    let normalized = normalized_rule_name(cited);
    if let Some(name) = roster
        .iter()
        .find(|name| normalized_rule_name(name) == normalized)
    {
        return Some(name.clone());
    }

    let cited_words = rule_name_words(&normalized);
    let mut wrapping = roster.iter().filter(|name| {
        let normalized_name = normalized_rule_name(name);
        let words = rule_name_words(&normalized_name);
        contains_run(&cited_words, &words) || contains_run(&words, &cited_words)
    });
    match (wrapping.next(), wrapping.next()) {
        (Some(only), None) => Some(only.clone()),
        _ => sole_shown(),
    }
}

/// Split a [`normalized_rule_name`] into its `-` separated words.
fn rule_name_words(normalized: &str) -> Vec<&str> {
    normalized
        .split('-')
        .filter(|word| !word.is_empty())
        .collect()
}

/// Whether `needle`'s words appear in `haystack` as one unbroken run.
///
/// Whole words, never raw substrings: `r` sits inside `numbers` as text but
/// names no part of `magic-numbers`, and attributing a finding to a rule on
/// that evidence is a guess. `magic-numbers` inside `no-magic-numbers` is a
/// real citation of the same rule.
fn contains_run(haystack: &[&str], needle: &[&str]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// Flatten a rule name to its comparable form: lower case, every run of
/// non-alphanumeric characters collapsed to one `-`, and no leading or trailing
/// `-`. `Magic Numbers`, `magic_numbers`, and `magic-numbers` all reduce to the
/// same string.
fn normalized_rule_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        match ch.is_alphanumeric() {
            true => out.extend(ch.to_lowercase()),
            false if !out.ends_with('-') => out.push('-'),
            false => {}
        }
    }
    out.trim_matches('-').to_string()
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

/// The line that opens every per-validator suffix: `# Validator: ` immediately
/// followed by the validator name. The single source of truth shared by
/// [`render_validator_suffix`] and the tests that key scripts/assertions on the
/// header, so a format change lands in one place.
pub(crate) const VALIDATOR_HEADER: &str = "# Validator: ";

/// The mandate section header that follows the validator line in every suffix.
/// Shared by [`render_validator_suffix`] and the header-keyed tests so the
/// format stays synchronized in one place.
pub(crate) const MANDATE_HEADER: &str = "## Mandate\n\n";

/// The finding output contract for `subject`, shared verbatim by every fan-out
/// prompt of that subject.
///
/// Composed from a shared [`FINDING_FIELDS`] body and three subject-specific
/// sections — what to read, what to REVIEW against what to CONSIDER, and how
/// completely to report — so the finding shape cannot drift between the two
/// subjects while the boundary statement differs exactly where it must.
///
/// It instructs the agent to emit a JSON array of findings, each carrying the
/// four load-bearing fields the [`Finding`](crate::review::types::Finding)
/// type and the verify stage require:
/// `rule`, `claim` (what + why it matters), `evidence` (a cited probe proof), and
/// `suggestion` (the fix).
///
/// The contract is explicit that the reply must be the JSON array as plain
/// message text and that tools must NOT be called: review sessions still
/// advertise the agent's intrinsic tools, and without this instruction small
/// models deliver their findings as a hallucinated tool call (e.g. invoking the
/// validator name as a tool), leaving the parsed message empty and failing the
/// task.
pub(crate) fn output_contract(subject: ReviewSubject) -> String {
    let (reading, scope, completeness) = match subject {
        ReviewSubject::Diffs => (READING_FILES_DIFFS, REVIEW_SCOPE_DIFFS, COMPLETENESS_DIFFS),
        ReviewSubject::Files => (READING_FILES_WHOLE, REVIEW_SCOPE_WHOLE, COMPLETENESS_WHOLE),
    };
    format!("{reading}{scope}{FINDING_FIELDS}{completeness}")
}

/// The reading-files section under [`ReviewSubject::Diffs`]: the change is
/// inlined with a context band, and reading further is expected rather than
/// discouraged, because the block is deliberately not the whole file.
const READING_FILES_DIFFS: &str = "\
## Reading files

The change under review is already provided — each file's added and modified lines \
are inlined above, with the unchanged lines around them for context, and long \
unchanged stretches elided. `read_file`/`glob`/`grep` are available: reach for them \
when you need more of a file than its block shows in order to judge a marked line, \
or for another file entirely — cross-file duplication evidence, a changed symbol's \
callers, a type defined elsewhere.

";

/// The reading-files section under [`ReviewSubject::Files`]: the named files
/// are inlined whole, so re-reading them is pure cost.
const READING_FILES_WHOLE: &str = "\
## Reading files

The files under review are already provided in full — their COMPLETE current \
contents are inlined above, so do NOT `read_file` (or `glob`/`grep`) them; you \
already have them. `read_file`/`glob`/`grep` remain available, but only for OTHER \
files: cross-file duplication evidence, a changed symbol's callers, or a type \
defined elsewhere. Reach for them only when a finding genuinely depends on a file \
that is not already inlined here.

";

/// The review-scope section under [`ReviewSubject::Diffs`] — the REVIEW vs
/// CONSIDER statement, in the place the agent reads its instructions.
///
/// It states the enforcement, not only the instruction: a finding off the
/// change is refuted by the verify guard, so the model is told the truth about
/// what happens to one rather than merely asked not to write it.
const REVIEW_SCOPE_DIFFS: &str = "\
## Review scope

REVIEW the lines marked `+` — the lines this change added or modified. Every finding \
you report must land on one of them; a finding that lands anywhere else is refuted \
before it reaches the report.

CONSIDER everything else: the unmarked lines printed around the change, the \"What \
changed\" semantic diff, the probe evidence, and any file you read. Context is what \
you judge the change against, and it is never itself the subject of a finding. A \
defect that was already there before this change is out of scope, however real it \
is, and even when a rule below plainly matches it.

";

/// The review-scope section under [`ReviewSubject::Files`], where the caller
/// named these files and the whole of each is the subject.
const REVIEW_SCOPE_WHOLE: &str = "\
## Review scope

REVIEW the whole current file, not only the changed lines. The caller asked about \
these files themselves, so each is inlined above in full: review every line of it. \
Pre-existing instances of a rule — ones that were already there before the last \
change, anywhere in a named file — are in scope and must be reported now, in this \
same pass, alongside instances in the changed region.

CONSIDER the \"What changed\" semantic diff and the probe evidence: they are \
orientation only. The diff tells you what the last change touched; it is NOT the \
review boundary and NOT where to limit your search.

";

/// The finding object contract — the same fields whatever the subject is, so
/// the parser and the verify stage read one shape.
const FINDING_FIELDS: &str = "\
## Output contract

Once you have reviewed what the section above puts in scope (reading other files only \
if needed), reply with your findings as a JSON array, written directly as the plain \
text of your reply — the reply is parsed as JSON. The findings reply itself must be \
plain JSON text, never a tool call: a tool call is not a valid way to report \
findings.

Each finding is one object with these fields:

- `file`: the path of the file the finding is about.
- `line`: the 1-based line number the finding points at.
- `rule`: which rule of this validator fired — copy one of the `### Rule:` \
names listed above, spelled exactly as it appears there. A name that is not \
one of them cannot be traced back to a rule.
- `claim`: what is wrong AND why it matters — one concern per finding.
- `evidence`: the proof the issue is real — cite the injected probe result \
(e.g. \"per `duplicates`: 0.94 at `bar.rs:88`\") or a `file:line` citation.
- `suggestion`: the fix.

";

/// The completeness demand under [`ReviewSubject::Diffs`]: every match on a
/// marked line, and none anywhere else.
const COMPLETENESS_DIFFS: &str = "\
Report every occurrence of every rule that fires, in this single pass — across every \
marked line, not just the first: when a rule matches on several marked lines, emit a \
separate finding for each match — one finding per `file:line`. Do not stop at the \
first match and do not collapse repeated matches into one finding; list them ALL, so \
the change can be fixed in one go rather than re-reviewed match by match.

Report only real issues. If you find none, emit an empty array `[]`.
";

/// The completeness demand under [`ReviewSubject::Files`]: every match in the
/// whole file, changed or not.
const COMPLETENESS_WHOLE: &str = "\
Report every occurrence of every rule that fires, in this single pass — across the \
WHOLE file, not just the changed lines: when a rule matches on several lines, emit a \
separate finding for each match — one finding per `file:line`. Do not stop at the \
first match and do not collapse repeated matches into one finding; list them ALL, \
including instances that sit outside the changed region, so the whole file can be \
fixed in one go rather than re-reviewed match by match.

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

/// The follow-up "any more?" prompt [`sweep_until_dry`] tacks onto a
/// validator's review session for `subject`, repeated each sweep until the
/// model goes dry.
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
pub(crate) fn followup_prompt(subject: ReviewSubject) -> &'static str {
    match subject {
        ReviewSubject::Diffs => FOLLOWUP_PROMPT_DIFFS,
        ReviewSubject::Files => FOLLOWUP_PROMPT_WHOLE,
    }
}

/// The follow-up sweep under [`ReviewSubject::Diffs`] — a sweep of the marked
/// lines only.
///
/// The whole-file sweep asks for "pre-existing matches outside the changed
/// region", which is the exact opposite of what a diff review wants. Sweeping
/// the change instead keeps the recall gain the loop exists for without
/// spending four extra turns collecting findings the guard then refutes.
const FOLLOWUP_PROMPT_DIFFS: &str = "\
## Completeness re-scan

You just reported your findings for these files. Before we finish, scan the SAME \
marked lines again — the lines this change added or modified, already provided \
above — and report any ADDITIONAL violations of the same rules that you have NOT \
already named. Stay on the marked lines: an unmarked line is context, never a \
finding. This is a within-change completeness sweep, not a new review.

Reply with ONLY the additional, not-already-listed findings, as a JSON array in \
the exact same object shape as before (`file`, `line`, `rule`, `claim`, \
`evidence`, `suggestion`), written directly as the plain text of your reply — \
never a tool call. If you have now named every instance and none remain, reply \
with an empty array `[]`.
";

/// The follow-up sweep under [`ReviewSubject::Files`] — a sweep of the whole
/// of each named file.
const FOLLOWUP_PROMPT_WHOLE: &str = "\
## Completeness re-scan

You just reported your findings for these files. Before we finish, scan the SAME \
files again — their full current contents are already provided above — and report \
any ADDITIONAL violations of the same rules that you have NOT already named: \
matches outside the changed region, or further lines the same rule \
fires on. This is a within-file completeness sweep of the whole file, not a new \
review.

Reply with ONLY the additional, not-already-listed findings, as a JSON array in \
the exact same object shape as before (`file`, `line`, `rule`, `claim`, \
`evidence`, `suggestion`), written directly as the plain text of your reply — \
never a tool call. If you have now named every instance and none remain, reply \
with an empty array `[]`.
";

#[cfg(test)]
mod tests;
