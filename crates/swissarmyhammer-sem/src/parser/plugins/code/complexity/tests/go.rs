//! Go.

use super::*;
// -----------------------------------------------------------------
// Go
// -----------------------------------------------------------------

#[test]
fn go_switch_scores_once_and_arms_open_no_nesting() {
    let scored = only_function_for(
        "src/lib.go",
        r#"
package main

func classify(value int) int {
	switch value {
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
fn go_if_else_if_else_chain_is_flat() {
    let scored = only_function_for(
        "src/lib.go",
        r#"
package main

func pick(a bool, b bool) int {
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
fn go_nested_loops_deepen_the_score() {
    let scored = only_function_for(
        "src/lib.go",
        r#"
package main

func deep(a bool, b bool, items []int) int {
	if a {
		for _, item := range items {
			for b {
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
    assert_eq!(scored.max_loop_nesting, 2, "for > for is two loops deep");
    assert!(scored.exceeds_gates());
}

#[test]
fn go_boolean_run_scores_once_mixed_run_scores_twice() {
    let and_only = only_function_for(
        "src/lib.go",
        r#"
package main

func allThree(a bool, b bool, c bool) int {
	if a && b && c {
		return 1
	}
	return 0
}
"#,
    );
    assert_eq!(and_only.cognitive_score, 2);
    assert_eq!(and_only.max_boolean_operands, 3);

    let mixed = only_function_for(
        "src/lib.go",
        r#"
package main

func mixed(a bool, b bool, c bool) int {
	if a && b || c {
		return 1
	}
	return 0
}
"#,
    );
    assert_eq!(mixed.cognitive_score, 3);
}

#[test]
fn go_test_name_and_signature_exempts_the_function() {
    let file = cognitive_complexity(
        "src/add_test.go",
        r#"
package main

import "testing"

func TestDeeplyNested(t *testing.T) {
	a := true
	b := true
	items := []int{1}
	if a {
		for _, item := range items {
			for b {
				if item > 0 {
					return
				}
			}
		}
	}
}
"#,
    )
    .expect("go is a mapped language");
    let scored = &file.functions[0];

    assert!(
        scored.is_test,
        "TestXxx(t *testing.T) is the real go test convention"
    );
    assert_eq!(scored.max_nesting_depth, 4, "the depth is still measured");
    assert!(
        !scored.exceeds_gates(),
        "a test is exempt even at depth 4: {scored:?}"
    );
}

#[test]
fn go_helper_named_test_prefix_without_testing_param_is_not_a_test() {
    // `TestXxx` alone is not enough: without a `*testing.T` parameter it
    // is an ordinary helper, not a `go test` entry point.
    let file = cognitive_complexity(
        "src/lib.go",
        r#"
package main

func TestHelper(a int) int {
	return a + 1
}
"#,
    )
    .expect("go is a mapped language");

    assert!(
        !file.functions[0].is_test,
        "a TestXxx helper with no *testing.T parameter is not a real go test"
    );
}

#[test]
fn go_repeated_scoring_never_drifts() {
    let source = r#"
package main

func pick(a bool, b bool) int {
	if a {
		return 1
	} else if b {
		return 2
	}
	return 0
}
"#;
    let first = cognitive_complexity("src/lib.go", source).expect("go is mapped");
    for run in 1..DETERMINISM_RUNS {
        let again = cognitive_complexity("src/lib.go", source).expect("go is mapped");
        assert_eq!(again, first, "run {run} drifted from run 0");
    }
}
