---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzdmm6jabhv5rrstdv9f26re
  text: |-
    Measured the real `tree_sitter_python` grammar with a throwaway test that printed the s-expression of a pytest/unittest sample, then deleted that test. What the parse showed:

    - `assert 1 == 1` is an `assert_statement` — an assertion whatever it names, so it goes in `assertion_kinds`, like Java's.
    - `except ValueError:` is an `except_clause` — the catch kind.
    - `pytest.raises` and `self.assertEqual` both put the last segment in an `identifier` leaf, so the default `name_kinds` reads them.
    - `@pytest.mark.skip("why")` parses as `decorated_definition (decorator ...) definition: (function_definition ...)`, so the decorator is a preceding named sibling and `definition_attributes` already finds it. `attribute_marker_name` reduces it to `skip`.
    - `body: (block ...)` — the default `body_field: "body"` works.
    - A method in a class parses as a `function_definition` under `class_definition > block`, and `for_each_function` walks every named child, so `unittest` methods are measured too.

    Two facts worth keeping:

    1. `skip_markers` is a SUBSTRING match (`word_matches`), so `skip` already covers `skipif`. The row lists `["skip", "xfail"]` and the doc says why, rather than carrying a third entry that can never match on its own.
    2. Python cannot have a comments-only body — the language needs a statement — so `TestDefect::CommentsOnly` is unreachable for it. Nothing to test there.

    Two existing tests named Python as the language with no census mapping. Both now use JavaScript's `it(...)`, which is still unrecognized:
    - `crates/swissarmyhammer-sem/.../complexity/test_census.rs::a_language_with_no_census_mapping_is_not_measured`
    - `crates/swissarmyhammer-validators/src/review/tree_sitter_probes.rs::a_language_with_no_census_mapping_reports_one_not_computed_row`
  timestamp: 2026-08-07T08:14:54.538065+00:00
- actor: claude-code
  id: 01kzdmmd04enxjwtv6ca9bv71d
  text: |-
    ### implement — changed
    - evidence: 3 files — crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs (PYTHON_SPEC gains `test_name_prefix: Some("test_")`, plus the two new tests), crates/swissarmyhammer-sem/src/parser/plugins/code/complexity/test_census.rs (new PYTHON_CENSUS row, plus the two new tests), crates/swissarmyhammer-validators/src/review/tree_sitter_probes.rs (the not-computed test moves to JavaScript). All four tests the card asked for went RED first, then GREEN. `cargo test -p swissarmyhammer-sem --lib` 327 passed 0 failed; `cargo test -p swissarmyhammer-validators` 482 passed 0 failed; clippy clean; `cargo fmt --check` clean; `cargo check --workspace --all-targets` clean.
    - next: /review
  timestamp: 2026-08-07T08:15:01.124237+00:00
position_column: doing
position_ordinal: '8480'
title: Detect pytest `def test_foo` as a test definition
---
`PYTHON_SPEC.test_name_prefix` is `None` in `crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs`, so Python tests are recognized only through a `@...test` decorator — a spelling almost no Python project uses. Both pytest and `unittest` mark a test by the `test_` prefix at the definition, exactly the name convention Ruby and Fortran already use through `test_name_prefix`.

Two probes are blind to Python because of this:
- `complexity` never exempts a Python test from its gates, although `cognitive-complexity.md` says a test is exempt.
- `assertion-census` cannot map Python at all (found while building it, task ^cysg4xv); a mapping would report a file full of untested tests as clean, so Python is deliberately left out of `TEST_CENSUS_SPECS`.

Work:
- Set `test_name_prefix: Some("test_")` on `PYTHON_SPEC`.
- Add the Python row to `TEST_CENSUS_SPECS` in `complexity/test_census.rs`: `assert` statement kind, the `assert`/`raises`/`expect` words, the `skip`/`skipif`/`xfail` markers, `except_clause` as the catch kind.
- Tests: a `def test_foo` is marked as a test; a `def helper` beside it is not; a zero-assertion Python test yields a census row; `@pytest.mark.skip` yields one.

#tool-validators