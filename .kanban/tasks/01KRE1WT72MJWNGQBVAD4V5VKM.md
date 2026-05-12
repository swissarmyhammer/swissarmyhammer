---
assignees:
- claude-code
depends_on:
- 01KRE1VDTC4MNKN3YPR619NDQK
- 01KRE1SSN9AX8R67XC58HHQKKB
position_column: todo
position_ordinal: a580
title: Render perspective tab bar from command registry
---
## What

Integration point. `<PerspectiveTabBar>` stops hardcoding which buttons it shows — instead it queries the live command registry for every command whose `tab_button` is set AND whose scope chain matches the active perspective (including `view_kinds` filtering, which now finally does its job).

This task lands the registry-driven rendering path WITHOUT removing any hardcoded buttons yet. The existing `<FilterFocusButton>`, `<GroupPopoverButton>`, `<AddPerspectiveButton>` stay in place; the new render path produces zero buttons until commands are annotated (handled by individual migration tasks). Once a command flips on `tab_button`, the new path renders it; the per-command migration then removes the corresponding hardcoded button in a controlled handoff.

### Files to modify

- `kanban-app/ui/src/components/perspective-tab-bar.tsx`:
  - Add a `useScopedTabCommands(perspectiveId, activeView)` hook that:
    - Calls the existing `commands_for_scope` bridge (the same dispatch path the palette already uses) with a scope chain `["perspective:${perspectiveId}", "view:${activeView.id}", "board:${activeBoardId}"]` — same shape the palette uses for this surface.
    - Filters the returned list to commands with `tab_button != null`.
    - Returns the list, with backend-supplied `options` on each enum param already populated.
  - In the per-tab render, map `tabCommands` to `<CommandButton command={cmd} surface={"perspective_tab"} perspectiveId={p.id} />` rendered alongside the existing hardcoded buttons. Position the new buttons next to the hardcoded ones; the order will collapse once migrations delete the hardcoded counterparts.

- The bridge that reaches the Rust `commands_for_scope` from the UI — likely `kanban-app/ui/src/lib/commands.ts` or similar — must already expose this. If the existing palette wiring uses it, the hook just reuses that path. If not, this task includes a thin Tauri-invoke wrapper.

### Behavior

- Today, the tab bar has 3 hardcoded buttons (Filter, Group, Add) per perspective.
- After this task, it has 3 hardcoded buttons + 0 registry-rendered buttons (no commands carry `tab_button` yet) — net visual change: none.
- After Filter is migrated, it has 2 hardcoded buttons + 1 registry-rendered button (Filter). After all three are migrated, it has 0 hardcoded buttons + 3 registry-rendered buttons.
- During the transition, both render paths coexist deliberately. The migration tasks own the handoff per command.

### Out of scope

- Annotating individual commands with `tab_button` — separate per-command tasks.
- Deleting the hardcoded buttons — also per-command tasks (the deletion is the final step of each migration).

## Acceptance Criteria

- [ ] `useScopedTabCommands` hook queries `commands_for_scope` with a scope chain that contains `perspective:`, `view:`, and `board:` monikers and returns only commands whose `tab_button != null`.
- [ ] `<PerspectiveTabBar>` renders one `<CommandButton>` per item in `tabCommands`, alongside the existing hardcoded buttons.
- [ ] When zero commands have `tab_button` set (current state of the YAMLs at task time), zero `<CommandButton>`s render and the tab bar is visually identical to today.
- [ ] When a synthetic test fixture sets `tab_button` on a command, the corresponding `<CommandButton>` appears in the tab bar at runtime.
- [ ] `pnpm -C kanban-app/ui test perspective-tab-bar` passes including a new fixture-driven case.

## Tests

- [ ] `kanban-app/ui/src/components/perspective-tab-bar.registry-driven.test.tsx` (new):
  - `renders_command_button_for_each_tab_button_tagged_command` — mount with a fixture `commands_for_scope` that returns one command with `tab_button: { icon: "filter" }`, assert one `<CommandButton>` renders.
  - `renders_zero_command_buttons_when_no_commands_have_tab_button` — assert the registry-rendered slot is empty when no command in scope carries `tab_button`.
  - `respects_view_kinds_filter` — fixture returns a command with `view_kinds: [grid]` and `tab_button`, mount with active view kind `board`, assert the button does NOT render (uses the existing `filter_by_view_kind` pass).
- [ ] Update `perspective-tab-bar.test.tsx` to NOT regress on the existing hardcoded buttons — they stay until their migrations land.
- [ ] Run: `pnpm -C kanban-app/ui test perspective-tab-bar` — green.

## Workflow

- Use `/tdd` — write the three new test cases first against a mocked `commands_for_scope`, then wire the hook + rendering.
- Look at how the command palette consumes `commands_for_scope` today (search `kanban-app/ui/src` for the bridge) — match that pattern. Do NOT invent a parallel query path.
- The new buttons must use the same Pressable / spatial-nav moniker pattern as the existing hardcoded ones so keyboard navigation through the tab bar stays seamless during the transition. #command-driven-ui