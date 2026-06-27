//! `check` — the composed inner-loop / CI verb: doctor (static) + observe +
//! evaluate + compare-to-golden.
//!
//! Per `ideas/expect.md` §"`check` decomposes into three separable verbs" and
//! §"Two different things are being checked", `check` is `doctor` plus execution.
//! For every spec in a `<scope>` it:
//!
//! 1. **Runs the doctor pass first** ([`diagnose`]) and *refuses to run a
//!    malformed spec* — a CI failure is never ambiguous between "bad spec" and
//!    "bad code". A spec with any error finding becomes a [`CheckStatus::Malformed`]
//!    entry and is **never observed**.
//! 2. For a well-formed spec, **observes** the running system (via the injected
//!    `observe` seam — kept abstract so this composition is decoupled from the
//!    surface-specific driver and is deterministically testable), **evaluates**
//!    the received observation against the criteria ([`evaluate_spec`] — "does the
//!    code meet the spec?"), and **compares** it to the approved golden
//!    ([`ledger_state`]/[`compare_tiered`] — "did the verdict drift?").
//! 3. **Derives** a per-expectation [`CheckStatus`] and a teaching message that
//!    routes a failure to either *the program is wrong* ([`CheckStatus::Failed`])
//!    or *fix the spec / the criterion is uncheckable* ([`CheckStatus::Malformed`]).
//!
//! The whole report rolls up to a single [`CheckReport::exit_code`] the CLI maps
//! to a process exit: a bare `expect expectations check` exits non-zero on a
//! malformed spec, an unmet expectation, or an unapproved drift; a `new`
//! expectation (no golden) fails in CI.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::ExpectConfig;
use crate::doctor::{diagnose, DiagnosticStatus, DoctorFacts, FieldDiagnostic};
use crate::error::ExpectError;
use crate::evaluate::evaluate_repeated;
use crate::ledger::{
    compare_tiered, ledger_state, read_golden, GradingSeam, LedgerComparison, ScrubberSet,
};
use crate::loader::{ExpectationLoader, RawSpec};
use crate::spec::{Expectation, EXPECT_EXTENSION};
use crate::types::{ExpectationVerdict, LedgerState, Observation};

/// Exit code when every checked expectation passed — the green CI result.
pub const CHECK_EXIT_OK: i32 = 0;

/// Exit code when the *code* is wrong: an unmet expectation, an unapproved drift,
/// an observe failure, or a `new` expectation under CI. Distinct from
/// [`CHECK_EXIT_MALFORMED`] so a CI failure is never ambiguous between bad code
/// and a bad spec.
pub const CHECK_EXIT_FAILED: i32 = 1;

/// Exit code when a *spec* is malformed and the doctor gate refused to run it.
/// Held distinct from [`CHECK_EXIT_FAILED`] so "fix the spec" never reads as "fix
/// the code".
pub const CHECK_EXIT_MALFORMED: i32 = 2;

/// Teaching message for a [`CheckStatus::Passed`] expectation.
const PASSED_MESSAGE: &str = "the code meets this expectation and matches its approved golden";

/// Teaching message for a [`CheckStatus::Failed`] expectation — routes the failure
/// to "the program is wrong".
const FAILED_MESSAGE: &str = "the code does not meet this expectation — the program is wrong";

/// Teaching message for a [`CheckStatus::Drifted`] expectation.
const DRIFTED_MESSAGE: &str =
    "the run drifted from the approved golden — review the diff and approve, or fix the code";

/// Teaching message for a [`CheckStatus::Stale`] expectation.
const STALE_MESSAGE: &str =
    "the spec was edited since its golden was approved — re-approve the baseline";

/// Teaching message for a [`CheckStatus::New`] expectation.
const NEW_MESSAGE: &str = "no approved golden yet — observe and approve to baseline (fails in CI)";

/// Teaching message base for a malformed spec — routes the failure to "fix the
/// spec", not "fix the code".
const MALFORMED_MESSAGE: &str =
    "the spec is malformed and was not run — fix the spec, not the code";

/// Teaching message base for a malformed spec whose fault is an uncheckable
/// criterion — routes the failure to "sharpen the criterion".
const UNCHECKABLE_MESSAGE: &str =
    "the spec has uncheckable criteria and was not run — sharpen them (see the suggestions)";

/// Teaching message for a [`CheckStatus::Errored`] expectation.
const OBSERVE_FAILED_MESSAGE: &str = "could not observe the system under test";

/// The doctor `field` an uncheckable-criterion finding is reported under, used to
/// route a malformed entry's message to the "sharpen the criterion" teaching.
const CRITERIA_FIELD: &str = "criteria";

