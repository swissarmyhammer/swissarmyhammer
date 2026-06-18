//! Engine stage 4 — synthesize: dedup, severity-rank, render the dated checklist.
//!
//! This is the final, deterministic, LLM-free stage and the pipeline's single
//! barrier. [`run_review`] drives stages 1–3 to completion — fan-out and verify
//! both drain the shared [`AgentPool`](crate::validators::AgentPool) by awaiting
//! every task they submit — then hands the resulting `Vec<`[`VerifiedFinding`]`>`
//! to [`synthesize`], which turns it into the deduped, severity-ranked
//! [`ReviewReport`].
//!
//! # What synthesis does
//!
//! [`synthesize`] is pure and clock-free: the timestamp is an **input**, never
//! read inside the engine, so the same findings always render the same report.
//! It:
//!
//! 1. **Counts** confirmed vs refuted across every input finding.
//! 2. **Drops refuted** findings ([`VerifiedFinding::confirmed`] is `false`).
//! 3. **Dedups conservatively** — it collapses only *exact repeats*
//!    (same `file`, `line`, `validator`, `rule`, and byte-identical `claim`).
//!    There is no fuzzy/similarity matching, and findings from *different*
//!    validators on the same `file:line` are distinct lenses, never merged.
//! 4. **Groups by severity** into Blockers / Warnings / Nits, ordering each
//!    section by `file:line` so co-located concerns render together (grouping is
//!    not merging — every surviving concern is its own checklist item).
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
use crate::review::scope::{scope_review, Scope, WorkList};
use crate::review::types::{Finding, Severity, VerifiedFinding};
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

/// The per-severity and per-verdict tallies a [`ReviewReport`] carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReviewCounts {
    /// Confirmed blocker findings rendered under `### Blockers`.
    pub blockers: usize,
    /// Confirmed warning findings rendered under `### Warnings`.
    pub warnings: usize,
    /// Confirmed nit findings rendered under `### Nits`.
    pub nits: usize,
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
    /// The per-severity / per-verdict counts for the tool/skill summary.
    pub counts: ReviewCounts,
}

/// Synthesize verified findings into the dated, deduped, severity-ranked report.
///
/// Pure and deterministic: `now` is the already-formatted local timestamp the
/// caller read from the clock (`YYYY-MM-DD HH:MM`), rendered verbatim into the
/// section header so the engine itself never reads time. See the module docs for
/// the full drop/dedup/group/render contract.
///
/// `tally` is the fan-out task outcome from [`run_fleet`]. When any task failed,
/// a clearly visible warning line is rendered directly under the dated header so
/// an incomplete run cannot be mistaken for a clean diff, and the tally is
/// carried through into [`ReviewCounts`]. When the run attempted zero tasks and
/// kept no findings — the resolved scope was empty — the report states
/// "Nothing in scope to review." so an empty scope cannot be mistaken for a
/// clean review either.
pub fn synthesize(verified: Vec<VerifiedFinding>, tally: &FleetTally, now: &str) -> ReviewReport {
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

    for (severity, heading) in SECTIONS {
        let mut section: Vec<&VerifiedFinding> = kept
            .iter()
            .filter(|v| v.finding.severity == *severity)
            .collect();
        if section.is_empty() {
            continue;
        }
        // Group by `file:line` so co-located findings render together; stable so
        // exact-input order is otherwise preserved.
        section.sort_by(|a, b| {
            (a.finding.file.as_str(), a.finding.line)
                .cmp(&(b.finding.file.as_str(), b.finding.line))
        });

        match severity {
            Severity::Blocker => counts.blockers = section.len(),
            Severity::Warning => counts.warnings = section.len(),
            Severity::Nit => counts.nits = section.len(),
        }

        let _ = write!(markdown, "\n### {heading}\n");
        for verified in section {
            let _ = writeln!(markdown, "{}", render_item(&verified.finding));
        }
    }

    tracing::info!(
        blockers = counts.blockers,
        warnings = counts.warnings,
        nits = counts.nits,
        confirmed = counts.confirmed,
        refuted = counts.refuted,
        tasks_attempted = counts.tasks_attempted,
        tasks_failed = counts.tasks_failed,
        "review synthesis complete"
    );

    ReviewReport { markdown, counts }
}

