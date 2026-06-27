//! The drift ledger: the golden store, evidence scrubbers, and the per-criterion
//! tier compare. Per `ideas/expect.md` §"The Drift Ledger".
//!
//! The ledger is modeled on snapshot/approval UI testing, adapted for
//! non-determinism. Its three moving parts live here:
//!
//! - **[`Golden`]** — the approved, *scrubbed* [`Observation`] a human signed off
//!   on, together with the **frozen** [`CompiledAssertion`] set and the pinned
//!   grading [`GradingPins`] (model, embedder, thresholds). The golden stores an
//!   *observation*, never a verdict: the verdict is always re-derived by
//!   [`evaluate`] on both sides, so a baseline stays re-evaluable against edited
//!   criteria or a changed grading model. Goldens are committed at
//!   `.expect/goldens/<repo-rel>.golden.json`, addressed through the safe-join
//!   [`golden_path`].
//! - **Scrubbers** ([`ScrubberSet`]) — normalize volatile content (timestamps,
//!   UUIDs, ports, temp paths, run-specific ids) out of an observation *before*
//!   comparison, the proven approval-testing lever that keeps the ledger stable
//!   without masking real changes. The set is configurable; [`ScrubberSet::default_set`]
//!   carries the standard scrubbers.
//! - **Compare** ([`compare`]) — `evaluate(received)` vs `evaluate(golden)`, per
//!   criterion, **field-wise by tier**: a deterministic criterion drifts when its
//!   matched value changes; a tolerance criterion when its score leaves the band;
//!   a judgment criterion when its approved evidence diverges past the similarity
//!   threshold. The verdict is re-derived on both sides — never the stored source
//!   of truth.
//!
//! The tolerance band and judgment similarity comparisons are **stubbed** here
//! (a strict exact band and exact-evidence equality respectively); the full
//! Tier-2/Tier-3 semantics land in their own tasks.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::assertion::{compile, AssertionOutcome, CompileError, CompiledAssertion};
use crate::config::ExpectConfig;
use crate::error::ExpectError;
use crate::evaluate::evaluate;
use crate::observe::golden_path;
use crate::spec::{Criterion, Expectation};
use crate::types::{
    CliState, CriterionVerdict, LedgerState, Observation, SurfaceState, Trajectory, VerdictTier,
};

// ---------------------------------------------------------------------------
// Scrubber patterns and placeholders.
// ---------------------------------------------------------------------------

/// Matches an ISO-8601-ish date-time, with optional fractional seconds and
/// timezone (`2026-06-26T14:44:30.076793Z`, `2026-06-26 14:44:30+02:00`).
const TIMESTAMP_PATTERN: &str =
    r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?";

/// The placeholder a scrubbed timestamp collapses to.
const TIMESTAMP_PLACEHOLDER: &str = "<TIMESTAMP>";

/// Matches a canonical 8-4-4-4-12 hex UUID, either case.
const UUID_PATTERN: &str =
    r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}";

/// The placeholder a scrubbed UUID collapses to.
const UUID_PLACEHOLDER: &str = "<UUID>";

/// Matches a 26-character Crockford-base32 ULID (the run-specific id this repo
/// mints), bounded so it does not swallow longer alphanumeric runs.
const ULID_PATTERN: &str = r"\b[0-9A-HJKMNP-TV-Z]{26}\b";

/// The placeholder a scrubbed run id collapses to.
const ULID_PLACEHOLDER: &str = "<ID>";

/// Matches a temp path under `/tmp` or `/var/folders` (optionally `/private`-prefixed,
/// as macOS reports), capturing the volatile leaf so the whole path normalizes.
const TEMP_PATH_PATTERN: &str = r"(?:/private)?/(?:tmp|var/folders)/[A-Za-z0-9._/+-]+";

/// The placeholder a scrubbed temp path collapses to.
const TEMP_PATH_PLACEHOLDER: &str = "<TMP>";

/// Matches a `host:port` pair on a loopback host, capturing the host so only the
/// volatile port is normalized.
const PORT_PATTERN: &str = r"(?P<host>localhost|127\.0\.0\.1|0\.0\.0\.0):\d{2,5}";

/// The replacement for a scrubbed port: the preserved host plus a placeholder.
const PORT_REPLACEMENT: &str = "$host:<PORT>";

/// The wall-clock [`Checkpoint::duration`](crate::Checkpoint) a scrubbed
/// observation carries. Timing is genuinely volatile run-to-run, so it is
/// normalized to a constant rather than frozen into the golden.
const NORMALIZED_DURATION: Duration = Duration::ZERO;

// ---------------------------------------------------------------------------
// Tier compare tuning (stubs for Tier 2/3).
// ---------------------------------------------------------------------------

/// The Tier-2 score band the compare allows before a tolerance criterion counts
/// as drifted. A **stub**: a strict near-exact band so any meaningful score
/// movement surfaces; the real adaptive band lands in the Tier-2 task.
const STUB_TOLERANCE_BAND: f32 = f32::EPSILON;

// ---------------------------------------------------------------------------
// Scrubbers.
// ---------------------------------------------------------------------------

/// One named, regex-driven normalization applied to volatile content.
///
/// A scrubber pairs a compiled pattern with a replacement (which may reference
/// capture groups via `$name`, as the port scrubber does to preserve its host).
#[derive(Debug, Clone)]
pub struct Scrubber {
    name: String,
    pattern: regex::Regex,
    replacement: String,
}

impl Scrubber {
    /// Compile a scrubber from a `name`, a regex `pattern`, and a `replacement`.
    ///
    /// # Errors
    ///
    /// Returns [`regex::Error`] when `pattern` is not a valid regular expression.
    pub fn new(
        name: impl Into<String>,
        pattern: &str,
        replacement: impl Into<String>,
    ) -> Result<Self, regex::Error> {
        Ok(Self {
            name: name.into(),
            pattern: regex::Regex::new(pattern)?,
            replacement: replacement.into(),
        })
    }

    /// The scrubber's name, for diagnostics.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Apply this scrubber to `input`, replacing every match.
    fn apply(&self, input: &str) -> String {
        self.pattern
            .replace_all(input, self.replacement.as_str())
            .into_owned()
    }
}

/// A configurable, ordered set of [`Scrubber`]s applied to an observation before
/// comparison.
///
/// Order matters: the timestamp scrubber runs before the port scrubber so the
/// `HH:MM:SS` inside a timestamp is consumed as part of the timestamp rather than
/// mistaken for a port. The placeholders the scrubbers emit are deliberately not
/// matched by any pattern, so scrubbing is idempotent.
#[derive(Debug, Clone)]
pub struct ScrubberSet {
    scrubbers: Vec<Scrubber>,
}

impl ScrubberSet {
    /// Build a set from an explicit, ordered list of scrubbers.
    pub fn new(scrubbers: Vec<Scrubber>) -> Self {
        Self { scrubbers }
    }

    /// The standard scrubber set: timestamps, UUIDs, run-id ULIDs, temp paths,
    /// and loopback ports, in the order the compare relies on.
    pub fn default_set() -> Self {
        let scrubbers = vec![
            Scrubber::new("timestamp", TIMESTAMP_PATTERN, TIMESTAMP_PLACEHOLDER),
            Scrubber::new("uuid", UUID_PATTERN, UUID_PLACEHOLDER),
            Scrubber::new("run-id", ULID_PATTERN, ULID_PLACEHOLDER),
            Scrubber::new("temp-path", TEMP_PATH_PATTERN, TEMP_PATH_PLACEHOLDER),
            Scrubber::new("port", PORT_PATTERN, PORT_REPLACEMENT),
        ]
        .into_iter()
        .map(|scrubber| scrubber.expect("built-in scrubber patterns are valid"))
        .collect();
        Self::new(scrubbers)
    }

    /// Apply every scrubber, in order, to `input`.
    pub fn scrub_text(&self, input: &str) -> String {
        self.scrubbers
            .iter()
            .fold(input.to_string(), |text, scrubber| scrubber.apply(&text))
    }

