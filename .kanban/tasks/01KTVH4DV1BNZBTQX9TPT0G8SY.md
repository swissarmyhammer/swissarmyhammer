---
assignees:
- claude-code
position_column: todo
position_ordinal: '9780'
project: local-review
title: 'refactor(review): consolidate cross-crate review test fixtures onto a shared seam'
---
## What

Follow-up from the review of 01KTVAT6PM2WPDAENM9RW0QQAC. The in-crate fixture triplication in `swissarmyhammer-validators` (scope.rs / drive.rs / probes.rs) is now consolidated into `crates/swissarmyhammer-validators/src/review/test_support.rs` (`#[cfg(test)] pub(crate)`), but copies of the same fixtures still live in OTHER crates:

- `swissarmyhammer-tools` review tests carry their own TestRepo / index-seeding fixtures.
- The agent e2e tests carry a `TinyRepo` git fixture.

These can't import the validators crate's `pub(crate)` test_support module, so sharing needs a cross-crate seam — e.g. a `test-support` feature on `swissarmyhammer-validators` (the pattern `model-embedding` already uses for `MockEmbedder`) exporting the fixtures, with the tools/agent tests importing them and deleting their local copies.

## Acceptance Criteria

- [ ] One canonical home for TestRepo / index_conn / seed_* / loader fixtures, reachable from swissarmyhammer-tools and the agent e2e tests.
- [ ] Local copies in those crates deleted.
- [ ] `cargo test -p swissarmyhammer-validators -p swissarmyhammer-tools` green; clippy --all-targets -D warnings clean.