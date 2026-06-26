//! The pure domain model for the `expect` engine.
//!
//! These are the data structures from `ideas/expect.md` §"The Verdict Ladder":
//! the authoritative [`Observation`] of a run, the per-criterion and
//! per-expectation verdicts derived from it, and the closed enumerations that
//! describe surfaces, verdict tiers, and ledger state. Everything here is pure
//! data — no IO, no system access, no agent — so the engine layers above can
//! build, serialize, and reason over it freely.
//!
//! Every type round-trips through `serde_json`: an [`Observation`] is what
//! `observe` writes to `received/`, a scrubbed one is the committed golden, and
//! the verdict types are what `evaluate` produces. The closed enums serialize to
//! the lowercase string forms used throughout the spec (`"cli"`,
//! `"deterministic"`, `"drifted"`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

/// How `expect` perceives and acts on the system under test.
///
/// The closed set of six adapters from the frontmatter `surface` key. There is
/// no `custom` escape hatch — an unknown surface fails the parser loudly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Surface {
    /// Command-line: run argv, observe stdout/stderr/exit/files.
    Cli,
    /// Service/API: issue a request, observe status/headers/body.
    Http,
    /// Web UI via the DOM accessibility tree.
    Browser,
    /// Native desktop via the OS accessibility API.
    Gui,
    /// Filesystem state.
    File,
    /// Database state.
    Db,
}

/// Which layer of the verdict ladder decided a criterion.
///
/// The author never picks a tier; the cheapest faithful one wins at compile
/// time (see the spec's "How `evaluate` turns prose into a check").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerdictTier {
    /// Tier 1 — exact / regex / schema / exit-code / file-state. Free, never flaky.
    Deterministic,
    /// Tier 2 — embedding cosine / numeric tolerance / Levenshtein. Cheap, stable.
    Tolerance,
    /// Tier 3 — rubric grade against the stated intent by the expectation's model.
    Judgment,
}

/// The outcome of a single criterion.
///
/// The closed runtime enumeration "verdict per criterion" from the spec.
/// [`CriterionVerdict`] carries a boolean `pass`; this enum names the richer set
/// of outcomes (including the ones that are neither a clean pass nor a clean
/// fail) that the surrounding workflow routes on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CriterionStatus {
    /// The criterion held.
    Pass,
    /// The criterion did not hold.
    Fail,
    /// The criterion could not be evaluated.
    Error,
    /// Low confidence — routed to a human.
    Escalated,
}

/// The drift-ledger state of an expectation relative to its golden.
///
/// The closed runtime enumeration "ledger state per expectation" from the spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LedgerState {
    /// Matches the golden within tolerance.
    Approved,
    /// The verdict changed and is awaiting human approval.
    Drifted,
    /// No golden yet.
    New,
    /// The expectation was edited since its golden was approved.
    Stale,
}

/// An adapter's authoritative read of the system under test at one checkpoint.
///
/// Each surface reads ground truth differently (stdout/exit/files for cli; a
/// JSON body for http; rows for db; an a11y tree for browser/gui). The model
/// starts with a concrete [`CliState`] variant plus a generic [`Json`] variant
/// that holds any structured body until the remaining adapters land.
///
/// [`Json`]: SurfaceState::Json
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SurfaceState {
    /// The authoritative read of a CLI run.
    Cli(CliState),
    /// A generic structured body — the room left for the json/db/a11y adapters.
    Json {
        /// The structured state, as an arbitrary JSON value.
        body: serde_json::Value,
    },
}

/// The authoritative read of a CLI run: its streams, exit code, and any files
/// the adapter captured.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CliState {
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
    /// The process exit code, or `None` if it was terminated by a signal.
    pub exit_code: Option<i32>,
    /// Captured file state, keyed by path (sorted for stable serialization).
    pub files: BTreeMap<String, String>,
}

/// One authoritative snapshot in an observation's timeline.
///
/// The adapter captures state (and timing) after *each* `When` step plus a
/// final, because real criteria are multi-step, relational, and temporal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    /// The `When` step this snapshot follows, or `"final"`.
    pub after: String,
    /// The adapter's authoritative read at this point.
    pub state: SurfaceState,
    /// Wall-clock time to reach this checkpoint, serialized as explicit
    /// milliseconds under the `duration_ms` key.
    #[serde(rename = "duration_ms", with = "duration_ms")]
    pub duration: Duration,
}