    /// Return a copy of `observation` with every volatile string in its
    /// checkpoint states and driver trajectory normalized, and each checkpoint's
    /// wall-clock duration collapsed to [`NORMALIZED_DURATION`].
    ///
    /// The spec identity ([`Observation::path`]) is *not* scrubbed — it is the
    /// stable address, not volatile content.
    pub fn scrub_observation(&self, observation: &Observation) -> Observation {
        Observation {
            path: observation.path.clone(),
            checkpoints: observation
                .checkpoints
                .iter()
                .map(|checkpoint| {
                    let mut scrubbed = checkpoint.clone();
                    scrubbed.state = self.scrub_state(&checkpoint.state);
                    scrubbed.duration = NORMALIZED_DURATION;
                    scrubbed
                })
                .collect(),
            trajectory: Trajectory {
                steps: observation
                    .trajectory
                    .steps
                    .iter()
                    .map(|step| self.scrub_text(step))
                    .collect(),
            },
        }
    }

    /// Scrub one surface state's volatile content.
    fn scrub_state(&self, state: &SurfaceState) -> SurfaceState {
        match state {
            SurfaceState::Cli(cli) => SurfaceState::Cli(CliState {
                stdout: self.scrub_text(&cli.stdout),
                stderr: self.scrub_text(&cli.stderr),
                exit_code: cli.exit_code,
                files: cli
                    .files
                    .iter()
                    .map(|(path, contents)| (self.scrub_text(path), self.scrub_text(contents)))
                    .collect(),
            }),
            SurfaceState::Json { body } => SurfaceState::Json {
                body: self.scrub_json(body),
            },
        }
    }

    /// Recursively scrub every string (object keys and values, array elements)
    /// in a JSON body, leaving non-string scalars untouched.
    fn scrub_json(&self, value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::String(text) => serde_json::Value::String(self.scrub_text(text)),
            serde_json::Value::Array(items) => {
                serde_json::Value::Array(items.iter().map(|item| self.scrub_json(item)).collect())
            }
            serde_json::Value::Object(map) => serde_json::Value::Object(
                map.iter()
                    .map(|(key, val)| (self.scrub_text(key), self.scrub_json(val)))
                    .collect(),
            ),
            other => other.clone(),
        }
    }
}

impl Default for ScrubberSet {
    fn default() -> Self {
        Self::default_set()
    }
}

// ---------------------------------------------------------------------------
// The golden store.
// ---------------------------------------------------------------------------

/// The pinned grading inputs recorded in a golden, so a baseline's pass/fail
/// boundary is reproducible: changing the grading model or embedder is a
/// deliberate, reviewed re-baseline, never a silent boundary shift.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GradingPins {
    /// The named sah model that grades Tier-3 criteria.
    pub model: String,
    /// The pinned embedding model behind the Tier-2 similarity.
    pub embedder: String,
    /// The Tier-2 cosine similarity cutoff.
    pub similarity_threshold: f32,
}

impl GradingPins {
    /// Capture the grading pins from a repo's [`ExpectConfig`].
    pub fn from_config(config: &ExpectConfig) -> Self {
        Self {
            model: config.model.default.clone(),
            embedder: config.embedder.model.clone(),
            similarity_threshold: config.embedder.similarity_threshold,
        }
    }
}

/// An approved golden baseline: the scrubbed observation a human signed off on,
/// the frozen compiled-assertion set, and the pinned grading inputs.
///
/// The golden stores an *observation*, never a verdict — [`compare`] re-derives
/// the verdict on both sides — so the baseline stays re-evaluable against edited
/// criteria or a changed grading model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Golden {
    /// The approved, scrubbed observation. Its [`path`](Observation::path) is the
    /// golden's repo-relative identity.
    pub observation: Observation,
    /// The frozen assertions compiled at approve time and replayed (never
    /// recompiled) by [`evaluate`].
    pub assertions: Vec<CompiledAssertion>,
    /// The pinned grading model, embedder, and thresholds.
    pub grading: GradingPins,
    /// The [`spec_hash`] of the spec's criteria at approve time — the
    /// stale-detection fingerprint [`ledger_state`] recomputes against the
    /// current spec to flag a [`LedgerState::Stale`] edit since approval.
    pub spec_hash: String,
}

/// Persist `golden` to its mirrored committed slot under `repo_root`
/// (`.expect/goldens/<identity>.golden.json`), creating parent directories, and
/// return the path written.
///
/// # Errors
///
/// Returns [`ExpectError::Expectation`] when the golden's identity is unsafe (see
/// [`golden_path`]), [`ExpectError::Json`] when it cannot be serialized, or
/// [`ExpectError::Io`] when the file cannot be written.
pub fn write_golden(repo_root: &Path, golden: &Golden) -> Result<PathBuf, ExpectError> {
    let path = golden_path(repo_root, &golden.observation.path)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(golden)?;
    std::fs::write(&path, json)?;
    Ok(path)
}

/// Load the golden baseline for spec `identity` under `repo_root`, or `Ok(None)`
/// when no golden has been approved yet (a `new` expectation, not an error).
///
/// # Errors
///
/// Returns [`ExpectError::Expectation`] when `identity` is unsafe (see
/// [`golden_path`]), [`ExpectError::Io`] when the file exists but cannot be read,
/// or [`ExpectError::Json`] when it cannot be parsed.
pub fn read_golden(repo_root: &Path, identity: &str) -> Result<Option<Golden>, ExpectError> {
    let path = golden_path(repo_root, identity)?;
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(Some(serde_json::from_str(&text)?)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(ExpectError::Io(err)),
    }
}

// ---------------------------------------------------------------------------
// Per-criterion tier compare.
// ---------------------------------------------------------------------------

/// The field-wise drift comparison for a single criterion: the re-derived golden
/// and received verdicts plus whether the criterion drifted, by its tier's rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CriterionComparison {
    /// The criterion text being compared.
    pub criterion: String,
    /// Which tier's drift rule decided the comparison.
    pub tier: VerdictTier,
    /// Whether the criterion drifted from its golden.
    pub drifted: bool,
    /// The verdict re-derived from the golden observation.
    pub golden: CriterionVerdict,
    /// The verdict re-derived from the received observation.
    pub received: CriterionVerdict,
}

/// The drift comparison for a whole expectation: its per-criterion comparisons
/// and the derived ledger state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerComparison {
    /// The spec's repo-relative identity.
    pub path: String,
    /// [`LedgerState::Approved`] when no criterion drifted, else
    /// [`LedgerState::Drifted`].
    pub state: LedgerState,
    /// The per-criterion drift comparisons, in frozen-assertion order.
    pub criteria: Vec<CriterionComparison>,
}

/// Compare a `received` observation against its `golden`, per criterion, by tier.
///
/// Both sides are scrubbed (the golden again, idempotently) and re-graded with
/// the golden's frozen assertions, so the comparison is apples-to-apples and free
/// of volatile noise. The verdict is re-derived on both sides — never read from
/// storage. The overall [`LedgerState`] is [`LedgerState::Drifted`] if any
/// criterion drifted, else [`LedgerState::Approved`].
pub fn compare(
    golden: &Golden,
    received: &Observation,
    scrubbers: &ScrubberSet,
) -> LedgerComparison {
    let scrubbed_golden = scrubbers.scrub_observation(&golden.observation);
    let scrubbed_received = scrubbers.scrub_observation(received);

    let golden_verdict = evaluate(&scrubbed_golden, &golden.assertions);
    let received_verdict = evaluate(&scrubbed_received, &golden.assertions);

    let criteria: Vec<CriterionComparison> = golden_verdict
        .criteria
        .into_iter()
        .zip(received_verdict.criteria)
        .map(|(golden, received)| compare_criterion(golden, received))
        .collect();

    let state = if criteria.iter().any(|comparison| comparison.drifted) {
        LedgerState::Drifted
    } else {
        LedgerState::Approved
    };

    LedgerComparison {
        path: golden.observation.path.clone(),
        state,
        criteria,
    }
}

