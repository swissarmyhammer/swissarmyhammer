---
assignees:
- claude-code
depends_on:
- 01KRE1WT72MJWNGQBVAD4V5VKM
- 01KRE7VDF7RXHV39VPEVH23NN4
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffdb80
title: Migrate Filter tab button to command-driven rendering
---
## What

Replace the hardcoded `<FilterFocusButton>` with a registry-rendered `<CommandButton>` driven by a new no-arg `perspective.filter.focus` command. The command's only job is to broadcast the "focus the formula bar's filter editor" signal — the editor itself stays on the tab bar where it already lives.

This is the first command migration and exercises the full pipeline (param-shape metadata → tab-bar registry rendering) end to end for a NO-arg case. Group and Sort migrations later exercise the picker cases.

### Implementation notes

- **Pre-refactor fallback used.** The relocation task `01KRES4EHVAPQGM003FVEBDWED` (move `DynamicSources` / `commands_for_scope` / `execute` impls into domain crates) has not landed yet. `FocusFilterCmd` lives in `swissarmyhammer-kanban/src/commands/perspective_commands.rs` alongside the other perspective commands. The relocation task will move it.
- **Tauri event pattern matches existing markers.** `FocusFilterCmd` returns a `{"FocusFilter": { "perspective_id": "…" }}` envelope; the dispatcher's `handle_focus_filter` (added in `kanban-app/src/commands.rs`) emits `ui.focus.filter` on the same channel — same shape as `DragStart`/`CreateWindow`/etc.
- **Frontend subscription lives in `<FilterEditorBody>`.** A `useEffect` calls `listen("ui.focus.filter", …)` and calls `innerRef.current?.focus()` when the payload's `perspective_id` matches the editor's own id. Siblings ignore the event.
- **`isActive` highlight wired via per-command switch.** `isCommandActiveForPerspective` in `perspective-tab-bar.tsx` returns `Boolean(perspective.filter)` for `perspective.filter.focus`. Future group/sort migrations extend the switch as they land.
- **Spatial-nav moniker changed.** Legacy `perspective_tab.filter:{id}` is gone; new moniker is `perspective_tab.perspective.filter.focus:{id}` (built by `<CommandButton>` from `${surface}.${command.id}:${surfaceId}`).
- **Registry rendering gated on `isActive`.** `<RegistryTabButtons>` only mounts on the active perspective's tab — matches the legacy `<FilterFocusButton>`'s visual placement. Future migrations may unlatch this if needed (e.g. for tabs that always need a control).

### Files modified

- `swissarmyhammer-kanban/builtin/commands/perspective.yaml` — added `perspective.filter.focus`.
- `swissarmyhammer-kanban/src/commands/perspective_commands.rs` — added `FocusFilterCmd` + 3 tests.
- `swissarmyhammer-kanban/src/commands/mod.rs` — registered command, bumped count 61 → 62.
- `swissarmyhammer-kanban/tests/builtin_commands.rs` — bumped expected ids count 28 → 29, total 70 → 71.
- `swissarmyhammer-kanban/tests/composed_commands_registry.rs` — bumped snapshot/count 70 → 71.
- `kanban-app/src/commands.rs` — added `handle_focus_filter` post-execute side-effect.
- `kanban-app/ui/src/components/filter-editor.tsx` — added `ui.focus.filter` Tauri listener.
- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — deleted `<FilterFocusButton>` and `onFilterFocus` plumbing; passed perspective into `<RegistryTabButtons>` and wired `isActive` via `isCommandActiveForPerspective`.
- `kanban-app/ui/src/components/perspective-tab-bar.filter-migration.test.tsx` — NEW; 5 regression tests.
- `kanban-app/ui/src/components/perspective-tab-bar.filter-enter.spatial.test.tsx` — rewrote to assert Enter dispatches `perspective.filter.focus` on the new moniker.
- `kanban-app/ui/src/components/perspective-tab-bar.test.tsx` — removed 3 tests that exercised the deleted local-callback path (covered by the migration test file).
- `kanban-app/ui/src/components/perspective-tab-bar.registry-driven.test.tsx` — updated `renders_zero_command_buttons_…` to reflect that the hardcoded "Filter" button is gone.

## Acceptance Criteria

- [x] `perspective.filter.focus` exists in the YAML with `tab_button: { icon: "filter" }`, `scope: "entity:perspective"`, and no `view_kinds`.
- [x] Dispatching the command focuses the perspective's filter editor — verified by `filter_editor_focuses_on_ui_focus_filter_event_for_matching_perspective` in `perspective-tab-bar.filter-migration.test.tsx`.
- [x] `<FilterFocusButton>` is deleted from `perspective-tab-bar.tsx`.
- [x] The registry-rendered Filter button appears in the same position as before with the same icon and the same `isActive` highlight when a filter is set.
- [x] Existing right-click and palette assertions for `perspective.filter` and `perspective.clearFilter` continue to pass (verified by full test suite).
- [x] `cargo test -p swissarmyhammer-kanban -p kanban-app` and `pnpm -C kanban-app/ui test perspective-tab-bar filter` both pass.

## Tests

- [x] Unit tests in `swissarmyhammer-kanban` (FocusFilterCmd tests in `perspective_commands::tests`):
  - `focus_filter_command_dispatches_focus_event` — scope-resolves `perspective_id` and returns the `FocusFilter` marker.
  - `focus_filter_command_prefers_explicit_perspective_id_arg` — arg wins over scope.
  - `focus_filter_command_is_always_available`.