/// The per-expectation outcome of a `check`: how the spec and the code it
/// describes fared, from the doctor gate through the golden compare.
///
/// The serialized lowercase forms (`"passed"`, `"failed"`, `"drifted"`, …) are
/// deliberately distinct from [`LedgerState`]'s, so a check status and a raw
/// ledger state never collide on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    /// The code meets the spec and matches its approved golden.
    Passed,
    /// The code does not meet the spec — an unmet expectation (the program is
    /// wrong).
    Failed,
    /// The run diverged from the approved golden — unapproved drift awaiting a
    /// human.
    Drifted,
    /// The spec was edited since its golden was approved — the baseline is out of
    /// date.
    Stale,
    /// No golden yet — a candidate baseline. Fails only under CI.
    New,
    /// The doctor gate refused to run a malformed spec — never observed.
    Malformed,
    /// The system under test could not be observed (provision/drive/teardown
    /// failed).
    Errored,
}

impl CheckStatus {
    /// The exit code this status contributes, given whether the run is `ci` and
    /// whether the repo opts into `ci_autoapprove`.
    ///
    /// The single source of truth for the status → exit mapping; the report's exit
    /// code is the worst (numerically largest) over all entries.
    pub fn exit_code(self, ci: bool, ci_autoapprove: bool) -> i32 {
        match self {
            CheckStatus::Passed => CHECK_EXIT_OK,
            CheckStatus::Malformed => CHECK_EXIT_MALFORMED,
            CheckStatus::Failed | CheckStatus::Errored => CHECK_EXIT_FAILED,
            // An unapproved drift fails — suppressed only when CI is explicitly
            // configured to auto-approve drift.
            CheckStatus::Drifted | CheckStatus::Stale => {
                if ci && ci_autoapprove {
                    CHECK_EXIT_OK
                } else {
                    CHECK_EXIT_FAILED
                }
            }
            // A brand-new expectation is fine locally (observe → approve), but a
            // green baseline is never minted in CI.
            CheckStatus::New => {
                if ci {
                    CHECK_EXIT_FAILED
                } else {
                    CHECK_EXIT_OK
                }
            }
        }
    }
}

/// The cross-cutting policy a [`check`] pass reads: the doctor facts, the repo
/// config, the compare scrubbers, and whether this is a CI run.
///
/// A borrow-bundle (not a serde DTO) that keeps the [`check`] signature tidy while
/// every knob stays explicit and injected — nothing is read from the ambient
/// environment, so the policy is deterministic to test.
pub struct CheckOptions<'a> {
    /// The live facts the doctor gate validates dynamic fields against.
    pub facts: &'a DoctorFacts,
    /// The repo config; its `[approval]` section gates the CI drift behavior.
    pub config: &'a ExpectConfig,
    /// The scrubbers the golden compare normalizes volatile content with.
    pub scrubbers: &'a ScrubberSet,
    /// The Tier-2 embedder + Tier-3 grader panel the tiered golden compare threads
    /// through [`compare_tiered`], so a tiered golden is graded with the pinned
    /// grading seam rather than the embedder-free Tier-1 path.
    pub seam: &'a GradingSeam<'a>,
    /// Whether this is a CI run (`new` fails, drift never auto-approves unless
    /// configured).
    pub ci: bool,
}

/// One expectation's `check` result: its identity, derived [`CheckStatus`], a
/// teaching message, the doctor diagnostics (the static half), and — when the spec
/// was actually run — the meet verdict and the golden drift comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckEntry {
    /// The spec's repo-relative identity.
    pub path: String,
    /// The derived per-expectation outcome.
    pub status: CheckStatus,
    /// A one-line teaching message routing the outcome to its remedy.
    pub message: String,
    /// The doctor pass's per-field findings — the static half of `check`.
    pub diagnostics: Vec<FieldDiagnostic>,
    /// The meet verdict ([`evaluate_spec`] over the received run), when the spec
    /// was run (absent for a malformed or un-observable spec).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict: Option<ExpectationVerdict>,
    /// The old-vs-new golden comparison, present only for a [`CheckStatus::Drifted`]
    /// entry (the one outcome a reviewer must triage).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison: Option<LedgerComparison>,
}

/// The full `check` report: one [`CheckEntry`] per in-scope spec plus the
/// rolled-up [`exit_code`](CheckReport::exit_code) the CLI maps to a process exit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckReport {
    /// One entry per in-scope expectation, in discovery (sorted-identity) order.
    pub entries: Vec<CheckEntry>,
    /// The worst entry exit code, the value the CLI exits the process with.
    pub exit_code: i32,
}

