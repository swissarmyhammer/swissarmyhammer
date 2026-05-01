---
assignees:
- claude-code
depends_on:
- 01KMNKVP34WXDENDCKES86JKXC
position_column: done
position_ordinal: ffffffffffffffa080
title: Create DropZone React component + tests
---
## What

Create a `DropZone` component in `kanban-app/ui/src/components/drop-zone.tsx` that renders a preconfigured drop target. Each zone carries its placement data (`DropZoneDescriptor`) and handles HTML5 drag events internally.

```tsx
interface DropZoneProps {
  descriptor: DropZoneDescriptor;
  /** ID of the task currently being dragged (to hide no-op zones) */
  dragTaskId?: string | null;
  /** Whether a drag is currently active (expand zones to catch drops) */
  dragActive?: boolean;
  /** Called when a task is dropped on this zone */
  onDrop: (descriptor: DropZoneDescriptor, taskData: string) => void;
  /** Render as empty-column target (fills available space) vs between-card sliver */
  variant?: 'between' | 'empty-column';
}
```

### Two visual variants

**`between` (default)**: thin sliver (~4px) between cards
- Expands to ~24px when `dragActive` is true
- Highlights on dragover with the current `bg-primary rounded-full` indicator

**`empty-column`**: fills available vertical space in an empty column
- Shows the current empty-column placeholder (Inbox icon + \"No tasks\")
- Entire area is a drop target
- Highlights with dashed border on dragover

### Behavior
- **Drop**: calls `onDrop(descriptor, taskData)` — descriptor already has before/after/column/board
- **No-op hiding**: when `dragTaskId` matches the zone's `beforeId` or `afterId`, the zone collapses

### Data attributes for testing
- `data-drop-zone` — present on all zones
- `data-drop-before=\"{id}\"` — when zone has `beforeId`
- `data-drop-after=\"{id}\"` — when zone has `afterId`
- `data-drop-empty` — when zone is the empty-column variant

### Files
- **Create**: `kanban-app/ui/src/components/drop-zone.tsx`
- **Create**: `kanban-app/ui/src/components/drop-zone.test.tsx`

## Acceptance Criteria
- [ ] Component renders with `data-drop-zone` attribute
- [ ] Correct `data-drop-before` / `data-drop-after` attributes from descriptor
- [ ] Highlights on dragover, unhighlights on dragleave
- [ ] Calls `onDrop(descriptor, taskData)` when a task is dropped
- [ ] Collapses when `dragTaskId` matches adjacent task ID
- [ ] `empty-column` variant fills available space and shows placeholder
- [ ] `empty-column` variant highlights entire area on dragover

## Tests
- [ ] `drop-zone.test.tsx` — renders data-drop-zone attribute
- [ ] `drop-zone.test.tsx` — renders data-drop-before when descriptor has beforeId
- [ ] `drop-zone.test.tsx` — renders data-drop-after when descriptor has afterId
- [ ] `drop-zone.test.tsx` — renders data-drop-empty for empty-column variant
- [ ] `drop-zone.test.tsx` — fires onDrop with descriptor when drop event occurs
- [ ] `drop-zone.test.tsx` — empty-column zone fires onDrop (no before/after in descriptor)
- [ ] `pnpm vitest run src/components/drop-zone.test.tsx` passes