//! The pure `evaluate` step: replay compiled assertions over an observation.
//!
//! Per `ideas/expect.md` §"The Verdict Ladder", [`evaluate`] is the pure function
//! `(observation, criteria) -> verdict`: it **touches no system and consults no
//! model**, so it is re-runnable for free. It does not re-interpret a `Then` line
//! every run — it *replays* a set of already-compiled [`CompiledAssertion`]s (the
//! frozen assertions the golden carries), so a verdict over a stored observation
//! is apples-to-apples with the one that produced the golden.
//!
//! This is Tier 1 ([`VerdictTier::Deterministic`]): exact / regex / numeric /
//! exit-code / file-state, resolved through the assertion's locator and op (the
//! [`CompiledAssertion::evaluate`] replay engine). Each criterion yields a
//! structured [`CriterionVerdict`] — never a bare boolean — carrying the evidence
//! slice and a reason so the result can drive the next agent edit.
//!
//! A locator that **no longer binds** (or a checkpoint that is gone) is
//! *structural drift*, a verdict distinct from a plain value mismatch: it is
//! surfaced loudly via a [`STRUCTURAL_DRIFT_REASON`]-prefixed reason and a `None`
//! score (a value mismatch scores `0.0`), so the workflow can route drift to
//! re-approval rather than treat it as a code regression.
//!
//! [`evaluate`] is the headline pure replay. [`evaluate_spec`] is the wiring
//! convenience the `observation evaluate` / `golden evaluate` ops use today: until
//! the golden ledger freezes a compiled-assertion set alongside the approved
//! observation, it compiles each criterion fresh against the source observation,
//! then replays through [`evaluate`].

use crate::assertion::{
    compile, AssertionOutcome, BoundValue, CompileError, CompiledAssertion, Locator,
};
use crate::config::ExpectConfig;
use crate::grader::{JudgmentAssertion, JudgmentContext};
use crate::spec::{Criterion, Expectation};
use crate::types::{
    CriterionVerdict, Evidence, ExpectationVerdict, Observation, Reliability, VerdictTier,
};
use serde::{Deserialize, Serialize};

/// The score a clean Tier 1 pass carries (an exact match is total confidence).
const PASS_SCORE: f32 = 1.0;

/// The score a Tier 1 value mismatch carries — a *clean fail*, distinct from the
/// `None` score a structural-drift verdict carries.
const FAIL_SCORE: f32 = 0.0;

/// The `pass^k` requirement a single-observation [`evaluate`] reports: one run.
/// Reliability across repeated observations is composed by a higher layer.
const SINGLE_RUN_REQUIRED: u32 = 1;

/// The loud prefix that marks a structural-drift reason — a locator that stopped
/// binding, or a checkpoint that vanished. Distinguishes drift from a plain value
/// mismatch in the reason text; the `None` score is the structural counterpart.
pub const STRUCTURAL_DRIFT_REASON: &str = "structural drift";

/// The loud prefix that marks a Tier 2 *tolerance drift* reason — a value whose
/// closeness score left the golden's band. Distinct from a Tier 1 hard fail
/// (which carries [`FAIL_SCORE`]) and from [`STRUCTURAL_DRIFT_REASON`] (a `None`
/// score): a tolerance drift keeps its computed `score`, because the value still
/// bound — it simply moved out of band, so the workflow routes it to re-approval
/// rather than treating it as a code regression.
pub const TOLERANCE_DRIFT_REASON: &str = "tolerance drift";

/// Replay `criteria` over `observation`, producing the structured
/// [`ExpectationVerdict`].
///
/// Pure and re-runnable: it drives no system and calls no model, replaying each
/// already-[`compile`]d assertion through [`CompiledAssertion::evaluate`]. Every
/// criterion becomes one [`CriterionVerdict`]; the overall single-run outcome
/// (all criteria pass) is recorded as the [`Reliability`] of this one run.
pub fn evaluate(observation: &Observation, criteria: &[CompiledAssertion]) -> ExpectationVerdict {
    let criteria: Vec<CriterionVerdict> = criteria
        .iter()
        .map(|assertion| evaluate_assertion(assertion, observation))
        .collect();
    let overall = criteria.iter().all(|verdict| verdict.pass);
    ExpectationVerdict {
        path: observation.path.clone(),
        criteria,
        reliability: Reliability {
            required: SINGLE_RUN_REQUIRED,
            runs: vec![overall],
        },
    }
}

/// Replay one compiled `assertion` over `observation`, mapping the
/// [`AssertionOutcome`] to a structured [`CriterionVerdict`].
///
/// A `Holds` outcome is a pass (score [`PASS_SCORE`]); a `Violated` outcome is a
/// clean value-mismatch fail (score [`FAIL_SCORE`]); a `Drifted` or
/// `CheckpointMissing` outcome is *structural drift* — a non-pass with a `None`
/// score and a [`STRUCTURAL_DRIFT_REASON`]-prefixed reason, surfaced loudly.
pub fn evaluate_assertion(
    assertion: &CompiledAssertion,
    observation: &Observation,
) -> CriterionVerdict {
    match assertion.evaluate(observation) {
        AssertionOutcome::Holds => pass_verdict(assertion, observation),
        AssertionOutcome::Violated { found, expected } => fail_verdict(assertion, found, expected),
        AssertionOutcome::Drifted { locator } => drift_verdict(assertion, &locator),
        AssertionOutcome::CheckpointMissing { index } => missing_verdict(assertion, index),
    }
}

/// Compile each of `spec`'s criteria against `observation` and replay them,
/// producing the [`ExpectationVerdict`] for the `observation evaluate` /
/// `golden evaluate` ops.
///
/// A wiring convenience, not the frozen-replay path: until the golden ledger
/// (a later task) stores a compiled-assertion set alongside the approved
/// observation, the ops have nothing pre-compiled to replay, so this compiles
/// each criterion fresh against the source observation, then replays through
/// [`evaluate`]. The mapping of a criterion the compiler cannot turn into a Tier 1
/// assertion:
///
/// - [`CompileError::Unrecognized`] — the prose carries no deterministic
///   assertion at all (a Tier 2/3 criterion). It is **skipped**: a later tier
///   grades it.
/// - any other [`CompileError`] — the criterion *is* deterministic but does not
///   bind or hold against this observation. It surfaces as a non-pass
///   [`CriterionVerdict`] (never silently dropped), so an edited criterion that
///   no longer matches the baseline is loud.
pub fn evaluate_spec(spec: &Expectation, observation: &Observation) -> ExpectationVerdict {
    let criteria: Vec<CriterionVerdict> = spec
        .criteria
        .iter()
        .filter_map(|criterion| grade_criterion(criterion, observation))
        .collect();
    let overall = criteria.iter().all(|verdict| verdict.pass);
    ExpectationVerdict {
        path: observation.path.clone(),
        criteria,
        reliability: Reliability {
            required: SINGLE_RUN_REQUIRED,
            runs: vec![overall],
        },
    }
}

