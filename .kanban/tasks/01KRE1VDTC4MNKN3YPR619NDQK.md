---
assignees:
- claude-code
depends_on:
- 01KRE1R8AWHXM385ZFJJKXW2XB
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffd780
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

- [x] `<CommandButton>` renders an icon-only button using `command.tab_button.icon` looked up in `command-icon-registry.ts`.
- [x] Activating a `<CommandButton>` for a command whose every param is `from: args | scope` dispatches the command immediately with no popover.
- [x] Activating a `<CommandButton>` for a command with one or more `shape`-bearing params opens a `<CommandPopover>` mounted on the same anchor.
- [x] `<CommandPopover>` renders one form field per `shape`-bearing param, choosing the input type from `shape`.
- [x] Submitting `<CommandPopover>` calls `onCommit({...args})` with every picked param value; subsequent dispatch by `<CommandButton>` carries those args.
- [x] Spatial-nav moniker for the button is deterministic from `command.id` and a parent-supplied surface id; spatial-nav tests cover the moniker pattern.
- [x] An unknown icon name renders a fallback without crashing.
- [x] `pnpm -C kanban-app/ui test command-button command-popover` passes. (Project uses npm — verified with `npx vitest run command-button command-popover`: 3 files, 17 tests, all green. Full suite `npm test`: 225 files, 2133 tests, all green.)

## Tests

- [x] `kanban-app/ui/src/components/command-button.test.tsx` (new):
  - `renders_icon_from_tab_button_metadata` — `<CommandButton command={{ tab_button: { icon: "filter" }, ... }}>` includes a Filter icon.
  - `dispatches_immediately_when_no_pickable_params` — mount a command with `params: [{ name: "perspective_id", from: "scope" }]`, click the button, assert dispatcher received the command id + scope-resolved args, no popover opened.
  - `opens_popover_when_command_has_pickable_param` — mount a command with `params: [{ name: "field", shape: "enum", options: [{value:"a",label:"A"}] }]`, click the button, assert the popover opens.
  - `renders_fallback_icon_for_unknown_name` — `tab_button.icon = "no-such-icon"` renders `<HelpCircle>` (or similar) instead of crashing.
- [x] `kanban-app/ui/src/components/command-popover.test.tsx` (new):
  - `renders_select_for_enum_param_with_options` — popover with one enum param and two options shows a select with both options.
  - `renders_text_input_for_text_param`.
  - `renders_filter_editor_for_expression_param` — assert a CodeMirror editor instance mounts.
  - `commits_picked_values_via_oncommit` — fill the form, click submit, assert `onCommit` is called with `{ field: "a" }`.
  - `enum_param_with_empty_options_disables_the_field` — `options: []` renders the select as disabled.
- [x] Spatial-nav test: `command-button.spatial.test.tsx` — render two `<CommandButton>`s with different command ids in a focus root; assert their monikers are distinct and derived from `command.id`.
- [x] Run: `pnpm -C kanban-app/ui test command-button command-popover` — green.

## Implementation notes

- The pure expression editor was added as a sibling `<FilterExpressionEditor>` export inside `kanban-app/ui/src/components/filter-editor.tsx` (rather than renaming `<FilterEditor>`). Keeping the existing dispatch-coupled `<FilterEditor>` untouched preserves the formula-bar behavior contract pinned by 67 existing filter-editor tests while exposing a reusable, dispatch-agnostic CM6 + filter-DSL editor for the popover. Both share the `useFilterEditorExtensions` hook so mention autocomplete and submit/cancel keymaps stay consistent across surfaces.
- Project uses npm (not pnpm) — confirmed via `package.json` and the `preinstall: "npx only-allow npm"` hook. Test command run as `npx vitest run command-button command-popover`.

## Workflow

- Use `/tdd` — write the component tests first using a `MockDispatcher` (look at existing `dispatch-mock` patterns in the test directory), then implement.
- Match the existing tab-button visual style (look at `FilterFocusButton` for the highlight pattern, padding, focus ring) so the migrated UI looks identical.
- Follow the existing Pressable / spatial-nav moniker pattern from `<FilterFocusButton>` and `<GroupPopoverButton>` — leaf monikers `perspective_tab.${id}:${perspectiveId}`. The surface (perspective_tab vs other future surfaces) is a prop on `<CommandButton>`. #command-driven-ui

