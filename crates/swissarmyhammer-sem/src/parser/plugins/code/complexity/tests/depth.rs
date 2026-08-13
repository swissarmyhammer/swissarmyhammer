//! The traversal-depth cap.
//!
//! Nesting deeper than the cap stops the walk instead of growing the
//! stack, and the score the walk already has stands.

use super::*;
// -----------------------------------------------------------------
// Pathological depth — the traversal-depth cap
// -----------------------------------------------------------------

/// A source string with `depth` levels of `if true { ... }` nested inside
/// one function body — far beyond any plausible real function, and far
/// beyond [`MAX_TRAVERSAL_DEPTH`].
fn deeply_nested_if_source(depth: usize) -> String {
    let mut source = String::from("fn deep() -> u32 {\n");
    source.push_str(&"if true {\n".repeat(depth));
    source.push_str("1\n");
    source.push_str(&"}\n".repeat(depth));
    source.push_str("}\n");
    source
}

#[test]
fn pathological_nesting_does_not_crash_and_is_reported_as_partial() {
    // Thousands of levels — two orders of magnitude past MAX_TRAVERSAL_DEPTH,
    // and far past any plausible real function. Reaching this assertion at
    // all (rather than a stack-overflow abort) is the point of the test.
    let source = deeply_nested_if_source(5_000);

    let file = cognitive_complexity("src/lib.rs", &source).expect("rust is a mapped language");
    assert_eq!(
        file.functions.len(),
        1,
        "the outer function itself is still found and scored, got {:?}",
        file.functions
    );
    let scored = &file.functions[0];

    assert!(
        scored.is_partial,
        "nesting past the traversal cap must report a partial result, never a fabricated \
         score: {scored:?}"
    );
    assert!(
        scored.exceeds_gates(),
        "a partial result must never read as \"under the gates\": {scored:?}"
    );
}

#[test]
fn nesting_well_under_the_traversal_cap_is_never_marked_partial() {
    // A depth (well) below MAX_TRAVERSAL_DEPTH but far above every other
    // fixture in this suite, to pin the cap does not clip ordinary — if
    // unusually deep — real code.
    let source = deeply_nested_if_source(32);

    let file = cognitive_complexity("src/lib.rs", &source).expect("rust is a mapped language");
    let scored = &file.functions[0];

    assert!(
        !scored.is_partial,
        "32 levels of nesting sits far below the traversal cap: {scored:?}"
    );
    assert_eq!(scored.max_nesting_depth, 32);
}

#[test]
fn pinned_small_fixtures_are_unaffected_by_the_traversal_cap() {
    // Every depth 1-4 fixture already pinned above must produce the exact
    // same numbers with the cap in place — the cap changes nothing for any
    // plausible real function.
    let collect_line_tags = only_function(COLLECT_LINE_TAGS);
    assert!(!collect_line_tags.is_partial);
    assert_eq!(collect_line_tags.cognitive_score, 5);
    assert_eq!(collect_line_tags.max_nesting_depth, 2);

    let edit_line_markers = only_function(EDIT_LINE_MARKERS);
    assert!(!edit_line_markers.is_partial);
    assert_eq!(edit_line_markers.max_nesting_depth, 3);

    let deep = only_function(
        r#"
fn deep(a: bool, b: bool, items: &[u8]) -> u8 {
    if a {
        for item in items {
            while b {
                if *item > 0 {
                    return 1;
                }
            }
        }
    }
    0
}
"#,
    );
    assert!(!deep.is_partial);
    assert_eq!(deep.cognitive_score, 10);
    assert_eq!(deep.max_nesting_depth, 4);
}
