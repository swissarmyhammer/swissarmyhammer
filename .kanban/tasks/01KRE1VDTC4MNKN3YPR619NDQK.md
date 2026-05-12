---
assignees:
- claude-code
depends_on:
- 01KRE1R8AWHXM385ZFJJKXW2XB
position_column: todo
position_ordinal: a480
title: Generic CommandButton and CommandPopover React components
---
## What

Frontend foundation that renders any `tab_button`-tagged command from the registry. One component handles every case — no-arg "click and dispatch," single-param dropdown, multi-param form, expression editor. Param shapes drive the rendering; backend-supplied `options` drive the choices.

### Files to create

- `kanban-app/ui/src/components/command-button.tsx` — `<CommandButton command={CommandDef} />`:
  - Reads `command.tab_button.icon` (lucide-react name string). Looks the name up in a small icon registry; renders a fallback (`<HelpCircle>`) if unknown.
  - Wraps a Radix `<Pressable>` so spatial navigation works; moniker is derived deterministically from `command.id` plus the surface (e.g. `perspective_tab.${command.id}:${perspectiveId}` when rendered in the tab bar — the surface contributes the suffix via a prop).
  - On activation: walks `command.params`. If every param has `from: "args" | "scope"` (no `shape`), dispatch immediately with the resolved args/scope values. Otherwise open `<CommandPopover>`.
  - Visible state: highlighted when the underlying state the command represents is "active" (e.g. a filter is set on the perspective). Today's `<FilterFocusButton>` reads `filter` and `<GroupPopoverButton>` reads `group` to set the highlight; the migration tasks pass the relevant indicator into `<CommandButton>` via a small `isActive?: boolean` prop. Keep this orthogonal to the picker logic.

- `kanban-app/ui/src/components/command-popover.tsx` — `<CommandPopover command={CommandDef} onCommit={(args) => void} />`:
  - Renders a Radix Popover anchored to the button.
  - For each param in `command.params` with a `shape` set:
    - `enum` → `<select>` (or a small custom dropdown if we already have one) populated from `param.options` (backend-supplied) or `param.options_from`-derived (if backend left them empty, render disabled).
    - `text` → `<input type="text">` with a Submit button.
    - `expression` → `<FilterEditor>` (lifted out of `formula-bar.tsx` and exposed for reuse) — CodeMirror editor with filter-DSL grammar.
    - `number` / `date` / `boolean` → corresponding native inputs.
  - On submit, calls `onCommit({ [param.name]: pickedValue, ... })`. The parent (`<CommandButton>`) then dispatches the command with those args plus the resolved scope params.

- `kanban-app/ui/src/components/command-icon-registry.ts` — a small map `{ filter: <Filter>, group: <Group>, "arrow-up-down": <ArrowUpDown>, plus: <Plus>, ... }`. Add lucide imports as commands annotate themselves with new icons. Document that adding a new tab-button command's icon requires adding it to this map.

### Files to refactor (carefully, no behavior change)

- `kanban-app/ui/src/components/filter-editor.tsx` (new, OR extract from wherever the CM6 setup lives today — likely `formula-bar.tsx`) — pure component: `<FilterEditor value={string} onChange={(s) => void} onSubmit={() => void} />`. Move the existing tab-bar inline editor into this component AND let `<CommandPopover>` mount the same component for `shape: expression` params. The inline tab-bar editor stays where it is for direct typing; only the popover-hosted form re-mounts it. Pure relocation, no behavior change.

### Out of scope

- Annotating any specific command with `tab_button` — those are individual migration tasks.
- Removing existing hardcoded buttons (`<FilterFocusButton>`, `<GroupPopoverButton>`, `<AddPerspectiveButton>`) — they stay until the per-command migrations land.
- Reading the command registry into the tab bar — handled by the "tab bar reads from registry" task.

## Acceptance Criteria

- [ ] `<CommandButton>` renders an icon-only button using `command.tab_button.icon` looked up in `command-icon-registry.ts`.
- [ ] Activating a `<CommandButton>` for a command whose every param is `from: args | scope` dispatches the command immediately with no popover.
- [ ] Activating a `<CommandButton>` for a command with one or more `shape`-bearing params opens a `<CommandPopover>` mounted on the same anchor.
- [ ] `<CommandPopover>` renders one form field per `shape`-bearing param, choosing the input type from `shape`.
- [ ] Submitting `<CommandPopover>` calls `onCommit({...args})` with every picked param value; subsequent dispatch by `<CommandButton>` carries those args.
- [ ] Spatial-nav moniker for the button is deterministic from `command.id` and a parent-supplied surface id; spatial-nav tests cover the moniker pattern.
- [ ] An unknown icon name renders a fallback without crashing.
- [ ] `pnpm -C kanban-app/ui test command-button command-popover` passes.

## Tests

- [ ] `kanban-app/ui/src/components/command-button.test.tsx` (new):
  - `renders_icon_from_tab_button_metadata` — `<CommandButton command={{ tab_button: { icon: "filter" }, ... }}>` includes a Filter icon.
  - `dispatches_immediately_when_no_pickable_params` — mount a command with `params: [{ name: "perspective_id", from: "scope" }]`, click the button, assert dispatcher received the command id + scope-resolved args, no popover opened.
  - `opens_popover_when_command_has_pickable_param` — mount a command with `params: [{ name: "field", shape: "enum", options: [{value:"a",label:"A"}] }]`, click the button, assert the popover opens.
  - `renders_fallback_icon_for_unknown_name` — `tab_button.icon = "no-such-icon"` renders `<HelpCircle>` (or similar) instead of crashing.
- [ ] `kanban-app/ui/src/components/command-popover.test.tsx` (new):
  - `renders_select_for_enum_param_with_options` — popover with one enum param and two options shows a select with both options.
  - `renders_text_input_for_text_param`.
  - `renders_filter_editor_for_expression_param` — assert a CodeMirror editor instance mounts.
  - `commits_picked_values_via_oncommit` — fill the form, click submit, assert `onCommit` is called with `{ field: "a" }`.
  - `enum_param_with_empty_options_disables_the_field` — `options: []` renders the select as disabled.
- [ ] Spatial-nav test: `command-button.spatial.test.tsx` — render two `<CommandButton>`s with different command ids in a focus root; assert their monikers are distinct and derived from `command.id`.
- [ ] Run: `pnpm -C kanban-app/ui test command-button command-popover` — green.

## Workflow

- Use `/tdd` — write the component tests first using a `MockDispatcher` (look at existing `dispatch-mock` patterns in the test directory), then implement.
- Match the existing tab-button visual style (look at `FilterFocusButton` for the highlight pattern, padding, focus ring) so the migrated UI looks identical.
- Follow the existing Pressable / spatial-nav moniker pattern from `<FilterFocusButton>` and `<GroupPopoverButton>` — leaf monikers `perspective_tab.${id}:${perspectiveId}`. The surface (perspective_tab vs other future surfaces) is a prop on `<CommandButton>`. #command-driven-ui