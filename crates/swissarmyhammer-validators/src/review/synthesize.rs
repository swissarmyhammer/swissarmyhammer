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
//!    so the existing task-history parsing keeps working. Every item names its
//!    validator set and rule beside the `file:line`, so the reader of a card can
//!    open the rule that produced a finding without searching for it.

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
    as_borrowed_strings, batch_work_list, detected_project_type_keys, line_is_reviewed,
    scope_review, ExcludedFile, ExclusionKind, LineAnnotation, ReviewSubject, Scope, SkippedFile,
    WorkList,
};
use crate::review::tool_health::ToolHealthCache;
use crate::review::tool_install::{install_missing_tools, PoolInstallAgent};
use crate::review::tool_rules::{plan_tool_rules, start_tool_runs, ToolReport};
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
    /// Every file path the run did not review — distinct, sorted: the
    /// `skipped` over-cap paths plus the paths the scope stage excluded
    /// deliberately (an ignore rule matched it, or it is a validator set's own
    /// fixture data). Orchestrators gate on this list without parsing markdown;
    /// the markdown names each path's reason.
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

    /// Every file path the run did not review — distinct, sorted: the
    /// [`ReviewCounts::skipped`] over-cap paths plus the paths the scope stage
    /// excluded deliberately (an ignore rule matched it, or it is a validator
    /// set's own fixture data). Orchestrators gate on this list without parsing
    /// markdown; the markdown names each path's reason.
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
/// `excluded` names every [`ExcludedFile`] the scope stage dropped before any
/// validator paired with it — a file an ignore pattern matched, or a validator
/// set's own fixture data. Each is rendered as a named note with its reason and
/// each path joins [`ReviewCounts::skipped_files`], so the exclusion is reported
/// rather than silent. It is deliberately NOT a finding and NOT counted by
/// [`ReviewCounts::skipped`]: a fixture is data the store declares, `sah doctor`
/// is its gate, and reviewing it would fire every rule the fail fixture exists
/// to make fire; an ignore exclusion is what the repository's own configuration
/// asked for.
///
/// When the exclusions cover every file the scope resolved, the report says so
/// explicitly. That review is CLEAN — it named what it excluded and why — and
/// it must read as neither an ordinary clean pass nor one of the empty results
/// that stay gaps: an empty scope, a stalled run, or a size-cap skip.
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
    excluded: &[ExcludedFile],
    tools: &ToolReport,
    scope: &ReviewedScope,
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
        skipped_files: not_reviewed_paths(&by_path, excluded),
        tool_errors: tools.errors().len(),
        ..ReviewCounts::default()
    };

    let mut markdown = String::new();
    let _ = writeln!(markdown, "## Review Findings ({now})");

    // The scope line, always — before any finding and before any warning. A
    // narrowed scope that reports nothing must never read as a clean result,
    // so the report states what was reviewed and how much was not, whether or
    // not anything was found.
    render_scope(&mut markdown, scope, counts.skipped_files.len());

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

    render_excluded_files(&mut markdown, excluded);
    render_tool_errors(&mut markdown, tools);
    render_tool_fallbacks(&mut markdown, tools);

    // An empty result names its cause, every time. A full exclusion is a
    // deliberate, wanted outcome — the fork workflow's whole point — so it says
    // so and passes; an empty scope is a gap and says THAT instead. The two are
    // mutually exclusive: the first needs exclusions, the second needs none.
    render_full_exclusion(&mut markdown, scope, excluded);

    // Say so explicitly when the resolved scope was empty (zero fan-out tasks,
    // zero tool activity, nothing skipped and nothing excluded either): a bare
    // findings header would read identically to a genuinely clean review.
    if tally.attempted == 0
        && kept.is_empty()
        && skipped.is_empty()
        && excluded.is_empty()
        && tools.is_inert()
    {
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
        skipped = counts.skipped,
        excluded = excluded.len(),
        "review synthesis complete"
    );

    ReviewReport { markdown, counts }
}

