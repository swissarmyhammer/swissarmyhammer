//! Java.

use super::*;
// -----------------------------------------------------------------
// Java
// -----------------------------------------------------------------

#[test]
fn java_switch_scores_once_and_arms_open_no_nesting() {
    let scored = method_in_class(
        "src/Foo.java",
        r#"
class Foo {
    int classify(int value) {
        switch (value) {
            case 0:
                return 10;
            case 1:
                return 11;
            default:
                return 15;
        }
    }
}
"#,
        "classify",
    );
    assert_eq!(
        scored.cognitive_score, 1,
        "a switch scores once for the whole construct"
    );
    assert_eq!(scored.max_nesting_depth, 1, "arms open no nesting level");
    assert_eq!(scored.branch_count, 3);
}

#[test]
fn java_if_else_if_else_chain_is_flat() {
    let scored = method_in_class(
        "src/Foo.java",
        r#"
class Foo {
    int pick(boolean a, boolean b) {
        if (a) {
            return 1;
        } else if (b) {
            return 2;
        } else {
            return 3;
        }
    }
}
"#,
        "pick",
    );
    assert_eq!(scored.cognitive_score, 3);
    assert_eq!(
        scored.max_nesting_depth, 1,
        "an else-if chain is flat, not a staircase"
    );
    assert_eq!(scored.max_else_if_chain, 1);
}

#[test]
fn java_three_way_else_if_chain_reports_the_longest_link() {
    let scored = method_in_class(
        "src/Foo.java",
        r#"
class Foo {
    int pick(boolean a, boolean b, boolean c) {
        if (a) {
            return 1;
        } else if (b) {
            return 2;
        } else if (c) {
            return 3;
        } else {
            return 4;
        }
    }
}
"#,
        "pick",
    );
    assert_eq!(scored.cognitive_score, 4);
    assert_eq!(scored.max_nesting_depth, 1);
    assert_eq!(scored.max_else_if_chain, 2, "two else-if links");
}

#[test]
fn java_nested_loops_deepen_the_score() {
    let scored = method_in_class(
        "src/Foo.java",
        r#"
class Foo {
    int deep(boolean a, boolean b, int[] items) {
        if (a) {
            for (int item : items) {
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
        "deep",
    );
    assert_eq!(scored.cognitive_score, 10);
    assert_eq!(scored.max_nesting_depth, 4);
    assert_eq!(
        scored.max_loop_nesting, 2,
        "enhanced-for > while is two loops deep"
    );
    assert!(scored.exceeds_gates());
}

#[test]
fn java_boolean_run_scores_once_mixed_run_scores_twice() {
    let and_only = method_in_class(
        "src/Foo.java",
        r#"
class Foo {
    int allThree(boolean a, boolean b, boolean c) {
        if (a && b && c) {
            return 1;
        }
        return 0;
    }
}
"#,
        "allThree",
    );
    assert_eq!(and_only.cognitive_score, 2);
    assert_eq!(and_only.max_boolean_operands, 3);

    let mixed = method_in_class(
        "src/Foo.java",
        r#"
class Foo {
    int mixed(boolean a, boolean b, boolean c) {
        if (a && b || c) {
            return 1;
        }
        return 0;
    }
}
"#,
        "mixed",
    );
    assert_eq!(mixed.cognitive_score, 3);
}

#[test]
fn java_test_annotation_at_the_definition_exempts_the_method() {
    let scored = method_in_class(
        "src/Foo.java",
        r#"
class Foo {
    @Test
    void deeplyNested() {
        boolean a = true, b = true;
        int[] items = {1};
        if (a) {
            for (int item : items) {
                while (b) {
                    if (item > 0) {
                        return;
                    }
                }
            }
        }
    }
}
"#,
        "deeplyNested",
    );

    assert!(scored.is_test, "@Test marks the method as a test");
    assert_eq!(scored.max_nesting_depth, 4, "the depth is still measured");
    assert!(
        !scored.exceeds_gates(),
        "a test is exempt even at depth 4: {scored:?}"
    );
}

#[test]
fn java_repeated_scoring_never_drifts() {
    let source = r#"
class Foo {
    int pick(boolean a, boolean b) {
        if (a) {
            return 1;
        } else if (b) {
            return 2;
        }
        return 0;
    }
}
"#;
    let first = cognitive_complexity("src/Foo.java", source).expect("java is mapped");
    for run in 1..DETERMINISM_RUNS {
        let again = cognitive_complexity("src/Foo.java", source).expect("java is mapped");
        assert_eq!(again, first, "run {run} drifted from run 0");
    }
}
