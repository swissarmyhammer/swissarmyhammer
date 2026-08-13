//! C and C++, the two rows that share `c_family_spec`.

use super::*;
// -----------------------------------------------------------------
// C
// -----------------------------------------------------------------

#[test]
fn c_switch_scores_once_and_arms_open_no_nesting() {
    let scored = only_function_for(
        "src/lib.c",
        r#"
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
"#,
    );
    assert_eq!(scored.cognitive_score, 1);
    assert_eq!(scored.max_nesting_depth, 1);
    assert_eq!(scored.branch_count, 3);
    assert_eq!(
        scored.name, "classify",
        "the name resolves through the declarator chain"
    );
}

#[test]
fn c_if_else_if_else_chain_is_flat() {
    let scored = only_function_for(
        "src/lib.c",
        r#"
int pick(int a, int b) {
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
fn c_nested_loops_deepen_the_score() {
    let scored = only_function_for(
        "src/lib.c",
        r#"
int deep(int a, int b, int *items, int n) {
    if (a) {
        for (int i = 0; i < n; i++) {
            while (b) {
                if (items[i] > 0) {
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
fn c_boolean_run_scores_once_mixed_run_scores_twice() {
    let and_only = only_function_for(
        "src/lib.c",
        r#"
int all_three(int a, int b, int c) {
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
        "src/lib.c",
        r#"
int mixed(int a, int b, int c) {
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
fn c_test_attribute_at_the_definition_exempts_the_function() {
    let file = cognitive_complexity(
        "src/lib.c",
        r#"
[[test]]
int deeply_nested_test(int a, int b, int *items, int n) {
    if (a) {
        for (int i = 0; i < n; i++) {
            while (b) {
                if (items[i] > 0) {
                    return 1;
                }
            }
        }
    }
    return 0;
}
"#,
    )
    .expect("c is a mapped language");
    let scored = &file.functions[0];

    assert!(scored.is_test, "[[test]] marks the definition as a test");
    assert_eq!(scored.max_nesting_depth, 4, "the depth is still measured");
    assert!(
        !scored.exceeds_gates(),
        "a test is exempt even at depth 4: {scored:?}"
    );
}

#[test]
fn c_repeated_scoring_never_drifts() {
    let source = r#"
int pick(int a, int b) {
    if (a) {
        return 1;
    } else if (b) {
        return 2;
    }
    return 0;
}
"#;
    let first = cognitive_complexity("src/lib.c", source).expect("c is mapped");
    for run in 1..DETERMINISM_RUNS {
        let again = cognitive_complexity("src/lib.c", source).expect("c is mapped");
        assert_eq!(again, first, "run {run} drifted from run 0");
    }
}

// -----------------------------------------------------------------
// C++
// -----------------------------------------------------------------

#[test]
fn cpp_switch_scores_once_and_arms_open_no_nesting() {
    let scored = only_function_for(
        "src/lib.cpp",
        r#"
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
"#,
    );
    assert_eq!(scored.cognitive_score, 1);
    assert_eq!(scored.max_nesting_depth, 1);
    assert_eq!(scored.branch_count, 3);
}

#[test]
fn cpp_if_else_if_else_chain_is_flat() {
    let scored = only_function_for(
        "src/lib.cpp",
        r#"
int pick(bool a, bool b) {
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
fn cpp_nested_loops_deepen_the_score() {
    let scored = only_function_for(
        "src/lib.cpp",
        r#"
int deep(bool a, bool b, int *items, int n) {
    if (a) {
        for (int i = 0; i < n; i++) {
            while (b) {
                if (items[i] > 0) {
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
fn cpp_boolean_run_scores_once_mixed_run_scores_twice() {
    let and_only = only_function_for(
        "src/lib.cpp",
        r#"
int all_three(bool a, bool b, bool c) {
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
        "src/lib.cpp",
        r#"
int mixed(bool a, bool b, bool c) {
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
fn cpp_test_attribute_at_the_definition_exempts_the_function() {
    let file = cognitive_complexity(
        "src/lib.cpp",
        r#"
[[test]]
int deeply_nested_test(bool a, bool b, int *items, int n) {
    if (a) {
        for (int i = 0; i < n; i++) {
            while (b) {
                if (items[i] > 0) {
                    return 1;
                }
            }
        }
    }
    return 0;
}
"#,
    )
    .expect("cpp is a mapped language");
    let scored = &file.functions[0];

    assert!(scored.is_test, "[[test]] marks the definition as a test");
    assert_eq!(scored.max_nesting_depth, 4);
    assert!(!scored.exceeds_gates());
}

#[test]
fn cpp_repeated_scoring_never_drifts() {
    let source = r#"
int pick(bool a, bool b) {
    if (a) {
        return 1;
    } else if (b) {
        return 2;
    }
    return 0;
}
"#;
    let first = cognitive_complexity("src/lib.cpp", source).expect("cpp is mapped");
    for run in 1..DETERMINISM_RUNS {
        let again = cognitive_complexity("src/lib.cpp", source).expect("cpp is mapped");
        assert_eq!(again, first, "run {run} drifted from run 0");
    }
}
