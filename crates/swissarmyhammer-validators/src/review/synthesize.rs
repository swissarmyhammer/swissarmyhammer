//! Engine stage 4 — synthesize: dedup, order, render the dated checklist.
//!
//! This is the final, deterministic, LLM-free stage and the pipeline's single
//! barrier. [`run_review`] drives stages 1–3 to completion — fan-out and verify
//! both drain the shared [`AgentPool`](crate::validators::AgentPool) by awaiting
//! every task they submit — then hands the resulting `Vec<`[`VerifiedFinding`]`>`
//! to [`synthesize`], which turns it into the deduped, ordered [`ReviewReport`].
//!
//! # What synthesis does
//!
//! Review is a **binary pass/fail** model: a confirmed finding is a failure,
//! full stop — there is no graded severity. [`synthesize`] is pure and
//! clock-free: the timestamp is an **input**, never read inside the engine, so
//! the same findings always render the same report. It:
//!
//! 1. **Counts** confirmed vs refuted across every input finding.
//! 2. **Drops refuted** findings ([`VerifiedFinding::confirmed`] is `false`).
//! 3. **Dedups conservatively** — it collapses only *exact repeats*
//!    (same `file`, `line`, `validator`, `rule`, and byte-identical `claim`).
//!    There is no fuzzy/similarity matching, and findings from *different*
//!    validators on the same `file:line` are distinct lenses, never merged.
//! 4. **Orders** the surviving findings by `file:line` into ONE flat checklist
//!    so co-located concerns render together (ordering is not merging — every
//!    surviving concern is its own checklist item).
//! 5. **Renders** the dated GFM section in the exact shape the review skill
//!    already writes onto kanban tasks (`builtin/skills/review/SKILL.md` step 8),
//!    so the existing task-history parsing keeps working.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use model_embedding::TextEmbedder;
use rusqlite::Connection;

use crate::error::AvpError;
use crate::review::fleet::{
    prompt_framing, rendered_file_block_bytes, run_fleet, FleetConfig, FleetOutcome,
    ReviewProgressSender,
};
use crate::review::scope::{
    as_borrowed_strings, batch_work_list, detected_project_type_keys, scope_review, Scope,
    SkippedFile, WorkList,
};
use crate::review::tool_install::{install_missing_tools, PoolInstallAgent};
use crate::review::tool_rules::{execute_tool_runs, plan_tool_rules, ToolReport};
use crate::review::types::{Finding, VerifiedFinding};
use crate::review::verify::{verify_findings, Candidate};
use crate::validators::{AgentPool, ValidatorLoader};

/// How many fan-out `(validator, file)` tasks a run submitted.
///
/// Newtype over `usize` so [`FleetTally::new`]'s two counts cannot be
/// transposed at a call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TasksAttempted(pub usize);

/// How many fan-out tasks failed and degraded to zero findings.
///
/// Newtype over `usize` so [`FleetTally::new`]'s two counts cannot be
/// transposed at a call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TasksFailed(pub usize);

/// The fan-out task tally synthesis carries into the report.
///
/// `attempted` is how many `(validator, file)` tasks [`run_fleet`] submitted;
/// `failed` is how many of those degraded to zero findings on failure. A run
/// where `failed` is a large fraction of `attempted` produced an empty findings
/// set not because the diff was clean but because the review did not actually
/// run — the tally is what makes the two distinguishable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FleetTally {
    /// How many fan-out tasks were attempted.
    attempted: usize,
    /// How many fan-out tasks failed (and degraded to zero findings).
    failed: usize,
}

impl FleetTally {
    /// A tally of `attempted` tasks of which `failed` failed.
    pub fn new(attempted: TasksAttempted, failed: TasksFailed) -> Self {
        Self {
            attempted: attempted.0,
            failed: failed.0,
        }
    }

    /// How many fan-out tasks were attempted.
    pub fn attempted(&self) -> usize {
        self.attempted
    }

    /// How many fan-out tasks failed (and degraded to zero findings).
    pub fn failed(&self) -> usize {
        self.failed
    }
}

impl From<&FleetOutcome> for FleetTally {
    fn from(outcome: &FleetOutcome) -> Self {
        Self::new(
            TasksAttempted(outcome.attempted()),
            TasksFailed(outcome.failed()),
        )
    }
}

/// The per-verdict tallies a [`ReviewReport`] carries.
///
/// Review is binary pass/fail — there is no graded severity — so the rendered
/// failures are a single `findings` count, not a per-tier breakdown.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReviewCounts {
    /// Confirmed findings rendered into the checklist (post-dedup).
    findings: usize,
    /// Findings confirmed (across every input, pre-dedup): the verifier's
    /// confirmations plus the engine-emitted skip findings, which are
    /// confirmed by construction.
    confirmed: usize,
    /// Findings the verifier refuted (across every input).
    refuted: usize,
    /// How many fan-out tasks were attempted (see [`FleetTally`]).
    tasks_attempted: usize,
    /// How many fan-out tasks failed and degraded to zero findings. A non-zero
    /// value means the rendered findings are INCOMPLETE.
    tasks_failed: usize,
    /// How many distinct file paths were excluded from review because the
    /// file's rendered block alone exceeded the per-file cap (see
    /// [`SkippedFile`](crate::review::scope::SkippedFile)). A non-zero value
    /// means the review cannot be clean: each skipped path also becomes a
    /// CONFIRMED finding, and the markdown names each skipped file.
    skipped: usize,
    /// The skipped file paths — distinct, sorted. The structured twin of
    /// `skipped`: orchestrators gate on this list without parsing markdown.
    skipped_files: Vec<String>,
    /// How many tool-rule runs broke (nonzero exit or a stdout-contract
    /// violation). A non-zero value means those rules judged nothing: the
    /// run is a tool error, not clean and not findings.
    tool_errors: usize,
}

impl ReviewCounts {
    /// Confirmed findings rendered into the checklist (post-dedup).
    pub fn findings(&self) -> usize {
        self.findings
    }

    /// Findings confirmed (across every input, pre-dedup): the verifier's
    /// confirmations plus the engine-emitted skip findings.
    pub fn confirmed(&self) -> usize {
        self.confirmed
    }

    /// Findings the verifier refuted (across every input).
    pub fn refuted(&self) -> usize {
        self.refuted
    }

    /// How many fan-out tasks were attempted (see [`FleetTally`]).
    pub fn tasks_attempted(&self) -> usize {
        self.tasks_attempted
    }

    /// How many fan-out tasks failed and degraded to zero findings. A non-zero
    /// value means the rendered findings are INCOMPLETE.
    pub fn tasks_failed(&self) -> usize {
        self.tasks_failed
    }

    /// How many distinct file paths were excluded from review because the
    /// file's rendered block alone exceeded the per-file cap. A non-zero value
    /// means the review cannot be clean: each skipped path also becomes a
    /// CONFIRMED finding, and the markdown names each one as a "not reviewed,
    /// too large" gap.
    pub fn skipped(&self) -> usize {
        self.skipped
    }

    /// The skipped file paths — distinct, sorted. The structured twin of
    /// [`ReviewCounts::skipped`]: orchestrators gate on this list without
    /// parsing markdown.
    pub fn skipped_files(&self) -> &[String] {
        &self.skipped_files
    }

    /// How many tool-rule runs broke. A non-zero value means those rules
    /// judged nothing: the run is a tool error, not clean and not findings.
    pub fn tool_errors(&self) -> usize {
        self.tool_errors
    }
}