/// Compare one re-derived golden/received criterion pair by the golden's tier.
fn compare_criterion(golden: CriterionVerdict, received: CriterionVerdict) -> CriterionComparison {
    let drifted = match golden.tier {
        VerdictTier::Deterministic => deterministic_drift(&golden, &received),
        VerdictTier::Tolerance => tolerance_drift(&golden, &received),
        VerdictTier::Judgment => judgment_drift(&golden, &received),
    };
    CriterionComparison {
        criterion: golden.criterion.clone(),
        tier: golden.tier,
        drifted,
        golden,
        received,
    }
}

/// Tier-1 drift: the matched value changed, or the pass/fail flipped. Because
/// both observations are scrubbed before grading, a volatile-only difference
/// never reaches the evidence — only a real value change drifts.
fn deterministic_drift(golden: &CriterionVerdict, received: &CriterionVerdict) -> bool {
    golden.pass != received.pass || golden.evidence != received.evidence
}

/// Tier-2 drift (**stub**): the score left the band. The full adaptive band lands
/// in the Tier-2 task; here a strict near-exact [`STUB_TOLERANCE_BAND`] is used,
/// falling back to the pass/fail flip when either side carries no score.
fn tolerance_drift(golden: &CriterionVerdict, received: &CriterionVerdict) -> bool {
    match (golden.score, received.score) {
        (Some(golden_score), Some(received_score)) => {
            (golden_score - received_score).abs() > STUB_TOLERANCE_BAND
        }
        _ => golden.pass != received.pass,
    }
}

/// Tier-3 drift (**stub**): the approved evidence diverged. The full impl takes
/// embedding similarity to the approved anchor against the pinned threshold; here
/// exact evidence equality (plus a pass/fail flip) stands in until the Tier-3
/// task lands.
fn judgment_drift(golden: &CriterionVerdict, received: &CriterionVerdict) -> bool {
    golden.pass != received.pass || golden.evidence != received.evidence
}

// ---------------------------------------------------------------------------
// The per-expectation ledger state and the unapproved-drift queue.
// ---------------------------------------------------------------------------

/// The prefix on a stored [`spec_hash`], naming the digest algorithm so the
/// fingerprint is self-describing (mirrors the review tracker's `sha256:` form).
const SPEC_HASH_PREFIX: &str = "sha256:";

/// The stale-detection hash of an expectation's grading-relevant content.
///
/// A golden freezes the assertions compiled from a spec's `## Then` criteria, so
/// the criteria are exactly what the baseline is approved *against*: editing one
/// invalidates it. [`approve`] stores this hash in the [`Golden`], and
/// [`ledger_state`] recomputes it from the current spec to detect a
/// [`LedgerState::Stale`] edit since approval.
///
/// The criteria texts are hashed in order, each length-prefixed so a boundary
/// shift between adjacent criteria (`["ab","c"]` vs `["a","bc"]`) cannot collide.
/// The ticked/unticked checkbox state is deliberately excluded — it is review
/// bookkeeping, not grading content.
pub fn spec_hash(spec: &Expectation) -> String {
    let mut hasher = Sha256::new();
    for criterion in &spec.criteria {
        hasher.update((criterion.text.len() as u64).to_le_bytes());
        hasher.update(criterion.text.as_bytes());
    }
    format!("{SPEC_HASH_PREFIX}{:x}", hasher.finalize())
}

/// Classify one expectation's drift-ledger state from its golden and last
/// received run.
///
/// The four-state model from `ideas/expect.md` §"The Drift Ledger", in
/// precedence order:
///
/// 1. [`New`](LedgerState::New) — no golden has been approved yet.
/// 2. [`Stale`](LedgerState::Stale) — the `*.expect.md` was edited since its
///    golden was approved, detected by comparing [`spec_hash`] of the current
///    spec against the hash frozen in the golden. Stale outranks drift: once the
///    criteria change, the golden's frozen assertions are out of date and the
///    baseline must be re-approved, so a drift comparison against them is moot.
/// 3. [`Drifted`](LedgerState::Drifted) — the spec is unchanged but the received
///    verdict diverged from the golden's, re-derived on both sides by [`compare`]
///    (never a stored verdict), awaiting human approval.
/// 4. [`Approved`](LedgerState::Approved) — a golden exists, the spec is
///    unchanged, and either the received run matches it or no new run has been
///    observed to contradict it.
pub fn ledger_state(
    spec: &Expectation,
    golden: Option<&Golden>,
    received: Option<&Observation>,
    scrubbers: &ScrubberSet,
) -> LedgerState {
    let Some(golden) = golden else {
        return LedgerState::New;
    };
    if golden.spec_hash != spec_hash(spec) {
        return LedgerState::Stale;
    }
    match received {
        Some(received) => compare(golden, received, scrubbers).state,
        None => LedgerState::Approved,
    }
}

/// One expectation's row in the drift ledger: its identity, derived
/// [`LedgerState`], and — only when it has [`Drifted`](LedgerState::Drifted) —
/// the old-vs-new [`compare`] evidence a reviewer triages before approving.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerEntry {
    /// The spec's repo-relative identity.
    pub path: String,
    /// The expectation's drift-ledger state.
    pub state: LedgerState,
    /// The re-derived old-vs-new comparison, present only for a drifted entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comparison: Option<LedgerComparison>,
}

/// Build one expectation's [`LedgerEntry`]: classify its [`ledger_state`] and, for
/// a drifted entry, attach the old-vs-new [`compare`] evidence awaiting approval.
///
/// The comparison is carried only for [`LedgerState::Drifted`] — the one state a
/// reviewer must act on — so an approved/new/stale row stays free of redundant
/// re-derived verdicts.
pub fn ledger_entry(
    spec: &Expectation,
    golden: Option<&Golden>,
    received: Option<&Observation>,
    scrubbers: &ScrubberSet,
) -> LedgerEntry {
    let state = ledger_state(spec, golden, received, scrubbers);
    let comparison = match (state, golden, received) {
        (LedgerState::Drifted, Some(golden), Some(received)) => {
            Some(compare(golden, received, scrubbers))
        }
        _ => None,
    };
    LedgerEntry {
        path: spec.path.clone(),
        state,
        comparison,
    }
}

/// Order a batch of ledger entries into the review queue: unapproved drift FIRST,
/// every other state after, preserving the input order within each group.
///
/// `expect expectations list` surfaces the pending old-vs-new diffs first
/// (`ideas/expect.md` §"The Drift Ledger") so the rows a human must act on lead
/// the survey. The sort is stable, so entries that share a rank keep their
/// incoming (caller-resolved) order.
pub fn ledger_queue(mut entries: Vec<LedgerEntry>) -> Vec<LedgerEntry> {
    entries.sort_by_key(|entry| drift_queue_rank(entry.state));
    entries
}

/// The review-queue sort rank of a ledger state: drift leads (0), every other
/// state follows (1). The single source of truth for the drifted-first ordering.
fn drift_queue_rank(state: LedgerState) -> u8 {
    match state {
        LedgerState::Drifted => 0,
        LedgerState::New | LedgerState::Approved | LedgerState::Stale => 1,
    }
}

// ---------------------------------------------------------------------------
// Approve: freeze assertions, render the binding diff, gate CI.
// ---------------------------------------------------------------------------

/// The arrow rendering one binding in an approve diff, read "value comes from
/// locator" (`40 ← $.total`). The diff shows the *binding*, not just the value,
/// so a mis-compiled locator is caught at review rather than baked into a golden.
pub const BINDING_ARROW: &str = " ← ";

