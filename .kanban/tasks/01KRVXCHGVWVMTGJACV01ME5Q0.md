---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8180
title: 'swissarmyhammer-entity: fix 12 pre-existing result_large_err clippy errors'
---
## What
`cargo clippy -p swissarmyhammer-entity --all-targets -- -D warnings` (and any clippy run that reaches the crate) fails with 12 `clippy::result_large_err` errors. Root cause: `EntityError::StaleChange` (`crates/swissarmyhammer-entity/src/error.rs:46`) makes `EntityError` ≥168 bytes, so every `Result<_, EntityError>`-returning fn trips the lint.

Error sites:
- `crates/swissarmyhammer-entity/src/changelog.rs` — lines 254, 464, 491
- `crates/swissarmyhammer-entity/src/context.rs` — lines 166, 190, 201
- `crates/swissarmyhammer-entity/src/io.rs` — lines 201, 318, 340, 392, 410, 433

Fix: shrink the `EntityError` enum — most likely `Box` the large payload of the `StaleChange` variant (and any other oversized variant) so the enum is small and the `Err` path is a pointer. Do NOT add `#[allow(clippy::result_large_err)]` — fix the size.

This is **pre-existing tech debt unrelated to the plugin platform** — discovered during plugin-arch task 01KRREBGRC9WTBRRXB7KS8WQT8 (which confirmed via `git stash` that the errors predate that task). It became a hard blocker once plugin-arch task 01KRREC7YF5ENG2M2E7DQYSDGS added a `swissarmyhammer-tools` dev-dependency to `swissarmyhammer-plugin`, dragging this crate into `cargo clippy -p swissarmyhammer-plugin --all-targets`'s reach.

## Resolution
Boxed both `serde_json::Value` payloads of `EntityError::StaleChange` (`expected` and `actual`) so the variant is `field: String` + two pointers. `serde_json::Value` is the heavy member; boxing both brings the enum well under the `result_large_err` threshold. `thiserror`'s `#[error(...)]` format string still works because `Box<serde_json::Value>: Display` derefs transparently. Updated the single construction site in `changelog.rs` to `Box::new(...)`. The match site in `apps/kanban-cli/src/commands/serve.rs` uses `{ .. }` and was unaffected.

**Bundled sibling fix — `claude-agent`.** Making `cargo clippy -p swissarmyhammer-plugin --all-targets -- -D warnings` clean (the actual goal — that command is dragged through the same dev-dep chain) also required clearing a separate pre-existing `clippy::large-enum-variant` error in `claude-agent`: `ToolCallContent::Content` was 360 bytes. Boxed its `ContentBlock` field the same way and updated all construction/match sites in `crates/claude-agent/src/{tool_types.rs,tools.rs,tool_call_lifecycle_tests.rs}`. Confirmed pre-existing via `git stash`. (5 pre-existing, unrelated `claude-agent` `test_acp_terminal_*` test failures found during verification are tracked separately as task 01KRXK1ZGYAHA8YSHJPM6D7503 — not in scope here.)

## Acceptance Criteria
- [x] `cargo clippy -p swissarmyhammer-entity --all-targets -- -D warnings` passes with zero warnings.
- [x] No `#[allow(clippy::result_large_err)]` was added — the enum was genuinely shrunk.
- [x] `EntityError` size is reasonable (the large variant payload is boxed).
- [x] `cargo clippy -p swissarmyhammer-plugin --all-targets -- -D warnings` passes clean (the gate this unblocks).

## Tests
- [x] `cargo test -p swissarmyhammer-entity` — all existing tests still pass.
- [x] `cargo build --workspace` succeeds.
- [x] `cargo clippy -p swissarmyhammer-entity --all-targets -- -D warnings` — clean.

## Workflow
- This is a refactor — the existing `swissarmyhammer-entity` suite is the regression gate.