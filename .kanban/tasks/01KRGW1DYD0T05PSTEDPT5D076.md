---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffdd80
title: Group dropdown is empty — runtime FieldDefs are missing groupable flag
---
## What

**ITERATION 4 (2026-05-13)** — RESOLVED.

User's frustration was correct: iter-1 and iter-2 both shipped passing regression tests yet the live Group By popover stayed empty. Root cause was at the consumer layer, not in the library — a place neither test exercised.

## User's stated requirement

> "we need to be able to group tasks on the board at least by project, tag, assignee — so without bullshit hardcoding why aren't at least these options available to group?"

## Root cause — exact stage

`kanban-app::commands::list_commands_for_scope` called `commands_for_scope` with `options_registry: None` (the LAST argument). The library design intentionally allows `None` (the native menu bar legitimately needs no enrichment), so this was not a library bug — it was a caller bug. With `None`, `enrich_options` is a no-op: every emitted `ParamDef.options` stays at the YAML value (`None`), so every picker (Group By, View, Sort, etc.) renders empty in the live app.

The bug shipped during the iter-1 / iter-2 churn around this code path. `KanbanContext::options_registry()` had been added precisely so consumers could thread the kanban built-in resolvers through, but `list_commands_for_scope` never started threading it.

## Why prior iterations did not catch this

- **Iter-1 test** (`perspective_group_command_emits_groupable_fields_from_live_field_loader`): used `AddPerspective::with_view_id(BOARD)`, taking the **strict path** in `entity_type_for_perspective`, AND passed `Some(&opts_registry)` directly to `commands_for_scope`. The test exercised the library contract (which was correct); the bug lived in the GUI shim above it.
- **Iter-2 test** (`perspective_group_options_use_active_view_when_perspective_view_id_is_none`): pinned the legacy `view_id: None` shape but for the **grid** view kind on a workspace with multiple grid-kind builtins (active-view tiebreaker code path). Same as iter-1, it passed `Some(&opts_registry)` directly to `commands_for_scope`, so the GUI shim bug remained invisible.

Neither test exercised the kanban-app → kanban-crate shim that drops the registry.

## What the new test exercises

`perspective_group_options_include_assignees_and_tags_for_board_task_perspective` (in `swissarmyhammer-kanban/tests/options_enrichment.rs`) calls a **new** helper `commands_for_scope_with_context` that pulls both `fields` AND `options_registry` from the active `KanbanContext`. The test asserts options are populated for `assignees`, `tags`, `project` on a legacy view-id-less board perspective.

Verified the test is sensitive to the regression: temporarily passing `None` for the registry inside the helper causes the test to fail at the explicit `options must be Some` assertion (with a message that names the iter-4 bug). Reverting to passing the registry → test passes.

## Fix shape

1. New helper `swissarmyhammer_kanban::scope_commands::commands_for_scope_with_context` takes `Option<&KanbanContext>` for the active context and pulls BOTH `fields()` AND `options_registry()` from the same object. This makes "forgetting one of the two" unrepresentable at the call site.
2. `kanban-app::list_commands_for_scope` switched to call the new helper (instead of calling `commands_for_scope` directly with `None` for the registry).
3. Removed all `[group-debug]` tracing/console.log instrumentation from `swissarmyhammer-kanban/src/dynamic_sources.rs`, `swissarmyhammer-perspectives/src/options_resolvers.rs`, `kanban-app/src/commands.rs`, `kanban-app/ui/src/components/perspective-tab-bar.tsx`, `kanban-app/ui/src/components/command-popover.tsx`.

## Tests

- [x] New test `perspective_group_options_include_assignees_and_tags_for_board_task_perspective` in `swissarmyhammer-kanban/tests/options_enrichment.rs` — failing RED reproduced by transient `None` in the helper; GREEN after restoring registry threading. Asserts `assignees`, `tags`, `project` all appear in the Group By options.
- [x] `cargo test -p swissarmyhammer-kanban` — all green (1141 lib + 10 options_enrichment + dynamic_sources + every other test).
- [x] `cargo test -p swissarmyhammer-perspectives -p kanban-app` — all green (106 + 66 tests).
- [x] `cd kanban-app/ui && npx vitest run command-popover perspective-tab-bar` — 15 test files, 96 tests passed.
- [x] `cargo clippy -p swissarmyhammer-kanban -p kanban-app -p swissarmyhammer-perspectives --tests` — clean.

## Hard requirements

- [x] The iter-4 test exists and FAILS on simulated regression (registry threading removed).
- [x] The fix makes that test pass.
- [x] Test is sensitive to the registry threading at the new helper layer.
- [x] No hardcoded field IDs that aren't in the builtin YAMLs — all three (assignees, tags, project) live in `swissarmyhammer-kanban/builtin/definitions/`.
- [x] Iter-1 and iter-2 tests still pass.
- [x] All `[group-debug]` tracing added in commit `38f8801ee` is removed.

#command-driven-ui #bug #iter4