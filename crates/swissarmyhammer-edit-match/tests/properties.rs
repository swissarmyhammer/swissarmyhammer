//! Property tests for the literal-find ladder.
//!
//! These perturb the whitespace / indentation / line-endings of a known span
//! lifted from generated content and assert the ladder still lands on that span
//! (via Exact or Normalized), and that genuinely ambiguous inputs are refused
//! rather than silently resolved.

use proptest::prelude::*;
use swissarmyhammer_edit_match::{find_match, MatchOutcome};

/// A single line of identifier-ish text (no newlines), guaranteed non-empty so
/// it survives a `join("\n")` round-trip as exactly one line.
fn ident_line() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,11}".prop_map(|s| s)
}

/// A small block of distinct lines. Distinctness keeps an exact substring from
/// matching in two places (which would correctly be Ambiguous, not Unique),
/// letting the oracle assert a single landing span.
fn distinct_block() -> impl Strategy<Value = Vec<String>> {
    proptest::collection::hash_set(ident_line(), 3..7)
        .prop_map(|set| set.into_iter().collect::<Vec<_>>())
}

proptest! {
    /// An exact substring of the content (a contiguous run of whole lines) is
    /// always located, and the returned span maps back to those exact bytes.
    #[test]
    fn exact_subspan_is_always_found(block in distinct_block()) {
        let content = format!("{}\n", block.join("\n"));
        // Pick the middle line as the find target.
        let mid = block.len() / 2;
        let find = &block[mid];

        match find_match(&content, find) {
            MatchOutcome::Unique { span, .. } => {
                prop_assert_eq!(&content[span], find.as_str());
            }
            // A line could coincidentally be a substring of another line's text;
            // in that case Ambiguous is the correct, honest answer.
            MatchOutcome::Ambiguous { .. } => {}
            MatchOutcome::NoMatch { .. } => {
                prop_assert!(false, "exact subspan must not be NoMatch");
            }
        }
    }

    /// Dropping leading indentation from a find still lands on the original
    /// indented line via the Normalized rung, and the span covers the original
    /// (indented) bytes.
    #[test]
    fn dropped_indentation_lands_on_original_via_normalized(
        block in distinct_block(),
        indent in 1usize..6,
    ) {
        let mid = block.len() / 2;
        let pad = " ".repeat(indent);
        // Indent only the target line in the content.
        let mut lines = block.clone();
        lines[mid] = format!("{pad}{}", block[mid]);
        let content = format!("{}\n", lines.join("\n"));

        // The find is the un-indented target line.
        let find = &block[mid];

        match find_match(&content, find) {
            MatchOutcome::Unique { span, .. } => {
                // Returned span must include the original indentation.
                let expected = format!("{pad}{}", block[mid]);
                prop_assert_eq!(&content[span], expected.as_str());
            }
            // If the un-indented line happens to also appear elsewhere, Ambiguous
            // is acceptable; the contract only forbids a silent wrong pick.
            MatchOutcome::Ambiguous { .. } => {}
            MatchOutcome::NoMatch { .. } => {
                prop_assert!(false, "dropped-indentation find must not be NoMatch");
            }
        }
    }

    /// A line that appears twice is never silently resolved to one location.
    #[test]
    fn duplicate_line_is_refused(line in ident_line(), filler in ident_line()) {
        prop_assume!(line != filler);
        let content = format!("{line}\n{filler}\n{line}\n");
        match find_match(&content, &line) {
            MatchOutcome::Ambiguous { candidates } => {
                prop_assert!(candidates.len() >= 2);
            }
            other => prop_assert!(false, "duplicate must be Ambiguous, got {:?}", other),
        }
    }
}
