---
assignees:
- claude-code
position_column: todo
position_ordinal: d280
title: 'kanban-app state.rs: FNV seed, JPEG magic bytes, and related literals'
---
Four production findings in `apps/kanban-app/src/state.rs`, split out of ^qsr5rdt's review. All pre-existing — that commit's only change to this file was `skills: Some(Selector::All)`.

## Items

- FNV seed `5381u64` — unnamed magic constant, real line ~1312.
- JPEG magic bytes `0xFF` / `0xD8` — real line ~1377. Name them, or use a format-detection helper if one exists in the workspace.
- Two further literals in the same production region.

## Line numbers are wrong on this file, badly

The engine cited 646, 704, and 301. All three are far off:

| Cited | Real | What is actually at the cited line |
|---|---|---|
| 646 | ~1312 | `pub fn new_for_test()` |
| 704 | ~1377 | — |
| 301 | ~1458 | `for task in tasks.iter_mut()` |

Grep for the literal, not the line.

## Excluded

Four findings on this file were dropped under the test-refactor exception — `mod tests` begins at line ~1391, so the magic `25`/`20`, `worker_threads = 2`, and `from_secs(2)` items are all test code. Do not "fix" those; they are exempt by policy.

## Acceptance

- No unnamed magic literal in the production region of `state.rs` at the cited sites.
- `cargo nextest run -p kanban-app`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean. #refactor #kanban-app