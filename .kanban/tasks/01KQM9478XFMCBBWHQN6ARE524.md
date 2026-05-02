---
assignees:
- claude-code
position_column: todo
position_ordinal: bf80
project: spatial-nav
title: 'Card drag handle: drop the FocusScope wrapper — handle is mouse-only, no keyboard target needed'
---
## What

`DragHandle` in `kanban-app/ui/src/components/entity-card.tsx` (lines 173–192) wraps the `<GripVertical>` button in a `<FocusScope moniker={asSegment(`card.drag-handle:${entityId}`)}>` leaf. The intent (per the in-source doc-comment, lines 159–172) was to give the spatial-nav graph a navigable atom inside the card zone for the drag-grip affordance.

But drag-and-drop on cards is **mouse-only**. `kanban-app/ui/src/components/board-view.tsx` line 588 wires `@dnd-kit` with `useSensor(PointerSensor, …)` — no `KeyboardSensor`. The drag handle has no keyboard activation story: pressing Enter / Space on the focused leaf does nothing useful (no `onClick` action, drag handlers come from `dragHandleProps` and only respond to pointer events). The leaf adds a tab/arrow stop that lands the user on a button they can't actually use without a mouse.

### Approach

Edit `entity-card.tsx` only. In `DragHandle` (lines 173–192):

1. Remove the `<FocusScope moniker={asSegment(`card.drag-handle:${entityId}`)}>` wrapper. The component returns the `<button>` directly.
2. Remove the now-unused `entityId` prop and update the single call site at line 121 to drop the `entityId={entity.id}` arg.
3. Update the doc-comment (lines 159–172) to state explicitly: "The drag handle is mouse-only — `@dnd-kit` is configured with `PointerSensor` and no `KeyboardSensor` (see `board-view.tsx::useSensor(PointerSensor, …)`). It is intentionally NOT a `<FocusScope>` because there is nothing the keyboard user could do once focus landed there."
4. The `onClick={(e) => e.stopPropagation()}` stays — it prevents click-bubble to the card body (which would dispatch `ui.inspect` via the `<Inspectable>` wrapper). Mouse activation of the drag handle is the only pointer story; keyboard is out of scope.

### Knock-on changes

- `kanban-app/ui/src/components/entity-card.scope-leaf.spatial.test.tsx` lines 283–306 (and the file's docstring at lines 28–30) currently pin the drag-handle leaf as a `card.drag-handle:{id}` scope under the card zone. Update both:
  - The doc-comment list of "inner leaf scopes" should read just "the inspect-button" (drag-handle removed).
  - The `it("the drag-handle leaf registers as a scope under the card zone")` test must invert: assert that **no** `spatial_register_scope` call has `segment === "card.drag-handle:task-1"`.
  - The neighbouring test `it("the inspect-button leaf registers as a scope under the card zone")` (or equivalent) stays as-is — inspect IS keyboard-activatable (Space → inspect via `<Inspectable>`).

### Why not keep it focusable but invisible to keyboard?

The cleanest answer is "don't register what cannot be acted on." A focusable element with no keyboard action is a tab-stop trap. The card's other inner leaf (`InspectButton`) has a real keyboard action (Space/Enter → dispatch `ui.inspect`); the drag handle has none.

## Acceptance Criteria
- [ ] `DragHandle` in `kanban-app/ui/src/components/entity-card.tsx` no longer wraps its `<button>` in a `<FocusScope>`. The component takes only `dragHandleProps` and returns `<button … onClick={…} {...dragHandleProps}><GripVertical /></button>`.
- [ ] The single call site (line ~120) is updated — no `entityId` prop passed.
- [ ] Doc-comment on `DragHandle` is rewritten to explain the mouse-only rationale and reference `board-view.tsx::useSensor(PointerSensor, …)` as the source of truth.
- [ ] `mockInvoke("spatial_register_scope", …)` calls during card mount no longer include any segment matching `^card\.drag-handle:`.
- [ ] Drag-and-drop still works: `dragHandleProps` are still spread onto the `<button>`, so dnd-kit's pointer sensor still picks up grip drags.
- [ ] Click on the drag handle still does NOT bubble to the card body's inspect handler (the `onClick={(e) => e.stopPropagation()}` is preserved).

## Tests
- [ ] Update `kanban-app/ui/src/components/entity-card.scope-leaf.spatial.test.tsx`:
  - Rewrite `it("the drag-handle leaf registers as a scope under the card zone")` (line ~283) to `it("the drag-handle does NOT register as a scope")`. Assert that `mockInvoke` was called zero times with `cmd === "spatial_register_scope"` and `args.segment` matching `/^card\.drag-handle:/`.
  - Update the file's top-of-file docstring (lines 28–30) to remove "drag-handle" from the list of inner leaf scopes.
- [ ] Existing card spatial tests stay green: `entity-card.spatial.test.tsx`, `entity-card.test.tsx`, `entity-card.field-icon-inside-zone.browser.test.tsx`, `column-view.spatial-nav.test.tsx`, `board-view.cross-column-nav.spatial.test.tsx`. In particular, the inspect-button leaf assertion in `scope-leaf.spatial.test.tsx` must remain green (inspect IS keyboard-actionable; only the drag handle is being demoted).
- [ ] Existing drag-drop tests stay green: `column-dragover.browser.test.tsx`, `sortable-task-card.test.tsx`, `sortable-column.tsx`'s tests. Confirm the pointer-driven drag path is unaffected by removing the FocusScope (the FocusScope was a sibling of the drag handlers, not in their event path).
- [ ] Run `cd kanban-app/ui && pnpm vitest run src/components/entity-card src/components/sortable-task-card src/components/column-dragover` and confirm green.

## Workflow
- Use `/tdd` — invert the existing scope-leaf assertion first (failing test demanding the drag handle NOT be a scope), watch it fail, then remove the wrapper from `DragHandle` and confirm it passes. Final pass: drag-drop browser tests stay green.
