---
assignees:
- claude-code
depends_on:
- 01KM32B3GRRPSW685JTMQAMPF3
position_column: todo
position_ordinal: '8480'
title: Wire view switching through command dispatch and palette
---
## What
View switching currently bypasses the command system. `ViewCommandScope` in App.tsx creates local `nav.view.&lt;id&gt;` CommandDefs that directly call `setActiveViewId` — they never go through `dispatch_command` and don't appear in the command palette. Meanwhile `ui.view.set` exists in YAML but is `visible: false`.

**Fix the YAML command (`swissarmyhammer-commands/builtin/commands/ui.yaml`):**
- Change `ui.view.set` from `visible: false` to `visible: true` so it appears in the palette
- Verify the backend `ui.view.set` handler in `swissarmyhammer-commands` calls `UIState::set_active_view`

**Fix the frontend (`kanban-app/ui/src/App.tsx`):**
- `ViewCommandScope` should generate per-view commands that go through `dispatch_command("ui.view.set", { view_id })` instead of directly calling `setActiveViewId`
- Each view command should have palette metadata (name like "View: Board", icon from view def)
- Remove the local-only `nav.view.&lt;id&gt;` pattern

**Fix the left nav (`kanban-app/ui/src/components/left-nav.tsx`):**
- `onClick` currently calls `executeCommand("nav.view.&lt;id&gt;")` — update to use `dispatch_command("ui.view.set", { view_id })` or the equivalent frontend dispatch

**Ensure the round-trip works:**
- Frontend dispatches `ui.view.set` → backend `UIState::set_active_view` → emits event → frontend `ViewsProvider` updates → also persists to `config.windows[label].active_view_id`

## Acceptance Criteria
- [ ] View switching commands appear in the command palette with view names
- [ ] Clicking a view in left-nav goes through the command system
- [ ] `ui.view.set` dispatches correctly and updates the active view
- [ ] View changes persist to per-window config

## Tests
- [ ] Manual: open command palette, type "View" — see all available views listed
- [ ] Manual: select a view from palette — switches correctly
- [ ] Manual: click view in left nav — still works
- [ ] `cargo nextest run -p swissarmyhammer-commands` passes (UIState tests)