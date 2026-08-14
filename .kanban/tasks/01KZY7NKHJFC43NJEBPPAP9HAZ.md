---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m00e00w21038ny1dshqc70rd
  text: |-
    Research done.

    Decision: KEEP `profile_memory` and wire it up. It does not go.

    Reasons:
    1. Six tests measure memory by hand in exactly the shape the helper was written for — build a `MemoryProfiler`, run one async operation, read `memory_delta()`. Five are in `file_tools_integrations/performance.rs`; the sixth is `test_high_concurrency_stress_test` in `file_tools_integrations/concurrency.rs`.
    2. The by-hand shape has a real defect the helper corrects. In `test_full_file_read_memory_usage` the profiler opens before the argument map is built and closes after a `match &result` block that formats and prints `r.content.len()`. The assertion then reports that window as the memory the READ cost. `profile_memory` brackets only the awaited operation.
    3. `#[allow(dead_code)]` is not an accepted marker under `dead-code-rust` in any case. That rule states `#[expect(dead_code, reason = "...")]` is the only staging marker, because `#[expect]` expires by itself. So the mark goes whichever way the decision falls.

    Placement: `profile_memory` moves to the parent module `file_tools_integrations.rs`, beside `MemoryProfiler` that it wraps, because it now has callers in two sibling modules and a private item in `performance.rs` is not visible from `concurrency.rs`. That is also where the parent module doc says the shared fixtures live.

    Sweep result: `rg '#!?\[allow\(' crates/swissarmyhammer-tools/tests` finds five marks in the whole test tree. Only one is a `dead_code` mark on a helper with no caller — the `profile_memory` one. The others:
    - `concurrency.rs` `#[allow(clippy::type_complexity)]` on `create_stress_test_operation` — a lint allow, the function has callers.
    - `properties.rs` `#[allow(clippy::useless_vec)]` on a test — a lint allow.
    - `final_http.rs` `#[allow(clippy::field_reassign_with_default)]` on a test — a lint allow.
    - `review_fixture.rs` module-level `#![allow(dead_code)]` — legitimate and documented: the file is `#[path]`-included into four separate test binaries (`review_progress_notifications_test.rs`, `review_progress_stdio_test.rs`, `review_global_subscriber.rs`, and `integration/mod.rs`), so an item used by one binary is unused in another. It hides no helper that has no caller anywhere.
  timestamp: 2026-08-14T15:24:36.098879+00:00
- actor: claude-code
  id: 01m00ebcw09j7s1k9bhymxrsgp
  text: |-
    Implementation landed.

    `profile_memory` moved from `file_tools_integrations/performance.rs` to the parent `file_tools_integrations.rs`, beside the `MemoryProfiler` it wraps, and the `#[allow(dead_code)]` went with the move. Six call sites now use it:
    - `performance.rs`: `test_full_file_read_memory_usage`, `test_offset_limit_read_memory_usage`, `test_large_file_write_memory_usage`, `test_large_file_edit_memory_usage` (two profiled segments — single edit and replace_all), `test_concurrent_operations_memory_usage`.
    - `concurrency.rs`: `test_high_concurrency_stress_test`.

    `MemoryProfiler::new()` and `memory_delta()` now have exactly one caller each — `profile_memory`. No test builds a profiler by hand any more.

    The parent module doc list now names the memory profiler among the fixtures the module carries, which it already did carry.

    Sweep verification, done by measurement rather than by reading the comment: I replaced the `#![allow(dead_code)]` in `review_fixture.rs` with a marker and ran `cargo check -p swissarmyhammer-tools --all-targets`. Eleven items reported "never used". I traced every one to a consumer:
    - `plant_project_validator_set`, `plant_validator_fixture`, `run_review_op`, `working_args`, `report_has_claim`, `FILE_PROJECT_FIXTURE` — `integration/review_e2e.rs`.
    - `PROJECT_VALIDATOR_SET` — `plant_project_validator_set` and `plant_validator_fixture` in the same file.
    - `planted_agent` — `review_progress_notifications_test.rs` and `review_global_subscriber.rs`.
    - `gated_planted_agent` — `review_progress_stdio_test.rs`.
    - `context_at` — `review_global_subscriber.rs`.
    - `extract_text` — `run_review_op` in the same file.

    Each warning means "unused in THIS binary", which is exactly the documented reason for that module-level allow. Nothing is hidden there. I restored the file; `git status` confirms it is unmodified.

    Verification:
    - `cargo nextest run -p swissarmyhammer-tools` — 1521 tests run, 1521 passed, 0 skipped.
    - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, no output.
    - `cargo fmt --check` — clean.
  timestamp: 2026-08-14T15:30:48.832942+00:00
- actor: claude-code
  id: 01m00ebjszgtn9kntyc1e5k2fp
  text: |-
    ### implement — changed
    - evidence: 3 files — crates/swissarmyhammer-tools/tests/integration/file_tools_integrations.rs, crates/swissarmyhammer-tools/tests/integration/file_tools_integrations/performance.rs, crates/swissarmyhammer-tools/tests/integration/file_tools_integrations/concurrency.rs. `cargo nextest run -p swissarmyhammer-tools` 1521 passed 0 failed 0 skipped; `cargo clippy --workspace --all-targets -- -D warnings` exit 0; `cargo fmt --check` clean.
    - next: ready for /review
  timestamp: 2026-08-14T15:30:54.911003+00:00
position_column: doing
position_ordinal: '8280'
title: Dead test helper profile_memory hides behind allow(dead_code)
---
`profile_memory` in `crates/swissarmyhammer-tools/tests/integration/file_tools_integrations/performance.rs` has no caller. It carries `#[allow(dead_code)]`, so the compiler does not report it.

Found while splitting the file for ^0fn6dbf. That card is a pure move, so the helper moved unchanged.

## What to do

- Decide whether the memory tests should call `profile_memory` or whether it should go.
- If it goes, delete it and its `#[allow(dead_code)]`.
- Sweep the same test tree for other `#[allow(dead_code)]` marks that hide a helper with no caller.

## Done when

- No helper in `file_tools_integrations/` is kept alive only by an `allow` mark.
- `cargo nextest run -p swissarmyhammer-tools` is green and `cargo clippy --workspace --all-targets -- -D warnings` is clean.

#tool-validators