/// The approval-relevant status of one expectation: how its received run relates
/// to its golden.
///
/// Drives both the CI gate and the [`ApproveMode`] selection. It overlaps with
/// [`LedgerState`] on the `New`/`Drifted`/`Approved` axis but adds the
/// [`Unobserved`](ApprovalStatus::Unobserved) case (no received run to promote),
/// which the ledger compare cannot express — approval is computed from the
/// *presence* of the two artifacts, not just from a compare of both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalStatus {
    /// No golden yet — a brand-new expectation, selected by `--missing`/`--all`.
    New,
    /// A golden exists and the received run drifted from it — selected by
    /// `--changed`/`--all`.
    Drifted,
    /// A golden exists and the received run matches it — nothing to approve.
    Approved,
    /// No received observation to promote — the spec must be observed first.
    Unobserved,
}

/// How an approve pass selects which in-scope expectations to promote, mirroring
/// the granular `--update-snapshots` modes of snapshot testing.
///
/// The absence of a mode is *not* a variant: a bare `approve` is a preview that
/// writes nothing and requires the reviewer to re-run with an explicit mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApproveMode {
    /// `--missing`: only brand-new expectations (no golden yet).
    Missing,
    /// `--changed`: only expectations whose received run drifted from the golden.
    Changed,
    /// `--all`: every in-scope expectation that needs approval (new or drifted).
    All,
}

impl ApproveMode {
    /// Whether this mode selects an expectation in the given [`ApprovalStatus`].
    ///
    /// An [`Approved`](ApprovalStatus::Approved) or
    /// [`Unobserved`](ApprovalStatus::Unobserved) expectation is never selected
    /// by any mode (nothing to promote); the table below is the single source of
    /// truth for the `--missing`/`--changed`/`--all` partition.
    pub fn selects(self, status: ApprovalStatus) -> bool {
        match (self, status) {
            (_, ApprovalStatus::Approved | ApprovalStatus::Unobserved) => false,
            (ApproveMode::Missing, ApprovalStatus::New) => true,
            (ApproveMode::Missing, ApprovalStatus::Drifted) => false,
            (ApproveMode::Changed, ApprovalStatus::Drifted) => true,
            (ApproveMode::Changed, ApprovalStatus::New) => false,
            (ApproveMode::All, ApprovalStatus::New | ApprovalStatus::Drifted) => true,
        }
    }
}

/// One row of an approve diff: a criterion bound to its compiled locator and the
/// value that locator resolves to in the approved observation.
///
/// The reviewer reads `criterion` → (`value` [`BINDING_ARROW`] `locator`) so a
/// mis-compiled locator is visible (a wrong `locator` that still resolves to the
/// right `value` is exactly the silent mis-read the binding view exposes).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApprovalBinding {
    /// The criterion prose the assertion is bound to.
    pub criterion: String,
    /// Where the value lives — the compiled (or reviewer-edited) locator.
    pub locator: String,
    /// The value that locator resolves to in the approved observation.
    pub value: String,
    /// The verdict tier the assertion's kind selected.
    pub tier: VerdictTier,
    /// Whether the locator diverges from a fresh compile of the criterion — i.e.
    /// a reviewer hand-edited it. A hand-edit is bound to the criterion prose:
    /// changing the prose discards it (no prior assertion matches), forcing a
    /// recompile and a fresh review.
    pub hand_edited: bool,
}

impl ApprovalBinding {
    /// Render the binding as `value ← locator` (e.g. `40 ← $.total`).
    pub fn render(&self) -> String {
        format!("{}{BINDING_ARROW}{}", self.value, self.locator)
    }
}

/// Why an approve attempt was refused before any golden was written.
#[derive(Debug, Error)]
pub enum ApproveError {
    /// A deterministic criterion compiled to a locator that does not bind and
    /// pass against the approved observation — a hallucinated locator, refused so
    /// it never reaches the golden. Reuses the compiler's self-verification
    /// ([`compile`] rejects a locator that fails to bind/pass its source run).
    #[error("cannot approve `{path}`: {source}")]
    Compile {
        /// The expectation's repo-relative identity.
        path: String,
        /// The underlying compile rejection.
        #[source]
        source: CompileError,
    },
}

/// Build the golden for `spec` from its `received` observation: scrub the run,
/// freeze its compiled assertions, and pin the grading inputs.
///
/// Compilation happens *here*, at approve time, bound against the **scrubbed**
/// observation that is actually stored — so each frozen assertion is guaranteed
/// to bind and pass against the golden it ships with ([`compile`] self-verifies,
/// and a criterion whose locator does not bind is refused as
/// [`ApproveError::Compile`], never written). A Tier-2/Tier-3 criterion that
/// carries no deterministic assertion ([`CompileError::Unrecognized`]) is left
/// out of the frozen set for a later tier to grade.
///
/// When a `prior` golden is supplied, a frozen assertion whose criterion prose is
/// unchanged and that still holds against the new observation is **preserved**
/// verbatim — this is how a reviewer's hand-edited locator survives a
/// re-approval. Changing the criterion prose breaks the match, so the criterion
/// is recompiled and re-reviewed.
///
/// # Errors
///
/// Returns [`ApproveError::Compile`] when a deterministic criterion compiles to a
/// locator that does not bind and pass against the scrubbed observation.
pub fn approve(
    spec: &Expectation,
    received: &Observation,
    grading: GradingPins,
    prior: Option<&Golden>,
    scrubbers: &ScrubberSet,
) -> Result<Golden, ApproveError> {
    let observation = scrubbers.scrub_observation(received);
    let assertions = freeze_assertions(spec, &observation, prior)?;
    Ok(Golden {
        observation,
        assertions,
        grading,
        spec_hash: spec_hash(spec),
    })
}

/// Compile (or preserve) the frozen assertion set for `spec` against the scrubbed
/// `observation`, the load-bearing half of [`approve`].
fn freeze_assertions(
    spec: &Expectation,
    observation: &Observation,
    prior: Option<&Golden>,
) -> Result<Vec<CompiledAssertion>, ApproveError> {
    let mut frozen = Vec::with_capacity(spec.criteria.len());
    for criterion in &spec.criteria {
        if let Some(preserved) = preserved_assertion(criterion, observation, prior) {
            frozen.push(preserved);
            continue;
        }
        match compile(criterion, observation) {
            Ok(assertion) => frozen.push(assertion),
            // A non-deterministic criterion is graded by a later tier, not frozen.
            Err(CompileError::Unrecognized { .. }) => {}
            // Any other rejection (a hallucinated locator above all) refuses the
            // whole approve: no unverified locator reaches the golden.
            Err(source) => {
                return Err(ApproveError::Compile {
                    path: spec.path.clone(),
                    source,
                })
            }
        }
    }
    Ok(frozen)
}

/// The prior frozen assertion for `criterion` that still holds against the new
/// `observation`, or `None` when there is no prior golden, the criterion prose
/// changed, or the prior assertion no longer holds.
///
/// Matching by criterion prose is what binds a reviewer hand-edit to the prose:
/// edit the prose and the match is lost, so the criterion is recompiled.
fn preserved_assertion(
    criterion: &Criterion,
    observation: &Observation,
    prior: Option<&Golden>,
) -> Option<CompiledAssertion> {
    let candidate = prior?
        .assertions
        .iter()
        .find(|assertion| assertion.criterion_text == criterion.text)?;
    (candidate.evaluate(observation) == AssertionOutcome::Holds).then(|| candidate.clone())
}

/// Render `golden`'s frozen assertions as the per-criterion binding diff a
/// reviewer reads before approving.
///
/// A pure view over the golden: each binding resolves its locator against the
/// stored observation and flags whether the locator was hand-edited (diverges
/// from a fresh compile of its criterion).
pub fn approval_diff(golden: &Golden) -> Vec<ApprovalBinding> {
    golden
        .assertions
        .iter()
        .map(|assertion| binding_of(assertion, &golden.observation))
        .collect()
}

/// Build the [`ApprovalBinding`] for one frozen `assertion` against `observation`.
fn binding_of(assertion: &CompiledAssertion, observation: &Observation) -> ApprovalBinding {
    let value = observation
        .checkpoints
        .get(assertion.checkpoint)
        .and_then(|checkpoint| assertion.locator.resolve(&checkpoint.state))
        .map(|value| value.to_string())
        .unwrap_or_default();
    ApprovalBinding {
        criterion: assertion.criterion_text.clone(),
        locator: assertion.locator.to_string(),
        value,
        tier: assertion.tier,
        hand_edited: is_hand_edited(assertion, observation),
    }
}

