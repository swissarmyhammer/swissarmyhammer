---
assignees:
- claude-code
depends_on:
- 01KRE1WT72MJWNGQBVAD4V5VKM
position_column: todo
position_ordinal: a780
title: Migrate Group tab button to command-driven rendering with field picker
---
## What

Replace the hardcoded `<GroupPopoverButton>` + `<GroupSelector>` with a registry-rendered `<CommandButton>` that opens a `<CommandPopover>` containing a single enum-shaped field picker. The picker options come from the backend `PerspectiveFieldsResolver` (registered in the resolver task).

This is the first migration that exercises the picker pipeline end-to-end: enum param → backend-supplied options → frontend dropdown → dispatch with picked value.

### Files to modify

- `swissarmyhammer-kanban/builtin/commands/perspective.yaml` — extend the existing `perspective.group` entry:
  ```yaml
  - id: perspective.group
    name: Group By
    scope: "entity:perspective"
    tab_button:
      icon: group
    undoable: true
    params:
      - name: group
        from: picker
        shape: enum
        options_from: "perspective.fields"
      - name: perspective_id
        from: scope
    keys: {}
  ```
  Changes from today: `name` from "Set Group" to "Group By" (matches user-facing button label); `params[0].from` from `args` to `picker`; new `shape: enum` and `options_from: "perspective.fields"`; new `tab_button` block. The existing palette-only behavior continues to work because the dispatcher still accepts `group` as an arg from any source.

- `kanban-app/ui/src/components/perspective-tab-bar.tsx`:
  - Delete `<GroupPopoverButton>` and the `import { Group } from "lucide-react"` if no other line uses it.
  - Delete the `groupOpen`/`setGroupOpen` state and the `GroupPopoverButton` invocation in the per-tab render.
  - The registry path (from the prerequisite tab-bar task) renders `perspective.group` as a `<CommandButton>` automatically.
  - Pass `isActive={Boolean(perspective.group)}` to the `<CommandButton>` so the highlight matches today.

- `kanban-app/ui/src/components/group-selector.tsx` — leave the underlying field-list component but remove the hardcoded Popover wrapper. The selector becomes a thin "enum dropdown of perspective fields" — or, if `<CommandPopover>`'s generic enum renderer is good enough, the dedicated component can be deleted entirely. Decide based on whether `<GroupSelector>` does anything special beyond rendering a list (e.g. virtualization, search/filter input, field-icon rendering).
  - If `<GroupSelector>` does have unique UX worth preserving, register it as the renderer for `options_from: "perspective.fields"` enum-shaped params inside `<CommandPopover>` (a per-options_from override on the generic enum renderer).
  - If not, delete it and let `<CommandPopover>` handle it generically.

### Behavior

- Group button on the tab bar visually identical to today.
- Clicking opens a popover with the same field list as today (sourced from `PerspectiveFieldsResolver` instead of being computed inline).
- Picking a field dispatches `perspective.group` with `group: <field>` — same dispatcher path as before; `available()` / `execute()` unchanged.
- `isActive` highlight when the perspective has a `group` set, same as today.

### Out of scope

- Migrating Filter, Sort, Add Perspective — separate tasks.
- Adding a Clear Group affordance to the tab bar (`perspective.clearGroup` already exists in the right-click context menu and stays there).

## Acceptance Criteria

- [ ] `perspective.group` YAML carries `tab_button: { icon: "group" }` and `params[0].shape: enum, options_from: "perspective.fields"`.
- [ ] Emitted `perspective.group` from `commands_for_scope` carries the resolved option list when a perspective is in scope; the option list matches the perspective's field set.
- [ ] `<GroupPopoverButton>` is deleted; the tab bar's Group affordance is the registry-rendered `<CommandButton>` + `<CommandPopover>`.
- [ ] Clicking the button → popover → picking a field dispatches `perspective.group` with the picked field and the scope-resolved perspective id.
- [ ] `isActive` highlight matches today's behavior (`Boolean(perspective.group)`).
- [ ] Existing palette/right-click tests for `perspective.group` and `perspective.clearGroup` continue to pass — this task changes the rendering, not the dispatch contract.
- [ ] `cargo test -p swissarmyhammer-kanban` and `pnpm -C kanban-app/ui test perspective-tab-bar group` both pass.

## Tests

- [ ] Frontend regression `kanban-app/ui/src/components/perspective-tab-bar.group-migration.test.tsx`:
  - `group_command_button_renders_with_group_icon` — mount with `perspective.group` carrying `tab_button: { icon: "group" }`, assert the icon is present.
  - `group_popover_renders_field_options_from_command_emission` — fixture `commands_for_scope` returns `perspective.group` with `params[0].options = [{value:"status",label:"Status"},{value:"assignee",label:"Assignee"}]`; click the button, assert the popover has both options as `<select>` entries.
  - `picking_a_group_field_dispatches_perspective_group_with_field_arg` — click button, pick "status", assert dispatcher receives `perspective.group` with `{ group: "status", perspective_id: ... }`.
  - `group_button_is_active_when_perspective_has_a_group_set` — fixture perspective with `group: "status"`, assert the rendered `<CommandButton>` has the highlighted state.
- [ ] Backend integration test in `swissarmyhammer-kanban/tests/options_enrichment.rs`: `perspective_group_command_carries_field_options_when_perspective_in_scope` — emit through `commands_for_scope` with a perspective that has 3 fields, assert the emitted `perspective.group` command's `params[0].options.len() == 3`.
- [ ] Update / remove the existing `group-popover-button.test.tsx` and `perspective-tab-bar.group-enter.spatial.test.tsx` to either reflect the new moniker / component shape OR be replaced by the new test file.
- [ ] Run: `cargo test -p swissarmyhammer-kanban` and `pnpm -C kanban-app/ui test perspective-tab-bar` — both green.

## Workflow

- Use `/tdd` — write the popover-renders-options and picking-dispatches tests first, let them fail, then change the YAML and delete the hardcoded button.
- Decide early whether `<GroupSelector>` survives as a renderer override or gets deleted; the choice depends on whether the existing field-list UI has any UX worth preserving over a plain `<select>`. Document the decision in the implementation notes on the task.
- The spatial moniker shape changes (`perspective_tab.group:` → `perspective_tab.perspective.group:`). Update or replace the affected spatial test. #command-driven-ui