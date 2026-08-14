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
- actor: claude-code
  id: 01m00ept8b2tv7kh9ht0dn2bw5
  text: |
    ### review — findings
    - evidence: 2 findings (2 confirmed, 1 refuted, 9 attempted) — crates/swissarmyhammer-tools/tests/integration/file_tools_integrations/concurrency.rs:457, crates/swissarmyhammer-tools/tests/integration/file_tools_integrations/performance.rs:301
    - scope: review sha c33d587eb~1..c33d587eb — the diffs only. 3 files reviewed, 0 not reviewed.
    - both findings land on lines this commit added or modified. concurrency.rs:457 is an added line. performance.rs:301 is a modified line (indent change as the loop moved into the closure).
    - premises verified by reading both lines. 0 findings dropped for a false premise. No test-code exception applied.
    - next: name the two constants, then re-review.
  timestamp: 2026-08-14T15:37:02.987252+00:00
- actor: claude-code
  id: 01m00eqr8wa55k0hqkzx42pryw
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 3 files. The decision went KEEP AND WIRE UP, not delete: `profile_memory` was not dead by design, it was dead because nobody connected it. Six tests were measuring memory by hand in exactly its shape — build a `MemoryProfiler`, run one async operation, read `memory_delta()` — five in performance.rs and one in concurrency.rs.
    - The by-hand shape carried a REAL DEFECT the helper corrects. In `test_full_file_read_memory_usage` the profiler opened before the argument map was built and closed after a block that formats and prints `r.content.len()`, so the assertion reported that whole window as the memory the read cost. `profile_memory` brackets only the awaited operation.
    - The helper moved to the parent module beside the `MemoryProfiler` it wraps, because a private item in performance.rs is not visible from the sibling concurrency.rs and the parent is where that tree's shared fixtures live. The `#[allow(dead_code)]` was deleted with the move. `MemoryProfiler::new()` and `memory_delta()` now have exactly one caller each.
    - Sweep: 5 allow marks in the whole test tree. One was the defect; the other four are lint allows on functions that have callers, plus the module-level mark on `review_fixture.rs`. That last one was VERIFIED rather than trusted — replacing it with a marker reports 11 unused items, and every one traces to a real consumer across the four test binaries that `#[path]`-include the file. Restored unmodified.
    - test: green — 1521 swissarmyhammer-tools tests, 0 skipped. fmt and clippy clean.
    - commit: c33d587eb
    - review: findings — concurrency.rs:457, performance.rs:301. Both `magic-numbers/no-magic-numbers`, both on lines this commit ADDED or MODIFIED, confirmed against the diff. 0 dropped for a false premise.

    **Two things this round demonstrates about ^apb04az's engine.** The scope fix held: no finding landed on a pre-existing test line, and `refuted: 1` is the guard rejecting a candidate before it reached the report. And the deleted existing-tests exception behaved correctly — this commit is ENTIRELY test code, so the old blanket exception would have swallowed both findings, while the new engine raised exactly the ones on lines the change authored.
  timestamp: 2026-08-14T15:37:33.724097+00:00
