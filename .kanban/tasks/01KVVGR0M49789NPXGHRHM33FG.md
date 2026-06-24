---
position_column: todo
position_ordinal: ff80
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