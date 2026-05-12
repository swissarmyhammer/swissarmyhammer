---
assignees:
- claude-code
depends_on:
- 01KRE1WT72MJWNGQBVAD4V5VKM
- 01KRE1SSN9AX8R67XC58HHQKKB
- 01KRE7VDF7RXHV39VPEVH23NN4
position_column: todo
position_ordinal: a880
title: Migrate Add Perspective and Sort tab buttons to command-driven rendering
---
## What

Final migration in the epic. Two affordances move from hardcoded UI to registry-rendered `<CommandButton>`s with `<CommandPopover>` pickers:

1. **Add Perspective** (`+` button): annotates the existing `perspective.save` command with `tab_button: { icon: "plus" }`. The picker has one text param (`name`); the existing implicit "Untitled" name behavior moves into the command's default arg if the user just clicks Save without typing.
2. **Sort** (tab affordance on grid views): annotates `perspective.sort.set` with `tab_button: { icon: "arrow-up-down" }`. The picker has two enum params (`field` from `perspective.fields`, `direction` from `sort.directions`). Because `perspective.sort.set` already carries `view_kinds: [grid]`, the button is automatically hidden on board views — the original bug, finally fixed by the same mechanism that handles every other view-restricted button.

This task closes out the epic: the tab bar has zero hardcoded button JSX after it lands.

### Post-refactor crate homes

This task depends on `01KRE7VDF7RXHV39VPEVH23NN4` (relocate DynamicSources and friends). After that refactor, file paths in this task that reference `swissarmyhammer-kanban/...` refer to the post-refactor homes:

- `perspective.save` and `perspective.sort.set` `execute` impls live in `swissarmyhammer-perspectives`.
- `commands_for_scope` and the emission infrastructure live in `swissarmyhammer-commands`.
- `PerspectiveFieldsResolver` and `SortDirectionsResolver` are registered from `swissarmyhammer-perspectives` and `swissarmyhammer-commands` respectively.
- The YAML stays in `swissarmyhammer-kanban/builtin/commands/perspective.yaml`.

### Files to modify

- `swissarmyhammer-kanban/builtin/commands/perspective.yaml`:
  - Find the existing `perspective.save` entry (it carries the new-perspective name as a `name` param today). Annotate:
    ```yaml
    tab_button:
      icon: plus
    params:
      - name: name
        from: picker
        shape: text
      - name: view_id
        from: scope
      # ... existing perspective_id, view fields stay as they are
    ```
    Keep all existing params; the change adds `tab_button` + sets the `name` param's `shape: text` and source `from: picker`.
  - Find `perspective.sort.set` and annotate:
    ```yaml
    tab_button:
      icon: arrow-up-down
    params:
      - name: field
        from: picker
        shape: enum
        options_from: "perspective.fields"
      - name: direction
        from: picker
        shape: enum
        options_from: "sort.directions"
      - name: perspective_id
        from: scope
    ```
    `view_kinds: [grid]` already present from the prior epic — leaves Sort hidden on board.

- `kanban-app/ui/src/components/perspective-tab-bar.tsx`:
  - Delete `<AddPerspectiveButton>` and its `import { Plus } from "lucide-react"` if unused after deletion.
  - The registry path renders both `perspective.save` (Add) and `perspective.sort.set` (Sort, grid only) automatically.
  - No state migration needed — `<CommandPopover>` owns the form state.

- `kanban-app/ui/src/components/command-icon-registry.ts` — add `"arrow-up-down": <ArrowUpDown>` from lucide-react. `plus`, `filter`, `group` should already be there from earlier tasks.

- `kanban-app/ui/src/components/perspective-context.tsx` (or wherever `useAutoCreateDefaultPerspective` lives — flagged in the prior epic as a deferred item): leave it. The auto-create path doesn't touch `<AddPerspectiveButton>` and is independent.

### Behavior