- actor: claude-code
  id: 01m00fcrtyzf7xre1xwjr9a9r4
  text: |-
    Both review findings worked. Neither premise was false: I read both cited lines and each held the literal the finding names.

    The two findings share one cause — an unnamed count literal handed to a concurrency helper or used as a loop bound. I removed that cause from each WHOLE file, not only the two cited lines, and then swept each file for every other literal the same `magic-numbers/no-magic-numbers` rule covers, because that rule reads the whole file and carries no exception for test code.

    Each constant is named for what the number MEANS in its test, never for its value. `STRESS_TEST_CONCURRENCY`, `MIXED_WRITE_OPERATIONS`, `EDIT_FIXTURE_LINES_PER_BLOCK`, `ACCESS_TIME_BUDGET` — not `ONE_HUNDRED`.

    **concurrency.rs — 1 cited site, 37 more found.**
    - 10 more sites of the exact cited cause: two `run_concurrent_test(..., 5, ...)` calls, four `spawn_*_operations(..., 30 | 20)` calls in the mixed test, three `spawn_concurrent_*(..., 50 | 25 | 25)` calls in the access test, and `let base_files = 20` driving a loop (a lowercase local, not a named constant).
    - 27 more sites of the same rule: the stress content size formula `1000 + (i % 10) * 500`, four `.repeat()` sizes, the `i % 4` glob selector and the `i % 2` grep selector, the windowed read triple `i % 3` / `i * 100` / `500`, the `i / 2` writer pairing, and every success floor, error ceiling, second budget and memory budget in the three verify paths.

    **performance.rs — 1 cited site, 21 more found.**
    - 3 more sites of the exact cited cause: three `for i in 0..1000` fixture loops.
    - 18 more sites of the same rule: the `.repeat()` fixture sizes, the four memory budget multiples, the offset/limit read window, `100 * 100`, `50_000_000`, and the trailing `assert_eq!(success_count, 20)`.

    Derived values are now derived, not restated. `MIXED_TOTAL_OPERATIONS` sums the four spawn counts; the mixed success floor is `MIXED_TOTAL_OPERATIONS - MIXED_MAX_ERRORS`; `ACCESS_TOTAL_OPERATIONS` sums its three; the safety total is `SAFETY_WRITE_OPERATIONS + SAFETY_READ_OPERATIONS`. The assertion messages that used to spell `100` or `110` or `2 minutes` now format the constant, so a changed count cannot leave a lying message behind.

    Two structural fixes came out of the same sweep rather than a bare rename:
    - `match i % 4 { 0 => ..., 1 => ..., 2 => ..., _ => ... }` and the `i % 2` if/else became `GLOB_PATTERNS` and `GREP_PATTERNS` slices indexed by `i % LEN`. The modulus is no longer a literal that can drift from the arm count.
    - The two read memory tests each built the SAME five-line fixture. Naming the constants in both copies would have left an exact duplicate on lines this change authored, so the block became `read_memory_fixture()` with two callers.
    - The `content.len() / 1024 / 1024` "{}MB" printlns became `MemoryProfiler::format_bytes(content.len())`, which deletes the unit literals and reports the real size instead of a floor-divided one.

    Time budgets are now `std::time::Duration` constants compared with `total_duration < BUDGET`, rather than `as_secs() < 120`.

    Sweep verification by measurement, not by reading: `rg` for every numeric literal in both files reports only constant definitions, `0`/`1` initializers and increments, `if delta >= 0`, and loop bounds that are named constants or parameters. Nothing unnamed is left.

    Verification:
    - `cargo nextest run -p swissarmyhammer-tools` — 1521 tests run, 1521 passed, 0 skipped.
    - The 9 tests in the two files, run alone — 9 passed.
    - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, no output.
    - `cargo fmt --check` — clean.
  timestamp: 2026-08-14T15:49:02.430113+00:00
- actor: claude-code
  id: 01m00fczgkz5pddj27trgzv2ds
  text: |-
    ### implement — changed
    - evidence: 2 files — crates/swissarmyhammer-tools/tests/integration/file_tools_integrations/concurrency.rs, crates/swissarmyhammer-tools/tests/integration/file_tools_integrations/performance.rs. Both findings flipped to `- [x]`. Cause removed across each whole file: 37 more sites in concurrency.rs, 21 more sites in performance.rs. `cargo nextest run -p swissarmyhammer-tools` 1521 passed 0 failed 0 skipped; `cargo clippy --workspace --all-targets -- -D warnings` exit 0; `cargo fmt --check` clean.
    - next: ready for /review
  timestamp: 2026-08-14T15:49:09.267044+00:00
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

## Review Findings (2026-08-14 10:32)

> Scope: `review sha c33d587eb~1..c33d587eb` — reviewed the diffs only — lines this change added or modified. 3 file(s) reviewed, 0 not reviewed.

- [x] `crates/swissarmyhammer-tools/tests/integration/file_tools_integrations/concurrency.rs:457` `magic-numbers/no-magic-numbers` — Hardcoded value `100` represents concurrency level for stress test and should be a named constant. Define `const STRESS_TEST_CONCURRENCY: usize = 100;` and use it.
- [x] `crates/swissarmyhammer-tools/tests/integration/file_tools_integrations/performance.rs:301` `magic-numbers/no-magic-numbers` — Hardcoded value `20` represents concurrent file operation count and should be a named constant. Define `const CONCURRENT_FILE_OPERATIONS: usize = 20;` and use it.
