---
assignees:
- claude-code
depends_on:
- 01KNW77BBFV75MK5PEW86938TQ
position_column: done
position_ordinal: ffffffffffffffffffffffffffffff80
project: expr-filter
title: 'filter-expr: delete defensive dead branch in parse()'
---
## What

`swissarmyhammer-filter-expr/src/parser.rs::parse()` (around lines 120-156) contains a defensive branch that fires only when chumsky reports `!result.has_errors()` AND `result.into_output().is_none()` — i.e., a "parser succeeded silently but produced nothing" state:

```rust
} else {
    // The parser succeeded; unwrap the output.
    result.into_output().ok_or_else(|| {
        vec![ParseError {
            message: "unexpected parse failure".to_string(),
            span: 0..input.len(),
        }]
    })
}
```

This branch is practically unreachable. Chumsky's `ParseResult::into_output()` returns `None` only when the parser hit errors, but we already branched on `!has_errors()` above. Confirmed by tarpaulin: lines 150-152 have **0 hits** after running the full 80-test suite.

Per the project's "don't add error handling for scenarios that can't happen" rule, this defensive fallback is dead code and should be removed, not tested. The correct replacement is to `.expect()` with a clear message — this preserves the invariant (crash loudly if chumsky ever violates its own contract) without carrying uncalled error-construction code in every release build.

**Depends on:** card `01KNW77BBFV75MK5PEW86938TQ` (the ParseError Display coverage card). After this card lands, the crate should be at effectively 100% coverage on reachable code.

## Acceptance Criteria

- [ ] Replace the `else` branch's `ok_or_else(...)` block with `result.into_output().expect("chumsky parser returned no errors and no output — this violates its own invariant")`
- [ ] The function's outer return type is unchanged (`Result<Expr, Vec<ParseError>>`)
- [ ] The success path must wrap in `Ok(...)` — currently `ok_or_else` returns a `Result` directly, but `.expect()` returns the unwrapped `Expr`, so the caller needs to add `.map(Ok).unwrap_or(Ok(expr))` — NO, just bind with `let expr = ...; Ok(expr)` in that branch
- [ ] `cargo test -p swissarmyhammer-filter-expr` — all 82 tests (80 existing + 2 from predecessor card) pass with no regressions
- [ ] `cargo clippy -p swissarmyhammer-filter-expr --all-targets -- -D warnings` stays clean
- [ ] `cargo tarpaulin -p swissarmyhammer-filter-expr --out lcov --output-dir .` reports 100% coverage on `parser.rs` (no more uncovered DA lines in the file)
- [ ] No behavior change for any existing input — every test that parses a valid expression still produces the same `Expr`, every test that produces an error still produces the same errors

## Tests

No new tests needed — this card REMOVES dead code. The 82 existing tests after card `01KNW77BBFV75MK5PEW86938TQ` lands already cover every reachable path through `parse()`. Verify:

- [ ] `cargo test -p swissarmyhammer-filter-expr` — green, same test count
- [ ] Coverage run shows parser.rs at 100%
- [ ] Grep confirms the `"unexpected parse failure"` string is no longer in the tree

## Workflow

- This is a defensive-code deletion, not a behavior change. Do the edit, run tests, run clippy, run tarpaulin to confirm 100%. No TDD cycle needed.
- Stay strictly scoped to the `else` branch of the `if result.has_errors()` block in `parse()`. Do not touch the error branch, the `input.trim().is_empty()` guard, the function signature, or anything else in parser.rs. #expr-filter