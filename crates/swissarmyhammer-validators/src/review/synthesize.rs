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

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use model_embedding::TextEmbedder;
use rusqlite::Connection;

use crate::error::AvpError;
use crate::review::fleet::{run_fleet, FleetConfig, FleetOutcome};
use crate::review::scope::{batch_work_list, scope_review, Scope, WorkList};
use crate::review::types::{Finding, VerifiedFinding};
use crate::review::verify::{verify_findings, Candidate};
use crate::validators::{AgentPool, ValidatorLoader};

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
    pub attempted: usize,
    /// How many fan-out tasks failed (and degraded to zero findings).
    pub failed: usize,
}

impl FleetTally {
    /// A tally of `attempted` tasks of which `failed` failed.
    pub fn new(attempted: usize, failed: usize) -> Self {
        Self { attempted, failed }
    }
}

impl From<&FleetOutcome> for FleetTally {
    fn from(outcome: &FleetOutcome) -> Self {
        Self {
            attempted: outcome.attempted,
            failed: outcome.failed,
        }
    }
}

/// The per-verdict tallies a [`ReviewReport`] carries.
///
/// Review is binary pass/fail — there is no graded severity — so the rendered
/// failures are a single `findings` count, not a per-tier breakdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReviewCounts {
    /// Confirmed findings rendered into the checklist (post-dedup).
    pub findings: usize,
    /// Findings the verifier confirmed (across every input, pre-dedup).
    pub confirmed: usize,
    /// Findings the verifier refuted (across every input).
    pub refuted: usize,
    /// How many fan-out tasks were attempted (see [`FleetTally`]).
    pub tasks_attempted: usize,
    /// How many fan-out tasks failed and degraded to zero findings. A non-zero
    /// value means the rendered findings are INCOMPLETE.
    pub tasks_failed: usize,
}

