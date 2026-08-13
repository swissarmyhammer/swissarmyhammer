//! Rust, the reference language.
//!
//! Match arms, if-else chains, nesting, boolean runs, labelled jumps, the
//! test exemption, an unmapped language, and the determinism of a score.

use super::*;

#[test]
fn collect_line_tags_sits_far_below_both_gates() {
    let scored = only_function(COLLECT_LINE_TAGS);

    assert_eq!(scored.cognitive_score, 5, "while + if + match, nested once");
    assert_eq!(
        scored.max_nesting_depth, 2,
        "the match sits inside one while, so depth is 2 — never the 4 the review reported"
    );
    assert!(
        !scored.exceeds_gates(),
        "the function the review flagged must not trip either gate: {scored:?}"
    );
}

#[test]
fn edit_line_markers_sits_below_both_gates() {
    let scored = only_function(EDIT_LINE_MARKERS);

    assert_eq!(
        scored.max_nesting_depth, 3,
        "while > match > if let is depth 3, one short of the gate"
    );
    assert!(
        scored.cognitive_score < COGNITIVE_COMPLEXITY_THRESHOLD,
        "score {} should stay under the gate",
        scored.cognitive_score
    );
    assert!(
        !scored.exceeds_gates(),
        "the second function the review flagged must not trip either gate: {scored:?}"
    );
}

#[test]
fn match_arms_are_one_decision_not_one_nesting_level_each() {
    let scored = only_function(
        r#"
fn classify(value: u8) -> u8 {
    match value {
        0 => 10,
        1 => 11,
        2 => 12,
        3 => 13,
        4 => 14,
        _ => 15,
    }
}
"#,
    );

    assert_eq!(
        scored.cognitive_score, 1,
        "a match scores once for the whole construct, never once per arm"
    );
    assert_eq!(
        scored.max_nesting_depth, 1,
        "arms are branches of one decision, so they open no nesting level"
    );
    assert_eq!(scored.branch_count, 6, "six arms are six decision points");
    assert!(
        !scored.exceeds_gates(),
        "many simple arms must stay unflagged: {scored:?}"
    );
}

#[test]
fn an_if_else_if_else_chain_scores_one_per_branch_with_no_nesting() {
    let scored = only_function(
        r#"
fn pick(a: bool, b: bool) -> u8 {
    if a {
        1
    } else if b {
        2
    } else {
        3
    }
}
"#,
    );

    assert_eq!(
        scored.cognitive_score, 3,
        "if + else if + else is 1 + 1 + 1"
    );
    assert_eq!(
        scored.max_nesting_depth, 1,
        "an else-if chain is a flat set of branches, not a staircase"
    );
    assert_eq!(scored.max_else_if_chain, 1, "one else-if link");
}

#[test]
fn nesting_deepens_the_score_and_trips_the_depth_gate() {
    let scored = only_function(
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

    assert_eq!(
        scored.cognitive_score, 10,
        "1 + 2 + 3 + 4 as each construct nests one deeper"
    );
    assert_eq!(
        scored.max_nesting_depth, 4,
        "if > for > while > if is depth 4"
    );
    assert_eq!(scored.max_loop_nesting, 2, "for > while is two loops deep");
    assert!(
        scored.exceeds_gates(),
        "genuinely deep nesting must still be flagged: {scored:?}"
    );
}

#[test]
fn one_run_of_like_boolean_operators_scores_once() {
    let scored = only_function(
        r#"
fn all_three(a: bool, b: bool, c: bool) -> u8 {
    if a && b && c {
        return 1;
    }
    0
}
"#,
    );

    assert_eq!(
        scored.cognitive_score, 2,
        "the if scores 1, the one && run 1"
    );
    assert_eq!(scored.max_boolean_operands, 3, "three operands in the run");
}

#[test]
fn mixed_boolean_operators_score_once_per_run() {
    let scored = only_function(
        r#"
fn mixed(a: bool, b: bool, c: bool) -> u8 {
    if a && b || c {
        return 1;
    }
    0
}
"#,
    );

    assert_eq!(
        scored.cognitive_score, 3,
        "the if scores 1, and && then || are two runs"
    );
}

#[test]
fn a_labelled_jump_adds_a_flat_increment() {
    let scored = only_function(
        r#"
fn scan(rows: &[u8], cols: &[u8]) -> u8 {
    'outer: for row in rows {
        for col in cols {
            if row == col {
                continue 'outer;
            }
        }
    }
    0
}
"#,
    );

    assert_eq!(
        scored.cognitive_score, 7,
        "for 1 + for 2 + if 3 + labelled continue 1"
    );
}

#[test]
fn a_break_carrying_a_value_is_not_a_labelled_jump() {
    // `break 5` returns a value from a loop. It is ordinary control flow, not
    // a jump to a label, so it must not add an increment.
    let scored = only_function(
        r#"
fn first(items: &[u8]) -> u8 {
    loop {
        break 5;
    }
}
"#,
    );

    assert_eq!(
        scored.cognitive_score, 1,
        "only the loop scores; a valued break is not a labelled jump"
    );
}

