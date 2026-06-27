//! Tier 3 — model judgment, the residual-of-the-residual.
//!
//! Per `ideas/expect.md` §"The Verdict Ladder" Tier 3, §"Tier 3 is the
//! residual-of-the-residual", and §"Reliability and Non-Determinism" (grading
//! hardening). This is the top rung: a rubric grade against the **withheld**
//! criteria, run only on the residual the cheap tiers (1 and 2) could not decide.
//!
//! A judgment criterion does **not** call the model every run. At evaluate, a
//! [`JudgmentAssertion`] first takes **embedding similarity to its frozen anchor**
//! (the approved evidence text): at or above [`sim_threshold`](JudgmentAssertion::sim_threshold)
//! the new evidence is essentially the approved evidence, so the criterion **passes
//! with no model call**. Only on *divergence* does the judge wake — "does this
//! *new* evidence still satisfy the rubric?":
//!
//! - **Yes** ⇒ it passes the rubric but the evidence changed ⇒ [`JUDGMENT_DRIFT_REASON`]
//!   drift, surfaced for re-approval (never a silent pass).
//! - **No** ⇒ a clean fail.
//!
//! The judge is a named sah [`model`](crate::config::ModelConfig) (resolved like
//! `review`/`rules`, pinned in the golden via [`GradingPins`](crate::GradingPins)),
//! and is grade is **binary form-filling per criterion** (one observable criterion
//! at a time, chain-of-thought + evidence) — never free-form scoring. The judge
//! must be a **different** model than the agent that drove the run: an agent grading
//! its own trajectory inflates the verdict. An optional small [panel](JudgmentContext::panel)
//! of named models grades borderline criteria, where disagreement is itself the
//! signal of a vaguely-worded criterion. Criteria resolved with confidence below
//! [`escalate_below_confidence`](JudgmentContext::escalate_below_confidence) route
//! to the human escalation queue rather than being auto-passed.

use crate::assertion::{BoundValue, Locator};
use crate::evaluate::{cosine_similarity, TextEmbedder, STRUCTURAL_DRIFT_REASON};
use crate::types::{CriterionVerdict, Evidence, Observation, VerdictTier};
use serde::{Deserialize, Serialize};

/// The loud prefix marking a Tier 3 *judgment drift* reason: the new evidence
/// diverged from the approved anchor, yet the judge ruled it still satisfies the
/// rubric. Distinct from a hard fail (the rubric is *not* satisfied) — a judgment
/// drift routes to re-approval, exactly as a Tier 2 tolerance drift does.
pub const JUDGMENT_DRIFT_REASON: &str = "judgment drift";

/// The loud prefix marking a Tier 3 *panel disagreement*: the panel of graders
/// split on a borderline criterion. Disagreement is itself the signal that the
/// criterion is vaguely worded, so the criterion is flagged and escalated rather
/// than resolved on a coin-flip majority.
pub const PANEL_DISAGREEMENT_REASON: &str = "panel disagreement";

/// The loud prefix marking a Tier 3 criterion that could not be graded because the
/// only eligible grader **is** the agent that drove the run. An agent grading its
/// own trajectory inflates the verdict, so the criterion is escalated to a human
/// instead of being self-graded.
pub const GRADER_IS_DRIVER_REASON: &str = "grader is the driver";

/// The consensus confidence of a split panel: none. A disagreeing panel has
/// reached no agreement, so the criterion carries zero confidence and always lands
/// in the escalation queue.
const NO_CONSENSUS_CONFIDENCE: f32 = 0.0;

/// The binary, single-criterion judgment a [`Grader`] renders.
///
/// G-Eval style: chain-of-thought `reason` plus a **binary** `pass` (never a
/// free-form numeric vibe), with a `confidence` the escalation queue reads. One
/// [`Grade`] answers exactly one observable criterion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Grade {
    /// Whether the new evidence satisfies the rubric.
    pub pass: bool,
    /// The grader's self-reported confidence in `[0.0, 1.0]`, read by the
    /// escalation queue (LLM confidence is miscalibrated, so the floor is tunable).
    pub confidence: f32,
    /// The chain-of-thought reasoning behind the binary judgment.
    pub reason: String,
}