/// Grade one criterion against `observation`: compile it fresh, then replay.
///
/// Returns `None` for a criterion that carries no Tier 1 assertion
/// ([`CompileError::Unrecognized`]) so it is left for a later tier; returns a
/// non-pass verdict for a deterministic criterion that fails to bind or hold.
fn grade_criterion(criterion: &Criterion, observation: &Observation) -> Option<CriterionVerdict> {
    match compile(criterion, observation) {
        Ok(assertion) => Some(evaluate_assertion(&assertion, observation)),
        Err(CompileError::Unrecognized { .. }) => None,
        Err(error) => Some(compile_failure_verdict(criterion, &error)),
    }
}

/// Build the passing verdict for a [`AssertionOutcome::Holds`], resolving the
/// located value once more for the evidence snippet.
fn pass_verdict(assertion: &CompiledAssertion, observation: &Observation) -> CriterionVerdict {
    let locator = assertion.locator.to_string();
    let snippet = located_snippet(assertion, observation).unwrap_or_default();
    CriterionVerdict {
        criterion: assertion.criterion_text.clone(),
        tier: assertion.tier,
        pass: true,
        score: Some(PASS_SCORE),
        evidence: vec![Evidence {
            locator: locator.clone(),
            snippet: snippet.clone(),
        }],
        reason: format!("`{locator}` resolves to {snippet}, as expected"),
        confidence: None,
    }
}

/// Build the value-mismatch verdict for a [`AssertionOutcome::Violated`].
fn fail_verdict(
    assertion: &CompiledAssertion,
    found: BoundValue,
    expected: BoundValue,
) -> CriterionVerdict {
    let locator = assertion.locator.to_string();
    CriterionVerdict {
        criterion: assertion.criterion_text.clone(),
        tier: assertion.tier,
        pass: false,
        score: Some(FAIL_SCORE),
        evidence: vec![Evidence {
            locator: locator.clone(),
            snippet: found.to_string(),
        }],
        reason: format!("expected {expected} at `{locator}`, found {found}"),
        confidence: None,
    }
}

/// Build the structural-drift verdict for a [`AssertionOutcome::Drifted`]: the
/// `locator` no longer binds, so the value cannot be read at all.
fn drift_verdict(assertion: &CompiledAssertion, locator: &str) -> CriterionVerdict {
    CriterionVerdict {
        criterion: assertion.criterion_text.clone(),
        tier: assertion.tier,
        pass: false,
        score: None,
        evidence: vec![Evidence {
            locator: locator.to_string(),
            snippet: String::new(),
        }],
        reason: format!("{STRUCTURAL_DRIFT_REASON}: locator `{locator}` no longer binds"),
        confidence: None,
    }
}

/// Build the structural-drift verdict for a [`AssertionOutcome::CheckpointMissing`]:
/// the checkpoint the assertion reads is absent from the observation's timeline.
fn missing_verdict(assertion: &CompiledAssertion, index: usize) -> CriterionVerdict {
    CriterionVerdict {
        criterion: assertion.criterion_text.clone(),
        tier: assertion.tier,
        pass: false,
        score: None,
        evidence: Vec::new(),
        reason: format!(
            "{STRUCTURAL_DRIFT_REASON}: checkpoint {index} is absent from the observation"
        ),
        confidence: None,
    }
}

/// Build the non-pass verdict for a criterion the compiler could not turn into a
/// binding Tier 1 assertion against the source observation.
fn compile_failure_verdict(criterion: &Criterion, error: &CompileError) -> CriterionVerdict {
    CriterionVerdict {
        criterion: criterion.text.clone(),
        tier: VerdictTier::Deterministic,
        pass: false,
        score: None,
        evidence: Vec::new(),
        reason: format!("criterion does not hold against the observation: {error}"),
        confidence: None,
    }
}

/// Resolve the located value of `assertion` against `observation` as a string for
/// the evidence snippet, or `None` if it does not bind.
fn located_snippet(assertion: &CompiledAssertion, observation: &Observation) -> Option<String> {
    let state = &observation.checkpoints.get(assertion.checkpoint)?.state;
    assertion
        .locator
        .resolve(state)
        .map(|value| value.to_string())
}

// ---------------------------------------------------------------------------
// Tier 2 — tolerance bands (embedding cosine / numeric / Levenshtein).
// ---------------------------------------------------------------------------

/// The embedding seam Tier 2 depends on, kept abstract so the evaluate layer
/// never hard-couples to a concrete embedding model.
///
/// Production wiring adapts the platform embedder (the
/// `EmbedderFactory`/`TextEmbedder` that `review` loads via
/// `default_embedder_factory`) to this trait; tests pass a deterministic stub so
/// Tier 2 resolution runs with no GPU and no 600 MB model load. The pinned
/// embedding model is itself recorded in the golden's
/// [`GradingPins`](crate::GradingPins) for reproducibility — changing it is a
/// reviewed re-baseline, never a silent boundary shift.
pub trait TextEmbedder {
    /// Embed `text` into a dense vector for cosine comparison.
    ///
    /// Two semantically-equivalent strings should map to vectors with high
    /// cosine similarity; unrelated strings to near-orthogonal vectors.
    fn embed(&self, text: &str) -> Vec<f32>;
}

/// A Tier 2 tolerance band: how a residual criterion's value is compared to its
/// frozen golden anchor when Tier 1 could not decide it.
///
/// The author never picks a band; the compiler selects the cheapest faithful one
/// (per `ideas/expect.md` §"How `evaluate` turns prose into a check"). Each
/// variant carries its own cutoff so the decision boundary is frozen into the
/// golden and stays reproducible.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "band", rename_all = "snake_case")]
pub enum ToleranceBand {
    /// Embedding cosine similarity to the anchor must be `>= threshold` (the
    /// pinned-embedder semantic band, default `0.80`). Catches a reworded but
    /// equivalent value an exact match would reject.
    Semantic {
        /// The cosine cutoff at or above which the values count as equivalent.
        threshold: f32,
    },
    /// The absolute numeric difference from the anchor must be `<= tolerance`.
    Numeric {
        /// The largest absolute deviation from the anchor still in band.
        tolerance: f64,
    },
    /// The normalized Levenshtein similarity to the anchor must be `>= threshold`
    /// — a near-string band for incidental edits (whitespace, a renamed token).
    NearString {
        /// The edit-distance similarity cutoff at or above which the strings
        /// count as the same.
        threshold: f32,
    },
}

