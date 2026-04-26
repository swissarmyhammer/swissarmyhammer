---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffdc80
title: Fix "New" so it works uniformly on every grid — prove via logs, fix stale view YAMLs, surface silent field-name drops
---
## What

User reports the feature is still broken: on the Tasks grid and Board, "New Task" works; but "New Tag" and "New Project" do not appear in ANY menu (palette, context menu, + button) on their respective grids. This is a regression from earlier today when at least "New Tag" was confirmed working via the + button.

**Non-negotiable outcome:** One code path. Works for every entity type declared in a grid view's YAML. If it works for one, it works for all. Full stop.

## Root Cause Found (2026-04-17)

`SetActiveViewCmd` (`swissarmyhammer-kanban/src/commands/ui_commands.rs`) changed `active_view` in UIState but did NOT update the `scope_chain`. When a user switched from the Board to the Tags/Projects grid, the backend's `scope_chain` still carried `view:01JMVIEW0000000000BOARD0` from the last `ui.setFocus` that ran while on the board. The command palette reads `scope_chain` from UIState → `list_commands_for_scope` saw the OLD view → `emit_entity_add` emitted `entity.add:task` (not `entity.add:tag` or `entity.add:project`) → user saw only "New Task" regardless of what grid they were on.

All of the backend unit tests passed because they constructed the scope chain directly with `view:{tags_grid.id}` and didn't exercise the view-switch → scope-chain path.

## Fix

`SetActiveViewCmd::execute` now reads the current `scope_chain`, rewrites every `view:*` moniker to point at the new view id, and writes the chain back to UIState. When the user next focuses anything in the new view, `ui.setFocus` will rebuild the full chain from scratch; this bridge keeps the palette/context-menu working in the interim (i.e. when the user hits Cmd+K immediately after switching views).

## Tests Landed

All tests pass on current HEAD. The new `set_active_view_rewrites_view_moniker_in_scope_chain` test has been verified to FAIL when the fix is removed, proving it catches the regression.

### Isolated per-entity-type dispatch tests (backend)
- [x] `dispatch_entity_add_task_creates_task` — `swissarmyhammer-kanban/tests/command_dispatch_integration.rs`
- [x] `dispatch_entity_add_tag_creates_tag`
- [x] `dispatch_entity_add_project_creates_project`

### Isolated per-entity-type emission tests (real registry)
- [x] `entity_add_task_emitted_for_tasks_grid_view_scope` — `swissarmyhammer-kanban/src/scope_commands.rs`
- [x] `entity_add_tag_emitted_for_tags_grid_view_scope`
- [x] `entity_add_project_emitted_for_projects_grid_view_scope`
- [x] `entity_add_emitted_for_every_builtin_view_with_entity_type_real_registry` (bonus cross-cutting guard)

### Cross-cutting regression guard
- [x] `list_commands_for_scope_emits_entity_add_for_every_grid_view` — `swissarmyhammer-kanban/tests/command_dispatch_integration.rs`

### Frontend per-entity-type rendering tests
- [x] Three palette tests — `kanban-app/ui/src/components/command-palette.test.tsx`
- [x] Three context-menu tests — `kanban-app/ui/src/lib/context-menu.test.tsx`

### Root-cause regression guard (NEW — closes the actual runtime bug)
- [x] `set_active_view_rewrites_view_moniker_in_scope_chain` — `swissarmyhammer-kanban/src/commands/ui_commands.rs`. Sets up a scope_chain with `view:BOARD`, dispatches `ui.view.set` with a new view_id, asserts the scope_chain now contains `view:NEW` and NOT `view:BOARD`. **Verified to FAIL on pre-fix code.**
- [x] `set_active_view_leaves_scope_chain_alone_when_no_view_moniker` — guards against synthesising a `view:*` moniker when none exists (e.g. before first focus).

