//! Fortran.

use super::*;
// -----------------------------------------------------------------
// Fortran
// -----------------------------------------------------------------

#[test]
fn fortran_select_case_scores_once_and_arms_open_no_nesting() {
    let scored = only_function_for(
        "src/lib.f90",
        r#"
subroutine classify(value, result)
  integer, intent(in) :: value
  integer, intent(out) :: result
  select case (value)
  case (0)
    result = 10
  case (1)
    result = 11
  case default
    result = 15
  end select
end subroutine classify
"#,
    );
    assert_eq!(scored.cognitive_score, 1);
    assert_eq!(scored.max_nesting_depth, 1, "arms open no nesting level");
    assert_eq!(scored.branch_count, 3);
    assert!(!scored.exceeds_gates());
}

#[test]
fn fortran_if_elseif_else_chain_is_flat() {
    let scored = only_function_for(
        "src/lib.f90",
        r#"
subroutine pick(a, b, result)
  logical, intent(in) :: a, b
  integer, intent(out) :: result
  if (a) then
    result = 1
  else if (b) then
    result = 2
  else
    result = 3
  end if
end subroutine pick
"#,
    );
    assert_eq!(
        scored.cognitive_score, 3,
        "if + else-if + else is 1 + 1 + 1"
    );
    assert_eq!(
        scored.max_nesting_depth, 1,
        "an else-if chain is flat, not a staircase"
    );
    assert_eq!(scored.max_else_if_chain, 1);
}

#[test]
fn fortran_nested_loops_deepen_the_score() {
    let scored = only_function_for(
        "src/lib.f90",
        r#"
subroutine deep(a, b, n, result)
  logical, intent(in) :: a, b
  integer, intent(in) :: n
  integer, intent(out) :: result
  integer :: i
  if (a) then
    do i = 1, n
      do while (b)
        if (i > 0) then
          result = 1
          return
        end if
      end do
    end do
  end if
  result = 0
end subroutine deep
"#,
    );
    assert_eq!(scored.cognitive_score, 10);
    assert_eq!(scored.max_nesting_depth, 4);
    assert_eq!(scored.max_loop_nesting, 2, "do > do is two loops deep");
    assert!(scored.exceeds_gates());
}

#[test]
fn fortran_boolean_run_scores_once_mixed_run_scores_twice() {
    let and_only = only_function_for(
        "src/lib.f90",
        r#"
subroutine all_three(a, b, c, result)
  logical, intent(in) :: a, b, c
  logical, intent(out) :: result
  if (a .and. b .and. c) then
    result = 1
  end if
end subroutine all_three
"#,
    );
    assert_eq!(and_only.cognitive_score, 2);
    assert_eq!(and_only.max_boolean_operands, 3);

    let mixed = only_function_for(
        "src/lib.f90",
        r#"
subroutine mixed(a, b, c, result)
  logical, intent(in) :: a, b, c
  logical, intent(out) :: result
  if (a .and. b .or. c) then
    result = 1
  end if
end subroutine mixed
"#,
    );
    assert_eq!(mixed.cognitive_score, 3);
}

#[test]
fn fortran_boolean_operators_are_recognized_regardless_of_case() {
    // Fortran is case-insensitive, and `tree_sitter_fortran`'s grammar
    // aliases `.and.`/`.or.` via a `caseInsensitive()` regex to the SAME
    // lowercase node kind no matter how the source spells it (verified
    // by reading `grammar.js`: `[caseInsensitive('.and.'), ...]` aliases
    // to the literal lowercase string, so `node.kind()` for an uppercase
    // `.AND.` token is still `".and."`). This asserts the real parsed
    // behavior rather than the grammar source: an uppercase run scores
    // identically to the lowercase run already covered above.
    let uppercase = only_function_for(
        "src/lib.f90",
        r#"
subroutine all_three_upper(a, b, c, result)
  logical, intent(in) :: a, b, c
  logical, intent(out) :: result
  if (a .AND. b .AND. c) then
    result = 1
  end if
end subroutine all_three_upper
"#,
    );
    assert_eq!(
        uppercase.cognitive_score, 2,
        "uppercase .AND. must be recognized exactly like lowercase .and."
    );
    assert_eq!(uppercase.max_boolean_operands, 3);

    let mixed_case = only_function_for(
        "src/lib.f90",
        r#"
subroutine mixed_upper(a, b, c, result)
  logical, intent(in) :: a, b, c
  logical, intent(out) :: result
  if (a .AND. b .OR. c) then
    result = 1
  end if
end subroutine mixed_upper
"#,
    );
    assert_eq!(
        mixed_case.cognitive_score, 3,
        "uppercase .AND./.OR. must mix into two sequences exactly like lowercase"
    );
}

#[test]
fn fortran_test_name_prefix_exempts_the_subroutine() {
    let file = cognitive_complexity(
        "src/lib.f90",
        r#"
subroutine test_deeply_nested(a, b, n, result)
  logical, intent(in) :: a, b
  integer, intent(in) :: n
  integer, intent(out) :: result
  integer :: i
  if (a) then
    do i = 1, n
      do while (b)
        if (i > 0) then
          result = 1
          return
        end if
      end do
    end do
  end if
end subroutine test_deeply_nested
"#,
    )
    .expect("fortran is a mapped language");
    let scored = &file.functions[0];

    assert!(
        scored.is_test,
        "FRUIT's test_* subroutine naming marks the subroutine"
    );
    assert_eq!(scored.max_nesting_depth, 4, "the depth is still measured");
    assert!(
        !scored.exceeds_gates(),
        "a test is exempt even at depth 4: {scored:?}"
    );
}

#[test]
fn fortran_uppercase_test_name_prefix_exempts_the_subroutine() {
    // Fortran identifiers are case-insensitive by language semantics —
    // `TEST_DEEPLY_NESTED`, `test_deeply_nested`, and `Test_Deeply_Nested`
    // all name the same subroutine — so FRUIT's all-caps naming style
    // must be recognized as a test exactly like the lowercase spelling
    // covered above.
    let file = cognitive_complexity(
        "src/lib.f90",
        r#"
subroutine TEST_DEEPLY_NESTED(a, b, n, result)
  logical, intent(in) :: a, b
  integer, intent(in) :: n
  integer, intent(out) :: result
  integer :: i
  if (a) then
    do i = 1, n
      do while (b)
        if (i > 0) then
          result = 1
          return
        end if
      end do
    end do
  end if
end subroutine TEST_DEEPLY_NESTED
"#,
    )
    .expect("fortran is a mapped language");
    let scored = &file.functions[0];

    assert!(
        scored.is_test,
        "an uppercase-named FRUIT test subroutine is still recognized as a test"
    );
    assert_eq!(scored.max_nesting_depth, 4, "the depth is still measured");
    assert!(
        !scored.exceeds_gates(),
        "a test is exempt even at depth 4: {scored:?}"
    );
}

#[test]
fn fortran_repeated_scoring_never_drifts() {
    let source = r#"
subroutine pick(a, b, result)
  logical, intent(in) :: a, b
  integer, intent(out) :: result
  if (a) then
    result = 1
  else if (b) then
    result = 2
  end if
end subroutine pick
"#;
    let first = cognitive_complexity("src/lib.f90", source).expect("fortran is mapped");
    for run in 1..DETERMINISM_RUNS {
        let again = cognitive_complexity("src/lib.f90", source).expect("fortran is mapped");
        assert_eq!(again, first, "run {run} drifted from run 0");
    }
}
