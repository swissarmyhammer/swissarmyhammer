---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffe80
project: expr-filter
title: 'filter-expr: add test for ParseError Display impl'
---
## What

`swissarmyhammer-filter-expr/src/parser.rs` lines 14-22 define a custom `Display` implementation for `ParseError`:

```rust
impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at {}..{}",
            self.message, self.span.start, self.span.end
        )
    }
}
```

Tarpaulin coverage run: the `write!` body (lines 15-17) has **0 hits**. No existing test exercises `ParseError::fmt` / `ParseError::to_string()` / `format!("{err}")`. The implementation is public, non-trivial (custom format string, not derived), and part of the crate's public error API surface — this is exactly the kind of thing that should have a test written alongside it via TDD.

**File:** `swissarmyhammer-filter-expr/src/parser.rs` lines 14-22
**Current coverage:** 54/60 lines = 90.0% in `parser.rs`, 67/73 = 91.8% crate-wide
**Uncovered lines closed by this card:** 15, 16, 17

## Acceptance Criteria

- [ ] A test in `swissarmyhammer-filter-expr/src/parser.rs` (inside the existing `#[cfg(test)] mod tests` block, in the "Error cases" section or a new "Display impl" section) constructs a `ParseError` directly or via `parse()` and asserts its `Display` output matches the expected format `"{message} at {start}..{end}"`
- [ ] At least one assertion on a real `ParseError` obtained from a failing `parse()` call (this is the realistic usage path — error is surfaced via `{err}` formatting somewhere in the stack)
- [ ] At least one assertion on a synthesized `ParseError { message: "test error".into(), span: 3..7 }` to lock down the exact format — this test would catch accidental changes to the format string
- [ ] Running `cargo tarpaulin -p swissarmyhammer-filter-expr --out lcov --output-dir .` and inspecting `lcov.info` shows lines 15, 16, 17 of `parser.rs` with hits > 0
- [ ] `cargo test -p swissarmyhammer-filter-expr` passes (80+1 new tests)

## Tests

Add to the tests module in `swissarmyhammer-filter-expr/src/parser.rs`:

- [ ] `display_impl_format()` — build `ParseError { message: "oops".into(), span: 3..7 }` and assert `format!("{err}") == "oops at 3..7"`
- [ ] `display_impl_from_real_parse_error()` — call `parse("$$")` (which fails per existing `error_has_span_info` test), take the first ParseError, format it, and assert the output matches the regex/contains the expected shape `"<something> at <n>..<m>"`. The exact message string depends on chumsky's output so use `contains` for the positional suffix.

Test command:
- [ ] `cargo test -p swissarmyhammer-filter-expr` — 82 tests pass (80 existing + 2 new)
- [ ] `cargo tarpaulin -p swissarmyhammer-filter-expr --out lcov --output-dir .` — parser.rs lines 15-17 now covered

## Workflow
- Use `/tdd` — write the failing tests first, confirm they fail because `Display::fmt` was never called in tests, then the tests should go green immediately (the implementation already exists; this card is about coverage not behavior). #coverage-gap #expr-filter