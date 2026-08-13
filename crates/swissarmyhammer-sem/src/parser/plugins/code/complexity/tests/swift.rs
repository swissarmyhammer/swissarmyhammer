//! Swift.

use super::*;
// -----------------------------------------------------------------
// Swift
// -----------------------------------------------------------------

#[test]
fn swift_switch_scores_once_and_arms_open_no_nesting() {
    let scored = only_function_for(
        "src/lib.swift",
        r#"
func classify(v: Int) -> Int {
    switch v {
    case 0:
        return 10
    case 1:
        return 11
    default:
        return 15
    }
}
"#,
    );
    assert_eq!(scored.cognitive_score, 1);
    assert_eq!(scored.max_nesting_depth, 1, "arms open no nesting level");
    assert_eq!(scored.branch_count, 3);
    assert!(!scored.exceeds_gates());
}

#[test]
fn swift_if_else_if_else_chain_is_flat() {
    let scored = only_function_for(
        "src/lib.swift",
        r#"
func pick(a: Bool, b: Bool) -> Int {
    if a {
        return 1
    } else if b {
        return 2
    } else {
        return 3
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
        "an else-if chain is flat, not a staircase"
    );
    assert_eq!(scored.max_else_if_chain, 1);
}

#[test]
fn swift_three_way_else_if_chain_increments_at_each_level() {
    // Swift's `if`/`else if`/`else` has no wrapping `else_clause`/
    // `alternative` field at all — the nested `if_statement` (or the
    // terminal `statements` body) is an EXTRA direct child of the SAME
    // outer `if_statement`, following an anonymous-but-named `else`
    // marker. This verifies `walk_marker_conditional` increments the
    // chain correctly at each of three levels, not just once.
    let scored = only_function_for(
        "src/lib.swift",
        r#"
func pick3(a: Bool, b: Bool, c: Bool) -> Int {
    if a {
        return 1
    } else if b {
        return 2
    } else if c {
        return 3
    } else {
        return 4
    }
}
"#,
    );
    assert_eq!(
        scored.cognitive_score, 4,
        "if + else-if + else-if + else is 1 + 1 + 1 + 1"
    );
    assert_eq!(
        scored.max_nesting_depth, 1,
        "a three-way else-if chain is still flat, not a staircase"
    );
    assert_eq!(scored.max_else_if_chain, 2, "two else-if links");
    assert_eq!(scored.branch_count, 4);
}

#[test]
fn swift_nested_loops_deepen_the_score() {
    let scored = only_function_for(
        "src/lib.swift",
        r#"
func deep(a: Bool, b: Bool, items: [Int]) -> Int {
    if a {
        for item in items {
            while b {
                if item > 0 {
                    return 1
                }
            }
        }
    }
    return 0
}
"#,
    );
    assert_eq!(scored.cognitive_score, 10);
    assert_eq!(scored.max_nesting_depth, 4);
    assert_eq!(scored.max_loop_nesting, 2, "for > while is two loops deep");
    assert!(scored.exceeds_gates());
}

#[test]
fn swift_boolean_run_scores_once_mixed_run_scores_twice() {
    let and_only = only_function_for(
        "src/lib.swift",
        r#"
func allThree(a: Bool, b: Bool, c: Bool) -> Bool {
    if a && b && c {
        return true
    }
    return false
}
"#,
    );
    assert_eq!(and_only.cognitive_score, 2);
    assert_eq!(and_only.max_boolean_operands, 3);

    let mixed = only_function_for(
        "src/lib.swift",
        r#"
func mixed(a: Bool, b: Bool, c: Bool) -> Bool {
    if a && b || c {
        return true
    }
    return false
}
"#,
    );
    assert_eq!(mixed.cognitive_score, 3);
}

#[test]
fn swift_test_attribute_at_the_definition_exempts_the_function() {
    let file = cognitive_complexity(
        "src/lib.swift",
        r#"
import Testing

@Test
func deeplyNested(a: Bool, b: Bool, items: [Int]) -> Int {
    if a {
        for item in items {
            while b {
                if item > 0 {
                    return 1
                }
            }
        }
    }
    return 0
}
"#,
    )
    .expect("swift is a mapped language");
    let scored = &file.functions[0];

    assert!(
        scored.is_test,
        "@Test is the real Swift Testing framework marker"
    );
    assert_eq!(scored.max_nesting_depth, 4, "the depth is still measured");
    assert!(
        !scored.exceeds_gates(),
        "a test is exempt even at depth 4: {scored:?}"
    );
}

#[test]
fn swift_repeated_scoring_never_drifts() {
    let source = r#"
func pick(a: Bool, b: Bool) -> Int {
    if a {
        return 1
    } else if b {
        return 2
    }
    return 0
}
"#;
    let first = cognitive_complexity("src/lib.swift", source).expect("swift is mapped");
    for run in 1..DETERMINISM_RUNS {
        let again = cognitive_complexity("src/lib.swift", source).expect("swift is mapped");
        assert_eq!(again, first, "run {run} drifted from run 0");
    }
}