/// A residual (Tier 1-undecidable) criterion compiled into a Tier 2 tolerance
/// check: a [`Locator`] into the received observation, the frozen golden
/// `anchor`, and the [`ToleranceBand`] that decides closeness.
///
/// Mirrors [`CompiledAssertion`] for Tier 1: it is *replayed* (never recompiled)
/// against a later observation by [`ToleranceAssertion::resolve`], and the
/// `criterion_text` keeps it bound to the prose it was compiled from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToleranceAssertion {
    /// Index into [`Observation::checkpoints`] this assertion reads.
    pub checkpoint: usize,
    /// Where in the checkpoint's state the received value lives.
    pub locator: Locator,
    /// The approved golden value the received value is compared against.
    pub anchor: BoundValue,
    /// The tolerance band that decides whether the received value is in range.
    pub band: ToleranceBand,
    /// The criterion prose this assertion is bound to.
    pub criterion_text: String,
}

impl ToleranceAssertion {
    /// Replay this Tier 2 assertion against `observation`, comparing the located
    /// value to the frozen [`anchor`](Self::anchor) within the
    /// [`band`](Self::band) using `embedder` for the semantic band.
    ///
    /// - The checkpoint is gone or the locator no longer binds ⇒ *structural
    ///   drift*: a non-pass with a `None` score and a [`STRUCTURAL_DRIFT_REASON`]
    ///   reason (identical to Tier 1's structural signal).
    /// - The value binds and stays in band ⇒ a pass carrying the closeness score.
    /// - The value binds but its score left the band ⇒ *tolerance drift*: a
    ///   non-pass that **keeps** its score and carries a [`TOLERANCE_DRIFT_REASON`]
    ///   reason, so it routes to re-approval rather than reading as a hard fail.
    pub fn resolve(
        &self,
        observation: &Observation,
        embedder: &dyn TextEmbedder,
    ) -> CriterionVerdict {
        let locator = self.locator.to_string();
        let Some(checkpoint) = observation.checkpoints.get(self.checkpoint) else {
            return self.structural_drift(
                &locator,
                format!(
                    "{STRUCTURAL_DRIFT_REASON}: checkpoint {} is absent from the observation",
                    self.checkpoint
                ),
            );
        };
        let Some(found) = self.locator.resolve(&checkpoint.state) else {
            return self.structural_drift(
                &locator,
                format!("{STRUCTURAL_DRIFT_REASON}: locator `{locator}` no longer binds"),
            );
        };

        let assessment = self.band.assess(&self.anchor, &found, embedder);
        let reason = if assessment.pass {
            format!(
                "`{locator}` holds within the Tier 2 tolerance band ({})",
                assessment.detail
            )
        } else {
            format!(
                "{TOLERANCE_DRIFT_REASON}: `{locator}` left the tolerance band ({})",
                assessment.detail
            )
        };
        CriterionVerdict {
            criterion: self.criterion_text.clone(),
            tier: VerdictTier::Tolerance,
            pass: assessment.pass,
            score: Some(assessment.score),
            evidence: vec![Evidence {
                locator,
                snippet: found.to_string(),
            }],
            reason,
            confidence: None,
        }
    }

    /// Build the structural-drift verdict shared by the checkpoint-missing and
    /// locator-unbound cases: a non-pass with a `None` score (the structural
    /// signal, distinct from a tolerance score) and the `reason` the caller
    /// already composed.
    fn structural_drift(&self, locator: &str, reason: String) -> CriterionVerdict {
        CriterionVerdict {
            criterion: self.criterion_text.clone(),
            tier: VerdictTier::Tolerance,
            pass: false,
            score: None,
            evidence: vec![Evidence {
                locator: locator.to_string(),
                snippet: String::new(),
            }],
            reason,
            confidence: None,
        }
    }
}

/// The closeness assessment of a located value against an anchor for one band.
struct BandAssessment {
    /// The closeness score in `[0.0, 1.0]` recorded on the verdict.
    score: f32,
    /// Whether the value is within band.
    pass: bool,
    /// A human description of the comparison, woven into the verdict reason.
    detail: String,
}

impl ToleranceBand {
    /// Assess `found` against `anchor` under this band, using `embedder` only for
    /// the [`Semantic`](ToleranceBand::Semantic) band.
    fn assess(
        &self,
        anchor: &BoundValue,
        found: &BoundValue,
        embedder: &dyn TextEmbedder,
    ) -> BandAssessment {
        match self {
            ToleranceBand::Semantic { threshold } => {
                let anchor_vec = embedder.embed(&anchor.to_string());
                let found_vec = embedder.embed(&found.to_string());
                let score = cosine_similarity(&anchor_vec, &found_vec).clamp(0.0, 1.0);
                BandAssessment {
                    score,
                    pass: score >= *threshold,
                    detail: format!("cosine {score:.2} vs threshold {threshold:.2}"),
                }
            }
            ToleranceBand::NearString { threshold } => {
                let score = levenshtein_similarity(&anchor.to_string(), &found.to_string());
                BandAssessment {
                    score,
                    pass: score >= *threshold,
                    detail: format!("edit similarity {score:.2} vs threshold {threshold:.2}"),
                }
            }
            ToleranceBand::Numeric { tolerance } => numeric_assessment(anchor, found, *tolerance),
        }
    }
}

/// Assess a numeric `found` value against a numeric `anchor` within an absolute
/// `tolerance`.
///
/// A non-numeric value on either side is itself out of band — the value changed
/// shape, not just magnitude — and scores `0.0`. The recorded score is the
/// in-band closeness `1 - |diff| / tolerance` clamped to `[0.0, 1.0]`, so an
/// exact match scores `1.0` and the band edge `0.0`.
fn numeric_assessment(anchor: &BoundValue, found: &BoundValue, tolerance: f64) -> BandAssessment {
    let (BoundValue::Number(anchor), BoundValue::Number(found)) = (anchor, found) else {
        return BandAssessment {
            score: 0.0,
            pass: false,
            detail: format!("{found} is not numeric"),
        };
    };
    let diff = (found - anchor).abs();
    let score = if tolerance > 0.0 {
        (1.0 - (diff / tolerance)).clamp(0.0, 1.0) as f32
    } else if diff == 0.0 {
        1.0
    } else {
        0.0
    };
    BandAssessment {
        score,
        pass: diff <= tolerance,
        detail: format!("|{found} - {anchor}| = {diff} vs tolerance {tolerance}"),
    }
}

/// The cosine similarity of two equal-length vectors, in `[-1.0, 1.0]`; `0.0`
/// when either vector has zero magnitude (no direction to compare).
///
/// `pub(crate)` so the Tier 3 [`grader`](crate::grader) module reuses the same
/// anchor-similarity math the Tier 2 semantic band uses, rather than re-deriving it.
pub(crate) fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    let dot: f32 = (0..len).map(|i| a[i] * b[i]).sum();
    let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

