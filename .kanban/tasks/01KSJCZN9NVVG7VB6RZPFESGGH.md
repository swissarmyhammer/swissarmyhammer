---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8d80
title: Fix `test_file_watcher_start_watching_sets_up_debouncer` — asserts debouncer unconditionally despite legitimate early-return
---
## What

`crates/swissarmyhammer-tools/src/mcp/file_watcher.rs::tests::test_file_watcher_start_watching_sets_up_debouncer` fails on bare main (verified on commit `b7ba81dd7`, not introduced by recent work). The test calls `watcher.start_watching(...)` and then, inside an `if result.is_ok()` arm, asserts `watcher.debouncer.is_some()` unconditionally — but `FileWatcher::start_watching` (around `file_watcher.rs:115-118`) legitimately returns `Ok(())` **without** setting `debouncer` when `PromptResolver::get_prompt_directories()` returns an empty list. The test environment hits exactly that case, so `result.is_ok()` is true but `debouncer` is `None`, and the assertion panics.

## Fix

Production contract preserved (option 1 from the card): "no prompt directories => Ok with no debouncer" is intentional design — the code explicitly logs a warning and returns `Ok(())` (file_watcher.rs:116-120). Callers in environments without prompt dirs may rely on this. The test was made deterministic instead:

- Creates a `TempDir` with `.git` (anchors it as the git root via `find_git_repository_root_from()`) and `.prompts/` (the dot-directory `get_prompt_directories()` returns).
- Uses `CurrentDirGuard` (from swissarmyhammer-common test_utils) to chdir into the temp dir for the duration of the test; the existing `#[serial_test::serial(cwd)]` annotation is kept for layered safety with other `serial(cwd)` tests.
- Because the prompt directory list is now guaranteed non-empty, `start_watching` must succeed AND install the debouncer. The assertions are unconditional.

## Acceptance Criteria
- [x] The test passes deterministically in both isolation and as part of `cargo test -p swissarmyhammer-tools` (full crate).
- [x] Either the assertion is gated on a deterministic precondition the test controls, or `start_watching`'s contract is tightened. (Chose precondition; contract preserved.)
- [x] No new `#[ignore]` / `#[allow(...)]` band-aids.
- [x] `cargo clippy -p swissarmyhammer-tools --all-targets -- -D warnings` clean.

## Tests
- [x] The fixed test passes.
- [x] `cargo test -p swissarmyhammer-tools file_watcher` green (45/45).
- [x] Full crate suite (`cargo test -p swissarmyhammer-tools`): 1048 lib + 7 + 1 + 8 + 17 + 1 + 1 + 147 = all passing, zero failures.

## Workflow
- Use `/tdd` only if you change the production contract; for pure test fixes, read the production code first and pick the shape that matches the existing contract. #test-failure