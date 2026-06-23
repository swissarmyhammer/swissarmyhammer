//! Deterministic tests pinning the fuzzy decision boundary.
//!
//! The fuzzy rung accepts a candidate as `Unique` only when its similarity is
//! `>= FUZZY_ACCEPT_THRESHOLD` AND it beats the runner-up by at least
//! `FUZZY_RUNNER_UP_MARGIN`. These tests construct inputs whose similarities
//! straddle those constants and assert the exact outcome, so a future change to
//! either constant forces a deliberate test update.

use swissarmyhammer_edit_match::{
    find_match, similarity, MatchOutcome, Rung, FUZZY_ACCEPT_THRESHOLD, FUZZY_BOUNDARY_EPSILON,
    FUZZY_RUNNER_UP_MARGIN,
};

/// `similarity` is normalized into `[0.0, 1.0]`: identical strings score 1.0,
/// fully dissimilar strings score 0.0. The boundary tests rely on this scale.
#[test]
fn similarity_is_normalized_zero_to_one() {
    assert_eq!(similarity("abc", "abc"), 1.0);
    assert_eq!(similarity("", ""), 1.0);
    assert_eq!(similarity("abc", ""), 0.0);
    let partial = similarity("kitten", "sitting");
    assert!(partial > 0.0 && partial < 1.0);
}

/// Construct a candidate line at a target similarity to `find` by editing a
/// known fraction of characters, then assert the chosen similarity lands on the
/// intended side of the boundary. We use 20-char strings so each single-char
/// edit moves similarity by exactly 0.05, giving clean control around 0.85.
fn line_of(c: char, n: usize) -> String {
    std::iter::repeat_n(c, n).collect()
}

#[test]
fn candidate_just_below_threshold_is_no_match() {
    // base: 20 identical chars. A candidate with 4 edits -> similarity 0.80,
    // which is FUZZY_ACCEPT_THRESHOLD - 0.05 (below threshold).
    let find = line_of('a', 20);
    let mut candidate: Vec<char> = find.chars().collect();
    for slot in candidate.iter_mut().take(4) {
        *slot = 'b';
    }
    let candidate: String = candidate.into_iter().collect();
    let sim = similarity(&find, &candidate);
    assert!(
        sim < FUZZY_ACCEPT_THRESHOLD,
        "candidate sim {sim} must be below threshold {FUZZY_ACCEPT_THRESHOLD}"
    );

    let content = format!("{candidate}\n");
    match find_match(&content, &find) {
        MatchOutcome::NoMatch { .. } => {}
        other => panic!("below-threshold candidate must be NoMatch, got {other:?}"),
    }
}

#[test]
fn single_candidate_at_threshold_is_unique() {
    // 3 edits over 20 chars -> similarity 0.85, exactly FUZZY_ACCEPT_THRESHOLD,
    // and it is the only candidate so the runner-up margin is trivially clear.
    let find = line_of('a', 20);
    let mut candidate: Vec<char> = find.chars().collect();
    for slot in candidate.iter_mut().take(3) {
        *slot = 'b';
    }
    let candidate: String = candidate.into_iter().collect();
    let sim = similarity(&find, &candidate);
    assert_eq!(
        sim, FUZZY_ACCEPT_THRESHOLD,
        "expected sim exactly at threshold"
    );

    let content = format!("{candidate}\nzzzzzzzzzzzzzzzzzzzz\n");
    match find_match(&content, &find) {
        MatchOutcome::Unique { rung, .. } => assert_eq!(rung, Rung::Fuzzy),
        other => panic!("at-threshold single candidate must be Unique Fuzzy, got {other:?}"),
    }
}

#[test]
fn two_candidates_above_threshold_within_margin_are_ambiguous() {
    // Two candidates both above threshold whose similarities differ by less than
    // FUZZY_RUNNER_UP_MARGIN -> Ambiguous (no confident winner).
    let find = line_of('a', 20);

    // best: 1 edit -> 0.95
    let mut best: Vec<char> = find.chars().collect();
    best[0] = 'b';
    let best: String = best.into_iter().collect();

    // runner-up: 2 edits -> 0.90; 0.95 - 0.90 = 0.05 < FUZZY_RUNNER_UP_MARGIN
    let mut runner: Vec<char> = find.chars().collect();
    runner[0] = 'b';
    runner[1] = 'b';
    let runner: String = runner.into_iter().collect();

    let s_best = similarity(&find, &best);
    let s_runner = similarity(&find, &runner);
    assert!(s_best >= FUZZY_ACCEPT_THRESHOLD && s_runner >= FUZZY_ACCEPT_THRESHOLD);
    assert!(
        s_best - s_runner < FUZZY_RUNNER_UP_MARGIN,
        "gap {} must be inside the runner-up margin {FUZZY_RUNNER_UP_MARGIN}",
        s_best - s_runner
    );

    let content = format!("{best}\n{runner}\n");
    match find_match(&content, &find) {
        MatchOutcome::Ambiguous { candidates } => assert!(candidates.len() >= 2),
        other => panic!("within-margin pair must be Ambiguous, got {other:?}"),
    }
}

#[test]
fn two_candidates_above_threshold_beyond_margin_pick_the_winner() {
    // best clears the runner-up by MORE than the margin -> Unique winner.
    let find = line_of('a', 20);

    // best: 0 edits is exact (would short-circuit at Exact); use 1 edit -> 0.95.
    let mut best: Vec<char> = find.chars().collect();
    best[0] = 'b';
    let best: String = best.into_iter().collect();

    // runner-up: 3 edits -> 0.85; 0.95 - 0.85 = 0.10 >= FUZZY_RUNNER_UP_MARGIN.
    let mut runner: Vec<char> = find.chars().collect();
    for slot in runner.iter_mut().take(3) {
        *slot = 'b';
    }
    let runner: String = runner.into_iter().collect();

    // The mathematical gap is exactly the margin (0.95 - 0.85), but f32 lands an
    // ULP low, so compare with the same epsilon production uses at the boundary.
    let gap = similarity(&find, &best) - similarity(&find, &runner);
    assert!(
        gap >= FUZZY_RUNNER_UP_MARGIN - FUZZY_BOUNDARY_EPSILON,
        "gap {gap} must reach the runner-up margin {FUZZY_RUNNER_UP_MARGIN}"
    );

    let content = format!("{best}\n{runner}\n");
    match find_match(&content, &find) {
        MatchOutcome::Unique { span, rung, .. } => {
            assert_eq!(rung, Rung::Fuzzy);
            assert_eq!(&content[span], &best);
        }
        other => panic!("beyond-margin winner must be Unique Fuzzy, got {other:?}"),
    }
}