/// One observable criterion handed to a [`Grader`]: the withheld `rubric` and the
/// `evidence` to judge against it.
///
/// Form-filling, one criterion at a time — never a free-form scoring of the whole
/// run. The borrowed fields keep the request allocation-free at the call site.
pub struct GradeRequest<'a> {
    /// The rubric the evidence must satisfy (the withheld grading criterion).
    pub rubric: &'a str,
    /// The new evidence text located from the observation.
    pub evidence: &'a str,
    /// The criterion prose, for the grader's context.
    pub criterion: &'a str,
}

/// A named model that renders a binary rubric judgment over one criterion.
///
/// The seam the Tier 3 evaluate layer depends on. Production wires a real sah
/// `model:` (resolved like `review`/`rules`) behind the ACP grading path; tests
/// pass a deterministic stub so judgment resolution runs with no model and no GPU.
/// The grader is **distinct** from the agent that drove the run — enforced against
/// [`JudgmentContext::driver_model`] in [`JudgmentAssertion::resolve`].
pub trait Grader {
    /// The named model this grader speaks for, for the driver-distinctness check
    /// and panel attribution.
    fn model(&self) -> &str;

    /// Render a binary pass/fail judgment of `request`'s evidence against its rubric.
    fn grade(&self, request: &GradeRequest) -> Grade;
}

/// A residual (Tier 1/2-undecidable) criterion compiled into a Tier 3 judgment
/// check: a [`Locator`] into the observation, the frozen approved `anchor` text,
/// the embedding-similarity `sim_threshold` that gates the model, and the withheld
/// `rubric` the judge applies on divergence.
///
/// Mirrors [`ToleranceAssertion`](crate::ToleranceAssertion) for Tier 2: it is
/// *replayed* (never recompiled) against a later observation by [`resolve`](Self::resolve),
/// and `criterion_text` keeps it bound to the prose it was compiled from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JudgmentAssertion {
    /// Index into [`Observation::checkpoints`] this assertion reads.
    pub checkpoint: usize,
    /// Where in the checkpoint's state the judged evidence lives.
    pub locator: Locator,
    /// The approved evidence text the new evidence is compared against by
    /// embedding similarity before the model is ever consulted.
    pub anchor: BoundValue,
    /// The cosine-similarity cutoff at or above which the new evidence counts as
    /// the approved evidence — a pass with no model call.
    pub sim_threshold: f32,
    /// The withheld rubric the judge applies when the evidence diverges from the
    /// anchor.
    pub rubric: String,
    /// The criterion prose this assertion is bound to.
    pub criterion_text: String,
}

/// The Tier 3 grading context threaded through [`JudgmentAssertion::resolve`].
///
/// Kept as one struct so the resolve signature stays shallow as the panel, the
/// driver-distinctness rule, and the escalation floor all travel together.
pub struct JudgmentContext<'a> {
    /// The panel of graders consulted on divergence; index 0 is the primary. A
    /// single-element slice is the ordinary (non-panel) case. Disagreement across a
    /// multi-model panel flags the criterion as borderline.
    pub panel: &'a [&'a dyn Grader],
    /// The named model that drove the run — a grader must not be the same model
    /// (an agent grading its own trajectory inflates the verdict).
    pub driver_model: &'a str,
    /// The grader-confidence floor below which a resolved criterion routes to the
    /// human escalation queue instead of being auto-decided (per-surface tunable).
    pub escalate_below_confidence: f32,
}