/// Whether `assertion` diverges from a fresh compile of its criterion against
/// `observation` — the signal that a reviewer hand-edited its locator.
fn is_hand_edited(assertion: &CompiledAssertion, observation: &Observation) -> bool {
    let criterion = Criterion {
        text: assertion.criterion_text.clone(),
        checked: false,
    };
    match compile(&criterion, observation) {
        Ok(fresh) => &fresh != assertion,
        Err(_) => true,
    }
}

/// The approval status of one expectation, from the presence and compare of its
/// `golden` and `received` artifacts.
pub fn approval_status(
    golden: Option<&Golden>,
    received: Option<&Observation>,
    scrubbers: &ScrubberSet,
) -> ApprovalStatus {
    let Some(received) = received else {
        return ApprovalStatus::Unobserved;
    };
    match golden {
        None => ApprovalStatus::New,
        Some(golden) => match compare(golden, received, scrubbers).state {
            LedgerState::Approved => ApprovalStatus::Approved,
            _ => ApprovalStatus::Drifted,
        },
    }
}

/// What an approve pass decides to do for one expectation under a chosen
/// [`ApproveMode`] and CI flag — the unit the tool op interprets.
#[derive(Debug, Clone, PartialEq)]
pub enum ApprovalDecision {
    /// Promote this golden (a local, human-gated approval).
    Write {
        /// The status that selected the spec.
        status: ApprovalStatus,
        /// The golden to write.
        golden: Golden,
    },
    /// Not selected by the chosen mode (already approved, unobserved, or the
    /// wrong kind of change for this mode).
    Skipped {
        /// The unselected status.
        status: ApprovalStatus,
    },
    /// Selected, but refused because CI is set: approve NEVER writes in CI, so an
    /// unapproved drift — or a brand-new baseline — is always a hard failure
    /// there. A green golden is only ever minted locally by observe + approve.
    RefusedInCi {
        /// The status that would have been written outside CI.
        status: ApprovalStatus,
    },
}

