//! Property tests for the hashline anchor round-trip invariants.
//!
//! These exercise the tag -> edit -> re-tag cycle, proximity resolution under
//! drift, mismatch rejection, and reformatting (re-indentation) tolerance over
//! generated inputs.

use proptest::prelude::*;
use swissarmyhammer_hashline::{apply, hash_line, parse_anchor, render_hash, tag, AnchorOp};

/// Lines of "code-ish" text with no embedded newlines or carriage returns, so
/// each generated element is exactly one line.
fn line_strategy() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            Just(' '),
            Just('\t'),
            proptest::char::range('a', 'z'),
            proptest::char::range('0', '9'),
            Just('='),
            Just(';'),
            Just('_'),
        ],
        0..20,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

/// A non-empty line: at least one non-whitespace character, so it survives a
/// `join("\n")` round-trip unambiguously (an empty final line would be elided
/// by the "no phantom trailing line" model that [`swissarmyhammer_hashline`]
/// uses, breaking the 1:1 index mapping the oracle relies on).
fn nonempty_line_strategy() -> impl Strategy<Value = String> {
    (line_strategy(), proptest::char::range('a', 'z'))
        .prop_map(|(prefix, c)| format!("{prefix}{c}"))
}

/// A vector of one-or-more lines. The final line is non-empty so that
/// `lines.join("\n")` decomposes back into exactly `lines.len()` lines.
fn lines_strategy() -> impl Strategy<Value = Vec<String>> {
    (
        proptest::collection::vec(line_strategy(), 0..11),
        nonempty_line_strategy(),
    )
        .prop_map(|(mut lines, last)| {
            lines.push(last);
            lines
        })
}

fn content_strategy() -> impl Strategy<Value = String> {
    lines_strategy().prop_map(|lines| lines.join("\n"))
}

proptest! {
    /// Every tagged line parses back to its line number and hash, and the
    /// trailing text after the pipe is the original line.
    #[test]
    fn tag_lines_parse_back(content in content_strategy()) {
        let tagged = tag(&content, 1);
        for (i, (orig, tagged_line)) in content.lines().zip(tagged.lines()).enumerate() {
            let n = i + 1;
            let (parsed_line, parsed_hash) =
                parse_anchor(tagged_line).expect("tagged line must parse");
            prop_assert_eq!(parsed_line, n);
            prop_assert_eq!(parsed_hash, hash_line(orig));
            // Text after the first pipe is the original line.
            let after_pipe = tagged_line.split_once('|').unwrap().1;
            prop_assert_eq!(after_pipe, orig);
        }
    }

    /// Tagging then re-tagging the same content is stable.
    #[test]
    fn tag_is_idempotent_on_hashes(content in content_strategy()) {
        prop_assert_eq!(tag(&content, 1), tag(&content, 1));
    }

    /// Replacing a single line via its exact anchor yields content where that
    /// line is the replacement and all others are unchanged.
    #[test]
    fn exact_anchor_replaces_target_line(lines in lines_strategy(), seed in any::<usize>()) {
        // Joining with "\n" yields content with no trailing newline, so the
        // round-trip through `apply` reconstructs exactly via `join("\n")`.
        let content = lines.join("\n");
        let idx = seed % lines.len();
        let target = &lines[idx];
        let op = AnchorOp {
            line: idx + 1,
            hash: hash_line(target),
            replacement: "REPLACED".to_string(),
        };
        let applied = apply(&content, &[op]).expect("exact anchor must resolve");
        let mut expected = lines.clone();
        expected[idx] = "REPLACED".to_string();
        prop_assert_eq!(applied.content, expected.join("\n"));
    }

    /// Re-indenting the target line (adding leading/trailing horizontal
    /// whitespace) does not break anchor resolution.
    #[test]
    fn reindentation_preserves_anchor(lines in lines_strategy(), seed in any::<usize>()) {
        let idx = seed % lines.len();
        let target = &lines[idx];
        let hash = hash_line(target);

        // Re-indent the target line in the actual content.
        let mut reindented = lines.clone();
        reindented[idx] = format!("\t  {}  ", target.trim_matches([' ', '\t']));
        let reindented_content = reindented.join("\n");

        let op = AnchorOp {
            line: idx + 1,
            hash,
            replacement: "REPLACED".to_string(),
        };
        let applied =
            apply(&reindented_content, &[op]).expect("re-indented line must still resolve");
        let mut expected = reindented.clone();
        expected[idx] = "REPLACED".to_string();
        prop_assert_eq!(applied.content, expected.join("\n"));
    }

    /// An anchor whose expected hash matches no line anywhere is rejected, and
    /// the error carries the freshly re-tagged current content.
    #[test]
    fn impossible_hash_is_rejected(content in content_strategy()) {
        // Find a hash value that no line in the content produces.
        let present: std::collections::HashSet<u8> =
            content.lines().map(hash_line).collect();
        let missing = (0u16..=255)
            .map(|v| v as u8)
            .find(|h| !present.contains(h));
        // If every possible hash is present (extremely unlikely for small
        // inputs) skip — there's no impossible hash to test.
        if let Some(missing) = missing {
            let op = AnchorOp {
                line: 1,
                hash: missing,
                replacement: "REPLACED".to_string(),
            };
            let err = apply(&content, &[op]).expect_err("impossible hash must be rejected");
            let swissarmyhammer_hashline::HashlineError::Mismatch { retagged, .. } = err;
            prop_assert_eq!(retagged, tag(&content, 1));
        }
    }
}

/// A concrete drift scenario: inserting lines above the target shifts it, and
/// proximity search still resolves the original anchor.
#[test]
fn proximity_resolves_drift() {
    // Original content was "alpha\nbravo\ncharlie\ndelta"; the anchor was made
    // against line 3 ("charlie").
    let hash = hash_line("charlie");
    // Now two lines inserted near the top; "charlie" drifted to line 5.
    let drifted = "alpha\nINSERT1\nINSERT2\nbravo\ncharlie\ndelta";
    let op = AnchorOp {
        line: 3,
        hash,
        replacement: "CHARLIE".to_string(),
    };
    let applied = apply(drifted, &[op]).expect("proximity must resolve drift");
    assert_eq!(
        applied.content,
        "alpha\nINSERT1\nINSERT2\nbravo\nCHARLIE\ndelta"
    );
}

/// Round-trip: tag content, parse an anchor out of the tagged output, use it to
/// drive an edit, then re-tag the result.
#[test]
fn tag_edit_retag_round_trip() {
    let content = "one\ntwo\nthree";
    let tagged = tag(content, 1);
    // Pull the anchor for the second line back out of the tagged stream.
    let second = tagged.lines().nth(1).unwrap();
    let (line, hash) = parse_anchor(second).expect("anchor parses");
    assert_eq!(render_hash(hash), render_hash(hash_line("two")));
    let op = AnchorOp {
        line,
        hash,
        replacement: "TWO".to_string(),
    };
    let applied = apply(content, &[op]).expect("edit resolves");
    assert_eq!(applied.content, "one\nTWO\nthree");
    // Re-tag the edited content; line 2 now hashes to TWO.
    let retagged = tag(&applied.content, 1);
    let (rline, rhash) = parse_anchor(retagged.lines().nth(1).unwrap()).unwrap();
    assert_eq!(rline, 2);
    assert_eq!(rhash, hash_line("TWO"));
}