- `+` button on the tab bar visually identical. Clicking opens a small popover with a `name` text input; submit dispatches `perspective.save` with the typed name AND `view_id` from scope (preserving the per-view-id scoping the prior epic introduced). Empty input falls back to the dispatcher's "Untitled" default.
- A new Sort button appears on grid views, hidden on board views (verified by the existing `view_kinds: [grid]` filter). Clicking opens a popover with two dropdowns — field + direction — populated from the backend resolvers. Submit dispatches `perspective.sort.set` with the picked args. This is the affordance the original perspective-sort bug was asking for, now properly registry-driven.

### Out of scope

- The `useAutoCreateDefaultPerspective` auto-create path stays as-is. It's a startup-time helper, not a tab-button affordance.
- Any column-header sort-toggle UI in the grid view (`data-table.tsx`'s `SortingState`) — that's tanstack-table's own column-click sort affordance and is separate from the tab-button surface.
- Polishing the popover styling beyond what `<CommandPopover>` already does.

## Acceptance Criteria

- [ ] `perspective.save` YAML carries `tab_button: { icon: "plus" }` and `params.name.shape: text`.
- [ ] `perspective.sort.set` YAML carries `tab_button: { icon: "arrow-up-down" }` and both enum params with their `options_from` keys.
- [ ] `<AddPerspectiveButton>` is deleted; the `+` affordance is the registry-rendered `<CommandButton>`.
- [ ] Submitting the Add popover dispatches `perspective.save` with `name` from the input and `view_id` from scope.
- [ ] On a grid view, a Sort `<CommandButton>` appears in the tab bar. On a board view, it does NOT appear — verified by a regression test.
- [ ] Submitting the Sort popover dispatches `perspective.sort.set` with the picked `field` + `direction` + scope-resolved `perspective_id`.
- [ ] `perspective-tab-bar.tsx` no longer contains any hardcoded affordance JSX (`<FilterFocusButton>`, `<GroupPopoverButton>`, `<AddPerspectiveButton>` are all deleted; the tab bar's button surface is 100% registry-rendered).
- [ ] `cargo test --workspace` and `pnpm -C kanban-app/ui test perspective-tab-bar` both pass.

## Tests

- [ ] Frontend regression `kanban-app/ui/src/components/perspective-tab-bar.add-and-sort-migration.test.tsx`:
  - `add_perspective_button_renders_with_plus_icon_from_registry`.
  - `submitting_add_popover_dispatches_perspective_save_with_name_and_view_id`.
  - `sort_button_appears_on_grid_view_and_disappears_on_board_view` — fixture with active view kind toggled grid/board; assert the Sort `<CommandButton>` mounts/unmounts. This is the regression test the user's original bug report needed and didn't get.
  - `submitting_sort_popover_dispatches_perspective_sort_set_with_field_and_direction`.
  - `tab_bar_has_no_hardcoded_button_jsx` — search the rendered DOM for any of `<FilterFocusButton>`, `<GroupPopoverButton>`, `<AddPerspectiveButton>` (by data-test attribute or by their unique classnames) — assert absent.
- [ ] Backend integration test (in whichever crate owns `commands_for_scope` post-refactor): `perspective_sort_set_command_carries_field_and_direction_options` — emit through `commands_for_scope` with a perspective + a grid view in scope, assert `field.options.len() > 0` (matches perspective fields) AND `direction.options == [{asc, Ascending}, {desc, Descending}]`.
- [ ] Update / delete `add-perspective-button.test.tsx` and any `perspective-tab-bar.test.tsx` cases that asserted on the deleted hardcoded components — they're replaced by the new test file.
- [ ] Run: `cargo test --workspace` and `pnpm -C kanban-app/ui test perspective-tab-bar` — both green.

## Workflow

- Use `/tdd` — start with `sort_button_appears_on_grid_view_and_disappears_on_board_view` (the original user-visible bug) and `tab_bar_has_no_hardcoded_button_jsx`. Let them fail. Then annotate YAML and delete the hardcoded JSX.
- This task lands the user-visible payoff for the entire epic — the sort affordance that should have appeared from the prior `view_kinds` task. Verify it manually before marking done: launch the app, switch between grid and board views, confirm Sort appears/disappears.
- After this task lands, the perspective tab bar is fully registry-driven. Document that in the implementation notes so future contributors know not to add hardcoded buttons here. #command-driven-ui