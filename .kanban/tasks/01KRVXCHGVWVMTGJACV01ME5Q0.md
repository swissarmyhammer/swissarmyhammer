---
assignees:
- claude-code
position_column: todo
position_ordinal: '9880'
title: 'swissarmyhammer-entity: fix 12 pre-existing result_large_err clippy errors'
---
## What
`cargo clippy -p swissarmyhammer-entity --all-targets -- -D warnings` (and any clippy run that reaches the crate) fails with 12 `clippy::result_large_err` errors. Root cause: `EntityError::StaleChange` (`crates/swissarmyhammer-entity/src/error.rs:46`) makes `EntityError` ≥168 bytes, so every `Result<_, EntityError>`-returning fn trips the lint.

Error sites:
- `crates/swissarmyhammer-entity/src/changelog.rs` — lines 254, 464, 491
- `crates/swissarmyhammer-entity/src/context.rs` — lines 166, 190, 201
- `crates/swissarmyhammer-entity/src/io.rs` — lines 201, 318, 340, 392, 410, 433

Fix: shrink the `EntityError` enum — most likely `Box` the large payload of the `StaleChange` variant (and any other oversized variant) so the enum is small and the `Err` path is a pointer. Do NOT add `#[allow(clippy::result_large_err)]` — fix the size.

This is **pre-existing tech debt unrelated to the plugin platform** — discovered during plugin-arch task 01KRREBGRC9WTBRRXB7KS8WQT8 (which confirmed via `git stash` that the errors predate that task). Tracked separately so the workspace clippy gate can be clean.

## Acceptance Criteria
- [ ] `cargo clippy -p swissarmyhammer-entity --all-targets -- -D warnings` passes with zero warnings.
- [ ] No `#[allow(clippy::result_large_err)]` was added — the enum was genuinely shrunk.
- [ ] `EntityError` size is reasonable (the large variant payload is boxed).

## Tests
- [ ] `cargo test -p swissarmyhammer-entity` — all existing tests still pass (boxing a variant changes construction/match sites; update them).
- [ ] `cargo build --workspace` succeeds.
- [ ] `cargo clippy -p swissarmyhammer-entity --all-targets -- -D warnings` — clean.

## Workflow
- Use `/tdd` only if adding behavior; this is a refactor — the existing `swissarmyhammer-entity` suite is the regression gate.