impl JudgmentAssertion {
    /// Replay this Tier 3 assertion against `observation` as the residual-of-the-
    /// residual: locate the evidence, take embedding similarity to the frozen
    /// [`anchor`](Self::anchor), and only wake the judge on divergence.
    ///
    /// - The checkpoint is gone or the locator no longer binds ⇒ *structural drift*
    ///   (a non-pass with a `None` score and a [`STRUCTURAL_DRIFT_REASON`] reason),
    ///   identical to the Tier 1/2 structural signal — no embedder, no model call.
    /// - The evidence's anchor similarity is `>= sim_threshold` ⇒ a **pass with no
    ///   model call**: it is essentially the approved evidence.
    /// - The evidence diverged ⇒ the judge wakes and grades the new evidence against
    ///   the rubric: a rubric *pass* is [`JUDGMENT_DRIFT_REASON`] drift (re-approval),
    ///   a rubric *fail* is a clean fail. A split [panel](JudgmentContext::panel), or
    ///   a grader that is the driver, is flagged and escalated (zero confidence).
    pub fn resolve(
        &self,
        observation: &Observation,
        embedder: &dyn TextEmbedder,
        context: &JudgmentContext,
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
        let found_text = found.to_string();

        // The residual-of-the-residual gate: embedding similarity to the approved
        // anchor decides the common case with no model call.
        let anchor_vec = embedder.embed(&self.anchor.to_string());
        let found_vec = embedder.embed(&found_text);
        let similarity = cosine_similarity(&anchor_vec, &found_vec).clamp(0.0, 1.0);
        if similarity >= self.sim_threshold {
            return self.anchor_match(&locator, found_text, similarity);
        }

        self.grade_divergence(&locator, found_text, similarity, context)
    }

    /// Wake the judge on divergence: grade the new evidence against the rubric with
    /// every eligible panelist (the driver is excluded — it must not grade its own
    /// trajectory), then route the result by consensus.
    fn grade_divergence(
        &self,
        locator: &str,
        found_text: String,
        similarity: f32,
        context: &JudgmentContext,
    ) -> CriterionVerdict {
        let eligible: Vec<&dyn Grader> = context
            .panel
            .iter()
            .copied()
            .filter(|grader| grader.model() != context.driver_model)
            .collect();
        if eligible.is_empty() {
            return self.escalation(
                locator,
                found_text,
                similarity,
                format!(
                    "{GRADER_IS_DRIVER_REASON}: every grader is the driving agent `{}`",
                    context.driver_model
                ),
            );
        }

        let request = GradeRequest {
            rubric: &self.rubric,
            evidence: &found_text,
            criterion: &self.criterion_text,
        };
        let grades: Vec<Grade> = eligible
            .iter()
            .map(|grader| grader.grade(&request))
            .collect();
        let passes = grades.iter().filter(|grade| grade.pass).count();

        if passes == grades.len() {
            // Rubric satisfied, but the evidence diverged from the anchor: drift.
            self.judgment_drift(locator, found_text, similarity, &grades)
        } else if passes == 0 {
            self.judgment_fail(locator, found_text, similarity, &grades)
        } else {
            // A split panel: disagreement is itself the signal of a vague criterion.
            self.panel_disagreement(locator, found_text, similarity, passes, grades.len())
        }
    }

    /// The anchor-match pass: the new evidence is essentially the approved evidence,
    /// decided with no model call (hence no grader `confidence`).
    fn anchor_match(&self, locator: &str, found_text: String, similarity: f32) -> CriterionVerdict {
        self.verdict(
            locator,
            found_text,
            true,
            Some(similarity),
            None,
            format!(
                "`{locator}` matches the approved anchor (similarity {similarity:.2} >= {:.2})",
                self.sim_threshold
            ),
        )
    }

