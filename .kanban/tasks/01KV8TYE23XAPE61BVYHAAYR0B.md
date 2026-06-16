---
position_column: todo
position_ordinal: fe80
title: Pre-existing clippy errors in swissarmyhammer-focus block `clippy -D warnings`
---
## What

`cargo clippy -p swissarmyhammer-focus --all-targets -- -D warnings` fails with two errors (clippy 1.95.0). Pre-existing — surfaced while running clippy across the dependency graph for an unrelated task (tags⇄body sync, `^8q2v2vf`); `swissarmyhammer-focus` was not modified by that work.

### Errors

1. `crates/swissarmyhammer-focus/src/state.rs:319` — `error: this function has too many arguments (8/7)` on `pub fn focus_lost(&mut self, registry, snapshot, ..., window: Option<WindowLabel>) -> Option<FocusChangedEvent>`. Fix: bundle related params into a struct (e.g. a `FocusLostCtx`/params struct) rather than `#[allow(clippy::too_many_arguments)]`.
2. `crates/swissarmyhammer-focus/src/navigate.rs:45` — `error: doc list item without indentation`. Fix: re-indent the continuation line of the doc-comment list item.

## Acceptance Criteria
- [ ] `cargo clippy -p swissarmyhammer-focus --all-targets -- -D warnings` passes with zero errors/warnings.
- [ ] No `#[allow(...)]` suppressions added; fixes address the root cause (param struct + doc indentation).

## Tests
- [ ] `cargo clippy -p swissarmyhammer-focus --all-targets -- -D warnings` → clean.
- [ ] `cargo test -p swissarmyhammer-focus` → green (the `focus_lost` refactor must not change behavior; update callers).

## Notes
- Discovered during `^8q2v2vf`. Independent of that change. #test-failure