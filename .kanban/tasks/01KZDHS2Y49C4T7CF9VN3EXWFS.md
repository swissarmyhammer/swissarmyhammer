---
assignees:
- claude-code
position_column: todo
position_ordinal: ff8c80
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