/// What a report says it reviewed: the op as the caller named it, what that op
/// makes the subject, and how many distinct files it reached.
///
/// Every report opens with this, whatever it found. A review whose scope was
/// narrowed — by the op, by `.reviewignore`, by a fixture exclusion, by an
/// over-cap file — and which then reports nothing is indistinguishable from a
/// review that read everything and found nothing, unless the report says which
/// it was. `^0fn6dbf` moved to `review` with zero findings after its agent
/// stalled, and that read as clean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedScope {
    /// The op and its target, as [`Scope::describe`] writes it.
    op: String,
    /// What the op REVIEWS — the diffs, or the files whole.
    subject: ReviewSubject,
    /// How many distinct files the run reviewed.
    files: usize,
    /// How many files the scope stage resolved, before any exclusion — the
    /// denominator a full exclusion is measured against.
    resolved: usize,
}

impl ReviewedScope {
    /// Describe the scope a run resolved, alongside how many distinct files it
    /// reviewed and how many the scope stage resolved before any exclusion.
    pub fn new(scope: &Scope, files: usize, resolved: usize) -> Self {
        Self {
            op: scope.describe(),
            subject: scope.subject(),
            files,
            resolved,
        }
    }
}

/// Render the report's scope line: the op, what it REVIEWS, the file count it
/// reached, and the count it did not.
///
/// A `> ` block quote so it reads as a header note rather than a finding, and
/// unconditional so `not_reviewed: 0` is stated rather than implied by silence.
fn render_scope(markdown: &mut String, scope: &ReviewedScope, not_reviewed: usize) {
    let _ = writeln!(
        markdown,
        "\n> Scope: `{}` — reviewed {}. {} file(s) reviewed, {} not reviewed.",
        scope.op,
        scope.subject.describe(),
        scope.files,
        not_reviewed
    );
}

/// Drop every finding that does not land on a line this review REVIEWS.
///
/// Fan-out findings are already filtered by the verify guard
/// ([`run_guard`](crate::review::verify::run_guard)), which refutes an
/// off-change candidate before it costs an agent turn. Tool-rule findings never
/// pass through verify — deterministic tool output is CONFIRMED by
/// construction — so under [`ReviewSubject::Diffs`] they are filtered here
/// instead, against the same per-line marks the guard reads, via the same
/// [`line_is_reviewed`] predicate. Without this a tool rule reports every
/// pre-existing instance in a changed file, which is exactly what a diff
/// subject excludes.
///
/// A finding on a path the work-list does not carry has no annotations to
/// judge it by, so it is kept — [`line_is_reviewed`] answers `true` on every
/// undecidable case.
fn retain_findings_on_the_change(
    findings: impl IntoIterator<Item = VerifiedFinding>,
    work: &WorkList,
) -> Vec<VerifiedFinding> {
    let findings = findings.into_iter();
    if matches!(work.subject(), ReviewSubject::Files) {
        return findings.collect();
    }
    let annotations: BTreeMap<&str, &[LineAnnotation]> = work
        .distinct_files()
        .map(|file| (file.path(), file.line_annotations()))
        .collect();
    findings
        .filter(|verified| {
            let marks = annotations
                .get(verified.finding.file.as_str())
                .copied()
                .unwrap_or(&[]);
            let kept = line_is_reviewed(work.subject(), marks, verified.finding.line);
            if !kept {
                tracing::debug!(
                    file = %verified.finding.file,
                    line = verified.finding.line,
                    validator = %verified.finding.validator,
                    "tool finding dropped: the cited line is not one this change touched"
                );
            }
            kept
        })
        .collect()
}

/// Every path the run did not review, distinct and sorted: the over-cap paths
/// the packer skipped plus the paths the scope stage excluded.
///
/// One list rather than two, because a consumer that gates on coverage asks one
/// question — which files did this run not read? The reason each path is on the
/// list is in the markdown, and [`ReviewCounts::skipped`] still counts the
/// over-cap half alone.
fn not_reviewed_paths(
    by_path: &BTreeMap<&str, SkipGroup<'_>>,
    excluded: &[ExcludedFile],
) -> Vec<String> {
    let paths: BTreeSet<&str> = by_path
        .keys()
        .copied()
        .chain(excluded.iter().map(ExcludedFile::path))
        .collect();
    paths.into_iter().map(str::to_string).collect()
}

