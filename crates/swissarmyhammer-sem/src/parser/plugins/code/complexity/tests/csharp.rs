//! C#.

use super::*;
// -----------------------------------------------------------------
// C#
// -----------------------------------------------------------------

#[test]
fn csharp_switch_scores_once_and_arms_open_no_nesting() {
    let scored = method_in_class(
        "src/Foo.cs",
        r#"
class Foo {
    int Classify(int value) {
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
        "Classify",
    );
    assert_eq!(scored.cognitive_score, 1);
    assert_eq!(scored.max_nesting_depth, 1);
    assert_eq!(scored.branch_count, 3);
}

#[test]
fn csharp_if_else_if_else_chain_is_flat() {
    let scored = method_in_class(
        "src/Foo.cs",
        r#"
class Foo {
    int Pick(bool a, bool b) {
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
        "Pick",
    );
    assert_eq!(scored.cognitive_score, 3);
    assert_eq!(scored.max_nesting_depth, 1);
    assert_eq!(scored.max_else_if_chain, 1);
}

#[test]
fn csharp_nested_loops_deepen_the_score() {
    let scored = method_in_class(
        "src/Foo.cs",
        r#"
class Foo {
    int Deep(bool a, bool b, int[] items) {
        if (a) {
            foreach (int item in items) {
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
        "Deep",
    );
    assert_eq!(scored.cognitive_score, 10);
    assert_eq!(scored.max_nesting_depth, 4);
    assert_eq!(
        scored.max_loop_nesting, 2,
        "foreach > while is two loops deep"
    );
    assert!(scored.exceeds_gates());
}

#[test]
fn csharp_boolean_run_scores_once_mixed_run_scores_twice() {
    let and_only = method_in_class(
        "src/Foo.cs",
        r#"
class Foo {
    int AllThree(bool a, bool b, bool c) {
        if (a && b && c) {
            return 1;
        }
        return 0;
    }
}
"#,
        "AllThree",
    );
    assert_eq!(and_only.cognitive_score, 2);
    assert_eq!(and_only.max_boolean_operands, 3);

    let mixed = method_in_class(
        "src/Foo.cs",
        r#"
class Foo {
    int Mixed(bool a, bool b, bool c) {
        if (a && b || c) {
            return 1;
        }
        return 0;
    }
}
"#,
        "Mixed",
    );
    assert_eq!(mixed.cognitive_score, 3);
}

#[test]
fn csharp_test_attribute_at_the_definition_exempts_the_method() {
    let scored = method_in_class(
        "src/Foo.cs",
        r#"
class Foo {
    [Test]
    void DeeplyNested() {
        bool a = true, b = true;
        int[] items = { 1 };
        if (a) {
            foreach (int item in items) {
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
        "DeeplyNested",
    );

    assert!(scored.is_test, "[Test] marks the method as a test");
    assert_eq!(scored.max_nesting_depth, 4);
    assert!(!scored.exceeds_gates());
}

#[test]
fn csharp_repeated_scoring_never_drifts() {
    let source = r#"
class Foo {
    int Pick(bool a, bool b) {
        if (a) {
            return 1;
        } else if (b) {
            return 2;
        }
        return 0;
    }
}
"#;
    let first = cognitive_complexity("src/Foo.cs", source).expect("csharp is mapped");
    for run in 1..DETERMINISM_RUNS {
        let again = cognitive_complexity("src/Foo.cs", source).expect("csharp is mapped");
        assert_eq!(again, first, "run {run} drifted from run 0");
    }
}
