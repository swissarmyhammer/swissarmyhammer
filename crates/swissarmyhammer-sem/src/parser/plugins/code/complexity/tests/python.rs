//! Python.

use super::*;
// -----------------------------------------------------------------
// Python
// -----------------------------------------------------------------

#[test]
fn python_match_scores_once_and_cases_open_no_nesting() {
    let scored = only_function_for(
        "src/lib.py",
        r#"
def classify(value):
    match value:
        case 0:
            return 10
        case 1:
            return 11
        case _:
            return 15
"#,
    );
    assert_eq!(
        scored.cognitive_score, 1,
        "a match scores once for the whole construct"
    );
    assert_eq!(scored.max_nesting_depth, 1, "cases open no nesting level");
    assert_eq!(
        scored.branch_count, 3,
        "three cases are three decision points"
    );
}

#[test]
fn python_if_elif_else_chain_is_flat() {
    let scored = only_function_for(
        "src/lib.py",
        r#"
def pick(a, b):
    if a:
        return 1
    elif b:
        return 2
    else:
        return 3
"#,
    );
    assert_eq!(scored.cognitive_score, 3, "if + elif + else is 1 + 1 + 1");
    assert_eq!(
        scored.max_nesting_depth, 1,
        "an elif chain is flat, not a staircase"
    );
    assert_eq!(scored.max_else_if_chain, 1);
}

#[test]
fn python_three_way_elif_chain_reports_the_longest_link() {
    let scored = only_function_for(
        "src/lib.py",
        r#"
def pick(a, b, c):
    if a:
        return 1
    elif b:
        return 2
    elif c:
        return 3
    else:
        return 4
"#,
    );
    assert_eq!(
        scored.cognitive_score, 4,
        "if + elif + elif + else is 1 + 1 + 1 + 1"
    );
    assert_eq!(scored.max_nesting_depth, 1);
    assert_eq!(scored.max_else_if_chain, 2, "two elif links");
}

#[test]
fn python_nested_loops_deepen_the_score() {
    let scored = only_function_for(
        "src/lib.py",
        r#"
def deep(a, b, items):
    if a:
        for item in items:
            while b:
                if item > 0:
                    return 1
    return 0
"#,
    );
    assert_eq!(scored.cognitive_score, 10);
    assert_eq!(scored.max_nesting_depth, 4);
    assert_eq!(scored.max_loop_nesting, 2, "for > while is two loops deep");
    assert!(scored.exceeds_gates());
}

#[test]
fn python_boolean_run_scores_once_mixed_run_scores_twice() {
    let and_only = only_function_for(
        "src/lib.py",
        r#"
def all_three(a, b, c):
    if a and b and c:
        return 1
    return 0
"#,
    );
    assert_eq!(
        and_only.cognitive_score, 2,
        "the if scores 1, the one `and` run 1"
    );
    assert_eq!(and_only.max_boolean_operands, 3);

    let mixed = only_function_for(
        "src/lib.py",
        r#"
def mixed(a, b, c):
    if a and b or c:
        return 1
    return 0
"#,
    );
    assert_eq!(
        mixed.cognitive_score, 3,
        "the if scores 1, and `and` then `or` are two runs"
    );
}

#[test]
fn python_test_decorator_at_the_definition_exempts_the_function() {
    let file = cognitive_complexity(
        "src/lib.py",
        r#"
@pytest.mark.test
def deeply_nested_test(a, b, items):
    if a:
        for item in items:
            while b:
                if item > 0:
                    return 1
    return 0
"#,
    )
    .expect("python is a mapped language");
    let scored = &file.functions[0];

    assert!(
        scored.is_test,
        "@pytest.mark.test marks the definition as a test"
    );
    assert_eq!(scored.max_nesting_depth, 4, "the depth is still measured");
    assert!(
        !scored.exceeds_gates(),
        "a test is exempt even at depth 4: {scored:?}"
    );
}

#[test]
fn python_test_name_prefix_exempts_the_function() {
    let file = cognitive_complexity(
        "src/lib.py",
        r#"
def test_deeply_nested(a, b, items):
    if a:
        for item in items:
            while b:
                if item > 0:
                    return 1
    return 0
"#,
    )
    .expect("python is a mapped language");
    let scored = &file.functions[0];

    assert!(scored.is_test, "pytest's def test_foo marks the function");
    assert_eq!(scored.max_nesting_depth, 4, "the depth is still measured");
    assert!(
        !scored.exceeds_gates(),
        "a test is exempt even at depth 4: {scored:?}"
    );
}

#[test]
fn python_a_helper_beside_a_test_is_not_a_test() {
    let file = cognitive_complexity(
        "src/test_lib.py",
        r#"
def test_something():
    return 1


def build_request(a):
    if a:
        return 1
    return 0
"#,
    )
    .expect("python is a mapped language");
    let helper = file
        .functions
        .iter()
        .find(|f| f.name == "build_request")
        .expect("the helper is scored");

    assert!(
        !helper.is_test,
        "a helper is judged at its own name, never by the file name"
    );
}

#[test]
fn python_repeated_scoring_never_drifts() {
    let source = r#"
def pick(a, b):
    if a:
        return 1
    elif b:
        return 2
    return 0
"#;
    let first = cognitive_complexity("src/lib.py", source).expect("python is mapped");
    for run in 1..DETERMINISM_RUNS {
        let again = cognitive_complexity("src/lib.py", source).expect("python is mapped");
        assert_eq!(again, first, "run {run} drifted from run 0");
    }
}