/// Render every file the scope stage excluded, naming the reason it was
/// dropped.
///
/// A note rather than a warning, and never a finding: the exclusion is
/// deliberate, so the reader needs to know it happened and why, not to fix it.
/// The two kinds render differently because they read differently — an ignore
/// pattern is a repository's own configuration and covers whole directories at
/// a time, a fixture is one file the validator store declares.
fn render_excluded_files(markdown: &mut String, excluded: &[ExcludedFile]) {
    render_ignored_files(markdown, excluded);
    render_fixture_files(markdown, excluded);
}

/// Render the files an ignore pattern excluded, grouped under the pattern and
/// the ignore file it came from.
///
/// Grouped rather than one line per file because a single pattern routinely
/// covers a whole directory of changed files — a board directory the finish
/// loop rewrites on every comment — and a report that names each of them buries
/// what it actually reviewed. Every path still rides in
/// [`ReviewCounts::skipped_files`] for a consumer that wants them.
fn render_ignored_files(markdown: &mut String, excluded: &[ExcludedFile]) {
    let by_pattern = count_by_reason(excluded, ExclusionKind::ReviewIgnore);
    if by_pattern.is_empty() {
        return;
    }
    let total: usize = by_pattern.values().sum();
    let _ = writeln!(
        markdown,
        "\n> {total} file(s) not reviewed — excluded by an ignore rule:"
    );
    for (pattern, count) in by_pattern {
        let _ = writeln!(markdown, "> - `{pattern}` — {count} file(s)");
    }
}

/// Render one note per file excluded as a validator set's own fixture data,
/// naming the file and the reason it was dropped.
fn render_fixture_files(markdown: &mut String, excluded: &[ExcludedFile]) {
    let fixtures: Vec<&ExcludedFile> = excluded
        .iter()
        .filter(|file| file.kind() == ExclusionKind::ValidatorFixture)
        .collect();
    if fixtures.is_empty() {
        return;
    }
    let _ = writeln!(
        markdown,
        "\n> {} file(s) not reviewed — excluded from the review scope:",
        fixtures.len()
    );
    for file in fixtures {
        let _ = writeln!(markdown, "> - `{}` — {}", file.path(), file.reason());
    }
}

/// How many files carry each distinct reason among the exclusions of `kind`.
///
/// Sorted by reason, so the rendered group order is the same on every run.
fn count_by_reason(excluded: &[ExcludedFile], kind: ExclusionKind) -> BTreeMap<&str, usize> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for file in excluded.iter().filter(|file| file.kind() == kind) {
        *counts.entry(file.reason()).or_default() += 1;
    }
    counts
}