/// The synthesized review report: the rendered markdown plus its tallies.
///
/// Constructed only by [`synthesize`]; consumers read it through the getters
/// (and [`ReviewReport::into_markdown`] when they need to own the rendered
/// section without cloning it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewReport {
    /// The dated GFM `## Review Findings (...)` section, ready to append to a
    /// kanban task's description verbatim.
    markdown: String,
    /// The per-verdict counts for the tool/skill summary.
    counts: ReviewCounts,
}

impl ReviewReport {
    /// The dated GFM `## Review Findings (...)` section, ready to append to a
    /// kanban task's description verbatim.
    pub fn markdown(&self) -> &str {
        &self.markdown
    }

    /// The per-verdict counts for the tool/skill summary.
    pub fn counts(&self) -> &ReviewCounts {
        &self.counts
    }

    /// Consume the report, yielding its rendered markdown without a clone.
    pub fn into_markdown(self) -> String {
        self.markdown
    }
}

/// Synthesize verified findings into the dated, deduped, ordered report.
///
/// Pure and deterministic: `now` is the already-formatted local timestamp the
/// caller read from the clock (`YYYY-MM-DD HH:MM`), rendered verbatim into the
/// section header so the engine itself never reads time. See the module docs for
/// the full drop/dedup/order/render contract.
///
/// `tally` is the fan-out task outcome from [`run_fleet`]. When any task failed,
/// a clearly visible warning line is rendered directly under the dated header so
/// an incomplete run cannot be mistaken for a clean diff, and the tally is
/// carried through into [`ReviewCounts`]. When the run attempted zero tasks and
/// kept no findings and skipped no files — the resolved scope was empty — the
/// report states "Nothing in scope to review." so an empty scope cannot be
/// mistaken for a clean review either.
///
/// `skipped` names every [`SkippedFile`] [`batch_work_list`] excluded because
/// the file's rendered block alone exceeded the per-file cap: each is rendered
/// as a named "not reviewed, too large" gap directly under the header (and any
/// incomplete-run banner), and their count rides into [`ReviewCounts::skipped`].
/// This is deliberately never an error — one oversized file must not block
/// review of every OTHER file in scope. It is a coverage FAILURE though: each
/// skipped path also enters the finding stream as one CONFIRMED finding, so a
/// review that contains an over-cap file can never end clean.
///
/// `tools` carries the run's tool-rule facts: each broken tool run is
/// rendered as a tool error (its raw stderr, never findings and never a clean
/// result) and counted in [`ReviewCounts::tool_errors`]; each tool rule on its
/// prompt fallback is noted so the reader knows the prompt rule ran instead.
///
/// `verified` is any iterable of [`VerifiedFinding`]s (a `Vec` being the common
/// caller) — it is collected once up front so a caller need not materialize a
/// `Vec` just to hand it over.
pub fn synthesize(
    verified: impl IntoIterator<Item = VerifiedFinding>,
    tally: &FleetTally,
    skipped: &[SkippedFile],
    tools: &ToolReport,
    now: &str,
) -> ReviewReport {
    // The skip list is per (validator, file) pair, but the reader cares about
    // the FILE, so the pairs are folded onto one entry per path first. Both
    // levels are sorted, so every use below is deterministic.
    let by_path = group_skips_by_path(skipped);

    // A skipped file is a coverage failure, not only a warning: each skipped
    // path becomes one CONFIRMED finding in the same stream as the verifier's
    // findings, so every later step (count, dedup, order, render) and every
    // consumer that gates on findings treats it with no special handling.
    let verified = verified
        .into_iter()
        .chain(skip_findings(&by_path))
        .collect::<Vec<_>>();
    let counts_confirmed = verified.iter().filter(|v| v.confirmed).count();
    let counts_refuted = verified.len() - counts_confirmed;

    // Keep only confirmed findings, then collapse exact repeats.
    let kept = dedup_exact(verified.into_iter().filter(|v| v.confirmed));

    let mut counts = ReviewCounts {
        confirmed: counts_confirmed,
        refuted: counts_refuted,
        tasks_attempted: tally.attempted,
        tasks_failed: tally.failed,
        // Distinct PATHS, not pairs: the reader counts files, and one file that
        // no validator could carry is one gap however many validators matched
        // it.
        skipped: by_path.len(),
        skipped_files: by_path.keys().map(|path| (*path).to_string()).collect(),
        tool_errors: tools.errors().len(),
        ..ReviewCounts::default()
    };

    let mut markdown = String::new();
    let _ = writeln!(markdown, "## Review Findings ({now})");

    // Flag an incomplete run loudly, right under the header, when any fan-out
    // task failed — otherwise an all-failed run is byte-identical to a clean diff.
    if tally.failed > 0 {
        let _ = writeln!(
            markdown,
            "\n> ⚠️ {}/{} review tasks failed — results are INCOMPLETE.",
            tally.failed, tally.attempted
        );
    }

    // Keep the warning block: existing consumers read this text. The failure
    // itself rides in the checklist findings below, one per path.
    if !by_path.is_empty() {
        let _ = writeln!(
            markdown,
            "\n> ⚠️ {} file(s) not reviewed — the rendered prompt would exceed the agent's prompt cap:",
            by_path.len()
        );
        for (path, group) in &by_path {
            let _ = writeln!(
                markdown,
                "> - `{}` — {} rendered bytes, over the {}-byte per-file cap; not reviewed by: {} (split the file)",
                path,
                group.largest,
                group.cap,
                group.validators.join(", ")
            );
        }
    }

    render_tool_errors(&mut markdown, tools);
    render_tool_fallbacks(&mut markdown, tools);

    // Say so explicitly when the resolved scope was empty (zero fan-out tasks,
    // zero tool activity, nothing skipped either): a bare findings header
    // would read identically to a genuinely clean review.
    if tally.attempted == 0 && kept.is_empty() && skipped.is_empty() && tools.is_inert() {
        let _ = writeln!(markdown, "\nNothing in scope to review.");
    }

    // Order the surviving findings into ONE flat checklist by `file:line` so
    // co-located concerns render together; the sort is stable so exact-input
    // order is otherwise preserved.
    let mut ordered: Vec<&VerifiedFinding> = kept.iter().collect();
    ordered.sort_by(|a, b| {
        (a.finding.file.as_str(), a.finding.line).cmp(&(b.finding.file.as_str(), b.finding.line))
    });
    counts.findings = ordered.len();

    if !ordered.is_empty() {
        markdown.push('\n');
        for verified in ordered {
            let _ = writeln!(markdown, "{}", render_item(&verified.finding));
        }
    }

    tracing::info!(
        findings = counts.findings,
        confirmed = counts.confirmed,
        refuted = counts.refuted,
        tasks_attempted = counts.tasks_attempted,
        tasks_failed = counts.tasks_failed,
        tool_errors = counts.tool_errors,
        "review synthesis complete"
    );

    ReviewReport { markdown, counts }
}

/// Render each broken tool run as a tool error block: never findings and
/// never a clean result. Each block names the rule and carries the raw stderr
/// so the diagnosing agent reads exactly what the tool said.
fn render_tool_errors(markdown: &mut String, tools: &ToolReport) {
    for error in tools.errors() {
        let _ = writeln!(
            markdown,
            "\n> ⚠️ tool rule '{}/{}' failed — the tool judged nothing, so its findings are missing:",
            error.validator(),
            error.rule()
        );
        for line in error.detail().lines() {
            let _ = writeln!(markdown, "> {line}");
        }
    }
}

