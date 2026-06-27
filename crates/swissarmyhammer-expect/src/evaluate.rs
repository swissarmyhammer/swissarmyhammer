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

use crate::assertion::{compile, AssertionOutcome, BoundValue, CompileError, CompiledAssertion};
use crate::spec::{Criterion, Expectation};
use crate::types::{
    CriterionVerdict, Evidence, ExpectationVerdict, Observation, Reliability, VerdictTier,
};

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

#[cfg(test)]
mod tests {
    use super::*;
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
}