/// Check every expectation in `scope` (and optional `tag`): doctor → observe →
/// evaluate → compare, deriving a per-expectation [`CheckStatus`].
///
/// Specs are discovered raw ([`ExpectationLoader::discover_raw`]) so a malformed
/// spec is *surfaced* rather than aborting the walk: each spec runs through the
/// doctor gate first, and one with any error finding becomes a
/// [`CheckStatus::Malformed`] entry whose spec is **never** handed to `observe`.
/// A well-formed spec is parsed, optionally tag-filtered, observed, evaluated, and
/// compared to its golden.
///
/// `observe` is the injected driver seam: `FnMut(&Expectation) -> Result<Vec<Observation>, _>`.
/// It returns **one observation per `pass^k` run** (the closure owns the adapter,
/// so it decides the repeat count and re-arranges the `Given` per run via
/// [`observe_repeated`](crate::observe::observe_repeated)); the composition stays
/// agnostic to the count and grades the runs with
/// [`evaluate_repeated`]. The pure composition never drives a system itself — the
/// tool layer supplies a closure that runs the real surface adapter and persists
/// the received run, while tests pass a deterministic stub. An `observe` error
/// becomes a [`CheckStatus::Errored`] entry rather than aborting the batch.
///
/// The `tag` filter narrows only *parseable* specs (tags live in the frontmatter):
/// a malformed spec's tags are unknowable, so the doctor gate is **scope-wide** and
/// reports a malformed spec even under a `tag` scope. This is deliberately
/// fail-safe — a broken spec is never silently dropped from a scoped CI run.
///
/// # Errors
///
/// Returns [`ExpectError`] only for a failure that aborts the whole pass: spec
/// discovery, or reading a golden from disk. Per-spec failures (a malformed spec,
/// a failed observe) are reported as entries, not errors.
pub fn check<O>(
    repo_root: &Path,
    scope: Option<&str>,
    tag: Option<&str>,
    options: &CheckOptions<'_>,
    mut observe: O,
) -> Result<CheckReport, ExpectError>
where
    O: FnMut(&Expectation) -> Result<Vec<Observation>, ExpectError>,
{
    let loader = ExpectationLoader::new(repo_root);
    let raw_specs = loader.discover_raw(scope)?;

    let mut entries = Vec::with_capacity(raw_specs.len());
    for raw in &raw_specs {
        if let Some(entry) = check_one(repo_root, raw, tag, options, &mut observe)? {
            entries.push(entry);
        }
    }

    let exit_code = entries
        .iter()
        .map(|entry| {
            entry
                .status
                .exit_code(options.ci, options.config.approval.ci_autoapprove)
        })
        .max()
        .unwrap_or(CHECK_EXIT_OK);

    Ok(CheckReport { entries, exit_code })
}

/// Check one raw spec, returning its [`CheckEntry`] — or `None` when a `tag`
/// filter excludes it from scope.
///
/// The doctor gate runs first: an error finding short-circuits to a
/// [`CheckStatus::Malformed`] entry before any parse or observe.
fn check_one<O>(
    repo_root: &Path,
    raw: &RawSpec,
    tag: Option<&str>,
    options: &CheckOptions<'_>,
    observe: &mut O,
) -> Result<Option<CheckEntry>, ExpectError>
where
    O: FnMut(&Expectation) -> Result<Vec<Observation>, ExpectError>,
{
    let diagnostics = diagnose(&raw.content, options.facts);
    if has_error(&diagnostics) {
        return Ok(Some(malformed_entry(&raw.path, diagnostics)));
    }

    // A doctor-clean spec parses (the gate's error findings are a superset of the
    // parser's rejections); a parse failure here is exceptional and routes to the
    // same "fix the spec" remedy as any other malformed spec.
    let file_path = repo_root.join(format!("{}{EXPECT_EXTENSION}", raw.path));
    let spec = match Expectation::parse(&raw.content, &file_path, repo_root) {
        Ok(spec) => spec,
        Err(err) => return Ok(Some(parse_failure_entry(&raw.path, diagnostics, &err))),
    };

    // A tag scope narrows to specs carrying the tag; an out-of-scope spec is
    // dropped, not reported.
    if let Some(tag) = tag {
        if !spec.frontmatter.tags.iter().any(|t| t == tag) {
            return Ok(None);
        }
    }

    // One observation per `pass^k` run; the verdict's reliability is graded across
    // all of them, while the golden compare uses the last (the `received` slot).
    let observations = match observe(&spec) {
        Ok(observations) => observations,
        Err(err) => return Ok(Some(errored_entry(&spec.path, diagnostics, &err))),
    };
    let received = match observations.last() {
        Some(received) => received,
        None => {
            return Ok(Some(errored_entry(
                &spec.path,
                diagnostics,
                &ExpectError::Surface(format!("observe produced no run for `{}`", spec.path)),
            )))
        }
    };

    let verdict = evaluate_repeated(&spec, &observations);
    let golden = read_golden(repo_root, &spec.path)?;
    let ledger = ledger_state(
        &spec,
        golden.as_ref(),
        Some(received),
        options.scrubbers,
        options.seam,
    );
    let status = derive_status(&verdict, ledger);

    // The old-vs-new evidence travels only with a drift — the one outcome a
    // reviewer must act on. A tiered golden is re-graded through the pinned seam.
    let comparison = match (status, &golden) {
        (CheckStatus::Drifted, Some(golden)) => Some(compare_tiered(
            golden,
            received,
            options.scrubbers,
            options.seam.embedder,
            options.seam.judgment,
        )),
        _ => None,
    };

    Ok(Some(CheckEntry {
        path: spec.path.clone(),
        status,
        message: run_message(status, &verdict),
        diagnostics,
        verdict: Some(verdict),
        comparison,
    }))
}