## Instrumentation Landed
- [x] `list_commands_for_scope` in `kanban-app/src/commands.rs` logs scope_chain, views_count, views_with_entity_type, boards/windows/perspectives counts, total command count, by_group breakdown, and the `entity.add:*` ids emitted.
- [x] `emit_entity_add` in `swissarmyhammer-kanban/src/scope_commands.rs` logs at debug level for each scope_moniker decision: view-not-found, entity_type-missing, entity_type-empty, and the final push.

## Acceptance Criteria

- [x] Three isolated dispatch tests pass: `dispatch_entity_add_{task,tag,project}_creates_{task,tag,project}`.
- [x] Three isolated emission tests pass: `entity_add_{task,tag,project}_emitted_for_{…}_grid_view_scope` with the real builtin registry.
- [x] Three palette frontend tests pass: one per entity type.
- [x] Three context-menu frontend tests pass: one per entity type.
- [x] Cross-cutting `list_commands_for_scope_emits_entity_add_for_every_grid_view` passes.
- [x] Root-cause regression guard `set_active_view_rewrites_view_moniker_in_scope_chain` passes AND fails when the fix is removed.
- [x] `cargo nextest run -p swissarmyhammer-kanban` → 983/983 pass.
- [x] `cargo nextest run -p kanban-app` → 71/71 pass.
- [x] `npx vitest run` in `kanban-app/ui` → 1183/1183 pass.
- [x] `cargo clippy -p swissarmyhammer-kanban -- -D warnings` clean.
- [ ] Live on running app: Tags grid + palette → "New Tag" appears — deferred to reviewer to confirm by rebuild + manual test with the new backend behavior.
- [ ] Live on running app: Projects grid + palette → "New Project" appears — same.
- [ ] Live on running app: right-click on Tags grid → "New Tag" — same.
- [ ] Live on running app: right-click on Projects grid → "New Project" — same.

## Workflow

- Used `/tdd` — wrote the tests first at every layer (backend dispatch, backend emission, frontend rendering, root-cause), found they all passed on HEAD, then instrumented + diagnosed in logs + code, found the actual bug (SetActiveViewCmd not updating scope_chain), added the inversion test, proved it would have caught the live regression, landed the fix, confirmed the inversion test now passes.

#entity

## Review Findings (2026-04-17 10:59)

Code review is clean — zero code findings. The fix in `swissarmyhammer-kanban/src/commands/ui_commands.rs` is minimal, correctly located at the seam that owns the state it mutates, idempotent (the `mutated` guard avoids spurious writes when the view id is unchanged or absent), and defended by two direct regression guards plus layered per-entity-type tests at backend and frontend. Instrumentation in `emit_entity_add` and `list_commands_for_scope` covers every decision point that can silently drop an `entity.add:*` command. View YAMLs (`board`, `tasks-grid`, `tags-grid`, `projects-grid`) all carry the required `entity_type`.

Confirmed locally:

- `cargo nextest run -p swissarmyhammer-kanban` targeted regression + per-type tests: 11/11 pass (including `set_active_view_rewrites_view_moniker_in_scope_chain` and `set_active_view_leaves_scope_chain_alone_when_no_view_moniker`).

Design notes (not findings):

- `SetActiveViewCmd::execute` returns only the `UIStateChange::ActiveView` produced by `set_active_view` and drops the `UIStateChange::ScopeChain` returned by the subsequent `set_scope_chain` call. This is fine because `emit_ui_state_change_if_needed` broadcasts the full UIState snapshot whenever any `UIStateChange` is returned — the rewritten scope_chain is included in that snapshot, and the frontend palette re-renders via its `useEffect([open, scopeChain])` on the next tick. No lost-event bug.

### Unchecked items blocking advance to `done`

Four live-app acceptance items remain unchecked (deferred to reviewer in the task body). Per the review skill, I do not flip author-owned checkboxes myself, and the task cannot advance to `done` until those four live-app verifications are confirmed by rebuilding and manually testing the app:

- Tags grid + palette → "New Tag" appears
- Projects grid + palette → "New Project" appears
- Right-click on Tags grid → "New Tag"
- Right-click on Projects grid → "New Project"

Task remains in `review`.