/// What the driver did, recorded for `observation get`.
///
/// This is the `received` transcript — useful for a human triaging a run, but
/// **never the verdict source**: the verdict is derived from the [`Checkpoint`]
/// timeline, not from what the driver claims it did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trajectory {
    /// The ordered actions the driver took.
    pub steps: Vec<String>,
}

/// One authoritative capture of a run: the checkpoint timeline plus the driver
/// trajectory. Addressed by its expectation's repo-relative path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    /// Repo-relative path of the spec — its identity.
    pub path: String,
    /// One checkpoint per `When` step plus a final — the authoritative timeline.
    pub checkpoints: Vec<Checkpoint>,
    /// What the driver did, for `observation get` — never the verdict source.
    pub trajectory: Trajectory,
}

/// The slice of an observation that justifies a criterion's verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// Where in the checkpoint state the value lives (a per-surface locator).
    pub locator: String,
    /// The actual captured content the verdict rests on.
    pub snippet: String,
}

/// The structured verdict for a single criterion.
///
/// Never a bare boolean — sparse pass/fail is too weak to drive the next agent
/// edit, so the tier, score, evidence, reasoning, and confidence travel with it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CriterionVerdict {
    /// The criterion text being judged.
    pub criterion: String,
    /// Which layer of the ladder decided it.
    pub tier: VerdictTier,
    /// Whether the criterion held.
    pub pass: bool,
    /// Continuous score, for tolerance bands / the judge.
    pub score: Option<f32>,
    /// The slice(s) of the observation that justify the call.
    pub evidence: Vec<Evidence>,
    /// Why — especially the judge's reasoning.
    pub reason: String,
    /// Grader confidence, for the human-escalation queue.
    pub confidence: Option<f32>,
}

/// The verdict for a whole expectation: its per-criterion verdicts plus the
/// `pass^k` reliability result across repeated observations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpectationVerdict {
    /// Repo-relative path of the spec — its identity.
    pub path: String,
    /// The per-criterion verdicts, in `Then`-checklist order.
    pub criteria: Vec<CriterionVerdict>,
    /// The `pass^k` result across repeated observations.
    pub reliability: Reliability,
}

/// The `pass^k` reliability result: how many runs were required to pass, and the
/// per-run pass/fail spread.
///
/// `pass^k` is the headline metric (not average pass rate), and the per-run
/// spread is kept so a 2-of-3 flake is visible rather than hidden behind an
/// average.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reliability {
    /// `k` in `pass^k` — the number of runs that must all pass.
    pub required: u32,
    /// The pass/fail outcome of each run, in order.
    pub runs: Vec<bool>,
}

impl Reliability {
    /// Whether `pass^k` is satisfied: at least `required` runs were recorded and
    /// every one of them passed.
    pub fn satisfied(&self) -> bool {
        self.runs.len() as u32 >= self.required && self.runs.iter().all(|&passed| passed)
    }
}