/// The normalized Levenshtein similarity of two strings: `1.0 - distance /
/// max_len`, in `[0.0, 1.0]`; `1.0` for two empty strings.
fn levenshtein_similarity(a: &str, b: &str) -> f32 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let max_len = a.len().max(b.len());
    if max_len == 0 {
        return 1.0;
    }
    1.0 - (levenshtein_distance(&a, &b) as f32 / max_len as f32)
}

/// The Levenshtein edit distance between two char slices (classic two-row DP).
fn levenshtein_distance(a: &[char], b: &[char]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// The effective Tier 2 cosine threshold for `spec`: its per-expectation
/// `similarity_threshold` override when set, else the repo `[embedder]` cutoff in
/// `config` (default `0.80`).
pub fn similarity_threshold(spec: &Expectation, config: &ExpectConfig) -> f32 {
    spec.frontmatter
        .similarity_threshold
        .unwrap_or(config.embedder.similarity_threshold)
}

/// The full verdict of a tiered evaluation: the per-criterion
/// [`ExpectationVerdict`] plus the criteria the ladder could not resolve with
/// enough confidence to auto-decide, routed to a human [escalation queue](Escalation).
///
/// The escalation queue is a distinct output from the verdict because an escalated
/// criterion is **never auto-passed** — it is a non-pass awaiting a human, not a
/// silent green.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TieredVerdict {
    /// The per-criterion verdicts across every tier, in `Then`-checklist order.
    pub verdict: ExpectationVerdict,
    /// The criteria a grader resolved below the confidence floor, surfaced for a
    /// human rather than auto-decided. Empty when every criterion cleared the floor.
    pub escalations: Vec<Escalation>,
}

/// A criterion the ladder resolved with too little confidence to auto-decide,
/// routed to the human escalation queue instead of being auto-passed.
///
/// Carries the grader's confidence and reason so a human triaging the queue sees
/// *why* it was escalated (a low single-grader confidence, a split panel, or a
/// grader that was the driver).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Escalation {
    /// The criterion text routed for human review.
    pub criterion: String,
    /// The grader confidence that fell below the escalation floor.
    pub confidence: f32,
    /// The verdict reason explaining the escalation.
    pub reason: String,
}

/// Resolve `tier1` (deterministic), then `tier2` (tolerance), then `tier3`
/// (judgment) over `observation`, each tier running **only** on the residual the
/// cheaper tiers could not decide.
///
/// The full verdict ladder: Tier 1 decides what it can and its verdicts come
/// first; the residual it could not compile is handed to Tier 2 (the rung that
/// consults `embedder` for its semantic band); and the residual *of that* is handed
/// to Tier 3, the [`grader`](crate::grader) rung, which gates the model behind
/// anchor similarity and consults the `judgment` context's panel only on
/// divergence. The single-run [`Reliability`] passes only when every criterion
/// across all three tiers passes, and any criterion a grader resolved below
/// [`JudgmentContext::escalate_below_confidence`] is collected into the
/// [escalation queue](TieredVerdict::escalations) rather than auto-passed.
pub fn evaluate_tiered(
    observation: &Observation,
    tier1: &[CompiledAssertion],
    tier2: &[ToleranceAssertion],
    tier3: &[JudgmentAssertion],
    embedder: &dyn TextEmbedder,
    judgment: &JudgmentContext,
) -> TieredVerdict {
    let mut criteria: Vec<CriterionVerdict> = tier1
        .iter()
        .map(|assertion| evaluate_assertion(assertion, observation))
        .collect();
    criteria.extend(
        tier2
            .iter()
            .map(|assertion| assertion.resolve(observation, embedder)),
    );
    criteria.extend(
        tier3
            .iter()
            .map(|assertion| assertion.resolve(observation, embedder, judgment)),
    );
    let overall = criteria.iter().all(|verdict| verdict.pass);
    let escalations = criteria
        .iter()
        .filter_map(|verdict| escalation_for(verdict, judgment.escalate_below_confidence))
        .collect();
    TieredVerdict {
        verdict: ExpectationVerdict {
            path: observation.path.clone(),
            criteria,
            reliability: Reliability {
                required: SINGLE_RUN_REQUIRED,
                runs: vec![overall],
            },
        },
        escalations,
    }
}

