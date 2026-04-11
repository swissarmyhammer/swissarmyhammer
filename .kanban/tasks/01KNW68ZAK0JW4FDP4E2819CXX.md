---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffb280
project: expr-filter
title: Update parse_error_on_invalid_input comment in filter-expr lib.rs to explain why $$garbage fails
---
**File:** swissarmyhammer-filter-expr/src/lib.rs (lines 218-221)

**Severity:** nit

**What:** After adding `$` as a valid sigil for `Expr::Project`, the existing test:

```rust
#[test]
fn parse_error_on_invalid_input() {
    let result = parse("$$garbage");
    assert!(result.is_err());
}
```

still passes, but for a different reason than its name implies. Before the change, `$` was not a recognized sigil, so `$$garbage` was "invalid input" in the literal sense. After the change, `$` IS a valid sigil — `$$garbage` fails because the first `$` is consumed as the Project sigil and the second `$` is now excluded from `is_body_char`, so the body parser's `at_least(1)` assertion fails.

The twin tests in `parser.rs` (`error_invalid_chars` and `error_has_span_info`) were updated by this commit with inline explanations of *why* `$$garbage` and `$$` now fail. The same reasoning should appear in `lib.rs`, but the test body there received no comment update. The card description (01KNVSFHMR89NG65Y8EESWF3E7) explicitly called out that these inline comments "should be updated so readers understand WHY the input fails" — `lib.rs` was missed.

Consider also renaming the test to something more descriptive (`parse_error_on_double_dollar` or `parse_error_on_empty_project_body`) since the existing name is also now imprecise — but renaming is optional; at minimum add an inline `//` comment matching the parser.rs ones.

**Suggestion:** Add a three-line comment before `let result = parse("$$garbage");` explaining why it fails, mirroring the wording in parser.rs `error_invalid_chars`.

**Subtasks:**
- [ ] Add inline comment to `parse_error_on_invalid_input` in swissarmyhammer-filter-expr/src/lib.rs matching the explanation in parser.rs::error_invalid_chars
- [ ] Optional: rename the test to `parse_error_on_double_dollar` so the name describes the actual failure mode
- [ ] Verification: `cargo test -p swissarmyhammer-filter-expr --lib tests::parse_error_on_invalid_input` passes
#review-finding #expr-filter