/// Serialize a [`Duration`] as an integer count of milliseconds.
///
/// `std::time::Duration`'s default serde form is a `{secs, nanos}` struct; the
/// `expect` wire format uses an explicit millisecond integer instead, so the
/// golden/received JSON reads `"duration_ms": 1500`.
mod duration_ms {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_millis() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Round-trip a value through serde_json and assert it equals the original.
    fn round_trip<T>(value: &T) -> T
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        let json = serde_json::to_string(value).expect("serialize");
        serde_json::from_str(&json).expect("deserialize")
    }

    #[test]
    fn surface_serializes_to_lowercase_strings() {
        for (variant, wire) in [
            (Surface::Cli, "\"cli\""),
            (Surface::Http, "\"http\""),
            (Surface::Browser, "\"browser\""),
            (Surface::Gui, "\"gui\""),
            (Surface::File, "\"file\""),
            (Surface::Db, "\"db\""),
        ] {
            assert_eq!(serde_json::to_string(&variant).unwrap(), wire);
            assert_eq!(round_trip(&variant), variant);
        }
    }

    #[test]
    fn verdict_tier_serializes_to_lowercase_strings() {
        for (variant, wire) in [
            (VerdictTier::Deterministic, "\"deterministic\""),
            (VerdictTier::Tolerance, "\"tolerance\""),
            (VerdictTier::Judgment, "\"judgment\""),
        ] {
            assert_eq!(serde_json::to_string(&variant).unwrap(), wire);
            assert_eq!(round_trip(&variant), variant);
        }
    }

    #[test]
    fn criterion_status_serializes_to_lowercase_strings() {
        for (variant, wire) in [
            (CriterionStatus::Pass, "\"pass\""),
            (CriterionStatus::Fail, "\"fail\""),
            (CriterionStatus::Error, "\"error\""),
            (CriterionStatus::Escalated, "\"escalated\""),
        ] {
            assert_eq!(serde_json::to_string(&variant).unwrap(), wire);
            assert_eq!(round_trip(&variant), variant);
        }
    }

    #[test]
    fn ledger_state_serializes_to_lowercase_strings() {
        for (variant, wire) in [
            (LedgerState::Approved, "\"approved\""),
            (LedgerState::Drifted, "\"drifted\""),
            (LedgerState::New, "\"new\""),
            (LedgerState::Stale, "\"stale\""),
        ] {
            assert_eq!(serde_json::to_string(&variant).unwrap(), wire);
            assert_eq!(round_trip(&variant), variant);
        }
    }

    #[test]
    fn checkpoint_duration_serializes_as_explicit_milliseconds() {
        let checkpoint = Checkpoint {
            after: "final".to_string(),
            state: SurfaceState::Json {
                body: serde_json::json!({"total": 40}),
            },
            duration: Duration::from_millis(1500),
        };
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&checkpoint).unwrap()).unwrap();
        assert_eq!(json["duration_ms"], serde_json::json!(1500));
        assert_eq!(round_trip(&checkpoint), checkpoint);
    }

    #[test]
    fn surface_state_cli_variant_round_trips() {
        let state = SurfaceState::Cli(CliState {
            stdout: "Total: $40\n".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            files: std::collections::BTreeMap::from([("out.txt".to_string(), "ok".to_string())]),
        });
        assert_eq!(round_trip(&state), state);
    }

    #[test]
    fn observation_round_trips() {
        let observation = Observation {
            path: "src/checkout/coupon".to_string(),
            checkpoints: vec![Checkpoint {
                after: "apply SAVE10".to_string(),
                state: SurfaceState::Cli(CliState {
                    stdout: "Total: $40\n".to_string(),
                    stderr: String::new(),
                    exit_code: Some(0),
                    files: std::collections::BTreeMap::new(),
                }),
                duration: Duration::from_millis(120),
            }],
            trajectory: Trajectory {
                steps: vec!["ran: checkout --apply SAVE10".to_string()],
            },
        };
        assert_eq!(round_trip(&observation), observation);
    }

    #[test]
    fn expectation_verdict_round_trips() {
        let verdict = ExpectationVerdict {
            path: "src/checkout/coupon".to_string(),
            criteria: vec![CriterionVerdict {
                criterion: "After the first apply, the total is $40".to_string(),
                tier: VerdictTier::Deterministic,
                pass: true,
                score: Some(1.0),
                evidence: vec![Evidence {
                    locator: "$.total".to_string(),
                    snippet: "40".to_string(),
                }],
                reason: "exact match".to_string(),
                confidence: None,
            }],
            reliability: Reliability {
                required: 3,
                runs: vec![true, true, true],
            },
        };
        assert_eq!(round_trip(&verdict), verdict);
    }

    #[test]
    fn reliability_satisfied_requires_all_required_runs_to_pass() {
        assert!(Reliability {
            required: 3,
            runs: vec![true, true, true],
        }
        .satisfied());

        // A flake: 2 of 3 passed — not satisfied.
        assert!(!Reliability {
            required: 3,
            runs: vec![true, false, true],
        }
        .satisfied());

        // Too few runs to establish pass^3.
        assert!(!Reliability {
            required: 3,
            runs: vec![true, true],
        }
        .satisfied());
    }
}