/// State a FULL exclusion: every file the scope resolved was excluded, so
/// nothing was left to review.
///
/// The notes above name each pattern and each reason; this line says they
/// covered the WHOLE scope. Without it a fully excluded run renders as a header
/// and a scope line, which is exactly what an ordinary clean pass renders as —
/// and that is the one thing this report must never be mistaken for. It is a
/// clean, passing review: the run read nothing because the repository asked it
/// to read nothing. The empty results that stay gaps — an empty scope, a
/// stalled run, a size-cap skip — each carry their own line instead.
fn render_full_exclusion(markdown: &mut String, scope: &ReviewedScope, excluded: &[ExcludedFile]) {
    if scope.resolved == 0 || excluded.len() < scope.resolved {
        return;
    }
    let _ = writeln!(
        markdown,
        "\n> Every file in scope was excluded — {} of {} file(s) — so nothing was left to review. The exclusions above are deliberate: this is a clean review, not an empty scope, a failed run, or a size-cap skip.",
        excluded.len(),
        scope.resolved
    );
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
/// The shape is `` - [ ] `file:line` `set/rule` — claim. suggestion. `` — the
/// location, then the [attribution](Finding::attribution) beside it, then the
/// claim (what + why it matters) followed by the suggestion when the agent
/// offered one, each terminated as a sentence. A finding with no suggestion
/// renders the claim alone.
///
/// The attribution is not decoration. An implementer who picks up a card with
/// open findings has to read the rule that produced each one — to fix it, and to
/// judge whether the rule measures the right thing — and without the set and the
/// rule on the item that means inferring the rule from the claim's wording and
/// searching the validator store for it. Naming the pair here turns that search
/// into opening one file.
fn render_item(finding: &Finding) -> String {
    let mut body = sentence(&finding.claim);
    if let Some(suggestion) = &finding.suggestion {
        let suggestion = suggestion.trim();
        if !suggestion.is_empty() {
            body.push(' ');
            body.push_str(&sentence(suggestion));
        }
    }
    format!(
        "- [ ] `{}:{}` `{}` — {}",
        finding.file,
        finding.line,
        finding.attribution(),
        body
    )
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
/// 2. [`plan_tool_rules`] + [`start_tool_runs`] — plan every matched tool
///    rule ONCE for the whole run, before any batching, and start the healthy
///    ones at the workspace root. The plan's suppression map rides into every
///    batch's fan-out so a superseded prompt rule is skipped per file; an
///    unhealthy tool suppresses nothing and is reported as a prompt fallback.
///    The scripts then run WHILE stage 4 works, because the fan-out needs only
///    that map and the tool findings are read at synthesis. Those findings
///    join the verified stream as CONFIRMED (deterministic tool output skips
///    the adversarial verify pass).
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
    // Kept past the move so the report can name the op the caller asked for,
    // not a reconstruction of it.
    let requested_scope = scope.clone();
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

    // The fixture half of each health check is proved once per (tool version,
    // rule content) and stored beside the workspace, so a review that changed
    // neither the tool nor the rule plans without running a fixture at all.
    // `sah doctor` never reads that store — it proves the rules and replaces
    // what is stored — so doctor stays the ground truth.
    let health = ToolHealthCache::open(repo_path);
    let tool_plan = plan_tool_rules(&work, loader, &project_types, Some(&health));
    health.save();

    let tool_attempted = tool_plan.runs().len();
    let (tool_runs, tool_fallbacks, suppression) = tool_plan.into_parts();

    // Stage 2b: start the planned scripts and let them run WHILE the fleet
    // works. The fan-out reads the plan's suppression map, which stage 2 has
    // already decided; the tool findings are read at synthesis and nowhere
    // earlier, so the tool runs no longer sit in front of the first fleet task.
    let tool_runs = start_tool_runs(tool_runs, repo_path, progress);

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
    // The cost function is subject-aware: a diff subject renders each file's
    // changed regions rather than the file, so the packer budgets the bytes
    // the agent actually receives.
    let subject = work.subject();
    let (batches, skipped) = batch_work_list(&work, budget, |file| {
        rendered_file_block_bytes(file, subject)
    });

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
        let outcome = verify_findings(candidates, pool, prime_session, progress, subject).await;

        // The batch (fan-out AND verify) has drained: release its prime pin so the
        // pinned cache entry does not outlive the batch. A run future dropped
        // before this point releases it from the guard's `Drop` instead.
        if let Some(guard) = prime {
            crate::review::fleet::unpin_prefix_session(guard).await;
        }

        verified.extend(outcome.verified);
    }

    // The fleet has drained, so this is the first moment the tool findings are
    // needed. Tool findings are already CONFIRMED — deterministic tool output
    // skips the adversarial verify pass — so they join the verified stream
    // directly.
    let (tool_findings, tool_errors) = tool_runs.finish().await.into_parts();
    // A tool rule reads whole files, so under a diff subject it reports
    // pre-existing instances the verify guard would have refuted had they come
    // from an agent. They never reach verify, so the same boundary is applied
    // here.
    verified.extend(retain_findings_on_the_change(tool_findings, &work));

    // Stage 5: synthesize the merged, deduped, ordered, dated report. The summed
    // tally rides into the report so the tool boundary can flag/fail an incomplete
    // run; the engine itself stays a pure data barrier and never errors on it. Any
    // oversized files stage 2 excluded ride in too, as a named gap, along with
    // the run's tool-rule facts (broken runs and prompt fallbacks).
    let report = synthesize(
        verified,
        &FleetTally::new(TasksAttempted(attempted), TasksFailed(failed)),
        &skipped,
        work.excluded(),
        &ToolReport::new(tool_attempted, tool_errors, tool_fallbacks),
        &ReviewedScope::new(
            &requested_scope,
            work.distinct_files().count(),
            work.resolved_files(),
        ),
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
fn build_candidates(
    work: &WorkList,
    findings: impl IntoIterator<Item = Finding>,
) -> Vec<Candidate> {
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

    /// How many files the fixture scope line reports as reviewed. These tests
    /// assert on findings and gap blocks, not on the scope line's count, so the
    /// number only has to be fixed.
    const SCOPE_FILES: usize = 1;

    /// [`synthesize`] under a fixed `review working` scope.
    ///
    /// Every test here asserts on what the findings, gaps and notes render to,
    /// none on which op produced them, so the scope is named once here rather
    /// than repeated at two dozen call sites.
    fn synthesize_working(
        verified: impl IntoIterator<Item = VerifiedFinding>,
        tally: &FleetTally,
        skipped: &[SkippedFile],
        excluded: &[ExcludedFile],
        tools: &ToolReport,
        now: &str,
    ) -> ReviewReport {
        synthesize(
            verified,
            tally,
            skipped,
            excluded,
            tools,
            // The scope resolved what it reviewed plus what it excluded, so a
            // fixture that carries an exclusion never reads as a FULL one.
            &ReviewedScope::new(&Scope::Working, SCOPE_FILES, SCOPE_FILES + excluded.len()),
            now,
        )
    }

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
        let report = synthesize_working(
            vec![],
            &FleetTally::new(
                TasksAttempted(ATTEMPTED_TASKS),
                TasksFailed(ATTEMPTED_TASKS),
            ),
            &[],
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
        let report = synthesize_working(
            vec![],
            &FleetTally::new(TasksAttempted(ATTEMPTED_TASKS), TasksFailed(0)),
            &[],
            &[],
            &ToolReport::default(),
            NOW,
        );

        assert!(
            !report.markdown.contains("INCOMPLETE"),
            "a fully successful tally raises no incomplete flag: {}",
            report.markdown
        );
        assert_eq!(report.counts.tasks_attempted, ATTEMPTED_TASKS);
        assert_eq!(report.counts.tasks_failed, 0);
    }

    #[test]
    fn renders_dated_header_with_the_input_timestamp_verbatim() {
        let report = synthesize_working(
            vec![],
            &FleetTally::default(),
            &[],
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
        let report = synthesize_working(
            vec![],
            &FleetTally::default(),
            &[],
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
        let report = synthesize_working(
            vec![],
            &FleetTally::default(),
            &skipped,
            &[],
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

    /// The path of the excluded validator fixture the tests below report on.
    const TEST_EXCLUDED_FIXTURE: &str =
        "builtin/validators/code-hygiene/fixtures/missing-docs-rust.fail.rs.tmpl";

    #[test]
    fn an_excluded_file_is_reported_with_its_reason_and_is_never_a_finding() {
        // A run whose scope carried a validator set's own fixture data. The
        // exclusion is deliberate — `sah doctor` is the fixture's gate — so it
        // must be REPORTED with its reason and listed among the files the run
        // did not review, and it must raise no finding at all.
        let excluded = vec![ExcludedFile::validator_fixture(TEST_EXCLUDED_FIXTURE)];

        let report = synthesize_working(
            vec![],
            &FleetTally::default(),
            &[],
            &excluded,
            &ToolReport::default(),
            NOW,
        );

        assert!(
            report.markdown.contains(TEST_EXCLUDED_FIXTURE),
            "the excluded file must be named: {}",
            report.markdown
        );
        assert!(
            report.markdown.contains("validator fixture"),
            "the reason must be named: {}",
            report.markdown
        );
        assert_eq!(
            report.counts.skipped_files,
            [TEST_EXCLUDED_FIXTURE],
            "the excluded path rides in the structured list too"
        );
        assert_eq!(
            report.counts.findings, 0,
            "an exclusion is never a finding: {}",
            report.markdown
        );
        assert_eq!(
            report.counts.confirmed, 0,
            "an exclusion never enters the finding stream: {}",
            report.markdown
        );
        assert_eq!(
            report.counts.skipped, 0,
            "an exclusion is not an over-cap coverage failure"
        );
        assert!(
            !report.markdown.contains("Nothing in scope to review"),
            "a scope carrying only excluded files is not an empty scope: {}",
            report.markdown
        );
    }

    #[test]
    fn an_excluded_file_and_a_skipped_file_share_one_not_reviewed_list() {
        // The two reasons a run does not read a file are different — one is a
        // coverage failure, one is deliberate — but a consumer gating on
        // coverage asks one question, so both paths ride in one sorted list.
        let skipped = vec![SkippedFile::for_test(
            "src/huge.rs",
            "duplication",
            TEST_OVERSIZE_RENDERED_BYTES,
            TEST_FILE_CAP_BYTES,
        )];
        let excluded = vec![ExcludedFile::validator_fixture(TEST_EXCLUDED_FIXTURE)];

        let report = synthesize_working(
            vec![],
            &FleetTally::default(),
            &skipped,
            &excluded,
            &ToolReport::default(),
            NOW,
        );

        assert_eq!(
            report.counts.skipped_files,
            [TEST_EXCLUDED_FIXTURE, "src/huge.rs"],
            "both paths ride in one distinct, sorted list"
        );
        assert_eq!(
            report.counts.skipped, 1,
            "only the over-cap path is an over-cap skip"
        );
        assert_eq!(
            report.counts.findings, 1,
            "only the over-cap path becomes a finding: {}",
            report.markdown
        );
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
        let report = synthesize_working(
            vec![],
            &FleetTally::default(),
            &skipped,
            &[],
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
        let report = synthesize_working(
            vec![],
            &FleetTally::default(),
            &skipped,
            &[],
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
        let report = synthesize_working(
            vec![],
            &FleetTally::default(),
            &skipped,
            &[],
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
        let report = synthesize_working(
            verified,
            &FleetTally::new(TasksAttempted(1), TasksFailed(0)),
            &skipped,
            &[],
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
        let report = synthesize_working(
            vec![],
            &FleetTally::new(TasksAttempted(ATTEMPTED_TASKS), TasksFailed(0)),
            &[],
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

        let _report = synthesize_working(
            verified,
            &FleetTally::default(),
            &[],
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

        let report = synthesize_working(
            verified,
            &FleetTally::default(),
            &[],
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
        let report = synthesize_working(
            vec![one.clone(), one],
            &FleetTally::default(),
            &[],
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
        let report = synthesize_working(
            vec![dup, dead],
            &FleetTally::default(),
            &[],
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
        let report = synthesize_working(
            verified,
            &FleetTally::default(),
            &[],
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
        let report = synthesize_working(
            verified,
            &FleetTally::default(),
            &[],
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
        let report = synthesize_working(
            verified,
            &FleetTally::default(),
            &[],
            &[],
            &ToolReport::default(),
            NOW,
        );

        // The scope line opens every report, whatever it found, so a narrowed
        // scope can never read as a clean result.
        let expected = "\
## Review Findings (2026-04-11 13:08)

> Scope: `review working` — reviewed the diffs only — lines this change added or modified. 1 file(s) reviewed, 0 not reviewed.

- [ ] `path/to/file.rs:10` `perf/unattributed` — What's wrong and suggested fix.
- [ ] `path/to/file.rs:42` `dead-code/no-unused` — What's wrong. Why it matters. Suggested fix.
- [ ] `path/to/file.rs:88` `style/unattributed` — Minor issue.
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
        let report = synthesize_working(
            verified,
            &FleetTally::default(),
            &[],
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

        let report = synthesize_working(vec![], &FleetTally::default(), &[], &[], &tools, NOW);

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

        let report = synthesize_working(vec![], &FleetTally::default(), &[], &[], &tools, NOW);

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
                "pair-check",
                &["function-length", "missing-docs"],
                "tool missing: no linter",
            )],
        );

        let report = synthesize_working(vec![], &FleetTally::default(), &[], &[], &tools, NOW);

        assert!(
            report
                .markdown
                .contains("prompt rules 'function-length', 'missing-docs' ran instead"),
            "the note must name every superseded prompt rule: {}",
            report.markdown
        );
    }

    #[test]
    fn an_inert_tool_report_keeps_the_nothing_in_scope_marker() {
        let report = synthesize_working(
            vec![],
            &FleetTally::default(),
            &[],
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
