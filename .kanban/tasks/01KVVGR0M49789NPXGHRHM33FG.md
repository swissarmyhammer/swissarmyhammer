---
comments:
- actor: claude-code
  id: 01kw35cbag3mfcmknn8hgktdwf
  text: |-
    Picked up; moved to doing. Implemented both items in apps/kanban-app/src/plugins.rs (test module).

    1. Consolidation: confirmed write_user_command_plugin and write_project_command_plugin shared a byte-identical TypeScript `index.ts` template, differing only in plugins-dir resolution + panic-message strings. Extracted a single `fn write_command_plugin(plugins_dir: &std::path::Path, id: &str, command_id: &str)` holding the shared body (create_dir_all + format! + write, with generic "plugin directory"/"plugin index.ts" expects). write_project_command_plugin is now a thin wrapper that resolves `board_dir.join(".kanban").join("plugins")` then delegates (kept because 5 callers pass a board_dir). write_user_command_plugin was a pure same-signature pass-through, so I deleted it and pointed its single caller at write_command_plugin(&user_plugins, ...) directly. No behavioral change.

    2. Constants: added four module-scope (test-mod) consts next to existing TIMEOUT/SETTLE — AVAILABLE_POLL (20ms, wait_for_available), COMMAND_POLL (100ms, list-command poll x3), WATCHER_SETTLE (300ms, OS watcher settle x4), HANDLE_DROP_POLL (50ms, Weak-upgrade poll). All 9 hardcoded Duration::from_millis sleep literals now reference these by name; zero literal sleeps remain.

    Verification: `cargo nextest run -p kanban-app plugins::` -> 12 passed (the 1 "leaky" flag on all_builtin_command_plugins_load_with_full_baseline is a pre-existing nextest process-leak note, unrelated). `cargo fmt` clean. clippy: plugins.rs produces ZERO warnings (verified via --no-deps). Two PRE-EXISTING rust-1.95.0 clippy lints block a strict `-D warnings` run but are in files I did not touch — `apps/kanban-app/src/menu.rs` (empty_line_after_doc_comments) and `crates/swissarmyhammer-window-service/src/shell.rs` (manual_contains). Left out of scope per "do ONLY these two items".
  timestamp: 2026-06-26T23:48:58.832546+00:00
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffff980
title: Consolidate duplicated write_user/project_command_plugin + extract poll-timeout constants in kanban-app plugins.rs
---
## What

Standing cleanup surfaced by the review of `^p5njvr0` (these are pre-existing, NOT introduced by that task — they were flagged because the review engine scans whole files).

In `apps/kanban-app/src/plugins.rs` the test helpers `write_user_command_plugin` (~line 1047) and `write_project_command_plugin` (~line 1680) are near-duplicates: both create a plugin directory, write identical TypeScript plugin code to `index.ts`, and differ only in how the plugins directory is resolved and in error-message strings. Extract a single parameterized helper `fn write_command_plugin(plugins_dir: &std::path::Path, id: &str, command_id: &str)` that both call sites invoke with the appropriate plugins directory, expressing the variation as data.

Also extract the repeated hardcoded polling timeouts in the same module into named module-level constants:
- ~line 305: `20` ms event-loop poll interval (in `wait_for_available`)
- ~line 340: `100` ms poll interval
- ~line 375: `300` ms OS file-watcher settle interval (appears multiple times)
- ~line 540: `50` ms weak-handle upgrade poll interval

(Line numbers are approximate — they shifted slightly after `^p5njvr0` added seeding calls. Locate by symbol/value.)

## Acceptance Criteria
- [ ] A single `write_command_plugin` helper replaces both `write_user_command_plugin` and `write_project_command_plugin` bodies; both call sites delegate to it. No behavioral change.
- [ ] The four hardcoded poll/settle timeouts are named `const` items at module scope and referenced by name.
- [ ] No duplicated TypeScript-plugin-writing template remains in the two former functions.

## Tests
- [ ] `cargo nextest run --package kanban-app plugins::` → all plugins tests still pass (these helpers are exercised by `plugins::tests` — `a_project_plugin_loads_in_its_board_only`, `a_project_plugin_shadows_a_user_plugin_with_the_same_id`, `watcher_picks_up_a_plugin_dropped_into_user_layer`, etc.).
- [ ] `cargo clippy -p kanban-app --tests -- -D warnings` shows no new warnings from the change.

## Workflow
- Use `/tdd` discipline: the existing `plugins::tests` are the regression net; keep them green through the refactor (pure refactor, no new behavior).