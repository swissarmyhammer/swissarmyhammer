---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffff9180
project: spatial-nav
title: 'Fix pre-existing failure: register_scope_with_zero_dim_warns_not_errors'
---
## Context

`cargo test -p swissarmyhammer-focus` has a pre-existing failure in `swissarmyhammer-focus/src/registry.rs::register_scope_with_zero_dim_warns_not_errors` (and very likely `register_scope_with_both_zero_rect_warns_not_errors`, which has the same shape). The failure reproduces on `HEAD` with the working tree stashed — it is unrelated to the spatial-nav redesign step 3 (`01KQW65Z689G7WWRYMBHX6MD7V`) work but blocks the literal "cargo test green" acceptance gate for the parent epic `01KQTC1VNQM9KC90S65P7QX9N1`.

## Goal

Make `cargo test -p swissarmyhammer-focus` green so that the parent epic's acceptance criteria can be evaluated cleanly.

## Investigation

The test expects a `tracing::WARN` at `register_scope` whose message contains `"zero dimension"` for a registered scope with a zero-height (or zero-width) rect. It also expects no width/height ERROR-level events for the same scope.

Likely root causes to check:

1. The validator on `register_scope` no longer emits a "zero dimension" warning at WARN level (was downgraded to TRACE/INFO, or upgraded to ERROR, or the message was reworded).
2. The capture helper changed shape so `e.message`, `e.is_op`, or `e.field` now resolves differently.
3. The fixture `rect_xywh(0.0, 0.0, 100.0, 0.0)` no longer produces a zero-dim rect (e.g. coordinate-clamping changed).

Reproduce:

```sh
cargo test -p swissarmyhammer-focus register_scope_with_zero_dim_warns_not_errors -- --nocapture
```

Then trace the validator code path inside `SpatialRegistry::register_scope` to see what tracing event(s) the zero-dim case emits today.

## Acceptance criteria

- `cargo test -p swissarmyhammer-focus` green (zero failures, zero warnings).
- The "zero dimension on registration is a WARN, not an ERROR" contract documented in the test comments still holds — if the implementation evolved deliberately, update the test to match; if the implementation regressed, fix the implementation.

## Files

- `swissarmyhammer-focus/src/registry.rs` — failing test and the surrounding validator code

## Tags

#stateless-nav

## Review Findings (2026-05-05 15:35)

### Warnings
- [x] `swissarmyhammer-focus/src/registry.rs:320-324` — Stale inline comment contradicts the implemented behavior. The comment says "Surface as a single warning" but the code in the matching `if is_registration && any_zero_dim` branch (lines 330-331) emits no log at all (the branch is intentionally empty). The doc-comment higher up (lines 267-269) was correctly updated to "The validator stays silent and continues". Update this inline comment to match — e.g. "Pre-layout transient: stay silent on registration; the first post-layout `update_rect` will surface a real rect or a real error." Otherwise future readers will assume the WARN got accidentally dropped.

### Nits
- [x] `swissarmyhammer-focus/src/registry.rs:330-351` — The empty `if` branch with an explanatory comment followed by a populated `else` is a slightly awkward shape now that the registration path is silent. Consider inverting to `if !(is_registration && any_zero_dim) { ... }` (and dropping the empty branch), or `if is_registration && any_zero_dim { return; }` early-return style. Either reads more naturally than "do nothing here, real work in the else." Not load-bearing — only worth doing if the file is touched again.

### Resolution (2026-05-05)

- Updated the inline comment to match the silent-on-registration behavior (no more "Surface as a single warning").
- Refactored the awkward empty-`if` / populated-`else` into a `suppress_zero_dim_errors` flag that gates only the width/height error branches. An early `return` was considered but rejected because it would also skip the subsequent `LARGE_COORD_BOUND` check on the registration-with-zero-dim path; the flag preserves that check while still flattening the structure.
- `cargo build -p swissarmyhammer-focus` and `cargo test -p swissarmyhammer-focus` both green; no warnings.