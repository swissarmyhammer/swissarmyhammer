//! Elixir.

use super::*;
// -----------------------------------------------------------------
// Elixir
// -----------------------------------------------------------------

#[test]
fn elixir_case_scores_once_and_arms_open_no_nesting() {
    let scored = only_function_for(
        "src/lib.ex",
        r#"
defmodule Foo do
  def classify(v) do
    case v do
      0 -> 10
      1 -> 11
      _ -> 15
    end
  end
end
"#,
    );
    assert_eq!(
        scored.cognitive_score, 1,
        "a case scores once for the whole construct"
    );
    assert_eq!(scored.max_nesting_depth, 1, "arms open no nesting level");
    assert_eq!(scored.branch_count, 3, "three stab_clause arms");
    assert!(!scored.exceeds_gates());
}

#[test]
fn elixir_if_else_chain_is_flat() {
    // Elixir has no `elsif` keyword: a chain is written as a genuinely
    // nested `if` inside the outer `else`. Structurally this is the SAME
    // shape Rust's `else_clause` wrapper produces — a transparent
    // wrapper (`else_block`) holding either the next `if` link or a
    // terminal body — so `walk_alternative`'s existing unwrap-then-check
    // mechanism scores it flat here too, with no elixir-specific policy.
    let scored = only_function_for(
        "src/lib.ex",
        r#"
defmodule Foo do
  def pick(a, b) do
    if a do
      1
    else
      if b do
        2
      else
        3
      end
    end
  end
end
"#,
    );
    assert_eq!(
        scored.cognitive_score, 3,
        "if + else-if + else is 1 + 1 + 1"
    );
    assert_eq!(
        scored.max_nesting_depth, 1,
        "the else-if chain is flat, not a staircase"
    );
    assert_eq!(scored.max_else_if_chain, 1);
}

#[test]
fn elixir_nested_conditionals_deepen_the_score() {
    // Elixir has no imperative loop construct at all — `for` is a
    // functional comprehension and recursion is idiomatic instead, so
    // `loop_kinds` is intentionally empty. This test substitutes nested
    // conditionals for nested loops to verify nesting still deepens the
    // score correctly; `max_loop_nesting` stays 0.
    let scored = only_function_for(
        "src/lib.ex",
        r#"
defmodule Foo do
  def deep(a, b, c, d) do
    if a do
      if b do
        if c do
          if d do
            1
          end
        end
      end
    end
  end
end
"#,
    );
    assert_eq!(
        scored.cognitive_score, 10,
        "1 + 2 + 3 + 4 as each if nests one deeper"
    );
    assert_eq!(scored.max_nesting_depth, 4);
    assert_eq!(
        scored.max_loop_nesting, 0,
        "elixir has no imperative loop construct"
    );
    assert!(scored.exceeds_gates());
}

#[test]
fn elixir_boolean_run_scores_once_mixed_run_scores_twice() {
    let and_only = only_function_for(
        "src/lib.ex",
        r#"
defmodule Foo do
  def all_three(a, b, c) do
    if a and b and c do
      1
    end
    0
  end
end
"#,
    );
    assert_eq!(
        and_only.cognitive_score, 2,
        "the if scores 1, the one `and` run 1"
    );
    assert_eq!(and_only.max_boolean_operands, 3);

    let mixed = only_function_for(
        "src/lib.ex",
        r#"
defmodule Foo do
  def mixed(a, b, c) do
    if a and b or c do
      1
    end
    0
  end
end
"#,
    );
    assert_eq!(
        mixed.cognitive_score, 3,
        "the if scores 1, and `and` then `or` are two runs"
    );
}

#[test]
fn elixir_test_macro_at_the_definition_exempts_it() {
    // ExUnit's `test "description" do ... end` is itself a `call` node
    // with `target: (identifier "test")` — exactly like `def` — so being
    // named `test` at the definition IS the marker; no attribute lookup
    // is needed or possible (elixir has no attribute/annotation node
    // kind at all).
    let file = cognitive_complexity(
        "src/lib_test.ex",
        r#"
defmodule Foo do
  test "deeply nested" do
    if a do
      if b do
        if c do
          if d do
            1
          end
        end
      end
    end
  end
end
"#,
    )
    .expect("elixir is a mapped language");
    let scored = &file.functions[0];

    assert!(scored.is_test, "the `test` macro itself marks the block");
    assert_eq!(scored.max_nesting_depth, 4, "the depth is still measured");
    assert!(
        !scored.exceeds_gates(),
        "a test is exempt even at depth 4: {scored:?}"
    );
}

#[test]
fn elixir_an_ordinary_call_is_never_mistaken_for_a_function_or_conditional() {
    // The exact concern the task calls out: verify that `function_kinds`
    // matching by call-target text produces no false positives on
    // ordinary function calls (a remote call like `Repo.insert(a)`,
    // whose `target` is a `dot` node, and a local call like
    // `helper(a)`, whose `target` is an `identifier` not in
    // `call_target_kinds`) — neither is scored as its own function, and
    // neither opens a nesting level.
    let file = cognitive_complexity(
        "src/lib.ex",
        r#"
defmodule Foo do
  def process(a) do
    Repo.insert(a)
    helper(a)
    a
  end

  defp helper(a) do
    a + 1
  end
end
"#,
    )
    .expect("elixir is a mapped language");

    let names: Vec<&str> = file.functions.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["process", "helper"],
        "only def/defp are scored as functions — Repo.insert and helper(a) are ordinary calls"
    );
    let process = &file.functions[0];
    assert_eq!(
        process.cognitive_score, 0,
        "an ordinary call opens no nesting level and adds no score"
    );
    assert_eq!(process.max_nesting_depth, 0);
}

#[test]
fn elixir_repeated_scoring_never_drifts() {
    let source = r#"
defmodule Foo do
  def pick(a, b) do
    if a do
      1
    else
      if b do
        2
      end
    end
  end
end
"#;
    let first = cognitive_complexity("src/lib.ex", source).expect("elixir is mapped");
    for run in 1..DETERMINISM_RUNS {
        let again = cognitive_complexity("src/lib.ex", source).expect("elixir is mapped");
        assert_eq!(again, first, "run {run} drifted from run 0");
    }
}