/// Render one note per tool rule on its prompt fallback: the reader must know
/// the prompt rule reviewed those files, not the tool.
fn render_tool_fallbacks(markdown: &mut String, tools: &ToolReport) {
    for fallback in tools.fallbacks() {
        let note = match fallback.supersedes().is_empty() {
            true => "no prompt rule is named to run instead".to_string(),
            false => format!("{} ran instead", fallback.supersedes().prompt_rule_phrase()),
        };
        let _ = writeln!(
            markdown,
            "\n> tool rule '{}/{}' is unavailable ({}); {note}.",
            fallback.validator(),
            fallback.rule(),
            fallback.detail()
        );
    }
}

/// Collapse only *exact* repeats, preserving first-seen order.
///
/// Two findings are the same concern only when their `file`, `line`, `validator`,
/// `rule`, and `claim` are all identical — the conservative key. Findings from
/// different validators (or with different claims) on the same `file:line` are
/// distinct lenses and are all kept. There is no fuzzy/similarity matching.
fn dedup_exact(findings: impl Iterator<Item = VerifiedFinding>) -> Vec<VerifiedFinding> {
    let mut seen: BTreeSet<(String, u32, String, Option<String>, String)> = BTreeSet::new();
    let mut kept = Vec::new();
    for verified in findings {
        let f = &verified.finding;
        let key = (
            f.file.clone(),
            f.line,
            f.validator.clone(),
            f.rule.clone(),
            f.claim.clone(),
        );
        if seen.insert(key) {
            kept.push(verified);
        }
    }
    kept
}

/// Render one finding as a GFM checklist item.
///
/// The shape matches the review skill verbatim: `` - [ ] `file:line` — claim.
/// suggestion. `` — the claim (what + why it matters) followed by the suggestion
/// when the agent offered one, each terminated as a sentence. A finding with no
/// suggestion renders the claim alone.
fn render_item(finding: &Finding) -> String {
    let mut body = sentence(&finding.claim);
    if let Some(suggestion) = &finding.suggestion {
        let suggestion = suggestion.trim();
        if !suggestion.is_empty() {
            body.push(' ');
            body.push_str(&sentence(suggestion));
        }
    }
    format!("- [ ] `{}:{}` — {}", finding.file, finding.line, body)
}

/// One path's worth of skips, folded from the per-(validator, file)
/// [`SkippedFile`] entries [`batch_work_list`] returns.
///
/// The packer's grain is the pair, because whether a file fits depends on the
/// probe evidence selected for the validator rendering it. The report's grain
/// is the FILE, so [`group_skips_by_path`] folds the pairs onto this.
#[derive(Debug)]
struct SkipGroup<'a> {
    /// The largest rendered block any of the path's skipped validators produced.
    largest: usize,
    /// The per-file cap every one of those blocks exceeded.
    cap: usize,
    /// The validators that could not carry the file, sorted.
    validators: Vec<&'a str>,
}

/// The line a file-level skip finding anchors to. The gap is about the whole
/// file, and a file starts at line 1.
const FILE_START_LINE: u32 = 1;

/// The validator name a skip finding carries. No real validator produced the
/// finding — the engine itself did — so the name identifies the engine.
const SKIP_FINDING_VALIDATOR: &str = "review-engine";

/// The rule name a skip finding cites.
const SKIP_FINDING_RULE: &str = "prompt-cap";

/// Turn each skipped path into one CONFIRMED [`VerifiedFinding`].
///
/// A file no validator could read is a coverage failure: every review of it
/// would read "clean" for that validator's dimension until the file shrinks.
/// So the skip enters the normal finding stream — confirmed by construction,
/// because the batch packer measured the rendered block over the per-file cap
/// deterministically — and no consumer needs special handling to fail the
/// gate. The grain is the PATH: one finding per file, with the claim naming
/// the validators that could not carry it.
fn skip_findings(by_path: &BTreeMap<&str, SkipGroup<'_>>) -> Vec<VerifiedFinding> {
    by_path
        .iter()
        .map(|(path, group)| VerifiedFinding {
            finding: Finding {
                file: (*path).to_string(),
                line: FILE_START_LINE,
                validator: SKIP_FINDING_VALIDATOR.to_string(),
                rule: Some(SKIP_FINDING_RULE.to_string()),
                claim: format!(
                    "This file exceeds the review prompt cap — {} rendered bytes against the \
                     {}-byte per-file cap — so these validators could not review it: {}",
                    group.largest,
                    group.cap,
                    group.validators.join(", ")
                ),
                evidence: format!(
                    "batch packer: the file's rendered block alone is {} bytes, over the \
                     {}-byte per-file cap",
                    group.largest, group.cap
                ),
                suggestion: Some(
                    "Split the file into smaller modules that fit the review prompt cap"
                        .to_string(),
                ),
            },
            confirmed: true,
            reason: "the batch packer measured the rendered block over the per-file cap — \
                     a deterministic measurement, not a judgement"
                .to_string(),
            decided_by: None,
        })
        .collect()
}

/// Fold the per-(validator, file) skip list onto one entry per path, sorted by
/// path, with each entry's validator list sorted.
///
/// Sorting both levels is what makes the rendered gap block deterministic
/// regardless of scope/scan order.
fn group_skips_by_path(skipped: &[SkippedFile]) -> BTreeMap<&str, SkipGroup<'_>> {
    let mut by_path: BTreeMap<&str, SkipGroup<'_>> = BTreeMap::new();
    for skip in skipped {
        let group = by_path.entry(skip.path()).or_insert_with(|| SkipGroup {
            largest: 0,
            cap: skip.cap(),
            validators: Vec::new(),
        });
        group.largest = group.largest.max(skip.size());
        group.validators.push(skip.validator());
    }
    for group in by_path.values_mut() {
        group.validators.sort_unstable();
    }
    by_path
}

/// Normalize a fragment into a sentence: trimmed and terminated with `.` unless
/// it already ends in sentence punctuation.
fn sentence(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.ends_with(['.', '!', '?']) {
        trimmed.to_string()
    } else {
        format!("{trimmed}.")
    }
}