/// Derive the post-observe status: an unmet criterion is [`CheckStatus::Failed`]
/// (the program is wrong) and outranks any drift; otherwise the golden ledger
/// state decides.
fn derive_status(verdict: &ExpectationVerdict, ledger: LedgerState) -> CheckStatus {
    if !verdict.reliability.satisfied() {
        return CheckStatus::Failed;
    }
    match ledger {
        LedgerState::Approved => CheckStatus::Passed,
        LedgerState::Drifted => CheckStatus::Drifted,
        LedgerState::New => CheckStatus::New,
        LedgerState::Stale => CheckStatus::Stale,
    }
}

/// Whether `diagnostics` carries any error finding (the doctor gate's refusal
/// signal).
fn has_error(diagnostics: &[FieldDiagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.status == DiagnosticStatus::Error)
}

/// Build a [`CheckStatus::Malformed`] entry — the doctor gate refused to run the
/// spec, so it carries the teaching diagnostics but no verdict.
fn malformed_entry(path: &str, diagnostics: Vec<FieldDiagnostic>) -> CheckEntry {
    let message = malformed_message(&diagnostics);
    CheckEntry {
        path: path.to_string(),
        status: CheckStatus::Malformed,
        message,
        diagnostics,
        verdict: None,
        comparison: None,
    }
}

/// Build a malformed entry for the exceptional case of a doctor-clean spec the
/// parser still rejects, appending the parse error as a synthesized finding.
fn parse_failure_entry(
    path: &str,
    mut diagnostics: Vec<FieldDiagnostic>,
    err: &ExpectError,
) -> CheckEntry {
    diagnostics.push(FieldDiagnostic {
        field: "frontmatter".to_string(),
        status: DiagnosticStatus::Error,
        message: format!("spec failed to parse: {err}"),
        allowed: None,
        suggestion: None,
        line: None,
    });
    malformed_entry(path, diagnostics)
}

/// Build a [`CheckStatus::Errored`] entry for a spec whose observe failed.
fn errored_entry(path: &str, diagnostics: Vec<FieldDiagnostic>, err: &ExpectError) -> CheckEntry {
    CheckEntry {
        path: path.to_string(),
        status: CheckStatus::Errored,
        message: format!("{OBSERVE_FAILED_MESSAGE}: {err}"),
        diagnostics,
        verdict: None,
        comparison: None,
    }
}

/// The teaching message for a malformed spec: routes an uncheckable criterion to
/// "sharpen it" and any other fault to "fix the spec", and names the error count.
fn malformed_message(diagnostics: &[FieldDiagnostic]) -> String {
    let errors = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.status == DiagnosticStatus::Error)
        .count();
    let uncheckable = diagnostics.iter().any(|diagnostic| {
        diagnostic.field == CRITERIA_FIELD && diagnostic.status == DiagnosticStatus::Error
    });
    let base = if uncheckable {
        UNCHECKABLE_MESSAGE
    } else {
        MALFORMED_MESSAGE
    };
    format!("{base} ({errors} error(s))")
}

/// The teaching message for a spec that was actually run, enriching a
/// [`CheckStatus::Failed`] with the failing criteria's reasons.
fn run_message(status: CheckStatus, verdict: &ExpectationVerdict) -> String {
    match status {
        CheckStatus::Passed => PASSED_MESSAGE.to_string(),
        CheckStatus::Failed => {
            let reasons: Vec<&str> = verdict
                .criteria
                .iter()
                .filter(|criterion| !criterion.pass)
                .map(|criterion| criterion.reason.as_str())
                .collect();
            let spread = reliability_spread(verdict);
            if reasons.is_empty() {
                format!("{FAILED_MESSAGE}{spread}")
            } else {
                format!("{FAILED_MESSAGE}: {}{spread}", reasons.join("; "))
            }
        }
        CheckStatus::Drifted => DRIFTED_MESSAGE.to_string(),
        CheckStatus::Stale => STALE_MESSAGE.to_string(),
        CheckStatus::New => NEW_MESSAGE.to_string(),
        // Malformed/Errored entries are built with their own messages and never
        // reach this run-time message path.
        CheckStatus::Malformed => MALFORMED_MESSAGE.to_string(),
        CheckStatus::Errored => OBSERVE_FAILED_MESSAGE.to_string(),
    }
}