#[test]
fn a_test_attribute_at_the_definition_exempts_the_function() {
    let file = cognitive_complexity(
        "src/lib.rs",
        r#"
#[test]
fn deeply_nested_test(a: bool, b: bool, items: &[u8]) {
    if a {
        for item in items {
            while b {
                if *item > 0 {
                    assert!(true);
                }
            }
        }
    }
}
"#,
    )
    .expect("rust is a mapped language");
    let scored = &file.functions[0];

    assert!(scored.is_test, "#[test] marks the definition as a test");
    assert_eq!(scored.max_nesting_depth, 4, "the depth is still measured");
    assert!(
        !scored.exceeds_gates(),
        "a test is exempt even at depth 4: {scored:?}"
    );
}

#[test]
fn a_namespaced_test_attribute_marks_a_test() {
    let file = cognitive_complexity("src/lib.rs", "#[tokio::test]\nasync fn spawns() {}\n")
        .expect("rust is a mapped language");

    assert!(
        file.functions[0].is_test,
        "#[tokio::test] is a test attribute"
    );
}

#[test]
fn a_helper_beside_tests_is_not_a_test() {
    let file = cognitive_complexity(
        "src/tag_parser_test.rs",
        r#"
#[test]
fn a_real_test() {}

fn build_request(a: bool) -> u8 {
    if a { 1 } else { 0 }
}
"#,
    )
    .expect("rust is a mapped language");
    let helper = file
        .functions
        .iter()
        .find(|f| f.name == "build_request")
        .expect("the helper is scored");

    assert!(
        !helper.is_test,
        "a helper is judged at its own definition, never by the file name"
    );
}

#[test]
fn an_unmapped_language_is_not_computed_rather_than_zero() {
    // Bash carries no `ComplexitySpec` row: it has no attribute/
    // annotation grammar construct at all, and its one real-world test
    // convention — bats-core's `# @test "description"` comment marker —
    // is unstructured free text inside a generic `comment` node (verified
    // by parsing a `# @test` comment immediately preceding a
    // `function_definition` and confirming it is a plain, unstructured
    // `comment` sibling exactly like an ordinary doc comment or license
    // header would be). Treating any comment as a potential test marker
    // would be unsafe and overbroad, so bash stays unmapped rather than
    // guessing.
    assert!(
        cognitive_complexity("src/app.sh", "f() {\n  echo 1\n}\n").is_none(),
        "a language with no spec must report not-computed, never a zero score"
    );
}

#[test]
fn a_non_code_path_is_not_computed() {
    assert!(
        cognitive_complexity("README.md", "# Title\n").is_none(),
        "a path with no language mapping must report not-computed"
    );
}

#[test]
fn methods_in_an_impl_block_are_scored_individually() {
    let file = cognitive_complexity(
        "src/lib.rs",
        r#"
impl Parser {
    fn simple(&self) -> u8 {
        0
    }

    fn branchy(&self, a: bool) -> u8 {
        if a { 1 } else { 0 }
    }
}
"#,
    )
    .expect("rust is a mapped language");

    let names: Vec<&str> = file.functions.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, vec!["simple", "branchy"], "both methods are scored");
    assert_eq!(file.functions[0].cognitive_score, 0);
    assert_eq!(file.functions[1].cognitive_score, 2, "if plus else");
}

#[test]
fn repeated_scoring_of_one_source_never_drifts() {
    // The whole point of the probe: the number handed to the model is a pure
    // function of the source, so N runs agree exactly. No model is involved,
    // so this belongs in CI rather than in a manual N-run review harness.
    let source = format!("{COLLECT_LINE_TAGS}\n{EDIT_LINE_MARKERS}");
    let first =
        cognitive_complexity("src/tag_parser.rs", &source).expect("rust is a mapped language");

    for run in 1..DETERMINISM_RUNS {
        let again =
            cognitive_complexity("src/tag_parser.rs", &source).expect("rust is a mapped language");
        assert_eq!(
            again, first,
            "run {run} produced a different score set than run 0"
        );
    }
}

#[test]
fn the_scorer_is_insensitive_to_the_order_functions_are_declared_in() {
    // Two files with the same functions in either order must score each
    // function the same. A scorer that leaked state between functions would
    // fail here even though every single-function test passed.
    let forward = cognitive_complexity(
        "src/a.rs",
        &format!("{COLLECT_LINE_TAGS}\n{EDIT_LINE_MARKERS}"),
    )
    .expect("rust is a mapped language");
    let reversed = cognitive_complexity(
        "src/a.rs",
        &format!("{EDIT_LINE_MARKERS}\n{COLLECT_LINE_TAGS}"),
    )
    .expect("rust is a mapped language");

    let score_of = |file: &FileComplexity, name: &str| -> u32 {
        file.functions
            .iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("{name} is scored"))
            .cognitive_score
    };

    assert_eq!(
        score_of(&forward, "collect_line_tags"),
        score_of(&reversed, "collect_line_tags")
    );
    assert_eq!(
        score_of(&forward, "edit_line_markers"),
        score_of(&reversed, "edit_line_markers")
    );
}
