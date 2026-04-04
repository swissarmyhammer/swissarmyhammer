---
assignees:
- claude-code
depends_on:
- 01KNC7N6PKWCAXPXXT2CZ4228P
position_column: todo
position_ordinal: '8580'
position_swimlane: container-refactor
title: Extract InspectorContainer from App.tsx
---
## What

Extract the inspector panel management into its own container. Currently, `InspectorPanel`, `InspectorSyncBridge`, panel stack state, backdrop overlay, and close handlers are all inline in App.tsx (lines 75-99, 156-182, 593-618, 740-820).

**Files to create/modify:**
- `kanban-app/ui/src/components/inspector-container.tsx` (NEW) — owns panel stack state, InspectorSyncBridge, backdrop, panel rendering, close handlers
- `kanban-app/ui/src/App.tsx` — remove InspectorSyncBridge, InspectorPanel, panelStack state, backdrop overlay

**Current state:**
- `InspectorSyncBridge` (App.tsx:75-99): Syncs backend UIState inspector_stack to local panelStack
- `panelStack` state + refs (App.tsx:156-158)
- `closeTopPanel` / `closeAll` handlers (App.tsx:166-182)
- Backdrop overlay + panel rendering (App.tsx:593-618)
- `InspectorPanel` component (App.tsx:740-820): Resolves entity, fetches from backend, renders in SlidePanel

**Target:** `InspectorContainer` is a sibling/overlay alongside the main content, not wrapping it. It reads UIState to know what panels to show and renders them as an overlay.

## Acceptance Criteria
- [ ] `InspectorContainer` exists as a standalone component file
- [ ] Panel stack state, sync bridge, and rendering logic moved out of App.tsx
- [ ] Inspector panels still open when clicking Inspect on entities
- [ ] Panels stack correctly with offset
- [ ] Backdrop click closes all panels
- [ ] Close button on panel closes top panel

## Tests
- [ ] Existing `inspector-focus-bridge.test.tsx` still passes
- [ ] Run `cd kanban-app && pnpm vitest run` — all tests pass
- [ ] Manual: inspect a task, verify panel opens; open multiple, verify stacking