## Review Findings (2026-05-12 09:40)

### Warnings
- [x] `kanban-app/ui/src/components/command-popover.test.tsx` — Multi-param happy path is not directly tested. `commits_picked_values_via_oncommit` covers only a single enum param. The codebase's intended sort command (`field` + `direction`) and any future two-shape command would exercise a path the test suite doesn't pin. Add a test that mounts a popover with two `shape`-bearing params, sets each, submits, and asserts `onCommit` receives both keys. The implementation already handles this via `setValues((prev) => ({ ...prev, [p.name]: v }))` — the test would just lock the contract. **Fixed:** added `commits_picked_values_for_multi_param_command` — mirrors the eventual `perspective.sort.set` shape (`field` + `direction` enums), picks both, asserts `onCommit` receives `{ field, direction }`.
- [x] `kanban-app/ui/src/components/command-popover.tsx:53-67,174-187` — `initialValueFor` returns `""` for enum-shaped params without a `default`, and `EnumField` renders a `<option value="">Pick…</option>` placeholder. Clicking Submit without picking dispatches `{ field: "" }`, which the backend will reject but the form will not. Consider one of: (a) disable the Submit button when any required enum slot is `""`, or (b) require the user to actively confirm before the form considers itself ready. The implementation today relies on backend validation as the only gate, which is functional but produces a worse UX than a frontend-side disable. Not a blocker because dispatch failures surface to console and migrations can override per-command. **Fixed:** Option (a). `submitDisabled` is `useMemo`-derived from `pickableParams` + `values`; any enum slot still empty disables the button (visual `opacity-50` + `disabled` attribute) and short-circuits `handleSubmit`. New test `submit_disabled_until_required_enum_param_is_picked` locks the contract — initial render disables, picking re-enables, clicking disabled is a no-op.

### Nits
- [x] `kanban-app/ui/src/components/command-button.tsx:60,130` — `surfaceId` is typed as `string` (required) but an empty string passes the type check and produces the moniker `${surface}.${command.id}:`. Two callers that both pass `""` collide silently. Migrations are out of scope here so no caller actually does this yet, but a defensive runtime check (`if (!surfaceId) throw …`) or a branded non-empty-string type would convert a future bug into an immediate crash. Worth waving off if the team decides migrations will always pass a real id. **Fixed:** Added runtime guards at the top of `CommandButton` — both `surfaceId` and `surface` must be non-empty strings; empty values throw with a message that names the offending command id. New test `throws_when_surfaceId_is_empty` pins it.
- [x] `kanban-app/ui/src/components/command-button.test.tsx` — `isActive` prop has no test that mounts with `isActive: true` and asserts the rendered icon's `fill`/`className` reflects the active state. The prop is wired (`text-primary` and `fill="currentColor"`), and a one-line assertion would lock the contract that migrations rely on. Low value but easy to add. **Fixed:** Added two tests — `isActive_highlights_icon` asserts `text-primary` on the button and `fill="currentColor"` on the SVG when `isActive=true`; `isActive_false_does_not_highlight` asserts the negative contract (no `text-primary`, `fill="none"`).
- [x] `kanban-app/ui/src/components/command-popover.tsx:144-155` — The `expression` branch wraps `<FilterExpressionEditor>` in `<div aria-label={param.name}>` for label association; the other branches put `aria-label` on the input itself. The form's `<label>` wrapper (line 236) already supplies a label via `<span>{p.name}</span>`. Two layers of labeling is harmless but reads slightly inconsistent — either drop the per-field `aria-label` and rely on the wrapping `<label>`, or document why the expression branch needs the extra wrapper (CM6's editor div isn't a native form control, so `<label>` alone may not announce correctly to AT). Worth a brief comment in the code if the wrapper is intentional. **Fixed:** Documented inline. The wrapper is intentional — CM6 mounts a contenteditable `<div>` rather than a native form control, so the wrapping `<label>` text doesn't reach AT via normal control association. The explicit `aria-label` on the wrapper supplies an accessible name to AT clients that walk the CM6 editor's `role=textbox` child. Comment added in the `case "expression":` branch.