/// A `" (P/N runs passed)"` suffix surfacing the `pass^k` per-run spread, or the
/// empty string for a single-run expectation.
///
/// Keeps a 2-of-3 flake visible in the teaching message — the spread is the
/// reason a [`CheckStatus::Failed`] verdict can hold even when the latest run's
/// criteria all passed.
fn reliability_spread(verdict: &ExpectationVerdict) -> String {
    let runs = verdict.reliability.runs.len();
    if runs <= 1 {
        String::new()
    } else {
        format!(" ({}/{runs} runs passed)", verdict.reliability.passed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::{approve, write_golden, GradingPins};
    use serde_json::{json, Value};
    use std::cell::RefCell;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// A spec body that states intent and carries the given Tier-1 `criteria` —
    /// doctor-clean (intent present, ≥1 checkable criterion).
    fn spec_content(criteria: &[&str]) -> String {
        let mut body =
            String::from("---\ndescription: a checked expectation\nsurface: cli\n---\n\nThe system under test reports a value.\n\n## Then\n");
        for criterion in criteria {
            body.push_str(&format!("- [ ] {criterion}\n"));
        }
        body
    }

    /// Write a spec at `repo/<identity>.expect.md`.
    fn write_spec(repo: &Path, identity: &str, criteria: &[&str]) {
        let file = repo.join(format!("{identity}{EXPECT_EXTENSION}"));
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, spec_content(criteria)).unwrap();
    }

    /// A single-checkpoint JSON observation for `identity` carrying `body`.
    fn json_observation(identity: &str, body: Value) -> Observation {
        serde_json::from_value(json!({
            "path": identity,
            "checkpoints": [{
                "after": "final",
                "state": { "kind": "json", "body": body },
                "duration_ms": 1
            }],
            "trajectory": { "steps": [] }
        }))
        .expect("observation json parses")
    }

    /// Default doctor facts — no pinned models, no project facts (so no dynamic
    /// findings fire; the fixtures pin no model and declare no setup).
    fn facts() -> DoctorFacts {
        DoctorFacts::default()
    }

    /// The default repo config (drift fails in CI; the documented defaults).
    fn config() -> ExpectConfig {
        ExpectConfig::default()
    }

    /// A zero-vector Tier-2 embedder for the check fixtures. Every fixture golden
    /// is Tier-1-only (numeric criteria), so the embedder is never consulted; it is
    /// present only to satisfy the [`GradingSeam`] the tiered compare threads.
    struct ZeroEmbedder;

    impl crate::evaluate::TextEmbedder for ZeroEmbedder {
        fn embed(&self, _text: &str) -> Vec<f32> {
            Vec::new()
        }
    }

    /// The Tier-1-only grading seam the check fixtures thread through the tiered
    /// compare (never consulted, since every fixture golden is deterministic). The
    /// never-touched embedder + empty judgment context are leaked to `'static` (a
    /// trait-object seam cannot be a `static`, which would require `Sync`).
    fn seam() -> &'static GradingSeam<'static> {
        let embedder: &'static dyn crate::evaluate::TextEmbedder =
            Box::leak(Box::new(ZeroEmbedder));
        let judgment: &'static crate::grader::JudgmentContext =
            Box::leak(Box::new(crate::grader::JudgmentContext {
                panel: &[],
                driver_model: "",
                escalate_below_confidence: 0.0,
            }));
        Box::leak(Box::new(GradingSeam { embedder, judgment }))
    }

    /// Build [`CheckOptions`] from borrowed `facts`/`config`/`scrubbers` and a
    /// `ci` flag, threading the Tier-1-only [`seam`].
    fn options<'a>(
        facts: &'a DoctorFacts,
        config: &'a ExpectConfig,
        scrubbers: &'a ScrubberSet,
        ci: bool,
    ) -> CheckOptions<'a> {
        CheckOptions {
            facts,
            config,
            scrubbers,
            seam: seam(),
            ci,
        }
    }

    /// Seed an approved golden for `identity` by freezing its on-disk criteria
    /// against an observation carrying `body` — the real approve path, so the
    /// frozen assertions are genuine.
    fn seed_golden(repo: &Path, identity: &str, body: Value) {
        let spec = ExpectationLoader::new(repo)
            .resolve_scope(Some(identity), None)
            .expect("resolve scope")
            .into_iter()
            .next()
            .expect("spec on disk");
        let observation = json_observation(identity, body);
        let golden = approve(
            &spec,
            &observation,
            GradingPins::from_config(&ExpectConfig::default()),
            None,
            &ScrubberSet::default_set(),
        )
        .expect("seed approve");
        write_golden(repo, &golden).expect("write seed golden");
    }

    /// Find an entry by its spec identity.
    fn entry<'a>(report: &'a CheckReport, path: &str) -> &'a CheckEntry {
        report
            .entries
            .iter()
            .find(|entry| entry.path == path)
            .unwrap_or_else(|| panic!("no entry for `{path}` in {:?}", report.entries))
    }

    /// A fixture repo with one passing spec, one failing (code wrong), one
    /// drifted, and one malformed — plus the goldens the passing/drifted specs
    /// need.
    fn fixture() -> TempDir {
        let repo = TempDir::new().unwrap();
        let root = repo.path();

        // passing: golden on {total:40}, observe will return {total:40}.
        write_spec(root, "passing", &["the total is $40"]);
        seed_golden(root, "passing", json!({ "total": 40 }));

        // failing: no golden, observe returns {total:50} -> criterion does not hold.
        write_spec(root, "failing", &["the total is $40"]);

        // drifted: invariant golden on 3 items, observe returns 5 items — the
        // relationship still holds (so it is not a fail) but the evidence moved.
        write_spec(
            root,
            "drifted",
            &["the item count equals the number of items"],
        );
        seed_golden(
            root,
            "drifted",
            json!({ "item_count": 3, "items": [{}, {}, {}] }),
        );

        // malformed: an unknown frontmatter key the doctor gate rejects.
        fs::write(
            root.join(format!("malformed{EXPECT_EXTENSION}")),
            "---\ndescription: a malformed spec\nsurfce: cli\n---\n\nIntent.\n\n## Then\n- [ ] the total is $40\n",
        )
        .unwrap();

        repo
    }

    /// A single `pass^k` run for `identity` carrying `body`, in the
    /// [`Vec<Observation>`] shape the `observe` seam now returns.
    fn one_run(identity: &str, body: Value) -> Result<Vec<Observation>, ExpectError> {
        Ok(vec![json_observation(identity, body)])
    }

    /// The observation each fixture spec resolves to; observing the malformed spec
    /// is a test failure (the doctor gate must block it).
    fn fixture_observation(spec: &Expectation) -> Result<Vec<Observation>, ExpectError> {
        let body = match spec.path.as_str() {
            "passing" => json!({ "total": 40 }),
            "failing" => json!({ "total": 50 }),
            "drifted" => json!({ "item_count": 5, "items": [{}, {}, {}, {}, {}] }),
            other => panic!("the doctor gate must block observing `{other}`"),
        };
        one_run(&spec.path, body)
    }

    #[test]
    fn check_reports_pass_fail_and_drift_per_spec_with_the_aggregate_exit_code() {
        let repo = fixture();
        let facts = facts();
        let config = config();
        let scrubbers = ScrubberSet::default_set();
        let observed = RefCell::new(Vec::new());

        let report = check(
            repo.path(),
            None,
            None,
            &options(&facts, &config, &scrubbers, false),
            |spec| {
                observed.borrow_mut().push(spec.path.clone());
                fixture_observation(spec)
            },
        )
        .expect("check runs");

        assert_eq!(entry(&report, "passing").status, CheckStatus::Passed);
        assert_eq!(entry(&report, "failing").status, CheckStatus::Failed);
        assert_eq!(entry(&report, "drifted").status, CheckStatus::Drifted);
        assert_eq!(entry(&report, "malformed").status, CheckStatus::Malformed);

        // The drift carries its re-derived old-vs-new evidence; the others do not.
        assert!(entry(&report, "drifted").comparison.is_some());
        assert!(entry(&report, "passing").comparison.is_none());

        // A failing criterion routes to "the program is wrong".
        assert!(entry(&report, "failing")
            .message
            .contains("program is wrong"));
        // A malformed spec routes to "fix the spec", never the code.
        assert!(entry(&report, "malformed").message.contains("fix the spec"));

        // The aggregate exit code is the worst per-spec code: malformed (2) wins.
        assert_eq!(report.exit_code, CHECK_EXIT_MALFORMED);
        assert_ne!(report.exit_code, CHECK_EXIT_OK);

        // The doctor gate blocked the malformed spec before any observe.
        let observed = observed.into_inner();
        assert!(
            !observed.contains(&"malformed".to_string()),
            "the malformed spec must never be observed, got {observed:?}"
        );
        assert!(observed.contains(&"passing".to_string()));
        assert!(observed.contains(&"failing".to_string()));
        assert!(observed.contains(&"drifted".to_string()));
    }

    #[test]
    fn doctor_gate_blocks_observe_of_a_malformed_spec() {
        let repo = TempDir::new().unwrap();
        fs::write(
            repo.path().join(format!("bad{EXPECT_EXTENSION}")),
            "---\ndescription: a malformed spec\nsurfce: cli\n---\n\nIntent.\n\n## Then\n- [ ] the total is $40\n",
        )
        .unwrap();
        let facts = facts();
        let config = config();
        let scrubbers = ScrubberSet::default_set();

        let report = check(
            repo.path(),
            None,
            None,
            &options(&facts, &config, &scrubbers, false),
            |spec| panic!("observe must not run for `{}`", spec.path),
        )
        .expect("check runs without observing");

        let bad = entry(&report, "bad");
        assert_eq!(bad.status, CheckStatus::Malformed);
        assert!(bad.verdict.is_none(), "a malformed spec is never evaluated");
        // The teaching diagnostics travel with the malformed entry.
        assert!(bad
            .diagnostics
            .iter()
            .any(|d| d.status == DiagnosticStatus::Error));
        assert_eq!(report.exit_code, CHECK_EXIT_MALFORMED);
    }

    #[test]
    fn an_uncheckable_criterion_routes_to_sharpen_not_fix_the_code() {
        let repo = TempDir::new().unwrap();
        // Intent present, but the only criterion is vague (no observable signal):
        // doctor flags it as an error, so the spec is malformed and never run.
        fs::write(
            repo.path().join(format!("vague{EXPECT_EXTENSION}")),
            "---\ndescription: a vague spec\nsurface: cli\n---\n\nThe checkout should be pleasant.\n\n## Then\n- [ ] the checkout feels fast\n",
        )
        .unwrap();
        let facts = facts();
        let config = config();
        let scrubbers = ScrubberSet::default_set();

        let report = check(
            repo.path(),
            None,
            None,
            &options(&facts, &config, &scrubbers, false),
            |spec| panic!("observe must not run for `{}`", spec.path),
        )
        .expect("check runs");

        let vague = entry(&report, "vague");
        assert_eq!(vague.status, CheckStatus::Malformed);
        assert!(
            vague.message.contains("sharpen"),
            "an uncheckable criterion routes to sharpening, got: {}",
            vague.message
        );
    }

    #[test]
    fn a_new_expectation_passes_locally_but_fails_in_ci() {
        let repo = TempDir::new().unwrap();
        write_spec(repo.path(), "fresh", &["the total is $40"]);
        let facts = facts();
        let config = config();
        let scrubbers = ScrubberSet::default_set();
        let observe = |spec: &Expectation| one_run(&spec.path, json!({ "total": 40 }));

        let local = check(
            repo.path(),
            None,
            None,
            &options(&facts, &config, &scrubbers, false),
            observe,
        )
        .expect("local check");
        assert_eq!(entry(&local, "fresh").status, CheckStatus::New);
        assert_eq!(local.exit_code, CHECK_EXIT_OK, "new passes locally");

        let ci = check(
            repo.path(),
            None,
            None,
            &options(&facts, &config, &scrubbers, true),
            observe,
        )
        .expect("ci check");
        assert_eq!(entry(&ci, "fresh").status, CheckStatus::New);
        assert_eq!(ci.exit_code, CHECK_EXIT_FAILED, "new fails in CI");
    }

    #[test]
    fn an_observe_failure_is_an_errored_entry_not_an_aborted_batch() {
        let repo = TempDir::new().unwrap();
        write_spec(repo.path(), "boom", &["the total is $40"]);
        write_spec(repo.path(), "ok", &["the total is $40"]);
        seed_golden(repo.path(), "ok", json!({ "total": 40 }));
        let facts = facts();
        let config = config();
        let scrubbers = ScrubberSet::default_set();

        let report = check(
            repo.path(),
            None,
            None,
            &options(&facts, &config, &scrubbers, false),
            |spec| {
                if spec.path == "boom" {
                    Err(ExpectError::Surface("provision failed".to_string()))
                } else {
                    one_run(&spec.path, json!({ "total": 40 }))
                }
            },
        )
        .expect("check runs");

        assert_eq!(entry(&report, "boom").status, CheckStatus::Errored);
        assert_eq!(entry(&report, "ok").status, CheckStatus::Passed);
        // One spec's observe failure does not abort the batch.
        assert_eq!(report.entries.len(), 2);
        assert_eq!(report.exit_code, CHECK_EXIT_FAILED);
    }

    #[test]
    fn a_tag_scope_narrows_the_checked_specs() {
        let repo = TempDir::new().unwrap();
        // Two specs, only one tagged `pricing`.
        fs::write(
            repo.path().join(format!("tagged{EXPECT_EXTENSION}")),
            "---\ndescription: tagged\nsurface: cli\ntags: [pricing]\n---\n\nIntent.\n\n## Then\n- [ ] the total is $40\n",
        )
        .unwrap();
        write_spec(repo.path(), "untagged", &["the total is $40"]);
        let facts = facts();
        let config = config();
        let scrubbers = ScrubberSet::default_set();

        let report = check(
            repo.path(),
            None,
            Some("pricing"),
            &options(&facts, &config, &scrubbers, false),
            |spec| one_run(&spec.path, json!({ "total": 40 })),
        )
        .expect("check runs");

        assert_eq!(report.entries.len(), 1, "only the tagged spec is checked");
        assert_eq!(report.entries[0].path, "tagged");
    }

    #[test]
    fn check_status_exit_codes_follow_the_ci_and_autoapprove_policy() {
        // Passed/Failed/Malformed are environment-independent.
        assert_eq!(CheckStatus::Passed.exit_code(false, false), CHECK_EXIT_OK);
        assert_eq!(
            CheckStatus::Failed.exit_code(false, false),
            CHECK_EXIT_FAILED
        );
        assert_eq!(
            CheckStatus::Malformed.exit_code(false, false),
            CHECK_EXIT_MALFORMED
        );

        // Drift fails unless CI explicitly auto-approves it.
        assert_eq!(
            CheckStatus::Drifted.exit_code(false, false),
            CHECK_EXIT_FAILED
        );
        assert_eq!(
            CheckStatus::Drifted.exit_code(true, false),
            CHECK_EXIT_FAILED
        );
        assert_eq!(CheckStatus::Drifted.exit_code(true, true), CHECK_EXIT_OK);

        // New is fine locally, fatal in CI.
        assert_eq!(CheckStatus::New.exit_code(false, false), CHECK_EXIT_OK);
        assert_eq!(CheckStatus::New.exit_code(true, false), CHECK_EXIT_FAILED);
    }

    /// The repo root the loader derives identities against (a no-op sanity guard
    /// that the fixture identities are forward-slash relative paths).
    #[test]
    fn fixture_identities_are_repo_relative() {
        let repo = fixture();
        let specs = ExpectationLoader::new(repo.path())
            .discover_raw(None)
            .expect("discover");
        let ids: Vec<&str> = specs.iter().map(|s| s.path.as_str()).collect();
        assert!(ids.contains(&"passing"));
        assert!(ids.contains(&"malformed"));
        let _ = PathBuf::from(repo.path());
    }

    #[test]
    fn a_malformed_spec_is_reported_even_under_a_tag_scope() {
        let repo = TempDir::new().unwrap();
        // A malformed spec (unparseable, so its tags are unknowable) alongside a
        // tagged well-formed spec: the tag narrowing drops untagged *parseable*
        // specs, but the doctor gate still surfaces the broken one (fail-safe).
        fs::write(
            repo.path().join(format!("broken{EXPECT_EXTENSION}")),
            "---\ndescription: a malformed spec\nsurfce: cli\n---\n\nIntent.\n\n## Then\n- [ ] the total is $40\n",
        )
        .unwrap();
        fs::write(
            repo.path().join(format!("tagged{EXPECT_EXTENSION}")),
            "---\ndescription: tagged\nsurface: cli\ntags: [pricing]\n---\n\nIntent.\n\n## Then\n- [ ] the total is $40\n",
        )
        .unwrap();
        // An untagged, parseable spec the tag scope must drop.
        write_spec(repo.path(), "untagged", &["the total is $40"]);
        let facts = facts();
        let config = config();
        let scrubbers = ScrubberSet::default_set();

        let report = check(
            repo.path(),
            None,
            Some("pricing"),
            &options(&facts, &config, &scrubbers, false),
            |spec| one_run(&spec.path, json!({ "total": 40 })),
        )
        .expect("check runs");

        assert_eq!(entry(&report, "broken").status, CheckStatus::Malformed);
        assert_eq!(entry(&report, "tagged").status, CheckStatus::New);
        assert!(
            report.entries.iter().all(|e| e.path != "untagged"),
            "an untagged parseable spec is narrowed out by the tag scope"
        );
        assert_eq!(report.exit_code, CHECK_EXIT_MALFORMED);
    }

    #[test]
    fn check_grades_pass_k_across_runs_and_fails_on_any_flake() {
        let repo = TempDir::new().unwrap();
        // A `pass^3` spec: all three runs must pass.
        fs::write(
            repo.path().join(format!("flaky{EXPECT_EXTENSION}")),
            "---\ndescription: a pass^3 spec\nsurface: cli\nreliability: pass^3\n---\n\nThe system under test reports a value.\n\n## Then\n- [ ] the total is $40\n",
        )
        .unwrap();
        seed_golden(repo.path(), "flaky", json!({ "total": 40 }));
        let facts = facts();
        let config = config();
        let scrubbers = ScrubberSet::default_set();

        // Three clean runs: pass^3 holds and matches the golden -> Passed.
        let clean = check(
            repo.path(),
            None,
            None,
            &options(&facts, &config, &scrubbers, false),
            |spec| {
                Ok(vec![
                    json_observation(&spec.path, json!({ "total": 40 })),
                    json_observation(&spec.path, json!({ "total": 40 })),
                    json_observation(&spec.path, json!({ "total": 40 })),
                ])
            },
        )
        .expect("clean check");
        let clean_entry = entry(&clean, "flaky");
        assert_eq!(clean_entry.status, CheckStatus::Passed);
        assert_eq!(
            clean_entry.verdict.as_ref().unwrap().reliability.runs,
            vec![true, true, true],
            "pass^3 runs observe three times"
        );

        // A 2-of-3 flake: the middle run drifts to 50, so pass^3 fails and the
        // teaching message surfaces the per-run spread rather than an average.
        let flaky = check(
            repo.path(),
            None,
            None,
            &options(&facts, &config, &scrubbers, false),
            |spec| {
                Ok(vec![
                    json_observation(&spec.path, json!({ "total": 40 })),
                    json_observation(&spec.path, json!({ "total": 50 })),
                    json_observation(&spec.path, json!({ "total": 40 })),
                ])
            },
        )
        .expect("flaky check");
        let flaky_entry = entry(&flaky, "flaky");
        assert_eq!(flaky_entry.status, CheckStatus::Failed);
        assert_eq!(
            flaky_entry.verdict.as_ref().unwrap().reliability.runs,
            vec![true, false, true]
        );
        assert!(
            flaky_entry.message.contains("2/3 runs passed"),
            "the per-run spread is visible, got: {}",
            flaky_entry.message
        );
        assert_eq!(flaky.exit_code, CHECK_EXIT_FAILED);
    }
}
