//! PHP.

use super::*;

/// Prefix a PHP fixture body with the `<?php` opening tag every
/// `only_function_for("src/lib.php", ...)` call needs.
fn php_source(body: &str) -> String {
    format!("<?php\n{body}")
}
// -----------------------------------------------------------------
// PHP
// -----------------------------------------------------------------

#[test]
fn php_switch_scores_once_and_arms_open_no_nesting() {
    let scored = only_function_for(
        "src/lib.php",
        &php_source(
            r#"
function classify($value) {
    switch ($value) {
        case 0:
            return 10;
        case 1:
            return 11;
        default:
            return 15;
    }
}
"#,
        ),
    );
    assert_eq!(scored.cognitive_score, 1);
    assert_eq!(scored.max_nesting_depth, 1);
    assert_eq!(scored.branch_count, 3);
}

#[test]
fn php_if_elseif_else_chain_is_flat() {
    let scored = only_function_for(
        "src/lib.php",
        &php_source(
            r#"
function pick($a, $b) {
    if ($a) {
        return 1;
    } elseif ($b) {
        return 2;
    } else {
        return 3;
    }
}
"#,
        ),
    );
    assert_eq!(scored.cognitive_score, 3);
    assert_eq!(scored.max_nesting_depth, 1);
    assert_eq!(scored.max_else_if_chain, 1);
}

#[test]
fn php_three_way_elseif_chain_reports_the_longest_link() {
    let scored = only_function_for(
        "src/lib.php",
        &php_source(
            r#"
function pick($a, $b, $c) {
    if ($a) {
        return 1;
    } elseif ($b) {
        return 2;
    } elseif ($c) {
        return 3;
    } else {
        return 4;
    }
}
"#,
        ),
    );
    assert_eq!(scored.cognitive_score, 4);
    assert_eq!(scored.max_nesting_depth, 1);
    assert_eq!(scored.max_else_if_chain, 2, "two elseif links");
}

#[test]
fn php_nested_loops_deepen_the_score() {
    let scored = only_function_for(
        "src/lib.php",
        &php_source(
            r#"
function deep($a, $b, $items) {
    if ($a) {
        foreach ($items as $item) {
            while ($b) {
                if ($item > 0) {
                    return 1;
                }
            }
        }
    }
    return 0;
}
"#,
        ),
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
fn php_boolean_run_scores_once_mixed_run_scores_twice() {
    let and_only = only_function_for(
        "src/lib.php",
        &php_source(
            r#"
function all_three($a, $b, $c) {
    if ($a && $b && $c) {
        return 1;
    }
    return 0;
}
"#,
        ),
    );
    assert_eq!(and_only.cognitive_score, 2);
    assert_eq!(and_only.max_boolean_operands, 3);

    let mixed = only_function_for(
        "src/lib.php",
        &php_source(
            r#"
function mixed($a, $b, $c) {
    if ($a && $b || $c) {
        return 1;
    }
    return 0;
}
"#,
        ),
    );
    assert_eq!(mixed.cognitive_score, 3);
}

#[test]
fn php_test_attribute_at_the_definition_exempts_the_method() {
    let file = cognitive_complexity(
        "src/FooTest.php",
        r#"<?php
class FooTest {
    #[Test]
    public function deeplyNested($a, $b, $items) {
        if ($a) {
            foreach ($items as $item) {
                while ($b) {
                    if ($item > 0) {
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
    .expect("php is a mapped language");
    let scored = &file.functions[0];

    assert!(scored.is_test, "#[Test] marks the method as a test");
    assert_eq!(scored.max_nesting_depth, 4, "the depth is still measured");
    assert!(
        !scored.exceeds_gates(),
        "a test is exempt even at depth 4: {scored:?}"
    );
}

#[test]
fn php_repeated_scoring_never_drifts() {
    let source = "<?php\nfunction pick($a, $b) {\n    if ($a) {\n        return 1;\n    } elseif ($b) {\n        return 2;\n    }\n    return 0;\n}\n";
    let first = cognitive_complexity("src/lib.php", source).expect("php is mapped");
    for run in 1..DETERMINISM_RUNS {
        let again = cognitive_complexity("src/lib.php", source).expect("php is mapped");
        assert_eq!(again, first, "run {run} drifted from run 0");
    }
}
