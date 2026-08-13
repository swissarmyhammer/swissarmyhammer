//! Ruby.

use super::*;
// -----------------------------------------------------------------
// Ruby
// -----------------------------------------------------------------

#[test]
fn ruby_case_scores_once_and_arms_open_no_nesting() {
    let scored = only_function_for(
        "src/lib.rb",
        r#"
def classify(value)
  case value
  when 0
    10
  when 1
    11
  else
    15
  end
end
"#,
    );
    assert_eq!(
        scored.cognitive_score, 1,
        "a case scores once for the whole construct"
    );
    assert_eq!(scored.max_nesting_depth, 1, "arms open no nesting level");
    assert_eq!(
        scored.branch_count, 3,
        "two whens plus the trailing else are three decision points"
    );
    assert!(!scored.exceeds_gates());
}

#[test]
fn ruby_if_elsif_else_chain_is_flat() {
    let scored = only_function_for(
        "src/lib.rb",
        r#"
def pick(a, b)
  if a
    1
  elsif b
    2
  else
    3
  end
end
"#,
    );
    assert_eq!(scored.cognitive_score, 3, "if + elsif + else is 1 + 1 + 1");
    assert_eq!(
        scored.max_nesting_depth, 1,
        "an elsif chain is flat, not a staircase"
    );
    assert_eq!(scored.max_else_if_chain, 1);
}

#[test]
fn ruby_nested_loops_deepen_the_score() {
    let scored = only_function_for(
        "src/lib.rb",
        r#"
def deep(a, b, items)
  if a
    for item in items
      while b
        if item > 0
          return 1
        end
      end
    end
  end
  0
end
"#,
    );
    assert_eq!(scored.cognitive_score, 10);
    assert_eq!(scored.max_nesting_depth, 4);
    assert_eq!(scored.max_loop_nesting, 2, "for > while is two loops deep");
    assert!(scored.exceeds_gates());
}

#[test]
fn ruby_boolean_run_scores_once_mixed_run_scores_twice() {
    let and_only = only_function_for(
        "src/lib.rb",
        r#"
def all_three(a, b, c)
  if a && b && c
    return 1
  end
  return 0
end
"#,
    );
    assert_eq!(and_only.cognitive_score, 2);
    assert_eq!(and_only.max_boolean_operands, 3);

    let mixed = only_function_for(
        "src/lib.rb",
        r#"
def mixed(a, b, c)
  if a && b || c
    return 1
  end
  return 0
end
"#,
    );
    assert_eq!(mixed.cognitive_score, 3);
}

#[test]
fn ruby_test_name_prefix_exempts_the_method() {
    let file = cognitive_complexity(
        "src/lib.rb",
        r#"
def test_deeply_nested(a, b, items)
  if a
    for item in items
      while b
        if item > 0
          return 1
        end
      end
    end
  end
end
"#,
    )
    .expect("ruby is a mapped language");
    let scored = &file.functions[0];

    assert!(scored.is_test, "minitest's def test_foo marks the method");
    assert_eq!(scored.max_nesting_depth, 4, "the depth is still measured");
    assert!(
        !scored.exceeds_gates(),
        "a test is exempt even at depth 4: {scored:?}"
    );
}

#[test]
fn ruby_a_helper_beside_a_test_is_not_a_test() {
    let file = cognitive_complexity(
        "src/lib_test.rb",
        r#"
def test_something
  1
end

def build_request(a)
  if a
    1
  else
    0
  end
end
"#,
    )
    .expect("ruby is a mapped language");
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
fn ruby_repeated_scoring_never_drifts() {
    let source = r#"
def pick(a, b)
  if a
    1
  elsif b
    2
  end
  0
end
"#;
    let first = cognitive_complexity("src/lib.rb", source).expect("ruby is mapped");
    for run in 1..DETERMINISM_RUNS {
        let again = cognitive_complexity("src/lib.rb", source).expect("ruby is mapped");
        assert_eq!(again, first, "run {run} drifted from run 0");
    }
}
