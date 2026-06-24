//! Edge-case and lower-rung behavior tests closing coverage gaps in the
//! literal-find ladder.
//!
//! These pin behaviors that the rung-level tests in `rungs.rs` do not reach:
//! Normalized-rung ambiguity, Anchor-rung non-unique / inverted anchors, CRLF
//! line handling and alignment, trailing-empty-line trimming, and the empty /
//! degenerate-input guards. Every test asserts the resulting `MatchOutcome`
//! variant and (where applicable) the byte spans against the **original**
//! content — not merely that a line executed.

use swissarmyhammer_edit_match::{find_match, similarity, MatchOutcome, Rung};

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

// ---------------------------------------------------------------------------
// Normalized rung — ambiguity (finalize_block_matches many-arm).
// ---------------------------------------------------------------------------

#[test]
fn normalized_block_duplicated_in_two_places_is_ambiguous_never_silent_pick() {
    // A de-indented multi-line `find` whose normalized block appears in two
    // distinct indented locations: Exact cannot match (indentation differs),
    // so the Normalized rung finds two windows and must report Ambiguous with
    // both candidate spans rather than silently picking one.
    let content = "\
fn a() {
    do_work();
    cleanup();
}
fn b() {
    do_work();
    cleanup();
}
";
    let find = "do_work();\ncleanup();"; // no indentation -> not an exact substring
    match find_match(content, find) {
        MatchOutcome::Ambiguous { candidates } => {
            assert_eq!(candidates.len(), 2, "expected both duplicated blocks");
            // Each candidate span must cover the original indented block.
            for c in &candidates {
                assert_eq!(&content[c.range.clone()], "    do_work();\n    cleanup();");
            }
            // The two spans are distinct locations.
            assert_ne!(candidates[0].range, candidates[1].range);
        }
        other => panic!("expected Ambiguous, got {other:?}"),
    }
}

#[test]
fn normalized_find_with_trailing_blank_line_trims_phantom_empty_line() {
    // A `find` ending in a blank line (`...\n\n`) yields a trailing
    // normalized-empty line via `str::lines()`. `trim_trailing_empty` must drop
    // it so the block length matches the intended two-line content and the span
    // covers exactly those two original lines (no phantom third line).
    let content = "header\n    keep one;\n    keep two;\nfooter\n";
    let find = "keep one;\nkeep two;\n\n"; // trailing blank line + de-indented
    let (span, rung, _conf) = unwrap_unique(find_match(content, find));
    assert_eq!(rung, Rung::Normalized);
    assert_eq!(&content[span], "    keep one;\n    keep two;");
}

// ---------------------------------------------------------------------------
// Anchor rung — non-unique anchors and inverted anchors (try_anchor).
// ---------------------------------------------------------------------------

#[test]
fn anchor_with_non_unique_first_line_does_not_match() {
    // The first anchor line ("open {") occurs on multiple content lines, so the
    // anchor rung must refuse (returns None and descends). Exact and Normalized
    // already fail because of the drifted interior, so the overall outcome is
    // not an Anchor match.
    let content = "\
open {
    alpha;
close }
open {
    beta;
close }
";
    // First line is duplicated; last line "close }" is also duplicated. Anchor
    // must descend; the drifted interior also prevents a fuzzy unique, giving
    // NoMatch.
    let find = "open {\n    DRIFTED INTERIOR\nclose }";
    match find_match(content, find) {
        MatchOutcome::NoMatch { .. } => {}
        other => panic!("expected NoMatch (anchor refused non-unique), got {other:?}"),
    }
}

#[test]
fn anchor_with_last_line_before_first_line_does_not_match() {
    // Both anchor lines are individually unique, but the last anchor line occurs
    // *before* the first in the content (end_idx <= start_idx), so the anchor
    // rung must refuse rather than span a backwards region.
    let content = "\
THE_END marker
some middle text
THE_START marker
";
    let find = "THE_START marker\ndrifted\nTHE_END marker";
    match find_match(content, find) {
        MatchOutcome::NoMatch { .. } => {}
        other => panic!("expected NoMatch (inverted anchors), got {other:?}"),
    }
}