    /// The judgment-drift verdict: the judge ruled the diverged evidence still
    /// satisfies the rubric, so it routes to re-approval rather than reading as a
    /// pass or a code regression.
    fn judgment_drift(
        &self,
        locator: &str,
        found_text: String,
        similarity: f32,
        grades: &[Grade],
    ) -> CriterionVerdict {
        self.verdict(
            locator,
            found_text,
            false,
            Some(similarity),
            Some(mean_confidence(grades)),
            format!(
                "{JUDGMENT_DRIFT_REASON}: `{locator}` diverged from the anchor but still satisfies the rubric ({})",
                first_reason(grades)
            ),
        )
    }

    /// The judgment-fail verdict: the diverged evidence does not satisfy the rubric.
    fn judgment_fail(
        &self,
        locator: &str,
        found_text: String,
        similarity: f32,
        grades: &[Grade],
    ) -> CriterionVerdict {
        self.verdict(
            locator,
            found_text,
            false,
            Some(similarity),
            Some(mean_confidence(grades)),
            format!(
                "`{locator}` does not satisfy the rubric ({})",
                first_reason(grades)
            ),
        )
    }

    /// The panel-disagreement verdict: the panel split, so the criterion is flagged
    /// with no consensus confidence and lands in the escalation queue.
    fn panel_disagreement(
        &self,
        locator: &str,
        found_text: String,
        similarity: f32,
        passes: usize,
        total: usize,
    ) -> CriterionVerdict {
        self.verdict(
            locator,
            found_text,
            false,
            Some(similarity),
            Some(NO_CONSENSUS_CONFIDENCE),
            format!(
                "{PANEL_DISAGREEMENT_REASON}: {passes} of {total} graders passed `{locator}` — the criterion is ambiguous"
            ),
        )
    }

    /// An escalation verdict carrying no consensus confidence, so it routes to the
    /// human queue rather than being auto-decided.
    fn escalation(
        &self,
        locator: &str,
        found_text: String,
        similarity: f32,
        reason: String,
    ) -> CriterionVerdict {
        self.verdict(
            locator,
            found_text,
            false,
            Some(similarity),
            Some(NO_CONSENSUS_CONFIDENCE),
            reason,
        )
    }

