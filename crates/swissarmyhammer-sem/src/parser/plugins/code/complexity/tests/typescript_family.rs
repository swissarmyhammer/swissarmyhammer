//! TypeScript, TSX and JavaScript.
//!
//! The three rows that share `typescript_family_spec`, each scored on the
//! same shapes: switch, if-else chain, nested loops, boolean runs, and the
//! test decorator.

use super::*;

// -----------------------------------------------------------------
// TypeScript
// -----------------------------------------------------------------

#[test]
fn typescript_switch_scores_once_and_arms_open_no_nesting() {
    let scored = only_function_for(
        "src/lib.ts",
        r#"
function classify(value: number): number {
    switch (value) {
        case 0:
            return 10;
        case 1:
            return 11;
        default:
            return 15;
    }
}
"#,
    );
    assert_eq!(
        scored.cognitive_score, 1,
        "a switch scores once for the whole construct"
    );
    assert_eq!(scored.max_nesting_depth, 1, "arms open no nesting level");
    assert_eq!(
        scored.branch_count, 3,
        "three arms are three decision points"
    );
    assert!(!scored.exceeds_gates());
}

#[test]
fn typescript_if_else_if_else_chain_is_flat() {
    let scored = only_function_for(
        "src/lib.ts",
        r#"
function pick(a: boolean, b: boolean): number {
    if (a) {
        return 1;
    } else if (b) {
        return 2;
    } else {
        return 3;
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
fn typescript_nested_loops_deepen_the_score() {
    let scored = only_function_for(
        "src/lib.ts",
        r#"
function deep(a: boolean, b: boolean, items: number[]): number {
    if (a) {
        for (const item of items) {
            while (b) {
                if (item > 0) {
                    return 1;
                }
            }
        }
    }
    return 0;
}
"#,
    );
    assert_eq!(
        scored.cognitive_score, 10,
        "1 + 2 + 3 + 4 as each construct nests one deeper"
    );
    assert_eq!(scored.max_nesting_depth, 4);
    assert_eq!(
        scored.max_loop_nesting, 2,
        "for-of > while is two loops deep"
    );
    assert!(scored.exceeds_gates());
}

#[test]
fn typescript_boolean_run_scores_once_mixed_run_scores_twice() {
    let and_only = only_function_for(
        "src/lib.ts",
        r#"
function allThree(a: boolean, b: boolean, c: boolean): number {
    if (a && b && c) {
        return 1;
    }
    return 0;
}
"#,
    );
    assert_eq!(
        and_only.cognitive_score, 2,
        "the if scores 1, the one && run 1"
    );
    assert_eq!(and_only.max_boolean_operands, 3);

    let mixed = only_function_for(
        "src/lib.ts",
        r#"
function mixed(a: boolean, b: boolean, c: boolean): number {
    if (a && b || c) {
        return 1;
    }
    return 0;
}
"#,
    );
    assert_eq!(
        mixed.cognitive_score, 3,
        "the if scores 1, and && then || are two runs"
    );
}

#[test]
fn typescript_test_decorator_at_the_definition_exempts_the_method() {
    let file = cognitive_complexity(
        "src/lib.ts",
        r#"
class Foo {
    @Test
    deeplyNested(a: boolean, b: boolean, items: number[]): number {
        if (a) {
            for (const item of items) {
                while (b) {
                    if (item > 0) {
                        return 1;
                    }
                }
            }
        }
        return 0;
    }
}
"#,
    )
    .expect("typescript is a mapped language");
    let scored = &file.functions[0];

    assert!(scored.is_test, "@Test marks the method as a test");
    assert_eq!(scored.max_nesting_depth, 4, "the depth is still measured");
    assert!(
        !scored.exceeds_gates(),
        "a test is exempt even at depth 4: {scored:?}"
    );
}

#[test]
fn typescript_repeated_scoring_never_drifts() {
    let source = r#"
function pick(a: boolean, b: boolean): number {
    if (a) {
        return 1;
    } else if (b) {
        return 2;
    }
    return 0;
}
"#;
    let first = cognitive_complexity("src/lib.ts", source).expect("typescript is mapped");
    for run in 1..DETERMINISM_RUNS {
        let again = cognitive_complexity("src/lib.ts", source).expect("typescript is mapped");
        assert_eq!(again, first, "run {run} drifted from run 0");
    }
}

// -----------------------------------------------------------------
// TSX
// -----------------------------------------------------------------

#[test]
fn tsx_switch_scores_once_and_arms_open_no_nesting() {
    let scored = only_function_for(
        "src/App.tsx",
        r#"
function classify(value: number): number {
    switch (value) {
        case 0:
            return 10;
        case 1:
            return 11;
        default:
            return 15;
    }
}
"#,
    );
    assert_eq!(scored.cognitive_score, 1);
    assert_eq!(scored.max_nesting_depth, 1);
    assert_eq!(scored.branch_count, 3);
}

#[test]
fn tsx_if_else_if_else_chain_is_flat() {
    let scored = only_function_for(
        "src/App.tsx",
        r#"
function pick(a: boolean, b: boolean): number {
    if (a) {
        return 1;
    } else if (b) {
        return 2;
    } else {
        return 3;
    }
}
"#,
    );
    assert_eq!(scored.cognitive_score, 3);
    assert_eq!(scored.max_nesting_depth, 1);
    assert_eq!(scored.max_else_if_chain, 1);
}

#[test]
fn tsx_nested_loops_deepen_the_score() {
    let scored = only_function_for(
        "src/App.tsx",
        r#"
function deep(a: boolean, b: boolean, items: number[]): number {
    if (a) {
        for (const item of items) {
            while (b) {
                if (item > 0) {
                    return 1;
                }
            }
        }
    }
    return 0;
}
"#,
    );
    assert_eq!(scored.cognitive_score, 10);
    assert_eq!(scored.max_nesting_depth, 4);
    assert_eq!(scored.max_loop_nesting, 2);
    assert!(scored.exceeds_gates());
}

#[test]
fn tsx_boolean_run_scores_once_mixed_run_scores_twice() {
    let and_only = only_function_for(
        "src/App.tsx",
        r#"
function allThree(a: boolean, b: boolean, c: boolean): number {
    if (a && b && c) {
        return 1;
    }
    return 0;
}
"#,
    );
    assert_eq!(and_only.cognitive_score, 2);
    assert_eq!(and_only.max_boolean_operands, 3);

    let mixed = only_function_for(
        "src/App.tsx",
        r#"
function mixed(a: boolean, b: boolean, c: boolean): number {
    if (a && b || c) {
        return 1;
    }
    return 0;
}
"#,
    );
    assert_eq!(mixed.cognitive_score, 3);
}

#[test]
fn tsx_test_decorator_at_the_definition_exempts_the_method() {
    let file = cognitive_complexity(
        "src/App.tsx",
        r#"
class Foo {
    @Test
    deeplyNested(a: boolean, b: boolean, items: number[]): number {
        if (a) {
            for (const item of items) {
                while (b) {
                    if (item > 0) {
                        return 1;
                    }
                }
            }
        }
        return 0;
    }
}
"#,
    )
    .expect("tsx is a mapped language");
    let scored = &file.functions[0];

    assert!(scored.is_test);
    assert_eq!(scored.max_nesting_depth, 4);
    assert!(!scored.exceeds_gates());
}

#[test]
fn tsx_repeated_scoring_never_drifts() {
    let source = r#"
function pick(a: boolean, b: boolean): number {
    if (a) {
        return 1;
    } else if (b) {
        return 2;
    }
    return 0;
}
"#;
    let first = cognitive_complexity("src/App.tsx", source).expect("tsx is mapped");
    for run in 1..DETERMINISM_RUNS {
        let again = cognitive_complexity("src/App.tsx", source).expect("tsx is mapped");
        assert_eq!(again, first, "run {run} drifted from run 0");
    }
}

// -----------------------------------------------------------------
// JavaScript
// -----------------------------------------------------------------

#[test]
fn javascript_switch_scores_once_and_arms_open_no_nesting() {
    let scored = only_function_for(
        "src/lib.js",
        r#"
function classify(value) {
    switch (value) {
        case 0:
            return 10;
        case 1:
            return 11;
        default:
            return 15;
    }
}
"#,
    );
    assert_eq!(scored.cognitive_score, 1);
    assert_eq!(scored.max_nesting_depth, 1);
    assert_eq!(scored.branch_count, 3);
}

#[test]
fn javascript_if_else_if_else_chain_is_flat() {
    let scored = only_function_for(
        "src/lib.js",
        r#"
function pick(a, b) {
    if (a) {
        return 1;
    } else if (b) {
        return 2;
    } else {
        return 3;
    }
}
"#,
    );
    assert_eq!(scored.cognitive_score, 3);
    assert_eq!(scored.max_nesting_depth, 1);
    assert_eq!(scored.max_else_if_chain, 1);
}

#[test]
fn javascript_nested_loops_deepen_the_score() {
    let scored = only_function_for(
        "src/lib.js",
        r#"
function deep(a, b, items) {
    if (a) {
        for (const item of items) {
            while (b) {
                if (item > 0) {
                    return 1;
                }
            }
        }
    }
    return 0;
}
"#,
    );
    assert_eq!(scored.cognitive_score, 10);
    assert_eq!(scored.max_nesting_depth, 4);
    assert_eq!(scored.max_loop_nesting, 2);
    assert!(scored.exceeds_gates());
}

#[test]
fn javascript_boolean_run_scores_once_mixed_run_scores_twice() {
    let and_only = only_function_for(
        "src/lib.js",
        r#"
function allThree(a, b, c) {
    if (a && b && c) {
        return 1;
    }
    return 0;
}
"#,
    );
    assert_eq!(and_only.cognitive_score, 2);
    assert_eq!(and_only.max_boolean_operands, 3);

    let mixed = only_function_for(
        "src/lib.js",
        r#"
function mixed(a, b, c) {
    if (a && b || c) {
        return 1;
    }
    return 0;
}
"#,
    );
    assert_eq!(mixed.cognitive_score, 3);
}

#[test]
fn javascript_test_decorator_at_the_definition_exempts_the_method() {
    let file = cognitive_complexity(
        "src/lib.js",
        r#"
class Foo {
    @Test
    deeplyNested(a, b, items) {
        if (a) {
            for (const item of items) {
                while (b) {
                    if (item > 0) {
                        return 1;
                    }
                }
            }
        }
        return 0;
    }
}
"#,
    )
    .expect("javascript is a mapped language");
    let scored = &file.functions[0];

    assert!(scored.is_test, "@Test marks the method as a test");
    assert_eq!(scored.max_nesting_depth, 4);
    assert!(!scored.exceeds_gates());
}

#[test]
fn javascript_repeated_scoring_never_drifts() {
    let source = r#"
function pick(a, b) {
    if (a) {
        return 1;
    } else if (b) {
        return 2;
    }
    return 0;
}
"#;
    let first = cognitive_complexity("src/lib.js", source).expect("javascript is mapped");
    for run in 1..DETERMINISM_RUNS {
        let again = cognitive_complexity("src/lib.js", source).expect("javascript is mapped");
        assert_eq!(again, first, "run {run} drifted from run 0");
    }
}

/// jest/mocha spell a test as a call whose callback holds the body, so the
/// callback is the definition the exemption has to land on — and the call's
/// description names it. An ordinary callback beside it stays anonymous and
/// stays no test.
#[test]
fn javascript_marks_a_jest_callback_as_a_test_and_names_it() {
    let file = cognitive_complexity(
        "src/lib.js",
        r#"
it("adds up", () => {
    expect(add(1, 1)).toBe(2);
});

items.forEach((item) => {
    consume(item);
});
"#,
    )
    .expect("javascript is a mapped language");

    let named: Vec<(&str, bool)> = file
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function.is_test))
        .collect();
    assert_eq!(named, [("adds up", true), ("<anonymous>", false)]);
}
