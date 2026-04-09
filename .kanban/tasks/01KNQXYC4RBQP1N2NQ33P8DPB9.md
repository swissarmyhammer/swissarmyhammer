---
assignees:
- claude-code
depends_on:
- 01KNQXXF5W7G4JP73C6ZCMKYKX
position_column: todo
position_ordinal: a380
project: spatial-nav
title: Add FocusLayer to inspector (stacks on top of root window layer)
---
## What

The root `<FocusLayer name="window">` is added in card 1 (at the app shell). This card adds the inspector's layer so it captures navigation when open, and replaces `useRestoreFocus` with layer stack semantics.

### Layer boundary semantics

**Layers are hard boundaries.** Navigation stays within the active (topmost) layer. You cannot arrow out of the inspector — dismiss it to return to the board. This is the "semi-modal" behavior.

When a FocusLayer mounts, it saves the currently focused moniker from the previous layer. When it unmounts, it restores that moniker via `setFocus`. This replaces the ad-hoc `useRestoreFocus()` hook entirely.

### Files to modify

1. **`kanban-app/ui/src/components/inspector-focus-bridge.tsx`** — Wrap the inspector content in `<FocusLayer name="inspector">`. When the inspector mounts, this pushes onto the layer stack and becomes the active layer. When it unmounts, the layer pops and focus returns to the window layer's previously focused element.

2. **`kanban-app/ui/src/components/inspectors-container.tsx`** — Remove `useRestoreFocus()` import and call. FocusLayer handles focus save/restore via the layer stack. This file currently imports and calls `useRestoreFocus` — that hook is now redundant.

3. **`kanban-app/ui/src/components/entity-inspector.tsx`** — The manual first-field focus on mount (lines ~160-180) should still work — it sets focus within the inspector layer. But verify it doesn't conflict with FocusLayer's own mount behavior.

4. **`kanban-app/ui/src/components/app-shell.tsx`** — Evaluate whether the command palette needs `<FocusLayer name="palette">`. It likely does if it should capture keyboard nav while open.

### Subtasks
- [ ] Add `<FocusLayer name="inspector">` to InspectorFocusBridge
- [ ] Remove `useRestoreFocus()` from inspectors-container.tsx — replaced by FocusLayer stack
- [ ] Verify inspector captures nav (arrows only move within inspector fields)
- [ ] Verify inspector close pops the layer, restoring window layer's previously focused moniker
- [ ] Verify entity-inspector's first-field focus on mount still works within the layer

## Acceptance Criteria
- [ ] Inspector navigation is captured — arrows don't escape to the board (hard boundary)
- [ ] Closing inspector restores focus to the element that was focused before it opened
- [ ] `useRestoreFocus` removed from inspectors-container.tsx
- [ ] Window layer navigation flows freely across board, toolbar, tab bar, perspective bar
- [ ] `nav.up` from top board card reaches column header, then toolbar, then tab bar
- [ ] Multiple inspector stack (open task A inspector, then task B inspector) works — each layer push/pop is independent via ULID key
- [ ] Existing `claimWhen` predicates still work (this card doesn't remove them yet)
- [ ] `pnpm vitest run` passes

## Tests
- [ ] `kanban-app/ui/src/components/inspector-focus-bridge.test.tsx` — inspector layer captures nav, teardown restores previous focus
- [ ] `kanban-app/ui/src/components/inspectors-container.test.tsx` — no more useRestoreFocus; focus save/restore via layer stack
- [ ] Integration test: with inspector closed, nav.up from top board element reaches toolbar/tab bar
- [ ] Integration test: open inspector A, open inspector B on top, close B — focus returns to A's last focused field, not the board
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.