/// Run the whole review pipeline end to end and synthesize the report.
///
/// This is the top-level engine entry point and the pipeline's **single
/// barrier**. It drives, in order:
///
/// 1. [`scope_review`] — resolve `scope` into the per-validator [`WorkList`]
///    (deterministic, LLM-free).
/// 2. [`plan_tool_rules`] + [`execute_tool_runs`] — plan every matched tool
///    rule ONCE for the whole run and execute the healthy ones at the
///    workspace root, before any batching. Tool findings join the verified
///    stream as CONFIRMED (deterministic tool output skips the adversarial
///    verify pass), and the plan's suppression map rides into every batch's
///    fan-out so a superseded prompt rule is skipped per file; an unhealthy
///    tool suppresses nothing and is reported as a prompt fallback.
/// 3. [`batch_work_list`] — split the work-list into budgeted batches at
///    whole-file granularity so no single prompt overflows the agent's prompt
///    cap. [`FleetConfig::batch_budget`] supplies both numbers, spent in
///    RENDERED bytes: [`FleetConfig::file_payload_budget`] (the cap less the
///    run's measured framing) sets the batch boundaries, so a small diff is one
///    batch and a large one is several; the constant
///    [`FleetConfig::file_block_cap`] decides the over-cap verdict, so a
///    (validator, file) pair over it is excluded and reported as a named gap
///    (see [`SkippedFile`]), never a hard error.
/// 4. For **each batch**, independently: [`run_fleet`] fans every validator out
///    across the shared `pool` over that batch's files (its own shared prime,
///    forked per validator), then [`verify_findings`] pairs each candidate back
///    with its file's ground-truth context ([`build_candidates`]) and runs the
///    adversarial refute pass on the **same** `pool` — forking that batch's prime
///    while it stays pinned, then releasing the pin once the batch has drained.
/// 5. [`synthesize`] — merge every batch's confirmed [`VerifiedFinding`]s and
///    turn them into the dated, deduped, ordered [`ReviewReport`] (synthesis dedups
///    by `file:line`, so cross-batch findings collapse the same as within a batch).
///
/// Because each batch awaits all the tasks it submits before the next begins, the
/// shared pool fully drains between batches and the prime pin never outlives its
/// batch. A one-batch run (the common small diff) is byte-for-byte the old single
/// fan-out → verify path. The engine never reads the clock: `now` is the
/// caller-supplied, already-formatted local timestamp (`YYYY-MM-DD HH:MM`)
/// rendered verbatim into the report header.
///
/// `progress` is the optional [`ReviewProgressSender`] handed through to
/// [`run_fleet`] so each batch emits its `Planned`/`PairStarted`/`PairDone`
/// events; `None` emits nothing.
///
/// # Errors
///
/// Returns the [`AvpError`] from [`scope_review`] on git or index failure, or when
/// a matched validator declares an unknown probe. [`batch_work_list`] never
/// errors: a file whose rendered block is over the per-file cap is
/// excluded and reported as a named
/// gap instead. Fan-out and verify failures never error either: a failed task
/// degrades to zero findings (fan-out) or a refute-by-default verdict (verify),
/// so the report is always produced.
#[allow(clippy::too_many_arguments)]
pub async fn run_review(
    scope: Scope,
    repo_path: &Path,
    loader: &ValidatorLoader,
    conn: &Connection,
    embedder: &dyn TextEmbedder,
    pool: &AgentPool,
    fleet_config: FleetConfig,
    progress: Option<&ReviewProgressSender>,
    now: &str,
) -> Result<ReviewReport, AvpError> {
    // Stage 1: scope → work-list (deterministic, LLM-free). The progress
    // sender rides along so the stage announces each file as it scopes it —
    // the run's FIRST events, emitted long before any fleet work exists.
    let work = scope_review(scope, repo_path, loader, conn, embedder, progress).await?;

    // Stage 2: run every healthy tool rule ONCE for the whole run,
    // before any batching — tools have no prompt budget, and a workspace-scope
    // tool must not run once per batch. The plan's suppression map rides into
    // every batch's fan-out so a superseded prompt rule is skipped per file;
    // an unhealthy tool suppresses nothing, and the report notes the fallback.
    let detected_types = detected_project_type_keys(repo_path);
    let project_types = as_borrowed_strings(&detected_types);

    // Stage 2a: install what the matched tool rules need before planning them.
    // The lifecycle tries each `install.commands` entry in order and, when all
    // of them fail, spends one bounded agent turn; the doctor check confirms
    // every attempt. Planning re-runs that same check, so an installed tool is
    // planned as healthy and a tool that is still missing falls back on its own.
    let installer = PoolInstallAgent::new(pool);
    let installs = install_missing_tools(&work, loader, &project_types, Some(&installer)).await;
    tracing::info!(
        tool_rules = installs.len(),
        still_missing = installs
            .iter()
            .filter(|install| !install.outcome().tool_present())
            .count(),
        "review run: tool-rule install lifecycle finished"
    );

    let tool_plan = plan_tool_rules(&work, loader, &project_types);
    let tool_outcome = execute_tool_runs(tool_plan.runs(), repo_path, progress);
    let tool_attempted = tool_plan.runs().len();
    let (_, tool_fallbacks, suppression) = tool_plan.into_parts();
    let (tool_findings, tool_errors) = tool_outcome.into_parts();

    // Stage 3: split the work-list into budgeted batches (whole-file
    // granularity). Two numbers, both spent in RENDERED bytes — measured by
    // running the fleet's own file renderer, so the packer's number and the
    // agent's number are the same bytes:
    //
    // - batch bytes: the agent's prompt cap less this run's measured framing
    //   (change purpose + shared evidence + the largest validator suffix). It
    //   decides where batch boundaries fall, so it moves with the run.
    // - the per-file cap: a constant. It decides which (validator, file) pair
    //   is excluded and reported as a named gap, so it must not move with the
    //   run — see `BatchBudget`.
    let prompt_framing = prompt_framing(&work, loader);
    let framing = prompt_framing.total();
    let budget = fleet_config.batch_budget(framing);
    let (batches, skipped) = batch_work_list(&work, budget, rendered_file_block_bytes);

    tracing::info!(
        validators = work.validators().len(),
        files = work.distinct_files().count(),
        batches = batches.len(),
        skipped = skipped.len(),
        file_cap = budget.file_cap(),
        batch_bytes = budget.batch_bytes(),
        framing,
        // The framing decomposition, so a run that gets tight on prompt budget
        // says WHICH term did it rather than only that the total was large.
        framing_purpose = prompt_framing.purpose(),
        framing_shared_evidence = prompt_framing.shared_evidence(),
        framing_validator_suffix = prompt_framing.validator_suffix(),
        framing_largest_validator = prompt_framing.largest_validator(),
        framing_cap = crate::review::fleet::MAX_FRAMING_BYTES,
        prompt_cap = crate::review::fleet::AGENT_PROMPT_CAP,
        "review run: scoped work-list ready, batched, fanning out"
    );

    // Stage 4: run the full fan-out → verify pipeline independently per batch,
    // accumulating every batch's verified findings and summing the task tally.
    let mut verified: Vec<VerifiedFinding> = Vec::new();
    let mut attempted = 0usize;
    let mut failed = 0usize;

    for (index, batch) in batches.iter().enumerate() {
        tracing::info!(
            batch = index + 1,
            of = batches.len(),
            files = batch.distinct_files().count(),
            "review run: fanning out batch"
        );

        // Fan out this batch: one shared prime over its files, forked per
        // validator. The outcome carries the tally and the batch's prime pin.
        let fleet = run_fleet(batch, loader, pool, &suppression, progress).await;
        attempted += fleet.attempted();
        failed += fleet.failed();
        let (fleet_findings, prime) = fleet.into_parts();

        // Verify this batch on the SAME pool — each verify task FORKS the batch's
        // shared prime while it stays pinned. Awaiting drains every verify task.
        let candidates = build_candidates(batch, fleet_findings);
        let prime_session = prime.as_ref().map(|g| g.session_id());
        let outcome = verify_findings(candidates, pool, prime_session, progress).await;

        // The batch (fan-out AND verify) has drained: release its prime pin so the
        // pinned cache entry does not outlive the batch. A run future dropped
        // before this point releases it from the guard's `Drop` instead.
        if let Some(guard) = prime {
            crate::review::fleet::unpin_prefix_session(guard).await;
        }

        verified.extend(outcome.verified);
    }

    // Tool findings are already CONFIRMED — deterministic tool output skips
    // the adversarial verify pass — so they join the verified stream directly.
    verified.extend(tool_findings);

    // Stage 5: synthesize the merged, deduped, ordered, dated report. The summed
    // tally rides into the report so the tool boundary can flag/fail an incomplete
    // run; the engine itself stays a pure data barrier and never errors on it. Any
    // oversized files stage 2 excluded ride in too, as a named gap, along with
    // the run's tool-rule facts (broken runs and prompt fallbacks).
    let report = synthesize(
        verified,
        &FleetTally::new(TasksAttempted(attempted), TasksFailed(failed)),
        &skipped,
        &ToolReport::new(tool_attempted, tool_errors, tool_fallbacks),
        now,
    );

    Ok(report)
}

