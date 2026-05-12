---
assignees:
- claude-code
depends_on:
- 01KRE1WT72MJWNGQBVAD4V5VKM
- 01KRE7VDF7RXHV39VPEVH23NN4
position_column: todo
position_ordinal: a680
title: Migrate Filter tab button to command-driven rendering
---
## What

Replace the hardcoded `<FilterFocusButton>` with a registry-rendered `<CommandButton>` driven by a new no-arg `perspective.filter.focus` command. The command's only job is to broadcast the "focus the formula bar's filter editor" signal — the editor itself stays on the tab bar where it already lives.

This is the first command migration and exercises the full pipeline (param-shape metadata → tab-bar registry rendering) end to end for a NO-arg case. Group and Sort migrations later exercise the picker cases.

### Post-refactor crate homes

This task depends on `01KRE7VDF7RXHV39VPEVH23NN4` (relocate DynamicSources and friends), so the Rust file paths below reference the **post-refactor** homes:

- The perspective command's `execute` impl lives in `swissarmyhammer-perspectives` (not in `swissarmyhammer-kanban/src/commands/perspective_commands.rs` as it does today — that file is being decomposed by the refactor task).
- The YAML stays in `swissarmyhammer-kanban/builtin/commands/perspective.yaml` for now (kanban-app still loads it; the YAML home is a separate question outside this epic).
- `commands_for_scope` and the surrounding emission infrastructure live in `swissarmyhammer-commands` after the refactor; this task doesn't touch them.

### Files to modify

- `swissarmyhammer-kanban/builtin/commands/perspective.yaml` — add a new entry:
  ```yaml
  - id: perspective.filter.focus
    name: Focus Filter
    scope: "entity:perspective"
    tab_button:
      icon: filter
    params:
      - name: perspective_id
        from: scope
    keys: {}
  ```
  Notes:
  - No `view_kinds` — filter is meaningful on every view kind.
  - No `context_menu: true` — the right-click already has Clear Filter; "Focus the filter editor" doesn't add value there.
  - The existing `perspective.filter` (Set Filter, palette-only with a `filter` arg) stays unchanged; this new command is a focus shortcut, not a replacement.

- `swissarmyhammer-perspectives/src/commands/focus.rs` (new, post-refactor home) — implement `FocusFilterCmd`:
  - Resolves `perspective_id` from scope.
  - Dispatches a UI broadcast (same channel `<FilterFocusButton>` currently uses internally) — likely a Tauri event named `ui.focus.filter` carrying the perspective id.
  - No state mutation, no undo entry, no log line beyond debug.

  Pre-refactor fallback: if the relocation in `01KRE7VDF7RXHV39VPEVH23NN4` has not landed yet when this task is picked up, the impl goes in `swissarmyhammer-kanban/src/commands/perspective_commands.rs` and the relocation task will move it. Surface this in the implementation notes so the refactor knows about it.

- `kanban-app/ui/src/components/perspective-tab-bar.tsx`:
  - Remove `<FilterFocusButton>` and its `import { Filter } from "lucide-react"` if no other line uses it.
  - The registry-driven rendering path (from the prerequisite tab-bar task) now picks up `perspective.filter.focus` automatically and renders it as a `<CommandButton>`.
  - Keep the active-state highlight: pass `isActive={Boolean(perspective.filter)}` to the `<CommandButton>`. (The migration task for `<CommandButton>` left `isActive` as an opt-in prop; here we wire it.)
  - Update the spatial-nav moniker if it changes (the new pattern from `<CommandButton>` should produce `perspective_tab.perspective.filter.focus:${perspectiveId}` — slightly different from today's `perspective_tab.filter:${perspectiveId}`; reflect that in the affected spatial tests).

- `kanban-app/ui/src/components/filter-editor.tsx` (or wherever the focus signal is broadcast today) — replace the local broadcast with subscription to the `ui.focus.filter` Tauri event. The command's dispatch path now triggers the same focus behavior the button previously triggered locally.

### Behavior

- Filter button on the tab bar visually identical to today.
- Clicking it dispatches `perspective.filter.focus` → focus moves to the CM6 editor. Same user-visible behavior as today, now via the dispatcher.
- The button responds to `view_kinds` filtering — though `perspective.filter.focus` has no `view_kinds`, so it stays visible on every view kind.

### Out of scope

- Migrating Group, Sort, Add Perspective — separate tasks.
- Adding the filter command to the palette / right-click (it doesn't belong there; focus shortcuts are tab-button-only).

## Acceptance Criteria

- [ ] `perspective.filter.focus` exists in the YAML with `tab_button: { icon: "filter" }`, `scope: "entity:perspective"`, and no `view_kinds`.
- [ ] Dispatching the command focuses the perspective's filter editor — verified by a regression test that simulates the dispatch and asserts the CM6 editor gains focus.
- [ ] `<FilterFocusButton>` is deleted from `perspective-tab-bar.tsx`.
- [ ] The registry-rendered Filter button appears in the same position as before with the same icon and the same `isActive` highlight when a filter is set.
- [ ] Existing right-click and palette assertions for `perspective.filter` and `perspective.clearFilter` continue to pass — this task adds a new command, doesn't change the existing ones.
- [ ] `cargo test --workspace` and `pnpm -C kanban-app/ui test perspective-tab-bar filter` both pass.

## Tests

- [ ] Unit test in `swissarmyhammer-perspectives` (mod tests for the new `focus` module): `focus_filter_command_dispatches_focus_event` — execute `FocusFilterCmd` with a scope-resolved `perspective_id`, assert the registered event channel receives `ui.focus.filter` with that id.
- [ ] Frontend regression test `kanban-app/ui/src/components/perspective-tab-bar.filter-migration.test.tsx`:
  - `filter_command_button_dispatches_perspective_filter_focus_on_click` — mount the tab bar, click the registry-rendered Filter `<CommandButton>`, assert dispatcher receives `perspective.filter.focus` with the correct `perspective_id`.
  - `filter_button_is_active_when_perspective_has_a_filter` — mount with `perspective.filter = "#bug"`, assert the `<CommandButton>` carries the highlighted state.
  - `filter_button_is_inactive_when_perspective_filter_is_undefined`.
  - `filter_button_uses_perspective_filter_focus_moniker` — spatial assertion on the new moniker shape.
- [ ] Update or remove any existing `<FilterFocusButton>` test that asserts on the deleted component path.
- [ ] Run: `cargo test --workspace` and `pnpm -C kanban-app/ui test perspective-tab-bar` — both green.

## Workflow

- Use `/tdd` — start with the focus-event dispatch test for `FocusFilterCmd` and the click-dispatches-command test on the tab bar; let those fail, then implement.
- The "broadcast a UI event from a backend command" pattern probably already exists — search for `Tauri.*emit` or look at how `perspective.set` notifies the UI of the active perspective change. Match that pattern, do NOT invent a new event bus.
- Delete `<FilterFocusButton>` as the final step, after the new command + button are both green. Don't leave a dangling component. #command-driven-ui