#[test]
fn anchor_with_empty_first_anchor_line_does_not_match() {
    // `find` has >= 2 lines but its first normalized line is empty (a leading
    // blank line). Anchor requires non-empty first/last anchors, so it refuses.
    let content = "alpha\nbeta\ngamma\n";
    // Two lines after trimming trailing empties: ["", "zeta-unmatched"].
    let find = "\nzeta-unmatched";
    // Exact/Normalized/Anchor all fail; the lone non-empty find line is
    // dissimilar to every content line, so NoMatch.
    match find_match(content, find) {
        MatchOutcome::NoMatch { .. } => {}
        other => panic!("expected NoMatch (empty first anchor), got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// CRLF line handling (physical_lines, is_line_aligned).
// ---------------------------------------------------------------------------

#[test]
fn exact_single_line_is_line_aligned_in_crlf_content_with_more_following() {
    // CRLF content with content after the matched line: the right boundary is a
    // `\r\n`, and the single-line `find` must still count as Exact, line-aligned,
    // with a span over the original line bytes (no trailing \r).
    let content = "alpha\r\nbeta\r\ngamma\r\n";
    let find = "beta";
    let (span, rung, _conf) = unwrap_unique(find_match(content, find));
    assert_eq!(rung, Rung::Exact);
    assert_eq!(&content[span.clone()], "beta");
    // "alpha\r\n" is 7 bytes, so beta starts at 7.
    assert_eq!(span, 7..11);
}

#[test]
fn exact_single_line_is_line_aligned_when_followed_by_trailing_cr_at_eof() {
    // The matched line is followed by a bare `\r` that is the final byte of
    // content (end + 1 == len): still line-aligned, still Exact.
    let content = "alpha\nbeta\r";
    let find = "beta";
    let (span, rung, _conf) = unwrap_unique(find_match(content, find));
    assert_eq!(rung, Rung::Exact);
    assert_eq!(&content[span.clone()], "beta");
    assert_eq!(span, 6..10);
}

#[test]
fn crlf_content_lines_are_split_without_carriage_returns() {
    // Multi-line normalized match over CRLF content: physical_lines must strip
    // the `\r` so normalized comparison succeeds and the span covers the
    // original CRLF bytes between line starts/ends (interior \r\n included).
    let content = "one\r\ntwo\r\nthree\r\n";
    let find = "one\ntwo"; // LF find vs CRLF content
    let (span, rung, _conf) = unwrap_unique(find_match(content, find));
    assert_eq!(rung, Rung::Normalized);
    // Span runs from start of "one" to end of "two" in the original bytes,
    // including the interior "\r\n".
    assert_eq!(&content[span], "one\r\ntwo");
}

// ---------------------------------------------------------------------------
// physical_lines — final line without a trailing newline.
// ---------------------------------------------------------------------------

#[test]
fn content_without_trailing_newline_includes_final_line_via_normalized() {
    // Content whose final line has no trailing newline, matched through the
    // Normalized rung (a de-indented `find` so Exact is skipped and
    // `physical_lines` runs). The final line lacks a terminating `\n`, so
    // `physical_lines` must still push it via the no-final-newline tail path,
    // and the span must cover the original indented final line.
    let content = "first\n    last-no-newline"; // no trailing newline; indented final line
    let find = "last-no-newline"; // de-indented -> Exact rejects, Normalized recovers
    let (span, rung, _conf) = unwrap_unique(find_match(content, find));
    assert_eq!(rung, Rung::Normalized);
    assert_eq!(&content[span.clone()], "    last-no-newline");
    assert_eq!(span.end, content.len());
}

// ---------------------------------------------------------------------------
// byte_offsets_of — a multi-line block that repeats to end-of-content.
// ---------------------------------------------------------------------------

#[test]
fn repeated_block_reaching_end_of_content_is_ambiguous() {
    // A multi-line `find` (matched as a raw substring) that repeats and whose
    // final occurrence ends exactly at end-of-content: byte_offsets_of must
    // advance past the last match and terminate, and the two occurrences are
    // reported Ambiguous (never a silent pick).
    let content = "x\ny\nx\ny\n";
    let find = "x\ny\n"; // multi-line, occurs twice, second ends at EOF
    match find_match(content, find) {
        MatchOutcome::Ambiguous { candidates } => {
            assert_eq!(candidates.len(), 2);
            assert_eq!(&content[candidates[0].range.clone()], "x\ny\n");
            assert_eq!(&content[candidates[1].range.clone()], "x\ny\n");
        }
        other => panic!("expected Ambiguous, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Empty-find and degenerate guards (try_exact, try_line_block, span_of,
// similarity/levenshtein).
// ---------------------------------------------------------------------------

#[test]
fn empty_find_against_blank_lines_is_ambiguous_blank_spans() {
    // An empty `find` skips the Exact and Normalized guards (both return None)
    // and reaches the fuzzy rung, where empty matches empty perfectly. With two
    // blank content lines it is Ambiguous, exercising span_of on empty ranges
    // (start == end falling back to start_line).
    let content = "a\n\n\nb\n";
    match find_match(content, "") {
        MatchOutcome::Ambiguous { candidates } => {
            assert_eq!(candidates.len(), 2, "two blank lines tie at perfect score");
            for c in &candidates {
                assert!(c.range.is_empty(), "blank-line span is an empty range");
                // Empty-range fallback: end_line collapses to start_line.
                assert_eq!(c.start_line, c.end_line);
                assert_eq!(c.text, "");
            }
        }
        other => panic!("expected Ambiguous over blank lines, got {other:?}"),
    }
}

#[test]
fn empty_find_against_content_without_blank_lines_is_no_match() {
    // Empty `find` with no blank content line to perfectly match: every line
    // scores 0, so the outcome is a sensible NoMatch.
    let content = "alpha\nbeta\n";
    match find_match(content, "") {
        MatchOutcome::NoMatch { .. } => {}
        other => panic!("expected NoMatch for empty find on non-blank content, got {other:?}"),
    }
}

#[test]
fn find_longer_than_content_does_not_match() {
    // A multi-line `find` with more lines than the content has: try_line_block
    // bails (find_norm.len() > content_norm.len()) and the ladder yields a
    // sensible non-Normalized outcome.
    let content = "only one line\n";
    let find = "alpha line\nbeta line\ngamma line";
    match find_match(content, find) {
        MatchOutcome::NoMatch { .. } => {}
        // Defensive: must never be a Normalized block match here.
        MatchOutcome::Unique { rung, .. } => {
            assert_ne!(rung, Rung::Normalized);
        }
        other => panic!("unexpected outcome {other:?}"),
    }
}

#[test]
fn similarity_with_empty_first_string_uses_full_distance() {
    // levenshtein("", "x") == len("x"), so similarity("", "x") == 0.0 and
    // similarity("", "") == 1.0 (the empty/empty identity).
    assert_eq!(similarity("", "x"), 0.0);
    assert_eq!(similarity("", "abc"), 0.0);
    assert_eq!(similarity("", ""), 1.0);
}