/// The severity sections in render order, each paired with its GFM heading.
const SECTIONS: &[(Severity, &str)] = &[
    (Severity::Blocker, "Blockers"),
    (Severity::Warning, "Warnings"),
    (Severity::Nit, "Nits"),
];

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
/// 2. [`run_fleet`] — fan every `(validator, file)` out across the shared `pool`
///    and collect the candidate [`Finding`]s. Awaiting it drains every fan-out
///    task.
/// 3. [`verify_findings`] — pair each candidate back with its file's ground-truth
///    context ([`build_candidates`]) and submit it to the **same** `pool` for the
///    adversarial refute pass. Awaiting it drains every verify task.
/// 4. [`synthesize`] — turn the surviving [`VerifiedFinding`]s into the dated,
///    deduped, severity-ranked [`ReviewReport`].
///
/// Because steps 2 and 3 each await all the tasks they submit before returning,
/// the moment [`verify_findings`] resolves the shared pool has fully drained —
/// all fan-out *and* all verify work is done — so synthesis is the natural
/// barrier and needs no separate pool-join. The engine never reads the clock:
/// `now` is the caller-supplied, already-formatted local timestamp
/// (`YYYY-MM-DD HH:MM`) rendered verbatim into the report header.
///
/// # Errors
///
/// Returns the [`AvpError`] from [`scope_review`] on git or index failure, or
/// when a matched validator declares an unknown probe. Fan-out and verify
/// failures never error: a failed task degrades to zero findings (fan-out) or a
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
    use_tracking: bool,
) -> Result<ReviewReport, AvpError> {
    // Only the working scope participates in incremental tracking; sha/file/glob
    // are explicit, one-shot targets whose files must never seed the baseline.
    let is_working = matches!(scope, Scope::Working);

    // Stage 1: scope → work-list (deterministic). `use_tracking` narrows a
    // working scope to files edited since their last review (the `/finish`
    // fix-loop win); it is inert for every other scope.
    let work = scope_review(scope, repo_path, loader, conn, embedder, use_tracking).await?;

    let total_files: usize = work.validators.iter().map(|v| v.files.len()).sum();
    tracing::info!(
        validators = work.validators.len(),
        files = total_files,
        "review run: scoped work-list ready, fanning out"
    );

    // Stage 2: fan out across the shared pool; awaiting drains every fan-out task.
    // The fleet outcome carries the task tally (attempted/failed) alongside the
    // findings, plus the run's shared primed-prefix pin guard (the change + all
    // diffs, primed once) so the verify stage can reuse the same prime.
    let fleet = run_fleet(&work, loader, pool, fleet_config).await;
    let tally = FleetTally::from(&fleet);
    // Destructure so verify can fork the prime while it is still pinned, then the
    // guard is released once verify has drained (below).
    let FleetOutcome {
        findings: fleet_findings,
        prime,
        ..
    } = fleet;

    // Stage 3: pair each candidate with its file's ground-truth context, then
    // verify on the SAME pool — each verify task FORKS the run's shared prime
    // (reusing the cached change + diffs) while it stays pinned. Awaiting drains
    // every verify task. Once this returns, the shared pool has fully drained —
    // the single barrier.
    let candidates = build_candidates(&work, fleet_findings);
    let prime_session = prime.as_ref().map(|g| g.session_id());
    let outcome = verify_findings(candidates, pool, prime_session).await;

    // The whole run (fan-out AND verify) has drained: release the shared prime's
    // pin so the pinned cache entry does not outlive the run. A run future
    // dropped before this point releases it from the guard's `Drop` instead.
    if let Some(guard) = prime {
        crate::review::fleet::unpin_prefix_session(guard).await;
    }

    // Stage 4: synthesize the deduped, severity-ranked, dated report. The tally
    // rides into the report so the tool boundary can flag/fail an incomplete run;
    // the engine itself stays a pure data barrier and never errors on it.
    let report = synthesize(outcome.verified, &tally, now);

    // Record the incremental-tracking baseline through the single shared helper
    // every pipeline driver reaches: for a working-scope pass that actually ran,
    // it stamps a fresh `.validators/.hashes/` entry per reviewed file so the next
    // `review working` subtracts it unless its content (or the rules) changed. The
    // helper owns the gate and the best-effort error handling, so there is exactly
    // one recording site for both the pure and the agent-driven drivers.
    crate::review::tracking::record_baseline_if_working(
        is_working,
        repo_path,
        loader,
        &work,
        &tally,
        &crate::review::tracking::now_rfc3339(),
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
        severity: Severity,
        claim: &str,
        suggestion: Option<&str>,
    ) -> VerifiedFinding {
        VerifiedFinding {
            finding: Finding {
                file: file.to_string(),
                line,
                validator: validator.to_string(),
                rule: rule.map(String::from),
                severity,
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
                severity: Severity::Blocker,
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
    fn synthesize_logs_the_final_severity_and_verdict_counts() {
        let verified = vec![
            confirmed(
                "src/a.rs",
                42,
                "dead-code",
                Some("no-unused"),
                Severity::Blocker,
                "`foo` is never called",
                Some("Delete it"),
            ),
            refuted("src/a.rs", 99, "dead-code", "`bar` is never called"),
        ];

        let _report = synthesize(verified, &FleetTally::default(), NOW);

        // The synthesis summary reports the per-severity + per-verdict tallies.
        assert!(logs_contain("review synthesis complete"));
        assert!(logs_contain("blockers=1"));
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
                Severity::Blocker,
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
        // Counts reflect both verdicts; only the confirmed blocker is rendered.
        assert_eq!(report.counts.confirmed, 1);
        assert_eq!(report.counts.refuted, 1);
        assert_eq!(report.counts.blockers, 1);
    }

    #[test]
    fn collapses_exact_repeats_into_one_item() {
        // Two byte-identical findings (same file, line, validator, rule, claim).
        let one = confirmed(
            "src/a.rs",
            42,
            "dead-code",
            Some("no-unused"),
            Severity::Blocker,
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
        assert_eq!(report.counts.blockers, 1);
        // Both were confirmed, so the confirmed count is the pre-dedup total.
        assert_eq!(report.counts.confirmed, 2);
    }

    #[test]
    fn keeps_two_validators_on_the_same_file_line_and_groups_them() {
        // duplication and dead-code both flag src/a.rs:42 — distinct lenses, both
        // kept, and rendered adjacently because they share a file:line.
        let dup = confirmed(
            "src/a.rs",
            42,
            "duplication",
            Some("no-copy-paste"),
            Severity::Blocker,
            "Duplicated block also lives in b.rs",
            Some("Extract a shared helper"),
        );
        let dead = confirmed(
            "src/a.rs",
            42,
            "dead-code",
            Some("no-unused"),
            Severity::Blocker,
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
        assert_eq!(report.counts.blockers, 2);

        // They render adjacently under the same severity (grouped by file:line).
        let both = report.markdown.matches("`src/a.rs:42`").count();
        assert_eq!(
            both, 2,
            "both co-located findings are kept: {}",
            report.markdown
        );
    }

    #[test]
    fn groups_by_severity_and_omits_empty_sections() {
        let verified = vec![
            confirmed(
                "src/a.rs",
                10,
                "dead-code",
                None,
                Severity::Blocker,
                "Blocker concern",
                None,
            ),
            confirmed(
                "src/b.rs",
                20,
                "style",
                None,
                Severity::Nit,
                "Nit concern",
                None,
            ),
        ];
        let report = synthesize(verified, &FleetTally::default(), NOW);

        assert!(
            report.markdown.contains("### Blockers"),
            "{}",
            report.markdown
        );
        assert!(report.markdown.contains("### Nits"), "{}", report.markdown);
        // No warnings → the section is omitted entirely.
        assert!(
            !report.markdown.contains("### Warnings"),
            "empty severity sections are omitted: {}",
            report.markdown
        );
        assert_eq!(report.counts.blockers, 1);
        assert_eq!(report.counts.warnings, 0);
        assert_eq!(report.counts.nits, 1);
    }

    #[test]
    fn renders_the_exact_skill_section_format() {
        // A full snapshot against the documented `builtin/skills/review/SKILL.md`
        // step-8 layout: header, severity subsections, one `- [ ]` item each.
        let verified = vec![
            confirmed(
                "path/to/file.rs",
                42,
                "dead-code",
                Some("no-unused"),
                Severity::Blocker,
                "What's wrong. Why it matters",
                Some("Suggested fix"),
            ),
            confirmed(
                "path/to/file.rs",
                10,
                "perf",
                None,
                Severity::Warning,
                "What's wrong and suggested fix",
                None,
            ),
            confirmed(
                "path/to/file.rs",
                88,
                "style",
                None,
                Severity::Nit,
                "Minor issue",
                None,
            ),
        ];
        let report = synthesize(verified, &FleetTally::default(), NOW);

        let expected = "\
## Review Findings (2026-04-11 13:08)

### Blockers
- [ ] `path/to/file.rs:42` — What's wrong. Why it matters. Suggested fix.

### Warnings
- [ ] `path/to/file.rs:10` — What's wrong and suggested fix.

### Nits
- [ ] `path/to/file.rs:88` — Minor issue.
";
        assert_eq!(report.markdown, expected);
    }

    #[test]
    fn orders_findings_by_file_line_within_a_severity() {
        // Submitted out of order; rendered grouped/ordered by file:line.
        let verified = vec![
            confirmed(
                "src/z.rs",
                5,
                "v",
                None,
                Severity::Warning,
                "z concern",
                None,
            ),
            confirmed(
                "src/a.rs",
                90,
                "v",
                None,
                Severity::Warning,
                "a90 concern",
                None,
            ),
            confirmed(
                "src/a.rs",
                9,
                "v",
                None,
                Severity::Warning,
                "a9 concern",
                None,
            ),
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
            severity: Severity::Warning,
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
            inlined_full: true,
            probe_results: vec![],
        }
    }

    /// A `ValidatorWork` carrying the given files for one validator.
    fn validator_work(name: &str, files: Vec<FileWork>) -> ValidatorWork {
        ValidatorWork {
            validator_name: name.to_string(),
            severity: crate::validators::Severity::Warn,
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