    /// The structural-drift verdict shared by the checkpoint-missing and
    /// locator-unbound cases: a non-pass with a `None` score (the structural signal,
    /// distinct from a judgment score) and no grader confidence.
    fn structural_drift(&self, locator: &str, reason: String) -> CriterionVerdict {
        CriterionVerdict {
            criterion: self.criterion_text.clone(),
            tier: VerdictTier::Judgment,
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

    /// Build a Tier 3 [`CriterionVerdict`] over the located evidence, the shared
    /// shape behind every non-structural judgment outcome.
    fn verdict(
        &self,
        locator: &str,
        found_text: String,
        pass: bool,
        score: Option<f32>,
        confidence: Option<f32>,
        reason: String,
    ) -> CriterionVerdict {
        CriterionVerdict {
            criterion: self.criterion_text.clone(),
            tier: VerdictTier::Judgment,
            pass,
            score,
            evidence: vec![Evidence {
                locator: locator.to_string(),
                snippet: found_text,
            }],
            reason,
            confidence,
        }
    }
}

/// The mean self-reported confidence across `grades` (never empty at the call site).
fn mean_confidence(grades: &[Grade]) -> f32 {
    let sum: f32 = grades.iter().map(|grade| grade.confidence).sum();
    sum / grades.len() as f32
}

/// The first grader's chain-of-thought reasoning, woven into the verdict reason.
fn first_reason(grades: &[Grade]) -> &str {
    grades
        .first()
        .map(|grade| grade.reason.as_str())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Checkpoint, SurfaceState, Trajectory};
    use serde_json::{json, Value};
    use std::cell::Cell;
    use std::collections::HashMap;
    use std::time::Duration;

    /// The pinned grader and driver model names reused across the Tier 3 fixtures.
    const GRADER_MODEL: &str = "qwen-coder-flash";
    const DRIVER_MODEL: &str = "claude-sonnet";
    /// The anchor-similarity cutoff the fixtures gate the model behind.
    const SIM_THRESHOLD: f32 = 0.85;
    /// The escalation floor the fixtures use (the repo default).
    const ESCALATE_BELOW: f32 = 0.6;

    /// A deterministic stub embedder mapping registered strings to fixed vectors
    /// (unknown text → the zero vector), counting calls so a test can assert the
    /// anchor short-circuit consulted it (or, paired with [`PanicGrader`], that it
    /// never reached the model).
    struct StubEmbedder {
        vectors: HashMap<String, Vec<f32>>,
    }

    impl StubEmbedder {
        fn new(pairs: &[(&str, &[f32])]) -> Self {
            StubEmbedder {
                vectors: pairs
                    .iter()
                    .map(|(text, vector)| (text.to_string(), vector.to_vec()))
                    .collect(),
            }
        }
    }

    impl TextEmbedder for StubEmbedder {
        fn embed(&self, text: &str) -> Vec<f32> {
            self.vectors
                .get(text)
                .cloned()
                .unwrap_or_else(|| vec![0.0, 0.0])
        }
    }

    /// A deterministic stub grader returning a fixed [`Grade`] and counting calls,
    /// so a test can assert the judge was (or was not) consulted.
    struct StubGrader {
        model: String,
        grade: Grade,
        calls: Cell<usize>,
    }

    impl StubGrader {
        fn new(model: &str, grade: Grade) -> Self {
            StubGrader {
                model: model.to_string(),
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
            &self.model
        }

        fn grade(&self, _request: &GradeRequest) -> Grade {
            self.calls.set(self.calls.get() + 1);
            self.grade.clone()
        }
    }

    /// A grader that must never be consulted — any call is a test failure. Proves
    /// the anchor short-circuit (and the structural-drift path) decides without
    /// touching the model.
    struct PanicGrader {
        model: String,
    }

    impl Grader for PanicGrader {
        fn model(&self) -> &str {
            &self.model
        }

        fn grade(&self, _request: &GradeRequest) -> Grade {
            panic!("the anchor short-circuit must not consult the grader");
        }
    }

    /// A passing [`Grade`] with `confidence`.
    fn grade_pass(confidence: f32) -> Grade {
        Grade {
            pass: true,
            confidence,
            reason: "the message still conveys the approved meaning".to_string(),
        }
    }

    /// A failing [`Grade`] with `confidence`.
    fn grade_fail(confidence: f32) -> Grade {
        Grade {
            pass: false,
            confidence,
            reason: "the message no longer conveys the approved meaning".to_string(),
        }
    }

    /// A single-checkpoint JSON observation carrying `body` at `final`.
    fn json_observation(body: Value) -> Observation {
        Observation {
            path: "src/checkout/coupon".to_string(),
            checkpoints: vec![Checkpoint {
                after: "final".to_string(),
                state: SurfaceState::Json { body },
                duration: Duration::from_millis(1),
            }],
            trajectory: Trajectory { steps: Vec::new() },
        }
    }

    /// A Tier 3 judgment assertion over `$.message` against `anchor`.
    fn judgment(anchor: &str) -> JudgmentAssertion {
        JudgmentAssertion {
            checkpoint: 0,
            locator: Locator::JsonPath {
                path: "$.message".to_string(),
            },
            anchor: BoundValue::Text(anchor.to_string()),
            sim_threshold: SIM_THRESHOLD,
            rubric: "conveys that the coupon is already applied".to_string(),
            criterion_text: "an error explains the coupon is already applied".to_string(),
        }
    }

    /// A judgment context with `panel`, the fixture driver, and the repo escalation
    /// floor.
    fn context<'a>(panel: &'a [&'a dyn Grader]) -> JudgmentContext<'a> {
        JudgmentContext {
            panel,
            driver_model: DRIVER_MODEL,
            escalate_below_confidence: ESCALATE_BELOW,
        }
    }

    #[test]
    fn tier3_anchor_match_passes_without_a_grader_call() {
        // The new message differs from the anchor textually, but the stub maps both
        // to near-parallel vectors (cosine >= 0.85), so the criterion passes as the
        // approved evidence — and the PanicGrader proves the model was never woken.
        const ANCHOR: &str = "the coupon is already applied";
        const REWORD: &str = "this coupon has already been used";
        let embedder = StubEmbedder::new(&[(ANCHOR, &[1.0, 0.0]), (REWORD, &[0.99, 0.10])]);
        let obs = json_observation(json!({ "message": REWORD }));
        let grader = PanicGrader {
            model: GRADER_MODEL.to_string(),
        };
        let panel: [&dyn Grader; 1] = [&grader];

        let verdict = judgment(ANCHOR).resolve(&obs, &embedder, &context(&panel));

        assert!(verdict.pass, "anchor match is a pass");
        assert_eq!(verdict.tier, VerdictTier::Judgment);
        assert_eq!(
            verdict.confidence, None,
            "no grader was consulted, so there is no grader confidence"
        );
        assert_eq!(verdict.evidence[0].snippet, REWORD);
    }

    #[test]
    fn tier3_divergent_evidence_that_satisfies_the_rubric_is_judgment_drift() {
        // The message diverged from the anchor (orthogonal vectors), so the judge
        // wakes; it rules the rubric still satisfied, so the criterion is drift
        // (re-approval), not a clean pass and not a fail.
        const ANCHOR: &str = "the coupon is already applied";
        const CHANGED: &str = "that promo code is already in use on this order";
        let embedder = StubEmbedder::new(&[(ANCHOR, &[1.0, 0.0]), (CHANGED, &[0.0, 1.0])]);
        let obs = json_observation(json!({ "message": CHANGED }));
        let grader = StubGrader::new(GRADER_MODEL, grade_pass(0.92));
        let panel: [&dyn Grader; 1] = [&grader];

        let verdict = judgment(ANCHOR).resolve(&obs, &embedder, &context(&panel));

        assert!(!verdict.pass, "drift is not a clean pass");
        assert_eq!(grader.calls(), 1, "divergence woke the judge once");
        assert!(verdict.reason.starts_with(JUDGMENT_DRIFT_REASON));
        assert_eq!(verdict.confidence, Some(0.92));
    }

    #[test]
    fn tier3_evidence_that_fails_the_rubric_is_a_fail() {
        const ANCHOR: &str = "the coupon is already applied";
        const CHANGED: &str = "the order has shipped";
        let embedder = StubEmbedder::new(&[(ANCHOR, &[1.0, 0.0]), (CHANGED, &[0.0, 1.0])]);
        let obs = json_observation(json!({ "message": CHANGED }));
        let grader = StubGrader::new(GRADER_MODEL, grade_fail(0.9));
        let panel: [&dyn Grader; 1] = [&grader];

        let verdict = judgment(ANCHOR).resolve(&obs, &embedder, &context(&panel));

        assert!(!verdict.pass);
        assert_eq!(grader.calls(), 1);
        assert!(
            !verdict.reason.starts_with(JUDGMENT_DRIFT_REASON),
            "a rubric fail is not drift"
        );
        assert_eq!(verdict.confidence, Some(0.9));
    }

    #[test]
    fn tier3_structural_drift_when_the_locator_no_longer_binds() {
        // The anchor expects $.message, but the body renamed the field: structural
        // drift, decided with no embedder reach into the model and no grader call.
        const ANCHOR: &str = "the coupon is already applied";
        let embedder = StubEmbedder::new(&[]);
        let obs = json_observation(json!({ "msg": "anything" }));
        let grader = PanicGrader {
            model: GRADER_MODEL.to_string(),
        };
        let panel: [&dyn Grader; 1] = [&grader];

        let verdict = judgment(ANCHOR).resolve(&obs, &embedder, &context(&panel));

        assert!(!verdict.pass);
        assert_eq!(
            verdict.score, None,
            "an unbound locator is structural drift, not a judgment score"
        );
        assert!(verdict.reason.starts_with(STRUCTURAL_DRIFT_REASON));
    }

    #[test]
    fn tier3_panel_disagreement_is_flagged_with_zero_confidence() {
        // A two-model panel splits on a borderline criterion: one passes, one fails.
        // Disagreement itself is the signal of a vague criterion, so the verdict is
        // flagged and carries zero confidence (it will land in the escalation queue).
        const ANCHOR: &str = "the coupon is already applied";
        const CHANGED: &str = "promo handling changed";
        let embedder = StubEmbedder::new(&[(ANCHOR, &[1.0, 0.0]), (CHANGED, &[0.0, 1.0])]);
        let obs = json_observation(json!({ "message": CHANGED }));
        let yes = StubGrader::new(GRADER_MODEL, grade_pass(0.8));
        let no = StubGrader::new("gpt-5", grade_fail(0.8));
        let panel: [&dyn Grader; 2] = [&yes, &no];

        let verdict = judgment(ANCHOR).resolve(&obs, &embedder, &context(&panel));

        assert!(!verdict.pass);
        assert!(verdict.reason.starts_with(PANEL_DISAGREEMENT_REASON));
        assert_eq!(verdict.confidence, Some(NO_CONSENSUS_CONFIDENCE));
        assert_eq!(yes.calls(), 1, "every panelist grades");
        assert_eq!(no.calls(), 1);
    }

    #[test]
    fn tier3_grader_that_is_the_driver_is_escalated_not_self_graded() {
        // The only grader is the model that drove the run; it must not grade its own
        // trajectory, so the criterion is escalated with zero confidence and the
        // grader is never consulted.
        const ANCHOR: &str = "the coupon is already applied";
        const CHANGED: &str = "the order has shipped";
        let embedder = StubEmbedder::new(&[(ANCHOR, &[1.0, 0.0]), (CHANGED, &[0.0, 1.0])]);
        let obs = json_observation(json!({ "message": CHANGED }));
        // The grader's model IS the driver — PanicGrader proves it is never called.
        let grader = PanicGrader {
            model: DRIVER_MODEL.to_string(),
        };
        let panel: [&dyn Grader; 1] = [&grader];

        let verdict = judgment(ANCHOR).resolve(&obs, &embedder, &context(&panel));

        assert!(!verdict.pass);
        assert!(verdict.reason.starts_with(GRADER_IS_DRIVER_REASON));
        assert_eq!(verdict.confidence, Some(NO_CONSENSUS_CONFIDENCE));
    }

    #[test]
    fn tier3_excludes_the_driver_from_a_mixed_panel() {
        // A two-model panel where one panelist is the driver: the driver is excluded
        // and the remaining eligible grader decides, so a unanimous-among-eligible
        // pass is judgment drift (not a disagreement, not an escalation).
        const ANCHOR: &str = "the coupon is already applied";
        const CHANGED: &str = "promo code already used";
        let embedder = StubEmbedder::new(&[(ANCHOR, &[1.0, 0.0]), (CHANGED, &[0.0, 1.0])]);
        let obs = json_observation(json!({ "message": CHANGED }));
        let driver_grader = PanicGrader {
            model: DRIVER_MODEL.to_string(),
        };
        let eligible = StubGrader::new(GRADER_MODEL, grade_pass(0.9));
        let panel: [&dyn Grader; 2] = [&driver_grader, &eligible];

        let verdict = judgment(ANCHOR).resolve(&obs, &embedder, &context(&panel));

        assert!(!verdict.pass);
        assert!(verdict.reason.starts_with(JUDGMENT_DRIFT_REASON));
        assert_eq!(eligible.calls(), 1, "only the eligible grader is consulted");
    }
}
