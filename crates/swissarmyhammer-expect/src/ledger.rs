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

use crate::assertion::CompiledAssertion;
use crate::config::ExpectConfig;
use crate::error::ExpectError;
use crate::evaluate::evaluate;
use crate::observe::golden_path;
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
}
