---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffb880
title: Add left/right keyboard navigation between pills in badge-list display
---
## What

When a multi-select field (tags, assignees, depends_on) is focused in the inspector, there's no way to navigate between individual pills with the keyboard. The inspector provides up/down (`j`/`k` or `ArrowUp`/`ArrowDown`) to move between fields, but within a badge-list display field, there's no left/right navigation to move focus between the pills.

Each pill is already a `FocusScope` with entity focus support (in `mention-pill.tsx:108`), and each has a context menu and commands. The missing piece is a horizontal nav layer that lets the user cycle through pills when a badge-list field is focused.

### Approach

Add a `usePillNav` hook (or inline state) in `BadgeListDisplay` that tracks which pill index is focused within the field, and wire `ArrowLeft`/`ArrowRight` (CUA) and `h`/`l` (vim) keys to move between pills.

### Architecture

- **Inspector field focus** (vertical, `useInspectorNav`): `j`/`k` moves between fields
- **Pill focus** (horizontal, new): `h`/`l` or `ArrowLeft`/`ArrowRight` moves between pills within a focused badge-list field
- When a badge-list field gains inspector focus, pill index defaults to 0 (first pill)
- When pill focus changes, call `setFocus(pillMoniker)` to update the entity focus system so the correct pill gets `data-focused`

### Files to modify

- **Modify**: `kanban-app/ui/src/components/fields/displays/badge-list-display.tsx` — add pill index state, pass focused prop to pills, register left/right commands
- **Modify**: `kanban-app/ui/src/components/entity-inspector.tsx` — pass `focused` prop down to displays so `BadgeListDisplay` knows when it's the active field
- **New hook** (optional): `kanban-app/ui/src/hooks/use-pill-nav.ts` — if the logic is non-trivial, extract to a reusable hook

### Key binding integration

Add commands to the inspector's `CommandScopeProvider` in `inspector-focus-bridge.tsx`:
- `inspector.pillLeft` — keys: `{ vim: "h", cua: "ArrowLeft" }` — move pill focus left
- `inspector.pillRight` — keys: `{ vim: "l", cua: "ArrowRight" }` — move pill focus right

These should only fire when the focused field is a badge-list display (not during edit mode).

## Acceptance Criteria

- [ ] When a badge-list field is focused, `ArrowLeft`/`ArrowRight` (CUA) or `h`/`l` (vim) moves focus between pills
- [ ] Focused pill shows `data-focused` attribute (visual indicator via existing CSS)
- [ ] Navigation wraps or clamps at boundaries (first/last pill)
- [ ] Up/down still moves between fields (no conflict with left/right)
- [ ] Entering edit mode (`i`/`Enter`) still opens the CM6 multi-select editor

## Tests

- [ ] `kanban-app/ui/src/components/fields/displays/badge-list-display.test.tsx` — new test: ArrowRight moves data-focused from first pill to second pill
- [ ] `kanban-app/ui/src/components/fields/displays/badge-list-display.test.tsx` — new test: ArrowLeft from first pill stays on first pill (clamp)
- [ ] `pnpm vitest run` passes