- [x] Frontend regression tests `kanban-app/ui/src/components/perspective-tab-bar.filter-migration.test.tsx`:
  - `filter_command_button_dispatches_perspective_filter_focus_on_click`.
  - `filter_button_is_active_when_perspective_has_a_filter`.
  - `filter_button_is_inactive_when_perspective_filter_is_undefined`.
  - `filter_button_uses_perspective_filter_focus_moniker` (spatial moniker assertion).
  - `filter_editor_focuses_on_ui_focus_filter_event_for_matching_perspective` (subscription + id-match guard).
- [x] Updated/removed legacy `<FilterFocusButton>` tests (3 in `perspective-tab-bar.test.tsx`, 1 in `filter-enter.spatial.test.tsx` rewritten, 1 in `registry-driven.test.tsx` updated).
- [x] `cargo test -p swissarmyhammer-kanban` and frontend `vitest run perspective-tab-bar` both green.

## Workflow

- Used `/tdd` — Rust test (`focus_filter_command_dispatches_focus_event`) and frontend tests (`filter_command_button_dispatches_perspective_filter_focus_on_click` + 4 more) drove the implementation.
- Matched the existing "broadcast a UI event from a backend command" pattern (`DragStart`/`CreateWindow` markers → dispatcher emits Tauri event); did not invent a new event bus.
- Deleted `<FilterFocusButton>` as the final step after the new command + button were both green. #command-driven-ui

## Review Findings (2026-05-13 07:40)

Architecture / functionality / tests are clean. The marker envelope is load-bearing (domain crate cannot reach `AppHandle`); `handle_focus_filter` matches `handle_drag_start` exactly; `isCommandActiveForPerspective` is the right shape for Group/Sort to extend; the moniker change is coherent and the legacy moniker is genuinely gone; the registry-driven empty-list test's assertion is strengthened (not weakened); the 5 new migration tests each pin one specific contract. Two stale-doc nits below — neither is correctness or design, just leftover terminology in docstrings the migration didn't directly touch.

### Nits
- [x] `kanban-app/ui/src/components/perspective-tab-bar.tsx:591-594` — docstring for `ScopedPerspectiveTab` (in the `PerspectiveTabFocusable` JSDoc block above) still says "The filter icon and group icon to the right of the name remain as Pressable leaves (`perspective_tab.filter:{id}`, `perspective_tab.group:{id}`)". After this migration the filter leaf is `perspective_tab.perspective.filter.focus:{id}`, not `perspective_tab.filter:{id}`. The companion docstring at line 680-689 was updated correctly — this older sibling was overlooked. Suggested fix: replace `perspective_tab.filter:{id}` with `perspective_tab.perspective.filter.focus:{id}` in that paragraph and note it is now a `<CommandButton>` leaf rather than a Pressable. **Fixed (2026-05-13): updated the older sibling docstring to describe the filter affordance as a `<CommandButton>` leaf with moniker `perspective_tab.perspective.filter.focus:{id}`, matching the companion paragraph.**
- [x] `kanban-app/ui/src/components/perspective-tab-bar.test.tsx:695, 723` — doc comments in the "rect regression — first paint" describe section still describe the active tab as widened by inline `<FilterFocusButton>` + `<GroupPopoverButton>` chrome. After this migration `<FilterFocusButton>` is gone; the active-tab chrome is `<GroupPopoverButton>` + the registry-rendered `<CommandButton>` from `<RegistryTabButtons>`. Suggested fix: update both occurrences to say "`<GroupPopoverButton>` + registry-rendered `<CommandButton>` chrome" (the test itself is untouched — it asserts non-zero rects regardless of which buttons widen the tab). **Fixed (2026-05-13): both doc comments now read "`<GroupPopoverButton>` + registry-rendered `<CommandButton>` chrome". Test body untouched.**

## Review Findings (2026-05-13 07:42)

Both previously flagged nits are confirmed fixed and align with each other — `ScopedPerspectiveTab`'s older JSDoc paragraph and the `PerspectiveTabFocusable` companion docstring now describe the same moniker shape (`perspective_tab.perspective.filter.focus:{id}`), and the test's rect-regression doc comments now name `<GroupPopoverButton>` + registry-rendered `<CommandButton>` chrome accurately. Grep for the legacy `perspective_tab.filter:` (no `.focus`) shows only legitimate residue: the deletion-assertion in `perspective-tab-bar.filter-migration.test.tsx`, the historical "legacy moniker is gone" comment in `perspective-tab-bar.tsx:692`, and the rewritten `filter-enter.spatial.test.tsx`. One genuinely new stale-doc miss surfaced — see below. Same class as the previous round, in a file the iteration-1 review didn't grep.

### Nits
- [x] `kanban-app/ui/src/components/perspective-spatial-nav.guards.node.test.ts:13` — header docstring describing files-under-guard still lists the filter leaf as `perspective_tab.filter:{id}` ("when rendered, the `perspective_tab.filter:{id}` / `perspective_tab.group:{id}` icon button"). After this migration the filter affordance registers as `perspective_tab.perspective.filter.focus:{id}` (the group leaf is unchanged). The test body does not assert on this token — only the JSDoc overview drifted. Suggested fix: replace `perspective_tab.filter:{id}` with `perspective_tab.perspective.filter.focus:{id}` in that sentence and optionally note it is a `<CommandButton>` leaf rather than a Pressable, mirroring the wording already used in `perspective-tab-bar.tsx`. **Fixed (2026-05-13): header docstring now names the filter leaf as `perspective_tab.perspective.filter.focus:{id}` `<CommandButton>` leaf (built from `${surface}.${command.id}:${surfaceId}`); the group leaf is preserved as the `perspective_tab.group:{id}` Pressable. Mirrors the wording in `perspective-tab-bar.tsx`.**