/// The synthesized review report: the rendered markdown plus its tallies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewReport {
    /// The dated GFM `## Review Findings (...)` section, ready to append to a
    /// kanban task's description verbatim.
    pub markdown: String,
    /// The per-verdict counts for the tool/skill summary.
    pub counts: ReviewCounts,
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
/// kept no findings — the resolved scope was empty — the report states
/// "Nothing in scope to review." so an empty scope cannot be mistaken for a
/// clean review either.
///
/// `verified` is any iterable of [`VerifiedFinding`]s (a `Vec` being the common
/// caller) — it is collected once up front so a caller need not materialize a
/// `Vec` just to hand it over.
pub fn synthesize(
    verified: impl IntoIterator<Item = VerifiedFinding>,
    tally: &FleetTally,
    now: &str,
) -> ReviewReport {
    let verified = verified.into_iter().collect::<Vec<_>>();
    let counts_confirmed = verified.iter().filter(|v| v.confirmed).count();
    let counts_refuted = verified.len() - counts_confirmed;

    // Keep only confirmed findings, then collapse exact repeats.
    let kept = dedup_exact(verified.into_iter().filter(|v| v.confirmed));

    let mut counts = ReviewCounts {
        confirmed: counts_confirmed,
        refuted: counts_refuted,
        tasks_attempted: tally.attempted,
        tasks_failed: tally.failed,
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

    // Say so explicitly when the resolved scope was empty (zero fan-out tasks):
    // a bare findings header would read identically to a genuinely clean review.
    if tally.attempted == 0 && kept.is_empty() {
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
        "review synthesis complete"
    );

    ReviewReport { markdown, counts }
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
/// 2. [`batch_work_list`] — split the work-list into content-budgeted batches at
///    whole-file granularity ([`FleetConfig::batch_size`]) so no single shared
///    prime overflows the model's context. A small diff is one batch; a large one
///    is several. A single file larger than `batch_size` is a hard error here.
/// 3. For **each batch**, independently: [`run_fleet`] fans every validator out
///    across the shared `pool` over that batch's files (its own shared prime,
///    forked per validator), then [`verify_findings`] pairs each candidate back
///    with its file's ground-truth context ([`build_candidates`]) and runs the
///    adversarial refute pass on the **same** `pool` — forking that batch's prime
///    while it stays pinned, then releasing the pin once the batch has drained.
/// 4. [`synthesize`] — merge every batch's confirmed [`VerifiedFinding`]s and
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
/// # Errors
///
/// Returns the [`AvpError`] from [`scope_review`] on git or index failure, or when
/// a matched validator declares an unknown probe, or from [`batch_work_list`] when
/// a single file's inlined source exceeds `batch_size`. Fan-out and verify failures
/// never error: a failed task degrades to zero findings (fan-out) or a
/// refute-by-default verdict (verify), so the report is always produced.
#[allow(clippy::too_many_arguments)]
pub async fn run_review(
    scope: Scope,
    repo_path: &Path,
    loader: &ValidatorLoader,
    conn: &Connection,
    embedder: &dyn TextEmbedder,
    pool: &AgentPool,
    fleet_config: FleetConfig,
    now: &str,
) -> Result<ReviewReport, AvpError> {
    // Stage 1: scope → work-list (deterministic, LLM-free).
    let work = scope_review(scope, repo_path, loader, conn, embedder).await?;

    // Stage 2: split the work-list into content-budgeted batches (whole-file
    // granularity). A single file over `batch_size` is a hard error here.
    let batches = batch_work_list(&work, fleet_config.batch_size)?;

    tracing::info!(
        validators = work.validators.len(),
        files = work.distinct_files().count(),
        batches = batches.len(),
        batch_size = fleet_config.batch_size,
        "review run: scoped work-list ready, batched, fanning out"
    );

    // Stage 3: run the full fan-out → verify pipeline independently per batch,
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
        let fleet = run_fleet(batch, loader, pool).await;
        attempted += fleet.attempted;
        failed += fleet.failed;
        let FleetOutcome {
            findings: fleet_findings,
            prime,
            ..
        } = fleet;

        // Verify this batch on the SAME pool — each verify task FORKS the batch's
        // shared prime while it stays pinned. Awaiting drains every verify task.
        let candidates = build_candidates(batch, fleet_findings);
        let prime_session = prime.as_ref().map(|g| g.session_id());
        let outcome = verify_findings(candidates, pool, prime_session).await;

        // The batch (fan-out AND verify) has drained: release its prime pin so the
        // pinned cache entry does not outlive the batch. A run future dropped
        // before this point releases it from the guard's `Drop` instead.
        if let Some(guard) = prime {
            crate::review::fleet::unpin_prefix_session(guard).await;
        }

        verified.extend(outcome.verified);
    }

    // Stage 4: synthesize the merged, deduped, ordered, dated report. The summed
    // tally rides into the report so the tool boundary can flag/fail an incomplete
    // run; the engine itself stays a pure data barrier and never errors on it.
    let report = synthesize(verified, &FleetTally::new(attempted, failed), now);

    Ok(report)
}

/// Pair each fan-out [`Finding`] back with the ground-truth context its file
/// carries in the [`WorkList`], producing the [`Candidate`]s the verify stage
/// checks.
///
/// A finding is tagged with its `validator` and the `file` it is about; the
/// matching [`ValidatorWork`](crate::review::ValidatorWork) /
/// [`FileWork`](crate::review::FileWork) in `work` holds that file's
/// `source_slice` and shared `probe_results`. This reuses the stage-1 data
/// verbatim — it never re-derives a slice or re-runs a probe. A finding whose
/// `(validator, file)` is not in the work-list (an agent inventing a path) yields
/// empty context rather than being dropped, so it still reaches the verifier and
/// refutes by default there.
fn build_candidates(work: &WorkList, findings: Vec<Finding>) -> Vec<Candidate> {
    findings
        .into_iter()
        .map(|finding| {
            let context = work
                .validators
                .iter()
                .find(|v| v.validator_name == finding.validator)
                .and_then(|v| v.files.iter().find(|f| f.path == finding.file));
            let (source_slice, probe_results) = match context {
                Some(file) => (file.source_slice.clone(), file.probe_results.clone()),
                None => (String::new(), Vec::new()),
            };
            Candidate {
                finding,
                source_slice,
                probe_results,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::scope::{FileWork, ValidatorWork};
    use crate::review::types::RefutingLayer;

    /// The fixture timestamp passed as `now` to every `synthesize` call. Kept
    /// inline (not interpolated) inside the byte-for-byte snapshot strings so
    /// those stay readable.
    const NOW: &str = "2026-04-11 13:08";

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
        let report = synthesize(vec![], &FleetTally::new(60, 60), NOW);

        assert_eq!(report.counts.tasks_attempted, 60);
        assert_eq!(report.counts.tasks_failed, 60);
        assert!(
            report.markdown.contains("60/60 review tasks failed"),
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
        let report = synthesize(vec![], &FleetTally::new(8, 0), NOW);

        assert_eq!(report.markdown, "## Review Findings (2026-04-11 13:08)\n");
        assert_eq!(report.counts.tasks_attempted, 8);
        assert_eq!(report.counts.tasks_failed, 0);
    }

    #[test]
    fn renders_dated_header_with_the_input_timestamp_verbatim() {
        let report = synthesize(vec![], &FleetTally::default(), NOW);
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
        let report = synthesize(vec![], &FleetTally::default(), NOW);
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
    fn an_attempted_clean_run_carries_no_nothing_in_scope_marker() {
        // Tasks ran and found nothing — that is a clean review, not an empty
        // scope, so the marker must not appear.
        let report = synthesize(vec![], &FleetTally::new(8, 0), NOW);
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

        let _report = synthesize(verified, &FleetTally::default(), NOW);

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

        let report = synthesize(verified, &FleetTally::default(), NOW);

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
        let report = synthesize(vec![one.clone(), one], &FleetTally::default(), NOW);

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
        let report = synthesize(vec![dup, dead], &FleetTally::default(), NOW);

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
        let report = synthesize(verified, &FleetTally::default(), NOW);

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
        let report = synthesize(verified, &FleetTally::default(), NOW);

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
        let report = synthesize(verified, &FleetTally::default(), NOW);

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
        let report = synthesize(verified, &FleetTally::default(), NOW);

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
        FileWork {
            path: path.to_string(),
            semantic_diff: vec![],
            changed_symbols: vec![],
            source_slice: format!("// slice for {path}"),
            probe_results: vec![],
        }
    }

    /// A `ValidatorWork` carrying the given files for one validator.
    fn validator_work(name: &str, files: Vec<FileWork>) -> ValidatorWork {
        ValidatorWork {
            validator_name: name.to_string(),
            rules: vec![],
            probes: vec![],
            files,
        }
    }

    #[test]
    fn build_candidates_pairs_each_finding_with_its_files_context() {
        let work = WorkList {
            change_purpose: "p".to_string(),
            validators: vec![validator_work("dedup", vec![file_work("src/a.rs")])],
        };
        let candidates = build_candidates(&work, vec![finding("src/a.rs", 42, "dedup", "dup")]);

        assert_eq!(candidates.len(), 1);
        // The candidate reuses the work-list's bounded slice verbatim.
        assert_eq!(candidates[0].source_slice, "// slice for src/a.rs");
    }

    #[test]
    fn build_candidates_resolves_each_finding_to_its_own_validators_context() {
        // Two validators flag the SAME file:line — each candidate must pick up its
        // own validator's file context, not the other's.
        let work = WorkList {
            change_purpose: "p".to_string(),
            validators: vec![
                validator_work("dead-code", vec![file_work("src/a.rs")]),
                validator_work("duplication", vec![file_work("src/a.rs")]),
            ],
        };
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
        let work = WorkList {
            change_purpose: "p".to_string(),
            validators: vec![validator_work("dedup", vec![file_work("src/a.rs")])],
        };
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
}