/// Decide what an approve pass should do for one expectation.
///
/// Classifies the spec ([`approval_status`]), applies the [`ApproveMode`]
/// selection, then enforces the CI gate: under `ci`, a *selected* spec is
/// [`ApprovalDecision::RefusedInCi`] (strict first-run — a `new` expectation can
/// never be baselined in CI, and a drift is never silently re-approved there);
/// otherwise it builds the golden to write.
///
/// The CI flag is **injected**, never read from the ambient environment, so this
/// policy is deterministic to test.
///
/// # Errors
///
/// Returns [`ApproveError`] when the spec is selected for a write but a
/// deterministic criterion fails to compile against its observation.
pub fn decide_approval(
    spec: &Expectation,
    golden: Option<&Golden>,
    received: Option<&Observation>,
    mode: ApproveMode,
    grading: GradingPins,
    ci: bool,
    scrubbers: &ScrubberSet,
) -> Result<ApprovalDecision, ApproveError> {
    let status = approval_status(golden, received, scrubbers);
    if !mode.selects(status) {
        return Ok(ApprovalDecision::Skipped { status });
    }
    if ci {
        return Ok(ApprovalDecision::RefusedInCi { status });
    }
    // `selects` is true only for New/Drifted, both of which carry a received
    // observation (Unobserved and Approved are never selected).
    let received = received.expect("a selected expectation carries a received observation");
    let golden = approve(spec, received, grading, golden, scrubbers)?;
    Ok(ApprovalDecision::Write { status, golden })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assertion::compile;
    use crate::spec::Criterion;
    use crate::types::{Checkpoint, Evidence, Trajectory};
    use serde_json::json;
    use std::time::Duration;
    use tempfile::TempDir;

    /// The coupon spec identity reused across the ledger fixtures.
    const COUPON: &str = "src/checkout/coupon";

    /// A single-checkpoint JSON observation for [`COUPON`] carrying `body`.
    fn json_observation(body: serde_json::Value) -> Observation {
        Observation {
            path: COUPON.to_string(),
            checkpoints: vec![Checkpoint {
                after: "final".to_string(),
                state: SurfaceState::Json { body },
                duration: Duration::from_millis(1),
            }],
            trajectory: Trajectory { steps: Vec::new() },
        }
    }

    /// A single-checkpoint cli observation for [`COUPON`] with `stdout`.
    fn cli_observation(stdout: &str) -> Observation {
        Observation {
            path: COUPON.to_string(),
            checkpoints: vec![Checkpoint {
                after: "final".to_string(),
                state: SurfaceState::Cli(CliState {
                    stdout: stdout.to_string(),
                    stderr: String::new(),
                    exit_code: Some(0),
                    files: std::collections::BTreeMap::new(),
                }),
                duration: Duration::from_millis(1),
            }],
            trajectory: Trajectory { steps: Vec::new() },
        }
    }

    /// Compile `text` against `observation` — assertions are produced by the real
    /// compiler, never hand-built, so the frozen-replay path is end-to-end.
    fn assertion_for(text: &str, observation: &Observation) -> CompiledAssertion {
        compile(
            &Criterion {
                text: text.to_string(),
                checked: false,
            },
            observation,
        )
        .expect("criterion compiles")
    }

    /// A golden over `observation` freezing `assertions`, with default grading.
    fn golden(observation: Observation, assertions: Vec<CompiledAssertion>) -> Golden {
        Golden {
            observation,
            assertions,
            grading: GradingPins::from_config(&ExpectConfig::default()),
            // These fixtures exercise the `compare` path, which never reads the
            // spec hash; the stale-detection hash is exercised by the
            // `ledger_state` fixtures, which build goldens through `approve`.
            spec_hash: String::new(),
        }
    }

    #[test]
    fn golden_round_trips_through_its_mirrored_path() {
        let repo = TempDir::new().unwrap();
        let observation = json_observation(json!({ "total": 40 }));
        let assertion = assertion_for("the total is $40", &observation);
        let golden = golden(observation, vec![assertion]);

        let written = write_golden(repo.path(), &golden).expect("write golden");
        assert_eq!(
            written,
            repo.path()
                .join(".expect/goldens/src/checkout/coupon.golden.json"),
            "the golden mirrors the spec's repo-relative identity",
        );

        let loaded = read_golden(repo.path(), COUPON)
            .expect("read golden")
            .expect("golden present");
        assert_eq!(loaded, golden, "the golden round-trips byte-for-byte");
    }

    #[test]
    fn read_golden_returns_none_when_no_golden_is_approved() {
        let repo = TempDir::new().unwrap();
        let loaded = read_golden(repo.path(), COUPON).expect("read absent golden");
        assert!(loaded.is_none(), "an unapproved golden reads as None");
    }

    #[test]
    fn grading_pins_capture_the_config_model_embedder_and_threshold() {
        let config = ExpectConfig::default();
        let pins = GradingPins::from_config(&config);
        assert_eq!(pins.model, config.model.default);
        assert_eq!(pins.embedder, config.embedder.model);
        assert_eq!(
            pins.similarity_threshold,
            config.embedder.similarity_threshold
        );
    }

    #[test]
    fn scrubbers_normalize_every_volatile_kind_so_two_runs_compare_equal() {
        // Two runs differing only in volatile content: a timestamp, a UUID, a
        // run-id ULID, a loopback port, and a temp path.
        let mut first = cli_observation(
            "run 01ARZ3NDEKTSV4RRFFQ69G5FAV started 2026-06-26T14:44:30.076793Z \
             id 550e8400-e29b-41d4-a716-446655440000 on localhost:8080 wrote /tmp/build-abc123/out",
        );
        let mut second = cli_observation(
            "run 01BX5ZZKBKACTAV9WEVGEMMVRZ started 2027-12-31T23:59:59Z \
             id 6ba7b810-9dad-11d1-80b4-00c04fd430c8 on localhost:54321 wrote /tmp/build-zzz999/out",
        );
        // Wall-clock timing is volatile too: differ it across the runs to prove
        // `scrub_observation` normalizes the duration, not just the strings.
        first.checkpoints[0].duration = Duration::from_millis(17);
        second.checkpoints[0].duration = Duration::from_millis(983);

        let scrubbers = ScrubberSet::default_set();
        assert_eq!(
            scrubbers.scrub_observation(&first),
            scrubbers.scrub_observation(&second),
            "two runs differing only in volatile content scrub to the same observation",
        );
    }

    #[test]
    fn scrubbing_preserves_a_real_non_volatile_change() {
        // Same volatile envelope, a genuinely different value: scrubbing must not
        // mask it.
        let stable = "at 2026-06-26T14:44:30Z total=40";
        let changed = "at 2026-06-26T14:44:30Z total=50";
        let scrubbers = ScrubberSet::default_set();
        assert_ne!(
            scrubbers.scrub_observation(&cli_observation(stable)),
            scrubbers.scrub_observation(&cli_observation(changed)),
            "a real value change survives scrubbing",
        );
    }

    #[test]
    fn scrub_observation_is_idempotent() {
        let observation = cli_observation("started 2026-06-26T14:44:30Z on localhost:8080");
        let scrubbers = ScrubberSet::default_set();
        let once = scrubbers.scrub_observation(&observation);
        let twice = scrubbers.scrub_observation(&once);
        assert_eq!(once, twice, "placeholders are not themselves scrubbed");
    }

    #[test]
    fn scrub_observation_leaves_the_identity_untouched() {
        // The path is the stable address, never volatile content — even when it
        // happens to look scrubable, it must be preserved verbatim.
        let observation = json_observation(json!({ "total": 40 }));
        let scrubbed = ScrubberSet::default_set().scrub_observation(&observation);
        assert_eq!(scrubbed.path, COUPON);
    }

    #[test]
    fn tier1_compare_reports_an_unchanged_value_as_approved() {
        let baseline = json_observation(json!({ "total": 40 }));
        let assertion = assertion_for("the total is $40", &baseline);
        let golden = golden(baseline.clone(), vec![assertion]);

        let comparison = compare(&golden, &baseline, &ScrubberSet::default_set());

        assert_eq!(comparison.path, COUPON);
        assert_eq!(comparison.state, LedgerState::Approved);
        assert_eq!(comparison.criteria.len(), 1);
        assert!(!comparison.criteria[0].drifted);
        assert_eq!(comparison.criteria[0].tier, VerdictTier::Deterministic);
    }

    #[test]
    fn tier1_compare_flags_a_changed_matched_value_as_drift() {
        let baseline = json_observation(json!({ "total": 40 }));
        let assertion = assertion_for("the total is $40", &baseline);
        let golden = golden(baseline, vec![assertion]);

        // The received run's matched value changed: 40 → 50.
        let received = json_observation(json!({ "total": 50 }));
        let comparison = compare(&golden, &received, &ScrubberSet::default_set());

        assert_eq!(comparison.state, LedgerState::Drifted);
        assert!(comparison.criteria[0].drifted);
        // The verdict is re-derived on both sides, not read from storage.
        assert!(comparison.criteria[0].golden.pass);
        assert!(!comparison.criteria[0].received.pass);
    }

    #[test]
    fn compare_re_derives_a_clean_verdict_on_both_sides() {
        // `compare` never reads a stored verdict — it re-grades both observations
        // with the golden's frozen assertions, so the golden side passes and the
        // received side reflects the run, with no verdict persisted anywhere.
        let baseline =
            json_observation(json!({ "total": 40, "item_count": 3, "items": [{}, {}, {}] }));
        let assertions = vec![
            assertion_for("the total is $40", &baseline),
            assertion_for("the item count equals the number of items", &baseline),
        ];
        let golden = golden(baseline.clone(), assertions);

        let comparison = compare(&golden, &baseline, &ScrubberSet::default_set());

        assert_eq!(comparison.state, LedgerState::Approved);
        assert_eq!(comparison.criteria.len(), 2);
        assert!(comparison
            .criteria
            .iter()
            .all(|criterion| criterion.golden.pass && criterion.received.pass));
    }

    #[test]
    fn deterministic_drift_flags_a_flipped_pass() {
        let golden = CriterionVerdict {
            criterion: "the total is $40".to_string(),
            tier: VerdictTier::Deterministic,
            pass: true,
            score: Some(1.0),
            evidence: vec![Evidence {
                locator: "$.total".to_string(),
                snippet: "40".to_string(),
            }],
            reason: String::new(),
            confidence: None,
        };
        let mut received = golden.clone();
        received.pass = false;
        received.evidence[0].snippet = "50".to_string();
        assert!(deterministic_drift(&golden, &received));
        assert!(!deterministic_drift(&golden, &golden));
    }

    #[test]
    fn tolerance_drift_stub_flags_a_score_outside_the_band() {
        let base = CriterionVerdict {
            criterion: "the latency is acceptable".to_string(),
            tier: VerdictTier::Tolerance,
            pass: true,
            score: Some(0.90),
            evidence: Vec::new(),
            reason: String::new(),
            confidence: None,
        };
        let mut moved = base.clone();
        moved.score = Some(0.50);
        assert!(
            tolerance_drift(&base, &moved),
            "a score that left the band drifts"
        );
        assert!(
            !tolerance_drift(&base, &base),
            "an identical score stays within the band",
        );
    }

    // -----------------------------------------------------------------------
    // Approve: freeze, diff, select, gate.
    // -----------------------------------------------------------------------

    /// An expectation at [`COUPON`] carrying the given Tier-1 `criteria` — the
    /// minimal spec the approve path needs (only `path` and `criteria` are read).
    fn spec(criteria: &[&str]) -> Expectation {
        use crate::spec::{Frontmatter, Isolation, ReliabilityPolicy};
        use crate::types::Surface;
        Expectation {
            path: COUPON.to_string(),
            frontmatter: Frontmatter {
                description: "a coupon reduces the total".to_string(),
                surface: Surface::Cli,
                model: None,
                reliability: ReliabilityPolicy::default(),
                repeat: None,
                tiers: vec![VerdictTier::Deterministic],
                similarity_threshold: None,
                timeout: Duration::from_secs(60),
                tags: Vec::new(),
                setup: None,
                isolation: Isolation::Shared,
            },
            intent: String::new(),
            criteria: criteria
                .iter()
                .map(|text| Criterion {
                    text: text.to_string(),
                    checked: false,
                })
                .collect(),
            given: Vec::new(),
            when: Vec::new(),
            notes: None,
        }
    }

    /// The default grading pins reused across the approve fixtures.
    fn pins() -> GradingPins {
        GradingPins::from_config(&ExpectConfig::default())
    }

    #[test]
    fn approve_writes_a_scrubbed_golden_with_frozen_assertions() {
        // The received run carries volatile content (a timestamp) alongside the
        // real value; approve must store the scrubbed observation and freeze a
        // real compiled assertion against it.
        let spec = spec(&["the total is $40"]);
        let received = cli_observation("at 2026-06-26T14:44:30Z total is $40");

        let golden = approve(&spec, &received, pins(), None, &ScrubberSet::default_set())
            .expect("approve compiles and freezes");

        // The stored observation is scrubbed (the timestamp is normalized away).
        let SurfaceState::Cli(cli) = &golden.observation.checkpoints[0].state else {
            panic!("cli state");
        };
        assert!(
            cli.stdout.contains(TIMESTAMP_PLACEHOLDER),
            "the stored observation is scrubbed: {}",
            cli.stdout
        );

        // One frozen assertion, and it binds + passes against the stored golden —
        // exactly the self-verifying replay `compare` relies on.
        assert_eq!(golden.assertions.len(), 1);
        let verdict = evaluate(&golden.observation, &golden.assertions);
        assert!(
            verdict.criteria[0].pass,
            "the frozen assertion holds against the observation it was compiled from",
        );
    }

    #[test]
    fn approve_writes_the_golden_to_its_mirrored_path() {
        let repo = TempDir::new().unwrap();
        let spec = spec(&["the total is $40"]);
        let received = json_observation(json!({ "total": 40 }));

        let golden =
            approve(&spec, &received, pins(), None, &ScrubberSet::default_set()).expect("approve");
        write_golden(repo.path(), &golden).expect("write golden");

        let loaded = read_golden(repo.path(), COUPON)
            .expect("read golden")
            .expect("golden present");
        assert_eq!(loaded, golden, "the approved golden round-trips on disk");
    }

    #[test]
    fn approval_diff_shows_the_binding_not_just_the_value() {
        let spec = spec(&["the total is $40"]);
        let received = json_observation(json!({ "total": 40 }));
        let golden =
            approve(&spec, &received, pins(), None, &ScrubberSet::default_set()).expect("approve");

        let diff = approval_diff(&golden);

        assert_eq!(diff.len(), 1);
        let binding = &diff[0];
        assert_eq!(binding.criterion, "the total is $40");
        assert_eq!(binding.locator, "$.total");
        assert_eq!(binding.value, "40");
        assert!(!binding.hand_edited, "a fresh compile is not hand-edited");
        // The rendered binding carries the locator, so a mis-compiled locator is
        // visible at review — not just the value.
        assert_eq!(binding.render(), format!("40{BINDING_ARROW}$.total"));
        assert!(binding.render().contains("$.total"));
    }

    #[test]
    fn approve_rejects_a_hallucinated_locator_before_writing() {
        // The criterion names a value the observation does not carry: its locator
        // cannot bind/pass, so approve refuses — no hallucinated locator reaches
        // the golden.
        let spec = spec(&["the total is $999"]);
        let received = json_observation(json!({ "total": 40 }));

        let error = approve(&spec, &received, pins(), None, &ScrubberSet::default_set())
            .expect_err("approve must reject the hallucinated locator");

        assert!(
            matches!(error, ApproveError::Compile { ref path, .. } if path == COUPON),
            "got {error:?}",
        );
    }

    #[test]
    fn approve_preserves_a_reviewer_hand_edit_bound_to_unchanged_prose() {
        // A prior golden whose locator a reviewer hand-edited to a different (but
        // still-binding) field. Re-approving with the same prose preserves it.
        let spec = spec(&["the total is $40"]);
        let observation = json_observation(json!({ "total": 40, "grand_total": 40 }));
        let prior_golden = approve(
            &spec,
            &observation,
            pins(),
            None,
            &ScrubberSet::default_set(),
        )
        .expect("initial approve");
        let mut hand_edited = prior_golden.clone();
        hand_edited.assertions[0].locator = crate::assertion::Locator::JsonPath {
            path: "$.grand_total".to_string(),
        };

        let reapproved = approve(
            &spec,
            &observation,
            pins(),
            Some(&hand_edited),
            &ScrubberSet::default_set(),
        )
        .expect("re-approve");

        assert_eq!(
            reapproved.assertions[0].locator,
            crate::assertion::Locator::JsonPath {
                path: "$.grand_total".to_string()
            },
            "the reviewer's hand-edited locator survives a same-prose re-approval",
        );
        assert!(
            approval_diff(&reapproved)[0].hand_edited,
            "the preserved locator is flagged as hand-edited in the diff",
        );
    }

    #[test]
    fn changing_the_prose_discards_a_hand_edit_and_recompiles() {
        let original = spec(&["the total is $40"]);
        let observation = json_observation(json!({ "total": 40, "grand_total": 40 }));
        let prior = approve(
            &original,
            &observation,
            pins(),
            None,
            &ScrubberSet::default_set(),
        )
        .expect("initial approve");
        let mut hand_edited = prior.clone();
        hand_edited.assertions[0].locator = crate::assertion::Locator::JsonPath {
            path: "$.grand_total".to_string(),
        };

        // The criterion prose changed: the hand-edit no longer matches, so the
        // criterion is recompiled fresh (back to the durable `$.total`).
        let edited = spec(&["the total is now $40"]);
        let reapproved = approve(
            &edited,
            &observation,
            pins(),
            Some(&hand_edited),
            &ScrubberSet::default_set(),
        )
        .expect("re-approve edited prose");

        assert_eq!(
            reapproved.assertions[0].locator,
            crate::assertion::Locator::JsonPath {
                path: "$.total".to_string()
            },
            "editing the prose discards the hand-edit and recompiles",
        );
    }

    /// The selection table from the task: each mode selects exactly its subset of
    /// statuses. Parameterized so the `--missing`/`--changed`/`--all` partition is
    /// asserted against one source of truth.
    #[test]
    fn approve_modes_select_the_right_subset_of_statuses() {
        use ApprovalStatus::{Approved, Drifted, New, Unobserved};
        use ApproveMode::{All, Changed, Missing};
        let cases = [
            (Missing, New, true),
            (Missing, Drifted, false),
            (Missing, Approved, false),
            (Missing, Unobserved, false),
            (Changed, New, false),
            (Changed, Drifted, true),
            (Changed, Approved, false),
            (Changed, Unobserved, false),
            (All, New, true),
            (All, Drifted, true),
            (All, Approved, false),
            (All, Unobserved, false),
        ];
        for (mode, status, expected) in cases {
            assert_eq!(
                mode.selects(status),
                expected,
                "{mode:?} selecting {status:?}",
            );
        }
    }

    #[test]
    fn approval_status_classifies_new_drifted_approved_and_unobserved() {
        let scrubbers = ScrubberSet::default_set();
        let spec = spec(&["the total is $40"]);
        let baseline = json_observation(json!({ "total": 40 }));
        let golden = approve(&spec, &baseline, pins(), None, &scrubbers).expect("approve");

        // No received run at all.
        assert_eq!(
            approval_status(Some(&golden), None, &scrubbers),
            ApprovalStatus::Unobserved,
        );
        // No golden yet.
        assert_eq!(
            approval_status(None, Some(&baseline), &scrubbers),
            ApprovalStatus::New,
        );
        // Golden + matching received.
        assert_eq!(
            approval_status(Some(&golden), Some(&baseline), &scrubbers),
            ApprovalStatus::Approved,
        );
        // Golden + drifted received.
        let drifted = json_observation(json!({ "total": 50 }));
        assert_eq!(
            approval_status(Some(&golden), Some(&drifted), &scrubbers),
            ApprovalStatus::Drifted,
        );
    }

    #[test]
    fn decide_approval_writes_a_new_baseline_locally() {
        let scrubbers = ScrubberSet::default_set();
        let spec = spec(&["the total is $40"]);
        let received = json_observation(json!({ "total": 40 }));

        let decision = decide_approval(
            &spec,
            None,
            Some(&received),
            ApproveMode::Missing,
            pins(),
            false, // not CI: a local first run mints the baseline
            &scrubbers,
        )
        .expect("decide");

        assert!(
            matches!(
                decision,
                ApprovalDecision::Write {
                    status: ApprovalStatus::New,
                    ..
                }
            ),
            "got {decision:?}",
        );
    }

    #[test]
    fn ci_refuses_to_write_a_drift_a_hard_failure() {
        // A drifted spec under CI is never silently re-approved — approve refuses.
        let scrubbers = ScrubberSet::default_set();
        let spec = spec(&["the total is $40"]);
        let baseline = json_observation(json!({ "total": 40 }));
        let golden = approve(&spec, &baseline, pins(), None, &scrubbers).expect("approve");
        let drifted = json_observation(json!({ "total": 50 }));

        let decision = decide_approval(
            &spec,
            Some(&golden),
            Some(&drifted),
            ApproveMode::Changed,
            pins(),
            true, // CI
            &scrubbers,
        )
        .expect("decide");

        assert_eq!(
            decision,
            ApprovalDecision::RefusedInCi {
                status: ApprovalStatus::Drifted
            },
            "CI must refuse to write an unapproved drift",
        );
    }

    #[test]
    fn strict_first_run_a_new_expectation_cannot_be_baselined_in_ci() {
        // The load-bearing strict-first-run guard: a `new` expectation under CI is
        // refused, so a green baseline is never minted in CI.
        let scrubbers = ScrubberSet::default_set();
        let spec = spec(&["the total is $40"]);
        let received = json_observation(json!({ "total": 40 }));

        for mode in [ApproveMode::Missing, ApproveMode::All] {
            let decision = decide_approval(
                &spec,
                None,
                Some(&received),
                mode,
                pins(),
                true, // CI
                &scrubbers,
            )
            .expect("decide");
            assert_eq!(
                decision,
                ApprovalDecision::RefusedInCi {
                    status: ApprovalStatus::New
                },
                "{mode:?}: a new expectation must not be baselined in CI",
            );
        }
    }

    // -----------------------------------------------------------------------
    // The spec hash + the per-expectation ledger state classifier.
    // -----------------------------------------------------------------------

    #[test]
    fn spec_hash_is_stable_for_identical_criteria() {
        assert_eq!(
            spec_hash(&spec(&["the total is $40", "the discount is $5"])),
            spec_hash(&spec(&["the total is $40", "the discount is $5"])),
            "the same criteria must hash identically",
        );
    }

    #[test]
    fn spec_hash_changes_when_a_criterion_is_edited() {
        assert_ne!(
            spec_hash(&spec(&["the total is $40"])),
            spec_hash(&spec(&["the total is $50"])),
            "an edited criterion must change the spec hash",
        );
    }

    #[test]
    fn spec_hash_is_not_fooled_by_a_criterion_boundary_shift() {
        // Without length-prefixing, ["ab","c"] and ["a","bc"] would concatenate
        // identically; domain separation must keep them distinct.
        assert_ne!(
            spec_hash(&spec(&["ab", "c"])),
            spec_hash(&spec(&["a", "bc"])),
            "a boundary shift between adjacent criteria must change the hash",
        );
    }

    #[test]
    fn ledger_state_is_new_when_no_golden_exists() {
        let received = json_observation(json!({ "total": 40 }));
        assert_eq!(
            ledger_state(
                &spec(&["the total is $40"]),
                None,
                Some(&received),
                &ScrubberSet::default_set(),
            ),
            LedgerState::New,
        );
    }

    #[test]
    fn ledger_state_is_approved_when_the_received_verdict_matches_the_golden() {
        let scrubbers = ScrubberSet::default_set();
        let spec = spec(&["the total is $40"]);
        let baseline = json_observation(json!({ "total": 40 }));
        let golden = approve(&spec, &baseline, pins(), None, &scrubbers).expect("approve");
        assert_eq!(
            ledger_state(&spec, Some(&golden), Some(&baseline), &scrubbers),
            LedgerState::Approved,
        );
    }

    #[test]
    fn ledger_state_is_approved_when_no_new_run_has_been_observed() {
        let scrubbers = ScrubberSet::default_set();
        let spec = spec(&["the total is $40"]);
        let baseline = json_observation(json!({ "total": 40 }));
        let golden = approve(&spec, &baseline, pins(), None, &scrubbers).expect("approve");
        assert_eq!(
            ledger_state(&spec, Some(&golden), None, &scrubbers),
            LedgerState::Approved,
            "a golden with an unedited spec and no new run is the last approved state",
        );
    }

    #[test]
    fn ledger_state_is_drifted_when_the_received_verdict_changed() {
        let scrubbers = ScrubberSet::default_set();
        let spec = spec(&["the total is $40"]);
        let baseline = json_observation(json!({ "total": 40 }));
        let golden = approve(&spec, &baseline, pins(), None, &scrubbers).expect("approve");
        let drifted = json_observation(json!({ "total": 50 }));
        assert_eq!(
            ledger_state(&spec, Some(&golden), Some(&drifted), &scrubbers),
            LedgerState::Drifted,
        );
    }

    #[test]
    fn ledger_state_is_stale_when_the_spec_was_edited_after_approval() {
        let scrubbers = ScrubberSet::default_set();
        let original = spec(&["the total is $40"]);
        let baseline = json_observation(json!({ "total": 40 }));
        let golden = approve(&original, &baseline, pins(), None, &scrubbers).expect("approve");

        // The `*.expect.md` gained a criterion since approval: its hash no longer
        // matches the one frozen in the golden, so it is stale even though the
        // received run still matches the golden.
        let edited = spec(&["the total is $40", "the discount is $5"]);
        assert_eq!(
            ledger_state(&edited, Some(&golden), Some(&baseline), &scrubbers),
            LedgerState::Stale,
        );
    }

    #[test]
    fn ledger_state_stale_outranks_drift() {
        // An edited spec whose received run also drifted is reported as stale: the
        // golden's frozen assertions are out of date, so re-approval — not
        // drift-triage — is the right action.
        let scrubbers = ScrubberSet::default_set();
        let original = spec(&["the total is $40"]);
        let baseline = json_observation(json!({ "total": 40 }));
        let golden = approve(&original, &baseline, pins(), None, &scrubbers).expect("approve");

        let edited = spec(&["the total is $40", "the discount is $5"]);
        let drifted = json_observation(json!({ "total": 50 }));
        assert_eq!(
            ledger_state(&edited, Some(&golden), Some(&drifted), &scrubbers),
            LedgerState::Stale,
        );
    }

    #[test]
    fn ledger_entry_carries_old_vs_new_evidence_only_when_drifted() {
        let scrubbers = ScrubberSet::default_set();
        let spec = spec(&["the total is $40"]);
        let baseline = json_observation(json!({ "total": 40 }));
        let golden = approve(&spec, &baseline, pins(), None, &scrubbers).expect("approve");

        let approved = ledger_entry(&spec, Some(&golden), Some(&baseline), &scrubbers);
        assert_eq!(approved.state, LedgerState::Approved);
        assert!(
            approved.comparison.is_none(),
            "an approved entry carries no old-vs-new comparison",
        );

        let drifted_obs = json_observation(json!({ "total": 50 }));
        let drifted = ledger_entry(&spec, Some(&golden), Some(&drifted_obs), &scrubbers);
        assert_eq!(drifted.state, LedgerState::Drifted);
        let comparison = drifted
            .comparison
            .expect("a drifted entry carries old-vs-new evidence");
        assert!(comparison.criteria[0].drifted);
        // The verdict is re-derived on both sides, not read from storage.
        assert!(comparison.criteria[0].golden.pass);
        assert!(!comparison.criteria[0].received.pass);
    }

    #[test]
    fn ledger_queue_orders_unapproved_drift_first() {
        // Entries arrive in a non-drift-first order; the queue must surface the
        // drifted ones first and preserve the relative order otherwise.
        let entry = |path: &str, state: LedgerState| LedgerEntry {
            path: path.to_string(),
            state,
            comparison: None,
        };
        let queue = ledger_queue(vec![
            entry("approved", LedgerState::Approved),
            entry("drifted", LedgerState::Drifted),
            entry("new", LedgerState::New),
        ]);
        assert_eq!(
            queue.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(),
            vec!["drifted", "approved", "new"],
            "drift leads the queue; the rest keep their incoming order",
        );
    }

    #[test]
    fn decide_approval_skips_an_already_approved_spec() {
        let scrubbers = ScrubberSet::default_set();
        let spec = spec(&["the total is $40"]);
        let baseline = json_observation(json!({ "total": 40 }));
        let golden = approve(&spec, &baseline, pins(), None, &scrubbers).expect("approve");

        let decision = decide_approval(
            &spec,
            Some(&golden),
            Some(&baseline),
            ApproveMode::All,
            pins(),
            false,
            &scrubbers,
        )
        .expect("decide");

        assert_eq!(
            decision,
            ApprovalDecision::Skipped {
                status: ApprovalStatus::Approved
            },
            "an already-approved spec is nothing to do, even under --all",
        );
    }
}
