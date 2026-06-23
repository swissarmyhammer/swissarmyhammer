//! Unit tests for each rung of the literal-find ladder.
//!
//! Each test pins the behavior of one rung (Exact, Normalized, Anchor, Fuzzy)
//! and the no-match / ambiguous outcomes, asserting on byte spans against the
//! **original** content.

use swissarmyhammer_edit_match::{
    find_match, MatchOutcome, Rung, FUZZY_ACCEPT_THRESHOLD, FUZZY_RUNNER_UP_MARGIN,
};

/// Pull the unique span+rung out of an outcome, or panic with a useful message.
fn unwrap_unique(outcome: MatchOutcome) -> (std::ops::Range<usize>, Rung, f32) {
    match outcome {
        MatchOutcome::Unique {
            span,
            rung,
            confidence,
        } => (span, rung, confidence),
        other => panic!("expected Unique, got {other:?}"),
    }
}

#[test]
fn exact_substring_returns_unique_exact_with_correct_span() {
    let content = "alpha\nbeta\ngamma\n";
    let find = "beta";
    let (span, rung, confidence) = unwrap_unique(find_match(content, find));
    assert_eq!(rung, Rung::Exact);
    assert_eq!(&content[span.clone()], "beta");
    assert_eq!(span, 6..10);
    assert_eq!(confidence, 1.0);
}

#[test]
fn exact_match_is_preferred_when_both_exact_and_fuzzy_possible() {
    // The exact substring exists, so we must never descend to a lower rung.
    let content = "fn foo() {}\nfn bar() {}\n";
    let find = "fn bar() {}";
    let (_span, rung, _conf) = unwrap_unique(find_match(content, find));
    assert_eq!(rung, Rung::Exact);
}

#[test]
fn dropped_leading_indentation_misses_exact_matches_normalized_on_original_bytes() {
    // The find lost its leading indentation; Exact cannot match the indented
    // original, but Normalized should, and the returned span must cover the
    // **original indented bytes**.
    let content = "fn outer() {\n    let x = compute();\n}\n";
    let find = "let x = compute();"; // no leading 4 spaces
    let (span, rung, _conf) = unwrap_unique(find_match(content, find));
    assert_eq!(rung, Rung::Normalized);
    // Span covers the original indented line content (the indentation belongs to
    // the original bytes the caller will rewrite).
    assert_eq!(&content[span.clone()], "    let x = compute();");
}

#[test]
fn crlf_line_endings_in_find_match_lf_original_via_normalized() {
    let content = "first line\nsecond line\nthird line\n";
    let find = "first line\r\nsecond line"; // CRLF, original is LF
    let (span, rung, _conf) = unwrap_unique(find_match(content, find));
    assert_eq!(rung, Rung::Normalized);
    assert_eq!(&content[span.clone()], "first line\nsecond line");
}

#[test]
fn anchor_matches_unique_first_and_last_lines_tolerating_interior_drift() {
    // Exact and Normalized fail (interior line differs), but the first and last
    // lines of `find` each occur uniquely, so Anchor spans between them.
    let content = "fn f() {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n}\n";
    let find = "fn f() {\n    DIFFERENT INTERIOR\n}";
    let (span, rung, _conf) = unwrap_unique(find_match(content, find));
    assert_eq!(rung, Rung::Anchor);
    // The span runs from the start of the first anchor line to the end of the
    // last anchor line, covering the drifted interior in the original.
    assert_eq!(
        &content[span.clone()],
        "fn f() {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n}"
    );
}

#[test]
fn fuzzy_matches_single_close_candidate_above_threshold() {
    // No exact/normalized/anchor match, but one line is overwhelmingly the most
    // similar candidate and clears the threshold + runner-up margin.
    let content = "the quick brown fox\ncompletely unrelated text here\n";
    let find = "the quick brown fax"; // one-char typo vs line 1
    let (span, rung, confidence) = unwrap_unique(find_match(content, find));
    assert_eq!(rung, Rung::Fuzzy);
    assert_eq!(&content[span.clone()], "the quick brown fox");
    assert!(confidence >= FUZZY_ACCEPT_THRESHOLD);
}

#[test]
fn no_match_returns_near_miss_spans() {
    let content = "the quick brown fox\nanother line entirely\n";
    let find = "zzzzzzzzzzzzzzzzzzz"; // similar to nothing
    match find_match(content, find) {
        MatchOutcome::NoMatch { near } => {
            // near may be empty or contain best-effort low-confidence spans, but
            // it must not be a Unique/Ambiguous masquerade.
            for span in &near {
                assert!(span.range.end <= content.len());
            }
        }
        other => panic!("expected NoMatch, got {other:?}"),
    }
}

#[test]
fn two_equally_good_candidates_return_ambiguous_never_silent_pick() {
    // Two identical lines: an exact find matches both, so the outcome is
    // Ambiguous (a span you cannot uniquely place must never be silently picked).
    let content = "dup line\nmiddle\ndup line\n";
    let find = "dup line";
    match find_match(content, find) {
        MatchOutcome::Ambiguous { candidates } => {
            assert_eq!(candidates.len(), 2);
        }
        other => panic!("expected Ambiguous, got {other:?}"),
    }
}

#[test]
fn fuzzy_constants_have_expected_provisional_values() {
    // The constants are part of the public contract; tests below assert behavior
    // at their boundaries, so pin the values themselves too.
    assert_eq!(FUZZY_ACCEPT_THRESHOLD, 0.85);
    assert_eq!(FUZZY_RUNNER_UP_MARGIN, 0.10);
}
