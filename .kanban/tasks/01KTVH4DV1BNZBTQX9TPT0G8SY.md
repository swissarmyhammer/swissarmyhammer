---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffaf80
project: local-review
title: 'refactor(review): consolidate cross-crate review test fixtures onto a shared seam'
---
## What

Follow-up from the review of 01KTVAT6PM2WPDAENM9RW0QQAC. The in-crate fixture triplication in `swissarmyhammer-validators` (scope.rs / drive.rs / probes.rs) is now consolidated into `crates/swissarmyhammer-validators/src/review/test_support.rs`, exposed cross-crate behind the `test-support` feature. Copies of the same fixtures that still lived in OTHER crates are now removed:

- `swissarmyhammer-tools` review tests (`tests/integration/review_fixture.rs` and `src/mcp/tools/review/tests.rs`) carried their own TestRepo / on-disk-index-seeding fixtures.
- The agent e2e test (`crates/swissarmyhammer-agent/tests/review_real_model_e2e.rs`) carried a `TinyRepo` git fixture + `seed_empty_index`.

## How (done)

- Promoted the canonical fixtures in `test_support.rs` from `pub(crate)` to `pub` so they cross the crate boundary via the existing `test-support` feature: `TestRepo` (+ `Default`), `index_conn`, `seed_file`/`seed_chunk`/`seed_symbol`/`seed_call_edge`, `loader_with`, `ruleset`, `body`, `DIM`, `dup_emb`.
- Added one new shared seam, `on_disk_index_conn(root)`, that builds the schema-applied on-disk index at the production `<root>/.code-context/index.db` path (the in-memory `index_conn` is for the engine's own probe tests). The per-table row seeders are reused for scenario rows; only the boilerplate moved.
- `tools` `tests.rs` + `review_fixture.rs` now import `TestRepo` / `on_disk_index_conn` / `seed_*` / `body` / `DIM` from the seam and deleted their local copies; `review_fixture.rs` re-exports (`pub use`) the shared symbols so its sibling `#[path]`-included binaries keep importing them from `review_fixture`.
- agent e2e: added `swissarmyhammer-validators` dev-dep `features = ["test-support"]`, replaced `TinyRepo`/`seed_empty_index` with the shared `TestRepo`/`on_disk_index_conn`, and dropped the now-unused `git2`/`rusqlite`/`swissarmyhammer-code-context`/`tempfile` dev-deps. Also filled the stale `force: false` field on the test's `ReviewRequest`.

## Acceptance Criteria

- [x] One canonical home for TestRepo / index_conn / seed_* / loader fixtures, reachable from swissarmyhammer-tools and the agent e2e tests.
- [x] Local copies in those crates deleted.
- [x] `cargo test -p swissarmyhammer-validators -p swissarmyhammer-tools` green; clippy --all-targets -D warnings clean (verified with --all-features on all three touched crates; agent e2e compiles).

## Review Findings (2026-06-14 19:12)

Scope: this task's changed files only (`test_support.rs`, `tools/.../review/tests.rs`, `tools/tests/integration/review_fixture.rs`, `agent/tests/review_real_model_e2e.rs`). Findings attributable to pre-existing, untouched code were excluded per the review scope — see the "Out of scope" note below.

### Nits

- [x] `crates/swissarmyhammer-validators/src/review/test_support.rs` — Newly added `pub fn on_disk_index_conn` has no doc comment, and it sits beside the existing in-memory `index_conn`. The disk-vs-memory distinction and the side effect of creating `<root>/.code-context/index.db` are invisible at the call site. Add a `///` noting it opens an on-disk code-context index at `root/.code-context/index.db` (creating the dir), the disk counterpart to the in-memory `index_conn`, for tests that need the index discoverable by path.
- [x] `crates/swissarmyhammer-validators/src/review/test_support.rs` — Newly added `pub fn with_script` (the lowest-level multi-needle constructor) has no doc comment. The `ScriptedAgent` type doc directs callers to `new`/`with_config` only, so this third constructor's distinct purpose is undiscoverable. Add a `///` noting it is the lowest-level constructor taking pre-built multi-needle script entries (every needle in a tuple must match for the entry to fire), which `with_config` wraps for the single-needle common case.
- [x] `crates/swissarmyhammer-validators/src/review/test_support.rs` — Newly added `pub fn rebind_broadcast` has no doc comment; its purpose is non-obvious from the signature. Add a `///` explaining it produces a new agent reusing `base`'s script but bound to the given broadcast channel (and bridge setting), for tests that need the same script driven over a backend broadcast.

### Out of scope (recorded, not attributed to this task)

The engine also surfaced duplication findings on the in-scope files that belong to PRE-EXISTING code this task did not add or modify (verified against the working diff): `is_model_unavailable` / `resolve_qwen_test_config` duplicated with `ai_panel_e2e.rs` (introduced in 945a7583f), and the `scripted_factory` twin + bare `64`/`256` `broadcast::channel` capacity literals in `tools` review tests. None appear as added/changed lines in this task's diff, so they are excluded per the scope (test-only consolidation; pre-existing patterns in untouched code are not blockers). Worth a separate follow-up if the team wants the real-model skip helpers and `scripted_factory` hoisted to a shared home.