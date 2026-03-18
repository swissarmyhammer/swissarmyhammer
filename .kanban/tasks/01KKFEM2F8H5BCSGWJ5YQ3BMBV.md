---
depends_on:
- 01KKFEKNJVWK345BNYFVGG6S6C
position_column: done
position_ordinal: ffffff9080
title: Make non-embedding tests skip model loading
---
## What

Now that `embedding_enabled` exists and we have a real embedding test, update all other workspace tests to use `embedding_enabled: false`. This means they only parse + write chunks to DB (no model download/load/inference), making the test suite fast.

### Scope
Two test locations need updating:
1. `swissarmyhammer-treesitter/src/unified.rs` inline tests — `open_and_wait()` helper and any test using `Workspace::new()` / `Workspace::open()` that doesn't need embeddings
2. `swissarmyhammer-treesitter/tests/workspace_leader_reader.rs` — same pattern

The `open_and_wait` helper should accept an optional `IndexConfig` or have a `open_and_wait_no_embedding` variant. Tests that need embeddings (the one from card 2) explicitly pass `embedding_enabled: true`.

### Approach
- Add `open_and_wait_with_config(dir, config)` helper
- Default `open_and_wait` uses `embedding_enabled: false`
- The real duplicate detection test uses `open_and_wait_with_config` with `embedding_enabled: true`
- Remove the vacuous duplicate tests that were replaced by card 2

## Acceptance Criteria
- [ ] Non-embedding tests don't load any model
- [ ] Test suite runs significantly faster (seconds, not minutes)
- [ ] The real embedding test from card 2 still passes with embeddings enabled
- [ ] No test asserts `is_ok()` on duplicate results without checking actual content

## Tests
- [ ] `cargo test -p swissarmyhammer-treesitter` — all tests pass
- [ ] Timing: non-embedding tests complete in under 10 seconds total