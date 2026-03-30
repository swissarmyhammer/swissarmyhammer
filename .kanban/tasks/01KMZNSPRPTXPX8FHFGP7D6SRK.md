---
assignees:
- claude-code
position_column: todo
position_ordinal: '8280'
title: Fix inspect command — backend ui.inspect must sync with frontend InspectProvider
---
## What

Inspect is broken: context menu and palette dispatch `ui.inspect` to the backend, which pushes onto UIState's `inspector_stack`. But the frontend inspector panel is driven by `InspectProvider` (React context), not UIState. The two are disconnected.

### Root cause
- Entity YAML was changed from `entity.inspect` to `ui.inspect`
- The old flow: frontend `entity.inspect` → `inspectEntity(moniker)` → `InspectProvider.onInspect` → React state → panel opens
- The new flow: backend `ui.inspect` → UIState inspector_stack → `ui-state-changed` event → BUT frontend InspectProvider doesn't listen to this event

### Fix options (pick one)
**Option A**: Frontend listens to `ui-state-changed` and syncs inspector stack from UIState into InspectProvider state. The backend is the source of truth.

**Option B**: The context menu dispatches inspect through the frontend scope chain (keep client-side handling). But this breaks the \"backend is single source of truth\" principle.

**Recommended: Option A** — the inspector stack in UIState already works (tested in Rust). The frontend just needs to read it.

### Files to modify
- `kanban-app/ui/src/components/app-shell.tsx` — the InspectProvider's `onInspect`/`onDismiss` should call `invoke(\"dispatch_command\", { cmd: \"ui.inspect\", target: moniker })` instead of local React state
- OR: `kanban-app/ui/src/lib/inspect-context.tsx` — sync from `ui-state-changed` event
- `kanban-app/ui/src/components/inspector-focus-bridge.tsx` — may need to read stack from UIState snapshot

## Acceptance Criteria
- [ ] Double-click task card → inspector opens
- [ ] Right-click → Inspect Task → inspector opens
- [ ] Command palette → Inspect Task → inspector opens
- [ ] Inspector close button works
- [ ] Multiple inspectors stack correctly

## Tests
- [ ] Existing inspector tests still pass
- [ ] `cargo nextest run -p kanban-app` passes"
<parameter name="assignees">[]