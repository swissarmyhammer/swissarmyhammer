---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kv9bqed046e3rj010563775v
  text: 'Additional cross-cutting cleanup deferred here from card ^xqgghvd (review 2026-06-16 17:58, warning #5): the `swissarmyhammer-ui-state` crate has inconsistent acronym casing in its own public type names — `UIState` (struct, src/state.rs, SCREAMING acronym) vs `UiStateServer` (struct, src/service.rs, PascalCase acronym). Standardize to one convention. NOTE the scope: `UIState` alone appears in ~1209 references across ~250 files (rg `\bUIState\b`); renaming either public type is a crate-wide rename touching many unrelated consumers — exactly the CAUTION scope this project defers. The e2e test `builtin_app_shell_commands_e2e.rs` only *references* these external type names (it cannot fix the inconsistency locally without renaming the source structs), so it was not changed under ^xqgghvd. Modern Rust (RFC 430) treats acronyms as words in PascalCase, suggesting `UiState`/`UiStateServer`.'
  timestamp: 2026-06-16T23:19:38.656814+00:00
- actor: claude-code
  id: 01kv9bt1ac9f4thap1c40ecpce
  text: |-
    Third cross-cutting cleanup deferred here from card ^xqgghvd (review 2026-06-16 17:58, warning #2): the `write_plugin` test helper is duplicated across plugin-crate integration test files. CURRENT STATE (verified, the finding's line refs were stale): `fn write_plugin(dir, body)` appears in 4 files — `tests/sdk.rs:155`, `tests/callbacks.rs:104`, `tests/plugin_host.rs:75`, `tests/event_subscription_e2e.rs:116` (NOT in tool_module_exposure_test.rs / hot_reload_e2e.rs as the finding claimed). `tests/discovery.rs:75` has a related-but-different `write_plugin_in_layer`.

    IMPORTANT — the 4 copies are NOT byte-identical, so this is not a trivial move:
    - `sdk.rs` writes `entry.ts` and uses `async load(): Promise<unknown>` returning `globalThis.__result` (the SDK wire-shape tests capture the dispatch return value).
    - `callbacks.rs` / `plugin_host.rs` / `event_subscription_e2e.rs` write `index.ts` and use `async load(): Promise<void>` (no result capture).

    So consolidation needs a parameterized helper (entry filename + whether to capture/return `__result`), placed in a shared module. The existing `tests/support/mod.rs` is the example-plugin staging module and pulls heavy deps (`swissarmyhammer-tools`, `swissarmyhammer-git`); none of these 4 files currently `mod support;`, so adding the helper there would compile that whole module (and its deps) into 4 more test binaries. Better: add a small new `tests/plugin_fixture.rs`-style shared module (or a lean `write_plugin` in support) and `mod`-include it in the 4 files.

    Card ^xqgghvd touched only `sdk.rs` among these (for the bindCommandRun SDK test); the other 3 are unrelated to that card — consolidating forces edits across all 4 + a new/changed support module, exactly this project's deferred CAUTION scope.

    Plan:
    - [ ] Define a single parameterized `write_plugin` (entry filename + result-capture flag) in a shared test module.
    - [ ] Replace the 4 local copies (sdk.rs, callbacks.rs, plugin_host.rs, event_subscription_e2e.rs) with the shared import; preserve each call site's current entry-filename + result-capture behavior.
    - [ ] `cargo nextest run -p swissarmyhammer-plugin` green.
  timestamp: 2026-06-16T23:21:03.564812+00:00
- actor: wballard
  id: 01kv9cz8zh534yhy82rmhevqmy
  text: 'Picked up; moved to doing. Part A (copy_dir_recursive) done: verified all 10 copies byte-identical (shasum 5803335e across all 10), moved one copy into tests/integration/support.rs as `pub fn copy_dir_recursive` with a doc comment, deleted the 10 local defs, and added `copy_dir_recursive` to each file''s existing `use crate::support::{...}` import. Confirmed only one definition remains (support.rs). Gate run pending after B and C.'
  timestamp: 2026-06-16T23:41:23.825200+00:00
- actor: wballard
  id: 01kv9d3xnwn70mrgbyamgf2zwq
  text: 'Part B (write_plugin) done. Verified current state matches the comment: 4 copies in sdk.rs (entry.ts, Promise<unknown> capturing globalThis.__result), callbacks.rs/plugin_host.rs/event_subscription_e2e.rs (index.ts, Promise<void>). Created a NEW lean shared module tests/fixture/mod.rs (std-only, no swissarmyhammer-tools/git deps) with a parameterized `write_plugin(dir, entry_file, body, LoadResult::{Void,Captured})`. Each of the 4 files now `#[path = \"fixture/mod.rs\"] mod fixture;` and keeps a 1-line local `write_plugin(dir, body)` adapter binding its fixed entry filename + result flag, so all existing call sites stay unchanged and behavior is preserved. Did NOT touch tests/support/mod.rs (heavy deps), per the plan. Gate run next.'
  timestamp: 2026-06-16T23:43:56.092231+00:00
- actor: wballard
  id: 01kv9dgtb3dgraxx5gy2sad2ew
  text: |-
    Part C (UIState rename) done in one pass — completed, NOT deferred. Scoping decision: the card's Part C target is the swissarmyhammer-ui-state crate's own Rust type-name casing (UIState struct vs UiStateServer struct). Verified UiStateServer already exists (no UIStateServer anywhere). The Rust types needing the rename were UIState (struct), UIStateChange (enum), UIStateInner (private inner) — all in this crate, all SCREAMING-acronym. Renamed all three to UiState/UiStateChange/UiStateInner via word-boundary perl substitution (\bUIState\b etc.) across all 46 .rs files that reference them, plus the Debug `debug_struct(\"UiState\")` label + its test assertion (caught by the same \bUIState\b string-literal match).

    Deliberately did NOT rename the TypeScript/React symbol family (useUIState, UIStateProvider, mockUIState, UIStateContext, UIStateChangedEvent, UIStateSnapshot, UIStateChangeKind, useAppShellUIState, getUIStateResponse, useUIStateLoading, UIStateProbe, the TS-side UIStateChange mirror) — those are frontend naming, a separate convention domain, not this crate's Rust type identifiers, and the prompt scoped C to the Rust type only. Confirmed no auto-generated bindings (no ts-rs/specta/typeshare) couple the Rust enum name to TS, and UIStateChange serializes externally-tagged by variant so the wire shape is unchanged. The 4 .rs doc-comment mentions of UIStateProvider/useUIState (in apps/kanban-app/src/commands.rs) correctly LEFT intact — they name the React symbols. Also updated stale prose: ui-state & command-service Cargo.toml descriptions/comments and ARCHITECTURE.md.

    Verified: cargo build --workspace OK (exit 0); cargo nextest -p swissarmyhammer-ui-state 143/143 pass; zero UIState/UIStateChange/UIStateInner/UIStateServer Rust identifiers remain; UiStateServer consistent.
  timestamp: 2026-06-16T23:50:58.659985+00:00
- actor: wballard
  id: 01kv9dwnta4jvtppvcw8pmcscg
  text: 'GATE RESULTS (all required green):\n- cargo nextest -p swissarmyhammer-command-service: 166/166 pass (Part A).\n- cargo nextest -p swissarmyhammer-plugin: 175 pass, 2 fail — file_notes_e2e::file_notes_plugin_round_trips_through_files_tool and example_layering_e2e::committed_examples_coload_across_layers. Both are the PRE-EXISTING CWD-dependent failures named in this card (per ^xqgghvd). Proof they''re not mine: (a) git status shows neither file modified by me; (b) failure is environmental — ''the first note must exist at /var/folders/.../notes/hello.txt … relative-path write through the real files handler did not land it: No such file or directory''. All 4 Part-B files (sdk/callbacks/plugin_host/event_subscription) PASS. The 3 hot_reload tests the card mentions actually passed in my runs.\n- cargo build --workspace: OK (exit 0).\n- cargo nextest -p swissarmyhammer-ui-state: 143/143 pass.\n\nCLIPPY: `cargo clippy --workspace --all-targets -- -D warnings` surfaces 3 errors, ALL pre-existing and NOT from my changes:\n  1. swissarmyhammer-focus/src/navigate.rs:45 doc_lazy_continuation — focus crate has ZERO changes from me (git diff --stat empty).\n  2. swissarmyhammer-focus/src/state.rs:319 too_many_arguments — same, focus untouched.\n  3. swissarmyhammer-window-service/src/shell.rs:437 manual_contains — my ONLY change in shell.rs is a one-word doc comment (UIState→UiState); the lint-triggering `is_open` closure at line 437 is byte-identical on HEAD (verified via `git show HEAD:...`). A doc-comment word swap cannot create a manual_contains lint.\nNo NEW warnings introduced by my changes.\n\nChanged files: 10 command-service integration tests + support.rs (A); 4 plugin tests + new tests/fixture/mod.rs (B); ~46 .rs files for the UIState→UiState rename + ARCHITECTURE.md + 2 Cargo.toml (C).'
  timestamp: 2026-06-16T23:57:27.242944+00:00
- actor: claude-code
  id: 01kvznqk2s57bngz5sd4s7cmv8
  text: |-
    ## Review Findings (2026-06-25)

    Re-picked from review. Parts B (tests/fixture/mod.rs) and C (UIState→UiState rename, 0 UIState rust identifiers remain) are confirmed landed. But Part A (consolidate copy_dir_recursive) is INCOMPLETE:

    - [ ] `builtin_ai_commands_e2e.rs:65` still has its own local `fn copy_dir_recursive` definition — it was missed during the consolidation. All 11 other integration test files import the shared `copy_dir_recursive` from `support.rs`; only this one still defines its own copy. Delete the local def and route it through `use crate::support::{... copy_dir_recursive ...}` like the others, preserving byte-identical behavior.
    - [ ] `cargo nextest -p swissarmyhammer-command-service` green.
  timestamp: 2026-06-25T15:17:46.713416+00:00
- actor: claude-code
  id: 01kvznxtayh5nyz5scpxnwq613
  text: |-
    Re-picked from review; addressed the lone outstanding Part A finding.

    FIX: builtin_ai_commands_e2e.rs still had its own local `fn copy_dir_recursive` (missed during the original Part A pass). Verified its body was byte-identical to support.rs's `pub fn copy_dir_recursive` (same create_dir_all, same recursion, same .expect messages — only differing by `pub`). Deleted the local def + its doc comment, and added `copy_dir_recursive` to the file's existing import, preserving its pre-existing `super::support` style: `use super::support::{copy_dir_recursive, try_call_command};`. (Siblings use `crate::support::{...}`; both styles coexist in the suite — kept this file's existing `super::` form for a minimal change.)

    VERIFICATION (all fresh):
    - `rg -c 'fn copy_dir_recursive' crates/swissarmyhammer-command-service/tests/integration/` → exactly ONE match: support.rs:1. No other definition remains.
    - `cargo nextest run -p swissarmyhammer-command-service` → 173/173 passed, 0 failed, 0 skipped (suite has grown since the earlier 166/166 note; all green).
    - `cargo fmt` → clean, no reformatting of the touched file.
    - `cargo clippy -p swissarmyhammer-command-service --all-targets -- -D warnings` → the only errors are the SAME 3 pre-existing dependency-crate lints documented in the prior GATE comment: swissarmyhammer-focus doc_lazy_continuation + too_many_arguments, swissarmyhammer-window-service manual_contains. `git diff HEAD --stat` shows ZERO changes in focus/ or window-service/ — they're untouched by me. My only .rs change is builtin_ai_commands_e2e.rs (1 insertion, 16 deletions); deleting a duplicate fn + extending an import cannot introduce a clippy warning.
    - double-check agent: PASS (functional identity confirmed, single definition, no scope creep).

    Leaving in doing for /review.
  timestamp: 2026-06-25T15:21:10.750321+00:00
position_column: doing
position_ordinal: '80'
project: ui-command-cleanup
title: Consolidate duplicated copy_dir_recursive() across command-service integration tests into support.rs
---
## What

This card collects two cross-cutting test/source-naming cleanups deferred from card `01KTW3NAXYMT4XKA3QWXQGGHVD` (^xqgghvd) because each forces edits to several unrelated files (the review's explicit CAUTION clause).

### 1. Duplicated `copy_dir_recursive`

`fn copy_dir_recursive(source, destination)` is verbatim-duplicated across 10 integration test files in `crates/swissarmyhammer-command-service/tests/integration/`:

- builtin_app_shell_commands_e2e.rs
- builtin_file_commands_e2e.rs
- builtin_entity_commands_e2e.rs
- builtin_perspective_commands_e2e.rs
- builtin_nav_commands_e2e.rs
- builtin_board_commands_e2e.rs
- builtin_grid_commands_e2e.rs
- builtin_task_commands_e2e.rs
- builtin_kanban_misc_e2e.rs
- full_baseline_e2e.rs

Each copy is an independent maintenance burden (a bug or improvement in one is invisible to the others). All copies are byte-identical: a recursive directory copy used to stage a builtin plugin bundle into a temp workspace.

There is already a shared test-support module these tests import: `crate::support` (`tests/integration/support.rs`, e.g. `use crate::support::call_command;`). Move ONE copy of `copy_dir_recursive` into `support.rs` as `pub fn copy_dir_recursive(...)` (it already carries `#![allow(dead_code)]`), delete the 10 local copies, and add `use crate::support::copy_dir_recursive;` to each file.

### 2. Inconsistent acronym casing in `swissarmyhammer-ui-state` public types

The crate's own public type names disagree on acronym casing: `UIState` (struct, `src/state.rs`, SCREAMING acronym) vs `UiStateServer` (struct, `src/service.rs`, PascalCase acronym). Standardize to one convention (RFC 430 / modern Rust treats acronyms as words -> `UiState`/`UiStateServer`). NOTE scope: `UIState` alone appears in ~1209 references across ~250 files - a crate-wide rename through many unrelated consumers. The e2e test only references these external names, so the inconsistency cannot be fixed locally in any single test file.

### 3. Duplicated `write_plugin` test helper (added via comment)

Parameterized into a lean shared module; see comments.

## Why a separate task

Both surfaced as review findings on card ^xqgghvd, which touched a narrow set of files. Consolidating / renaming forces edits to many unrelated files - broader than that card's scope - so deferred here per the review's explicit CAUTION clause.

## Acceptance Criteria

- [x] `copy_dir_recursive` defined exactly once, in `support.rs`.
- [x] All 10 integration test files import it from `crate::support`; no local copies remain.
- [x] `write_plugin` consolidated into a single parameterized helper in a lean shared module; the 4 plugin-crate test files import it, each call site's entry-filename + result-capture behavior preserved.
- [x] `UIState`/`UiStateServer` acronym casing standardized to one convention crate-wide; no consumer left referencing a removed name. (UIState/UIStateChange/UIStateInner Rust types -> UiState/UiStateChange/UiStateInner; UiStateServer already correct; TS/React symbol family deliberately out of scope.)
- [x] `cargo nextest run -p swissarmyhammer-command-service` green (no behavior change). 166/166.
- [x] Workspace build + `swissarmyhammer-ui-state` tests green. build OK; 143/143.
- [x] No new clippy warnings. (3 surfaced lints are all pre-existing in untouched code; verified vs HEAD.)