/// Route `verdict` to the escalation queue when a grader resolved it with
/// confidence below `floor` — surfaced for a human, never auto-passed.
///
/// A criterion with no grader confidence (Tier 1/2, or a Tier 3 anchor-match pass)
/// carries `None` and is never escalated.
fn escalation_for(verdict: &CriterionVerdict, floor: f32) -> Option<Escalation> {
    let confidence = verdict.confidence?;
    (confidence < floor).then(|| Escalation {
        criterion: verdict.criterion.clone(),
        confidence,
        reason: verdict.reason.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grader::{Grade, GradeRequest, Grader};
    use crate::types::{Checkpoint, CliState, SurfaceState, Trajectory};
    use serde_json::{json, Value};
    use std::collections::BTreeMap;
    use std::time::Duration;

    /// A JSON-bodied checkpoint at `after` carrying `body`.
    fn json_checkpoint(after: &str, body: Value) -> Checkpoint {
        Checkpoint {
            after: after.to_string(),
            state: SurfaceState::Json { body },
            duration: Duration::from_millis(1),
        }
    }

    /// A cli checkpoint at `after` with `stdout` and exit code `exit`.
    fn cli_checkpoint(after: &str, stdout: &str, exit: i32) -> Checkpoint {
        Checkpoint {
            after: after.to_string(),
            state: SurfaceState::Cli(CliState {
                stdout: stdout.to_string(),
                stderr: String::new(),
                exit_code: Some(exit),
                files: BTreeMap::new(),
            }),
            duration: Duration::from_millis(1),
        }
    }

    /// An observation over `checkpoints` for the coupon spec identity.
    fn observation(checkpoints: Vec<Checkpoint>) -> Observation {
        Observation {
            path: "src/checkout/coupon".to_string(),
            checkpoints,
            trajectory: Trajectory { steps: Vec::new() },
        }
    }

    /// An unchecked criterion from `text`.
    fn criterion(text: &str) -> Criterion {
        Criterion {
            text: text.to_string(),
            checked: false,
        }
    }

    /// Compile `text` against `obs` — the assertions under test are produced by
    /// the real compiler, never hand-built, so the replay path is end-to-end.
    fn assertion_for(text: &str, obs: &Observation) -> CompiledAssertion {
        compile(&criterion(text), obs).expect("criterion compiles")
    }

    /// The sole verdict produced by evaluating a single assertion over `obs`.
    fn sole_verdict(assertion: &CompiledAssertion, obs: &Observation) -> CriterionVerdict {
        let verdict = evaluate(obs, std::slice::from_ref(assertion));
        assert_eq!(
            verdict.criteria.len(),
            1,
            "one criterion in, one verdict out"
        );
        verdict.criteria.into_iter().next().unwrap()
    }

    #[test]
    fn a_passing_literal_yields_a_pass_with_evidence_and_reason() {
        let obs = observation(vec![json_checkpoint("final", json!({ "total": 40 }))]);
        let assertion = assertion_for("the total is $40", &obs);

        let verdict = sole_verdict(&assertion, &obs);

        assert!(verdict.pass);
        assert_eq!(verdict.tier, VerdictTier::Deterministic);
        assert_eq!(verdict.score, Some(PASS_SCORE));
        assert_eq!(verdict.criterion, "the total is $40");
        assert_eq!(
            verdict.evidence,
            vec![Evidence {
                locator: "$.total".to_string(),
                snippet: "40".to_string(),
            }]
        );
        assert!(verdict.reason.contains("$.total"));
        assert!(verdict.reason.contains("40"));
    }

    #[test]
    fn a_value_mismatch_yields_a_clean_fail_distinct_from_drift() {
        // Compile against total=40, then replay against total=50: the locator
        // still binds but the value disagrees — a clean fail, not drift.
        let source = observation(vec![json_checkpoint("final", json!({ "total": 40 }))]);
        let assertion = assertion_for("the total is $40", &source);
        let changed = observation(vec![json_checkpoint("final", json!({ "total": 50 }))]);

        let verdict = sole_verdict(&assertion, &changed);

        assert!(!verdict.pass);
        // A clean value fail scores 0.0 — the structural counterpart (drift) is None.
        assert_eq!(verdict.score, Some(FAIL_SCORE));
        assert!(!verdict.reason.starts_with(STRUCTURAL_DRIFT_REASON));
        assert!(verdict.reason.contains("found 50"));
        assert!(verdict.reason.contains("expected 40"));
        assert_eq!(verdict.evidence[0].snippet, "50");
    }

    #[test]
    fn a_locator_that_stops_binding_yields_a_structural_drift_verdict() {
        // Compile against total=40, then replay against an observation where the
        // field was renamed — the locator no longer binds: drift, not a fail.
        let source = observation(vec![json_checkpoint("final", json!({ "total": 40 }))]);
        let assertion = assertion_for("the total is $40", &source);
        let drifted = observation(vec![json_checkpoint("final", json!({ "sum": 40 }))]);

        let verdict = sole_verdict(&assertion, &drifted);

        assert!(!verdict.pass);
        // Drift is structurally distinct from a value fail: None score + loud reason.
        assert_eq!(verdict.score, None);
        assert!(verdict.reason.starts_with(STRUCTURAL_DRIFT_REASON));
        assert!(verdict.reason.contains("$.total"));
        assert_eq!(verdict.evidence[0].locator, "$.total");
    }

    #[test]
    fn a_missing_checkpoint_yields_a_structural_drift_verdict() {
        // Compile an ordinal-bound assertion at the "second" checkpoint (index 2),
        // then replay against a shorter timeline — the checkpoint is gone: drift.
        let source = observation(vec![
            json_checkpoint("initial cart", json!({ "total": 50 })),
            json_checkpoint("apply", json!({ "total": 45 })),
            json_checkpoint("apply again", json!({ "total": 40 })),
        ]);
        let assertion = assertion_for("after the second apply, the total is $40", &source);
        assert_eq!(
            assertion.checkpoint, 2,
            "assertion binds the second-apply checkpoint"
        );
        let truncated = observation(vec![json_checkpoint(
            "initial cart",
            json!({ "total": 50 }),
        )]);

        let verdict = sole_verdict(&assertion, &truncated);

        assert!(!verdict.pass);
        assert_eq!(verdict.score, None);
        assert!(verdict.reason.starts_with(STRUCTURAL_DRIFT_REASON));
        assert!(verdict.reason.contains('2'));
    }

    #[test]
    fn an_exit_code_assertion_evaluates_deterministically() {
        let passing = observation(vec![cli_checkpoint("final", "done\n", 0)]);
        let assertion = assertion_for("the command exits with code 0", &passing);

        assert!(sole_verdict(&assertion, &passing).pass);

        // Replay against a non-zero exit: a clean value fail.
        let failing = observation(vec![cli_checkpoint("final", "boom\n", 1)]);
        let verdict = sole_verdict(&assertion, &failing);
        assert!(!verdict.pass);
        assert_eq!(verdict.score, Some(FAIL_SCORE));
    }

    #[test]
    fn a_stream_regex_assertion_evaluates_against_plain_text() {
        let source = observation(vec![cli_checkpoint("final", "Total: $40\n", 0)]);
        let assertion = assertion_for("the total is $40", &source);

        assert!(sole_verdict(&assertion, &source).pass);

        // The captured value changed in the same stream shape: a clean fail.
        let changed = observation(vec![cli_checkpoint("final", "Total: $50\n", 0)]);
        let verdict = sole_verdict(&assertion, &changed);
        assert!(!verdict.pass);
        assert_eq!(verdict.score, Some(FAIL_SCORE));
    }

    #[test]
    fn evaluate_is_pure_and_re_runnable_on_an_in_memory_fixture() {
        // The fixture is built entirely in memory — no SUT, no files, no process,
        // no model. Evaluating it twice yields byte-identical verdicts, proving
        // the function touches nothing external and is re-runnable for free.
        let obs = observation(vec![json_checkpoint(
            "final",
            json!({ "item_count": 3, "items": [{}, {}, {}] }),
        )]);
        let assertion = assertion_for("the item count equals the number of items", &obs);

        let first = evaluate(&obs, std::slice::from_ref(&assertion));
        let second = evaluate(&obs, std::slice::from_ref(&assertion));

        assert_eq!(first, second, "evaluate is deterministic and re-runnable");
        assert!(first.criteria[0].pass);
    }

    #[test]
    fn the_expectation_verdict_carries_the_path_and_single_run_reliability() {
        let obs = observation(vec![json_checkpoint("final", json!({ "total": 40 }))]);
        let assertion = assertion_for("the total is $40", &obs);

        let verdict = evaluate(&obs, std::slice::from_ref(&assertion));

        assert_eq!(verdict.path, "src/checkout/coupon");
        assert_eq!(verdict.reliability.required, SINGLE_RUN_REQUIRED);
        assert_eq!(verdict.reliability.runs, vec![true]);
        assert!(verdict.reliability.satisfied());
    }

    #[test]
    fn evaluate_spec_compiles_then_replays_every_recognized_criterion() {
        let obs = observation(vec![json_checkpoint(
            "final",
            json!({ "total": 40, "item_count": 3, "items": [{}, {}, {}] }),
        )]);
        let spec = spec_with_criteria(&[
            "the total is $40",
            "the item count equals the number of items",
        ]);

        let verdict = evaluate_spec(&spec, &obs);

        assert_eq!(verdict.criteria.len(), 2);
        assert!(verdict.criteria.iter().all(|c| c.pass));
        assert!(verdict.reliability.satisfied());
    }

    #[test]
    fn evaluate_spec_skips_a_non_tier1_criterion() {
        let obs = observation(vec![json_checkpoint("final", json!({ "total": 40 }))]);
        // The second criterion carries no deterministic assertion — it is left for
        // a later tier, not failed.
        let spec = spec_with_criteria(&["the total is $40", "the experience feels delightful"]);

        let verdict = evaluate_spec(&spec, &obs);

        assert_eq!(verdict.criteria.len(), 1, "the Tier-3 criterion is skipped");
        assert_eq!(verdict.criteria[0].criterion, "the total is $40");
    }

    #[test]
    fn evaluate_spec_surfaces_an_edited_criterion_that_no_longer_holds() {
        // An edited criterion asserts a value the observation does not carry: it is
        // deterministic but cannot bind/hold, so it surfaces as a non-pass verdict
        // rather than being silently dropped.
        let obs = observation(vec![json_checkpoint("final", json!({ "total": 40 }))]);
        let spec = spec_with_criteria(&["the total is $999"]);

        let verdict = evaluate_spec(&spec, &obs);

        assert_eq!(verdict.criteria.len(), 1);
        assert!(!verdict.criteria[0].pass);
        assert!(!verdict.reliability.satisfied());
    }

    /// An expectation carrying `criteria` and nothing else of interest, for the
    /// `evaluate_spec` tests.
    fn spec_with_criteria(criteria: &[&str]) -> Expectation {
        use crate::spec::{Frontmatter, Isolation, ReliabilityPolicy};
        use crate::types::{Surface, VerdictTier};
        Expectation {
            path: "src/checkout/coupon".to_string(),
            frontmatter: Frontmatter {
                description: "a coupon reduces the total".to_string(),
                surface: Surface::Cli,
                model: None,
                reliability: ReliabilityPolicy::default(),
                repeat: None,
                tiers: vec![
                    VerdictTier::Deterministic,
                    VerdictTier::Tolerance,
                    VerdictTier::Judgment,
                ],
                similarity_threshold: None,
                timeout: Duration::from_secs(60),
                tags: Vec::new(),
                setup: None,
                isolation: Isolation::Shared,
            },
            intent: String::new(),
            criteria: criteria.iter().map(|t| criterion(t)).collect(),
            given: Vec::new(),
            when: Vec::new(),
            notes: None,
        }
    }

    // -----------------------------------------------------------------------
    // Tier 2 — tolerance bands.
    // -----------------------------------------------------------------------

    use std::cell::Cell;
    use std::collections::HashMap;

    /// A deterministic stub embedder: it maps registered strings to fixed vectors
    /// (unknown text → the zero vector) and counts every `embed` call, so a test
    /// can both control cosine similarity and assert the embedder was (or was
    /// not) consulted — no GPU, no model load.
    struct StubEmbedder {
        vectors: HashMap<String, Vec<f32>>,
        calls: Cell<usize>,
    }

    impl StubEmbedder {
        fn new(pairs: &[(&str, &[f32])]) -> Self {
            StubEmbedder {
                vectors: pairs
                    .iter()
                    .map(|(text, vector)| (text.to_string(), vector.to_vec()))
                    .collect(),
                calls: Cell::new(0),
            }
        }

        fn calls(&self) -> usize {
            self.calls.get()
        }
    }

    impl TextEmbedder for StubEmbedder {
        fn embed(&self, text: &str) -> Vec<f32> {
            self.calls.set(self.calls.get() + 1);
            self.vectors
                .get(text)
                .cloned()
                .unwrap_or_else(|| vec![0.0, 0.0])
        }
    }

    /// An embedder that must never be consulted — any call is a test failure. The
    /// Tier 1 path takes no embedder, so this proves a deterministic criterion is
    /// decided without touching the Tier 2 seam.
    struct PanicEmbedder;

    impl TextEmbedder for PanicEmbedder {
        fn embed(&self, _text: &str) -> Vec<f32> {
            panic!("Tier 1 must not consult the embedder");
        }
    }

    /// A grader that must never be consulted — any call is a test failure. Proves
    /// the residual handed to Tier 3 is only what Tiers 1-2 could not decide.
    struct PanicGrader;

    impl Grader for PanicGrader {
        fn model(&self) -> &str {
            "panic-grader"
        }

        fn grade(&self, _request: &GradeRequest) -> Grade {
            panic!("Tiers 1-2 must not consult the grader");
        }
    }

    /// A deterministic stub grader returning a fixed [`Grade`] and counting calls.
    struct StubGrader {
        grade: Grade,
        calls: Cell<usize>,
    }

    impl StubGrader {
        fn new(grade: Grade) -> Self {
            StubGrader {
                grade,
                calls: Cell::new(0),
            }
        }

        fn calls(&self) -> usize {
            self.calls.get()
        }
    }

    impl Grader for StubGrader {
        fn model(&self) -> &str {
            "stub-grader"
        }

        fn grade(&self, _request: &GradeRequest) -> Grade {
            self.calls.set(self.calls.get() + 1);
            self.grade.clone()
        }
    }

    /// The driver model name the Tier 3 fixtures grade against (distinct from the
    /// stub grader, so the grader is never excluded as the driver).
    const DRIVER_MODEL: &str = "driver-agent";

    /// A judgment context with `panel`, the fixture driver, and the repo escalation
    /// floor.
    fn judgment_context<'a>(panel: &'a [&'a dyn Grader]) -> JudgmentContext<'a> {
        JudgmentContext {
            panel,
            driver_model: DRIVER_MODEL,
            escalate_below_confidence: ExpectConfig::default().approval.escalate_below_confidence,
        }
    }

    /// A Tier 3 judgment assertion over `$.message` against `anchor`.
    fn judgment_at(anchor: &str) -> JudgmentAssertion {
        JudgmentAssertion {
            checkpoint: 0,
            locator: Locator::JsonPath {
                path: "$.message".to_string(),
            },
            anchor: BoundValue::Text(anchor.to_string()),
            sim_threshold: 0.85,
            rubric: "conveys that the coupon is already applied".to_string(),
            criterion_text: "an error explains the coupon is already applied".to_string(),
        }
    }

    /// A single-checkpoint JSON observation carrying `body` at `final`.
    fn json_observation(body: Value) -> Observation {
        observation(vec![json_checkpoint("final", body)])
    }

    /// A Tier 2 assertion over the `final` checkpoint at json-path `$.<field>`.
    fn tolerance_at(field: &str, anchor: BoundValue, band: ToleranceBand) -> ToleranceAssertion {
        ToleranceAssertion {
            checkpoint: 0,
            locator: Locator::JsonPath {
                path: format!("$.{field}"),
            },
            anchor,
            band,
            criterion_text: format!("the {field} matches the approved value"),
        }
    }

    #[test]
    fn tier2_semantic_reword_passes_when_cosine_meets_threshold() {
        // The anchor and the received message are different strings — an exact
        // match would fail — but the stub maps them to near-parallel vectors, so
        // cosine >= 0.80 and Tier 2 accepts the semantically-equivalent reword.
        const ANCHOR: &str = "the coupon is already applied";
        const REWORD: &str = "this coupon has already been used";
        let embedder = StubEmbedder::new(&[(ANCHOR, &[1.0, 0.0]), (REWORD, &[0.96, 0.28])]);
        let obs = json_observation(json!({ "message": REWORD }));
        let assertion = tolerance_at(
            "message",
            BoundValue::Text(ANCHOR.to_string()),
            ToleranceBand::Semantic { threshold: 0.80 },
        );

        let verdict = assertion.resolve(&obs, &embedder);

        assert!(verdict.pass, "a semantically-equivalent reword is in band");
        assert_eq!(verdict.tier, VerdictTier::Tolerance);
        let score = verdict.score.expect("a bound Tier 2 value carries a score");
        assert!(score >= 0.80, "cosine {score} clears the 0.80 threshold");
        assert_eq!(verdict.evidence[0].snippet, REWORD);
    }

    #[test]
    fn tier2_semantic_divergence_is_reported_as_drift_not_a_hard_fail() {
        const ANCHOR: &str = "the coupon is already applied";
        const CHANGED: &str = "the order has shipped";
        let embedder = StubEmbedder::new(&[(ANCHOR, &[1.0, 0.0]), (CHANGED, &[0.0, 1.0])]);
        let obs = json_observation(json!({ "message": CHANGED }));
        let assertion = tolerance_at(
            "message",
            BoundValue::Text(ANCHOR.to_string()),
            ToleranceBand::Semantic { threshold: 0.80 },
        );

        let verdict = assertion.resolve(&obs, &embedder);

        assert!(!verdict.pass);
        // Drift, not a hard fail: the value still bound, so the score is kept
        // (Some) and the reason is the tolerance-drift signal — distinct from a
        // structural drift (None score) and from a Tier 1 clean fail.
        let score = verdict.score.expect("tolerance drift keeps its score");
        assert!(score < 0.80, "cosine {score} is below threshold");
        assert!(verdict.reason.starts_with(TOLERANCE_DRIFT_REASON));
        assert!(!verdict.reason.starts_with(STRUCTURAL_DRIFT_REASON));
    }

    #[test]
    fn tier2_numeric_passes_within_tolerance_and_drifts_outside() {
        const ANCHOR: f64 = 500.0;
        const TOLERANCE: f64 = 50.0;
        for (received, in_band) in [(530.0, true), (470.0, true), (600.0, false), (300.0, false)] {
            let embedder = StubEmbedder::new(&[]);
            let obs = json_observation(json!({ "latency_ms": received }));
            let assertion = tolerance_at(
                "latency_ms",
                BoundValue::Number(ANCHOR),
                ToleranceBand::Numeric {
                    tolerance: TOLERANCE,
                },
            );

            let verdict = assertion.resolve(&obs, &embedder);

            assert_eq!(
                verdict.pass, in_band,
                "{received} within {TOLERANCE} of {ANCHOR} should be {in_band}"
            );
            assert_eq!(verdict.tier, VerdictTier::Tolerance);
            assert_eq!(embedder.calls(), 0, "the numeric band never embeds");
            if !in_band {
                assert!(verdict.reason.starts_with(TOLERANCE_DRIFT_REASON));
            }
        }
    }

    #[test]
    fn tier2_near_string_passes_on_an_incidental_edit_and_drifts_on_a_rewrite() {
        const ANCHOR: &str = "colour";
        let embedder = StubEmbedder::new(&[]);
        // "color" is one deletion from "colour": similarity 1 - 1/6 ≈ 0.83 ≥ 0.80;
        // "banana" is a full rewrite well below the band.
        for (received, in_band) in [("color", true), ("banana", false)] {
            let obs = json_observation(json!({ "label": received }));
            let assertion = tolerance_at(
                "label",
                BoundValue::Text(ANCHOR.to_string()),
                ToleranceBand::NearString { threshold: 0.80 },
            );

            let verdict = assertion.resolve(&obs, &embedder);

            assert_eq!(verdict.pass, in_band, "`{received}` vs `{ANCHOR}`");
            assert_eq!(verdict.tier, VerdictTier::Tolerance);
        }
        assert_eq!(embedder.calls(), 0, "the near-string band never embeds");
    }

    #[test]
    fn tier2_structural_drift_when_the_locator_no_longer_binds() {
        let embedder = StubEmbedder::new(&[]);
        // The anchor expects $.message, but the received body renamed the field.
        let obs = json_observation(json!({ "msg": "anything" }));
        let assertion = tolerance_at(
            "message",
            BoundValue::Text("the coupon is already applied".to_string()),
            ToleranceBand::Semantic { threshold: 0.80 },
        );

        let verdict = assertion.resolve(&obs, &embedder);

        assert!(!verdict.pass);
        assert_eq!(
            verdict.score, None,
            "an unbound locator is structural drift, not a tolerance score"
        );
        assert!(verdict.reason.starts_with(STRUCTURAL_DRIFT_REASON));
    }

    #[test]
    fn tier1_resolution_never_consults_the_embedder() {
        // The Tier 1 path takes no embedder at all; `evaluate_tiered` routes a
        // deterministic criterion through it, so the panic-on-embed stub proves
        // Tier 2 runs only on the residual Tier 1 could not decide.
        let obs = json_observation(json!({ "total": 40 }));
        let tier1 = vec![assertion_for("the total is $40", &obs)];

        let outcome = evaluate_tiered(
            &obs,
            &tier1,
            &[],
            &[],
            &PanicEmbedder,
            &judgment_context(&[]),
        );

        assert_eq!(outcome.verdict.criteria.len(), 1);
        assert_eq!(outcome.verdict.criteria[0].tier, VerdictTier::Deterministic);
        assert!(outcome.verdict.criteria[0].pass);
        assert!(outcome.escalations.is_empty());
    }

    #[test]
    fn evaluate_tiered_runs_tier1_then_tier2_and_stops_before_tier3() {
        // One deterministic criterion and one tolerance criterion: Tier 1's
        // verdict comes first, Tier 2's second, and the embedder is consulted
        // only for the Tier 2 residual.
        const ANCHOR: &str = "the coupon is already applied";
        const REWORD: &str = "this coupon has already been used";
        let embedder = StubEmbedder::new(&[(ANCHOR, &[1.0, 0.0]), (REWORD, &[0.96, 0.28])]);
        let obs = json_observation(json!({ "total": 40, "message": REWORD }));
        let tier1 = vec![assertion_for("the total is $40", &obs)];
        let tier2 = vec![tolerance_at(
            "message",
            BoundValue::Text(ANCHOR.to_string()),
            ToleranceBand::Semantic { threshold: 0.80 },
        )];

        // No Tier 3 criteria, so the grader is never consulted (PanicGrader proves it).
        let panel: [&dyn Grader; 1] = [&PanicGrader];
        let outcome = evaluate_tiered(
            &obs,
            &tier1,
            &tier2,
            &[],
            &embedder,
            &judgment_context(&panel),
        );

        assert_eq!(outcome.verdict.criteria.len(), 2);
        assert_eq!(outcome.verdict.criteria[0].tier, VerdictTier::Deterministic);
        assert_eq!(outcome.verdict.criteria[1].tier, VerdictTier::Tolerance);
        assert!(outcome.verdict.criteria.iter().all(|c| c.pass));
        assert!(embedder.calls() > 0, "Tier 2 consulted the embedder");
        assert!(outcome.verdict.reliability.satisfied());
        assert!(outcome.escalations.is_empty());
    }

    #[test]
    fn evaluate_tiered_runs_tier3_only_on_the_residual_after_tiers_1_and_2() {
        // One deterministic, one tolerance, and one judgment criterion: the verdicts
        // come back in tier order, and the embedder is consulted for Tiers 2 and 3
        // while the grader is woken only for the Tier 3 residual that diverged.
        const SEMANTIC_ANCHOR: &str = "the coupon is already applied";
        const SEMANTIC_REWORD: &str = "this coupon has already been used";
        const JUDGE_ANCHOR: &str = "the order total dropped to $40";
        const JUDGE_CHANGED: &str = "your savings were applied at checkout";
        let embedder = StubEmbedder::new(&[
            (SEMANTIC_ANCHOR, &[1.0, 0.0]),
            (SEMANTIC_REWORD, &[0.96, 0.28]),
            (JUDGE_ANCHOR, &[1.0, 0.0]),
            (JUDGE_CHANGED, &[0.0, 1.0]),
        ]);
        let obs = json_observation(json!({
            "total": 40,
            "summary": SEMANTIC_REWORD,
            "message": JUDGE_CHANGED,
        }));
        let tier1 = vec![assertion_for("the total is $40", &obs)];
        let tier2 = vec![tolerance_at(
            "summary",
            BoundValue::Text(SEMANTIC_ANCHOR.to_string()),
            ToleranceBand::Semantic { threshold: 0.80 },
        )];
        let tier3 = vec![judgment_at(JUDGE_ANCHOR)];
        let grader = StubGrader::new(Grade {
            pass: true,
            confidence: 0.9,
            reason: "still conveys the discount".to_string(),
        });
        let panel: [&dyn Grader; 1] = [&grader];

        let outcome = evaluate_tiered(
            &obs,
            &tier1,
            &tier2,
            &tier3,
            &embedder,
            &judgment_context(&panel),
        );

        assert_eq!(outcome.verdict.criteria.len(), 3);
        assert_eq!(outcome.verdict.criteria[0].tier, VerdictTier::Deterministic);
        assert_eq!(outcome.verdict.criteria[1].tier, VerdictTier::Tolerance);
        assert_eq!(outcome.verdict.criteria[2].tier, VerdictTier::Judgment);
        assert_eq!(
            grader.calls(),
            1,
            "the judge wakes only on the Tier 3 residual"
        );
    }

    #[test]
    fn evaluate_tiered_routes_a_low_confidence_judgment_to_the_escalation_queue() {
        // A diverged judgment criterion the grader resolves below the confidence
        // floor lands in the escalation queue and is NOT auto-passed.
        const ANCHOR: &str = "the order total dropped to $40";
        const CHANGED: &str = "your savings were applied at checkout";
        let embedder = StubEmbedder::new(&[(ANCHOR, &[1.0, 0.0]), (CHANGED, &[0.0, 1.0])]);
        let obs = json_observation(json!({ "message": CHANGED }));
        let tier3 = vec![judgment_at(ANCHOR)];
        let floor = ExpectConfig::default().approval.escalate_below_confidence;
        let grader = StubGrader::new(Grade {
            pass: true,
            confidence: floor - 0.1,
            reason: "unsure the discount still reads".to_string(),
        });
        let panel: [&dyn Grader; 1] = [&grader];

        let outcome = evaluate_tiered(&obs, &[], &[], &tier3, &embedder, &judgment_context(&panel));

        assert_eq!(outcome.escalations.len(), 1, "low confidence escalates");
        assert_eq!(outcome.escalations[0].confidence, floor - 0.1);
        assert!(
            !outcome.verdict.criteria[0].pass,
            "an escalated criterion is never auto-passed"
        );
        assert!(!outcome.verdict.reliability.satisfied());
    }

    #[test]
    fn similarity_threshold_prefers_the_per_expectation_override_then_the_config_default() {
        let config = ExpectConfig::default();
        let mut spec = spec_with_criteria(&["the experience feels delightful"]);

        // No override: the repo `[embedder]` cutoff (the 0.80 default) applies —
        // the same value GradingPins freezes into the golden.
        spec.frontmatter.similarity_threshold = None;
        assert_eq!(
            similarity_threshold(&spec, &config),
            config.embedder.similarity_threshold
        );

        // A per-expectation override wins.
        spec.frontmatter.similarity_threshold = Some(0.95);
        assert_eq!(similarity_threshold(&spec, &config), 0.95);
    }

    #[test]
    fn cosine_similarity_is_one_for_parallel_and_zero_for_orthogonal_or_empty() {
        assert_eq!(cosine_similarity(&[1.0, 0.0], &[2.0, 0.0]), 1.0);
        assert_eq!(cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]), 0.0);
        assert_eq!(
            cosine_similarity(&[0.0, 0.0], &[1.0, 1.0]),
            0.0,
            "a zero-magnitude vector has no direction to compare"
        );
    }

    #[test]
    fn levenshtein_similarity_rewards_small_edits_and_punishes_rewrites() {
        assert_eq!(levenshtein_similarity("abc", "abc"), 1.0);
        assert_eq!(levenshtein_similarity("", ""), 1.0);
        assert!(levenshtein_similarity("colour", "color") > 0.80);
        assert!(levenshtein_similarity("colour", "banana") < 0.80);
    }
}