/// Pair each fan-out [`Finding`] back with the ground-truth context its file
/// carries in the [`WorkList`], producing the [`Candidate`]s the verify stage
/// checks.
///
/// A finding is tagged with its `validator` and the `file` it is about; the
/// matching [`ValidatorWork`](crate::review::ValidatorWork) /
/// [`FileWork`](crate::review::FileWork) in `work` holds that file's
/// `source_slice`, shared `probe_results`, and `line_annotations`. This reuses
/// the stage-1 data verbatim — it never re-derives a slice, re-runs a probe, or
/// recomputes blame. A finding whose `(validator, file)` is not in the
/// work-list (an agent inventing a path) yields empty context rather than being
/// dropped, so it still reaches the verifier and refutes by default there.
fn build_candidates(work: &WorkList, findings: Vec<Finding>) -> Vec<Candidate> {
    findings
        .into_iter()
        .map(|finding| {
            let context = work
                .validators()
                .iter()
                .find(|v| v.validator_name() == finding.validator)
                .and_then(|v| v.files().iter().find(|f| f.path() == finding.file));
            let (source_slice, probe_results, line_annotations) = match context {
                Some(file) => (
                    file.source_slice().to_string(),
                    file.probe_results().to_vec(),
                    file.line_annotations().to_vec(),
                ),
                None => (String::new(), Vec::new(), Vec::new()),
            };
            Candidate {
                finding,
                source_slice,
                probe_results,
                line_annotations,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::scope::{FileWork, ProbeNames, RuleNames, ValidatorWork};
    use crate::review::types::RefutingLayer;

    /// The fixture timestamp passed as `now` to every `synthesize` call. Kept
    /// inline (not interpolated) inside the byte-for-byte snapshot strings so
    /// those stay readable.
    const NOW: &str = "2026-04-11 13:08";

    /// How many fan-out tasks the tally fixtures pretend a run attempted. The
    /// exact count is immaterial — these tests assert on the attempted/failed
    /// relationship (all succeeded, or all failed), not the magnitude — so naming
    /// it keeps the `FleetTally::new` arguments from reading as bare literals.
    const ATTEMPTED_TASKS: usize = 8;

    /// The per-file cap the oversized-file fixtures pretend the packer
    /// enforced. The magnitude mirrors the real default cap so the
    /// rendered gap message carries a realistic byte count (asserted
    /// verbatim below).
    const TEST_FILE_CAP_BYTES: usize = 393_216;

    /// A rendered file block larger than [`TEST_FILE_CAP_BYTES`] — the
    /// size that forces the packer to skip the file.
    const TEST_OVERSIZE_RENDERED_BYTES: usize = 500_000;

    /// A second over-budget rendered size, distinct from
    /// [`TEST_OVERSIZE_RENDERED_BYTES`], for fixtures where two validators
    /// skip the same file with different per-pair sizes.
    const TEST_OVERSIZE_ALT_RENDERED_BYTES: usize = 400_000;

    /// A minimal cap for fixtures where only the over-cap relationship
    /// matters, never the magnitude.
    const TEST_TINY_CAP_BYTES: usize = 5;

    /// A rendered size over [`TEST_TINY_CAP_BYTES`].
    const TEST_TINY_OVERSIZE_BYTES: usize = 10;

    /// A second rendered size over [`TEST_TINY_CAP_BYTES`], distinct from
    /// [`TEST_TINY_OVERSIZE_BYTES`] to show per-pair sizes never affect
    /// per-path grouping.
    const TEST_TINY_OVERSIZE_ALT_BYTES: usize = 12;

    /// A confirmed finding builder with the load-bearing fields set.
    fn confirmed(
        file: &str,
        line: u32,
        validator: &str,
        rule: Option<&str>,
        claim: &str,
        suggestion: Option<&str>,
    ) -> VerifiedFinding {
        VerifiedFinding {
            finding: Finding {
                file: file.to_string(),
                line,
                validator: validator.to_string(),
                rule: rule.map(String::from),
                claim: claim.to_string(),
                evidence: "cited evidence".to_string(),
                suggestion: suggestion.map(String::from),
            },
            confirmed: true,
            reason: "confirmed".to_string(),
            decided_by: None,
        }
    }

    /// A refuted finding (must be dropped, but still counted as refuted).
    fn refuted(file: &str, line: u32, validator: &str, claim: &str) -> VerifiedFinding {
        VerifiedFinding {
            finding: Finding {
                file: file.to_string(),
                line,
                validator: validator.to_string(),
                rule: None,
                claim: claim.to_string(),
                evidence: "cited evidence".to_string(),
                suggestion: None,
            },
            confirmed: false,
            reason: "refuted by guard".to_string(),
            decided_by: Some(RefutingLayer::Guard),
        }
    }

    #[test]
    fn a_failed_task_tally_flags_an_incomplete_run_in_the_markdown_and_counts() {
        // No findings (every task degraded to zero) but a non-zero failed tally —
        // the report must visibly flag the incomplete run rather than rendering
        // byte-identically to a clean diff, and surface the tally in its counts.
        // Every attempted task failed (failed == attempted), so the run is fully
        // incomplete — the magnitude is immaterial.
        let report = synthesize(
            vec![],
            &FleetTally::new(
                TasksAttempted(ATTEMPTED_TASKS),
                TasksFailed(ATTEMPTED_TASKS),
            ),
            &[],
            &ToolReport::default(),
            NOW,
        );

        assert_eq!(report.counts.tasks_attempted, ATTEMPTED_TASKS);
        assert_eq!(report.counts.tasks_failed, ATTEMPTED_TASKS);
        assert!(
            report.markdown.contains(&format!(
                "{ATTEMPTED_TASKS}/{ATTEMPTED_TASKS} review tasks failed"
            )),
            "the incomplete run must be flagged: {}",
            report.markdown
        );
        assert!(
            report.markdown.contains("INCOMPLETE"),
            "the flag must name the run incomplete: {}",
            report.markdown
        );
    }

    #[test]
    fn a_fully_successful_tally_adds_no_failure_flag() {
        // Every task succeeded — no warning line, byte-identical to today's clean
        // report, and a zero failed tally.
        let report = synthesize(
            vec![],
            &FleetTally::new(TasksAttempted(ATTEMPTED_TASKS), TasksFailed(0)),
            &[],
            &ToolReport::default(),
            NOW,
        );

        assert_eq!(report.markdown, "## Review Findings (2026-04-11 13:08)\n");
        assert_eq!(report.counts.tasks_attempted, ATTEMPTED_TASKS);
        assert_eq!(report.counts.tasks_failed, 0);
    }

    #[test]
    fn renders_dated_header_with_the_input_timestamp_verbatim() {
        let report = synthesize(
            vec![],
            &FleetTally::default(),
            &[],
            &ToolReport::default(),
            NOW,
        );
        assert!(
            report
                .markdown
                .starts_with("## Review Findings (2026-04-11 13:08)\n"),
            "header must match the skill format byte-for-byte: {:?}",
            report.markdown
        );
    }

    #[test]
    fn an_empty_scope_renders_the_nothing_in_scope_marker() {
        // Zero attempted tasks means the resolved scope was empty — the report
        // must say so explicitly instead of rendering a bare findings header
        // that reads identically to a genuinely clean review.
        let report = synthesize(
            vec![],
            &FleetTally::default(),
            &[],
            &ToolReport::default(),
            NOW,
        );
        assert!(
            report
                .markdown
                .starts_with("## Review Findings (2026-04-11 13:08)\n"),
            "the dated header still renders: {:?}",
            report.markdown
        );
        assert!(
            report.markdown.contains("Nothing in scope to review"),
            "an empty scope must be unmistakable: {:?}",
            report.markdown
        );
        assert_eq!(report.counts, ReviewCounts::default());
    }

    #[test]
    fn a_skipped_file_renders_a_named_gap_and_counts_but_is_never_an_error() {
        // A run whose scope was ENTIRELY one oversized file: no fan-out tasks ran
        // (nothing packed), but the skip must be a named gap — not the "Nothing
        // in scope" marker, which would misleadingly claim there was nothing to
        // review at all.
        let skipped = vec![SkippedFile::for_test(
            "src/huge.rs",
            "duplication",
            TEST_OVERSIZE_RENDERED_BYTES,
            TEST_FILE_CAP_BYTES,
        )];
        let report = synthesize(
            vec![],
            &FleetTally::default(),
            &skipped,
            &ToolReport::default(),
            NOW,
        );

        assert!(
            report.markdown.contains("src/huge.rs"),
            "the skipped file must be named: {}",
            report.markdown
        );
        assert!(
            report
                .markdown
                .contains(&TEST_OVERSIZE_RENDERED_BYTES.to_string()),
            "the file's size must be named: {}",
            report.markdown
        );
        assert!(
            report.markdown.contains(&TEST_FILE_CAP_BYTES.to_string()),
            "the per-file cap must be named: {}",
            report.markdown
        );
        assert!(
            !report.markdown.contains("Nothing in scope to review"),
            "a skipped file is a gap, not an empty scope: {}",
            report.markdown
        );
        assert_eq!(report.counts.skipped, 1);
    }

    #[test]
    fn a_skipped_file_becomes_a_confirmed_checklist_finding() {
        // A file no validator could read must not let the review end clean:
        // the skip becomes one CONFIRMED checklist finding per path, so every
        // consumer that gates on findings fails without special handling.
        let skipped = vec![
            SkippedFile::for_test(
                "src/huge.rs",
                "duplication",
                TEST_OVERSIZE_RENDERED_BYTES,
                TEST_FILE_CAP_BYTES,
            ),
            SkippedFile::for_test(
                "src/huge.rs",
                "dead-code",
                TEST_OVERSIZE_ALT_RENDERED_BYTES,
                TEST_FILE_CAP_BYTES,
            ),
        ];
        let report = synthesize(
            vec![],
            &FleetTally::default(),
            &skipped,
            &ToolReport::default(),
            NOW,
        );

        assert!(
            report.markdown.contains("- [ ] `src/huge.rs:1`"),
            "the skip must render as a checklist finding: {}",
            report.markdown
        );
        assert!(
            report.markdown.contains("prompt cap"),
            "the finding must name the prompt cap: {}",
            report.markdown
        );
        assert_eq!(
            report.counts.findings, 1,
            "one finding per skipped path, not per (validator, file) pair"
        );
        assert_eq!(
            report.counts.confirmed, 1,
            "the skip finding is CONFIRMED so the gate cannot pass"
        );
        assert_eq!(report.counts.skipped, 1);
    }

    #[test]
    fn counts_carry_the_skipped_file_list_sorted_and_distinct() {
        // Orchestrators gate on structured data, not on markdown text: the
        // counts carry the skipped paths — distinct, sorted — next to the
        // `skipped` tally.
        let skipped = vec![
            SkippedFile::for_test(
                "src/z.rs",
                "v",
                TEST_TINY_OVERSIZE_BYTES,
                TEST_TINY_CAP_BYTES,
            ),
            SkippedFile::for_test(
                "src/a.rs",
                "v",
                TEST_TINY_OVERSIZE_BYTES,
                TEST_TINY_CAP_BYTES,
            ),
            SkippedFile::for_test(
                "src/a.rs",
                "w",
                TEST_TINY_OVERSIZE_ALT_BYTES,
                TEST_TINY_CAP_BYTES,
            ),
        ];
        let report = synthesize(
            vec![],
            &FleetTally::default(),
            &skipped,
            &ToolReport::default(),
            NOW,
        );

        assert_eq!(report.counts.skipped_files, ["src/a.rs", "src/z.rs"]);
        assert_eq!(report.counts.skipped, 2);
    }

    #[test]
    fn skipped_files_render_sorted_by_path_regardless_of_input_order() {
        let skipped = vec![
            SkippedFile::for_test(
                "src/z.rs",
                "v",
                TEST_TINY_OVERSIZE_BYTES,
                TEST_TINY_CAP_BYTES,
            ),
            SkippedFile::for_test(
                "src/a.rs",
                "v",
                TEST_TINY_OVERSIZE_BYTES,
                TEST_TINY_CAP_BYTES,
            ),
        ];
        let report = synthesize(
            vec![],
            &FleetTally::default(),
            &skipped,
            &ToolReport::default(),
            NOW,
        );

        let a = report.markdown.find("src/a.rs").unwrap();
        let z = report.markdown.find("src/z.rs").unwrap();
        assert!(a < z, "skipped files render sorted: {}", report.markdown);
        assert_eq!(report.counts.skipped, 2);
    }

    #[test]
    fn a_skipped_file_alongside_confirmed_findings_renders_both() {
        // The oversized file does not swallow findings from the files that DID
        // review — both the gap and the confirmed finding must render.
        let verified = vec![confirmed(
            "src/small.rs",
            3,
            "dead-code",
            None,
            "`foo` is never called",
            None,
        )];
        let skipped = vec![SkippedFile::for_test(
            "src/huge.rs",
            "duplication",
            TEST_TINY_OVERSIZE_BYTES,
            TEST_TINY_CAP_BYTES,
        )];
        let report = synthesize(
            verified,
            &FleetTally::new(TasksAttempted(1), TasksFailed(0)),
            &skipped,
            &ToolReport::default(),
            NOW,
        );

        assert!(
            report.markdown.contains("src/huge.rs"),
            "{}",
            report.markdown
        );
        assert!(
            report.markdown.contains("- [ ] `src/small.rs:3`"),
            "the reviewed file's finding must still render: {}",
            report.markdown
        );
        assert_eq!(report.counts.skipped, 1);
        // Two findings: the confirmed one on the reviewed file, plus the
        // engine-emitted skip finding on the oversized file.
        assert_eq!(report.counts.findings, 2);
        assert!(
            report.markdown.contains("- [ ] `src/huge.rs:1`"),
            "the skip must render as a checklist finding: {}",
            report.markdown
        );
    }

    #[test]
    fn an_attempted_clean_run_carries_no_nothing_in_scope_marker() {
        // Tasks ran and found nothing — that is a clean review, not an empty
        // scope, so the marker must not appear.
        let report = synthesize(
            vec![],
            &FleetTally::new(TasksAttempted(ATTEMPTED_TASKS), TasksFailed(0)),
            &[],
            &ToolReport::default(),
            NOW,
        );
        assert!(
            !report.markdown.contains("Nothing in scope"),
            "a clean attempted run is not an empty scope: {:?}",
            report.markdown
        );
    }

    #[test]
    #[tracing_test::traced_test]
    fn synthesize_logs_the_final_finding_and_verdict_counts() {
        let verified = vec![
            confirmed(
                "src/a.rs",
                42,
                "dead-code",
                Some("no-unused"),
                "`foo` is never called",
                Some("Delete it"),
            ),
            refuted("src/a.rs", 99, "dead-code", "`bar` is never called"),
        ];

        let _report = synthesize(
            verified,
            &FleetTally::default(),
            &[],
            &ToolReport::default(),
            NOW,
        );

        // The synthesis summary reports the rendered-finding + per-verdict tallies.
        assert!(logs_contain("review synthesis complete"));
        assert!(logs_contain("findings=1"));
        assert!(logs_contain("confirmed=1"));
        assert!(logs_contain("refuted=1"));
    }

    #[test]
    fn drops_refuted_findings_but_still_counts_them() {
        let verified = vec![
            confirmed(
                "src/a.rs",
                42,
                "dead-code",
                Some("no-unused"),
                "`foo` is never called",
                Some("Delete it"),
            ),
            refuted("src/a.rs", 99, "dead-code", "`bar` is never called"),
        ];

        let report = synthesize(
            verified,
            &FleetTally::default(),
            &[],
            &ToolReport::default(),
            NOW,
        );

        // The refuted finding does not appear in the rendered markdown.
        assert!(
            !report.markdown.contains("src/a.rs:99"),
            "{}",
            report.markdown
        );
        assert!(!report.markdown.contains("`bar`"), "{}", report.markdown);
        // The confirmed one does.
        assert!(
            report.markdown.contains("src/a.rs:42"),
            "{}",
            report.markdown
        );
        // Counts reflect both verdicts; only the confirmed finding is rendered.
        assert_eq!(report.counts.confirmed, 1);
        assert_eq!(report.counts.refuted, 1);
        assert_eq!(report.counts.findings, 1);
    }

    #[test]
    fn collapses_exact_repeats_into_one_item() {
        // Two byte-identical findings (same file, line, validator, rule, claim).
        let one = confirmed(
            "src/a.rs",
            42,
            "dead-code",
            Some("no-unused"),
            "`foo` is never called",
            Some("Delete it"),
        );
        let report = synthesize(
            vec![one.clone(), one],
            &FleetTally::default(),
            &[],
            &ToolReport::default(),
            NOW,
        );

        // Collapsed to a single checklist item.
        let occurrences = report.markdown.matches("src/a.rs:42").count();
        assert_eq!(
            occurrences, 1,
            "exact repeats collapse: {}",
            report.markdown
        );
        assert_eq!(report.counts.findings, 1);
        // Both were confirmed, so the confirmed count is the pre-dedup total.
        assert_eq!(report.counts.confirmed, 2);
    }

    #[test]
    fn keeps_two_validators_on_the_same_file_line_and_orders_them() {
        // duplication and dead-code both flag src/a.rs:42 — distinct lenses, both
        // kept, and rendered adjacently because they share a file:line.
        let dup = confirmed(
            "src/a.rs",
            42,
            "duplication",
            Some("no-copy-paste"),
            "Duplicated block also lives in b.rs",
            Some("Extract a shared helper"),
        );
        let dead = confirmed(
            "src/a.rs",
            42,
            "dead-code",
            Some("no-unused"),
            "`foo` is never called",
            Some("Delete it"),
        );
        let report = synthesize(
            vec![dup, dead],
            &FleetTally::default(),
            &[],
            &ToolReport::default(),
            NOW,
        );

        // Both findings survive — cross-validator findings are never merged.
        assert!(
            report.markdown.contains("Duplicated block"),
            "{}",
            report.markdown
        );
        assert!(
            report.markdown.contains("`foo` is never called"),
            "{}",
            report.markdown
        );
        assert_eq!(report.counts.findings, 2);

        // They render adjacently because they share a file:line.
        let both = report.markdown.matches("`src/a.rs:42`").count();
        assert_eq!(
            both, 2,
            "both co-located findings are kept: {}",
            report.markdown
        );
    }

    #[test]
    fn one_rule_matching_multiple_lines_renders_every_instance() {
        // The no-bail-fast / whole-file-sweep contract: a single rule firing on
        // N lines of ONE file touched by ONE commit yields N findings on the
        // first pass, all rendered — never collapsed to the first match, never
        // dribbled one-per-re-review. Same file, validator, rule, and claim;
        // only the line differs, so the conservative dedup key (which includes
        // the line) keeps each occurrence.
        let rule = Some("no-unused");
        let lines = [12u32, 34, 56, 78];
        let verified: Vec<_> = lines
            .iter()
            .map(|line| {
                confirmed(
                    "src/a.rs",
                    *line,
                    "dead-code",
                    rule,
                    "`foo` is never called",
                    None,
                )
            })
            .collect();
        let report = synthesize(
            verified,
            &FleetTally::default(),
            &[],
            &ToolReport::default(),
            NOW,
        );

        // Every occurrence survives as its own checklist item, one per file:line.
        for line in lines {
            assert!(
                report
                    .markdown
                    .contains(&format!("- [ ] `src/a.rs:{line}`")),
                "instance at line {line} must render: {}",
                report.markdown
            );
        }
        // Not collapsed: all N render and are counted on the first pass.
        assert_eq!(
            report.markdown.matches("- [ ] `src/a.rs:").count(),
            lines.len(),
            "every instance of the rule must render: {}",
            report.markdown
        );
        assert_eq!(report.counts.findings, lines.len());
    }

    #[test]
    fn renders_one_flat_findings_section_with_no_severity_grouping() {
        // Review is binary pass/fail: every confirmed finding renders as one flat
        // checklist item ordered by file:line — there are NO severity subsections.
        let verified = vec![
            confirmed("src/a.rs", 10, "dead-code", None, "First concern", None),
            confirmed("src/b.rs", 20, "style", None, "Second concern", None),
        ];
        let report = synthesize(
            verified,
            &FleetTally::default(),
            &[],
            &ToolReport::default(),
            NOW,
        );

        assert!(
            !report.markdown.contains("### Blockers")
                && !report.markdown.contains("### Warnings")
                && !report.markdown.contains("### Nits"),
            "no severity sections may render: {}",
            report.markdown
        );
        assert!(
            report.markdown.contains("- [ ] `src/a.rs:10`"),
            "{}",
            report.markdown
        );
        assert!(
            report.markdown.contains("- [ ] `src/b.rs:20`"),
            "{}",
            report.markdown
        );
        assert_eq!(report.counts.findings, 2);
    }

    #[test]
    fn renders_the_exact_skill_section_format() {
        // A full snapshot against the documented `builtin/skills/review/SKILL.md`
        // step-8 layout: the dated header then ONE flat checklist ordered by
        // `file:line` — no severity subsections.
        let verified = vec![
            confirmed(
                "path/to/file.rs",
                42,
                "dead-code",
                Some("no-unused"),
                "What's wrong. Why it matters",
                Some("Suggested fix"),
            ),
            confirmed(
                "path/to/file.rs",
                10,
                "perf",
                None,
                "What's wrong and suggested fix",
                None,
            ),
            confirmed("path/to/file.rs", 88, "style", None, "Minor issue", None),
        ];
        let report = synthesize(
            verified,
            &FleetTally::default(),
            &[],
            &ToolReport::default(),
            NOW,
        );

        let expected = "\
## Review Findings (2026-04-11 13:08)

- [ ] `path/to/file.rs:10` — What's wrong and suggested fix.
- [ ] `path/to/file.rs:42` — What's wrong. Why it matters. Suggested fix.
- [ ] `path/to/file.rs:88` — Minor issue.
";
        assert_eq!(report.markdown, expected);
    }

    #[test]
    fn orders_findings_by_file_line() {
        // Submitted out of order; rendered ordered by file:line.
        let verified = vec![
            confirmed("src/z.rs", 5, "v", None, "z concern", None),
            confirmed("src/a.rs", 90, "v", None, "a90 concern", None),
            confirmed("src/a.rs", 9, "v", None, "a9 concern", None),
        ];
        let report = synthesize(
            verified,
            &FleetTally::default(),
            &[],
            &ToolReport::default(),
            NOW,
        );

        let a9 = report.markdown.find("src/a.rs:9`").unwrap();
        let a90 = report.markdown.find("src/a.rs:90`").unwrap();
        let z5 = report.markdown.find("src/z.rs:5`").unwrap();
        assert!(a9 < a90, "a.rs:9 before a.rs:90: {}", report.markdown);
        assert!(a90 < z5, "a.rs before z.rs: {}", report.markdown);
    }

    // ---- candidate assembly (the pure half of `run_review`) --------------

    /// A bare `Finding` tagged with a validator/file (the shape `run_fleet`
    /// emits — context lives in the work-list, not on the finding).
    fn finding(file: &str, line: u32, validator: &str, claim: &str) -> Finding {
        Finding {
            file: file.to_string(),
            line,
            validator: validator.to_string(),
            rule: None,
            claim: claim.to_string(),
            evidence: "e".to_string(),
            suggestion: None,
        }
    }

    /// A `FileWork` carrying a distinctive source slice tagged with its path.
    fn file_work(path: &str) -> FileWork {
        FileWork::new(
            path.to_string(),
            vec![],
            vec![],
            format!("// slice for {path}"),
            vec![],
        )
    }

    /// A `ValidatorWork` carrying the given files for one validator.
    fn validator_work(name: &str, files: Vec<FileWork>) -> ValidatorWork {
        ValidatorWork::new(
            name.to_string(),
            RuleNames::default(),
            ProbeNames::default(),
            files,
        )
    }

    #[test]
    fn build_candidates_pairs_each_finding_with_its_files_context() {
        let work = WorkList::new(
            "p".to_string(),
            vec![validator_work("dedup", vec![file_work("src/a.rs")])],
        );
        let candidates = build_candidates(&work, vec![finding("src/a.rs", 42, "dedup", "dup")]);

        assert_eq!(candidates.len(), 1);
        // The candidate reuses the work-list's bounded slice verbatim.
        assert_eq!(candidates[0].source_slice, "// slice for src/a.rs");
    }

    #[test]
    fn build_candidates_resolves_each_finding_to_its_own_validators_context() {
        // Two validators flag the SAME file:line — each candidate must pick up its
        // own validator's file context, not the other's.
        let work = WorkList::new(
            "p".to_string(),
            vec![
                validator_work("dead-code", vec![file_work("src/a.rs")]),
                validator_work("duplication", vec![file_work("src/a.rs")]),
            ],
        );
        let candidates = build_candidates(
            &work,
            vec![
                finding("src/a.rs", 42, "dead-code", "`foo` is dead"),
                finding("src/a.rs", 42, "duplication", "dup of b.rs"),
            ],
        );

        // Both findings produce candidates (cross-validator, never merged).
        assert_eq!(candidates.len(), 2);
        assert!(candidates
            .iter()
            .all(|c| c.source_slice == "// slice for src/a.rs"));
        assert!(candidates
            .iter()
            .any(|c| c.finding.validator == "dead-code"));
        assert!(candidates
            .iter()
            .any(|c| c.finding.validator == "duplication"));
    }

    #[test]
    fn build_candidates_yields_empty_context_for_an_unknown_validator_or_file() {
        // A finding whose (validator, file) is not in the work-list still becomes
        // a candidate (empty context) so it reaches the verifier and refutes there.
        let work = WorkList::new(
            "p".to_string(),
            vec![validator_work("dedup", vec![file_work("src/a.rs")])],
        );
        let candidates = build_candidates(
            &work,
            vec![finding("src/invented.rs", 1, "ghost-validator", "made up")],
        );

        assert_eq!(
            candidates.len(),
            1,
            "an unmatched finding is kept, not dropped"
        );
        assert_eq!(candidates[0].source_slice, "");
        assert!(candidates[0].probe_results.is_empty());
    }

    // ---- tool-rule facts in the report ------------------------------------

    use crate::review::tool_rules::{ToolFallback, ToolRunError};

    #[test]
    fn a_tool_error_renders_the_rule_and_its_raw_stderr_and_counts() {
        let tools = ToolReport::new(
            1,
            vec![ToolRunError::for_test(
                "docs",
                "docs-tool",
                "line one of stderr\nline two of stderr",
            )],
            vec![],
        );

        let report = synthesize(vec![], &FleetTally::default(), &[], &tools, NOW);

        assert_eq!(report.counts().tool_errors(), 1);
        assert!(
            report
                .markdown
                .contains("tool rule 'docs/docs-tool' failed"),
            "the error block must name the rule: {}",
            report.markdown
        );
        assert!(
            report.markdown.contains("> line one of stderr"),
            "every raw stderr line must render, quoted: {}",
            report.markdown
        );
        assert!(
            report.markdown.contains("> line two of stderr"),
            "every raw stderr line must render, quoted: {}",
            report.markdown
        );
        assert!(
            !report.markdown.contains("Nothing in scope to review."),
            "a tool error must never read as an empty scope: {}",
            report.markdown
        );
    }

    #[test]
    fn a_tool_fallback_without_a_superseded_rule_says_none_is_named() {
        let tools = ToolReport::new(
            0,
            vec![],
            vec![ToolFallback::for_test(
                "docs",
                "docs-tool",
                &[],
                "tool missing: no ruff",
            )],
        );

        let report = synthesize(vec![], &FleetTally::default(), &[], &tools, NOW);

        assert!(
            report
                .markdown
                .contains("tool rule 'docs/docs-tool' is unavailable (tool missing: no ruff)"),
            "the fallback note must name the rule and the reason: {}",
            report.markdown
        );
        assert!(
            report
                .markdown
                .contains("no prompt rule is named to run instead"),
            "a fallback without supersedes must say so: {}",
            report.markdown
        );
    }

    /// A fallback whose tool rule names two prompt rules names BOTH in the
    /// report: the reader must know every prompt rule that reviewed the files.
    #[test]
    fn a_tool_fallback_names_every_superseded_prompt_rule() {
        let tools = ToolReport::new(
            0,
            vec![],
            vec![ToolFallback::for_test(
                "hygiene",
                "complexity-rust",
                &["cognitive-complexity", "function-length"],
                "tool missing: no clippy",
            )],
        );

        let report = synthesize(vec![], &FleetTally::default(), &[], &tools, NOW);

        assert!(
            report
                .markdown
                .contains("prompt rules 'cognitive-complexity', 'function-length' ran instead"),
            "the note must name every superseded prompt rule: {}",
            report.markdown
        );
    }

    #[test]
    fn an_inert_tool_report_keeps_the_nothing_in_scope_marker() {
        let report = synthesize(
            vec![],
            &FleetTally::default(),
            &[],
            &ToolReport::default(),
            NOW,
        );
        assert!(
            report.markdown.contains("Nothing in scope to review."),
            "an empty run with inert tools is an empty scope: {}",
            report.markdown
        );
    }
}
