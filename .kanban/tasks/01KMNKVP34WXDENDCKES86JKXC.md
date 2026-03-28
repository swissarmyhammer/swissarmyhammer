---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffc880
title: Extract computeDropZones pure function + tests
---
## What

Create a pure function `computeDropZones()` in `kanban-app/ui/src/lib/drop-zones.ts` that, given the sorted task IDs in a column plus the board/column identity, returns an array of drop zone descriptors. Each descriptor carries the **exact** placement data the backend needs — no runtime computation at drop time.

```ts
interface DropZoneDescriptor {
  /** Unique key for React rendering */
  key: string;
  /** Board path for cross-board drops */
  boardPath: string;
  /** Target column ID */
  columnId: string;
  /** Place the dropped task before this ID (mutually exclusive with afterId) */
  beforeId?: string;
  /** Place the dropped task after this ID (mutually exclusive with beforeId) */
  afterId?: string;
}

function computeDropZones(
  taskIds: string[],
  columnId: string,
  boardPath: string,
): DropZoneDescriptor[]
```

For `taskIds = ['A', 'B', 'C']`, returns:
```
[
  { key: 'before-A', boardPath, columnId, beforeId: 'A' },              // slot 0: before first
  { key: 'before-B', boardPath, columnId, beforeId: 'B' },              // slot 1: between A and B
  { key: 'before-C', boardPath, columnId, beforeId: 'C' },              // slot 2: between B and C
  { key: 'after-C',  boardPath, columnId, afterId: 'C' },               // slot 3: after last
]
```

For empty column `taskIds = []`:
```
[
  { key: 'empty', boardPath, columnId }   // no before/after = backend appends
]
```

Also delete the now-superseded `drop-placement.ts` and `drop-placement.test.ts` created earlier in this session.

### Files
- **Create**: `kanban-app/ui/src/lib/drop-zones.ts`
- **Create**: `kanban-app/ui/src/lib/drop-zones.test.ts`
- **Delete**: `kanban-app/ui/src/lib/drop-placement.ts`
- **Delete**: `kanban-app/ui/src/lib/drop-placement.test.ts`

## Acceptance Criteria
- [ ] `computeDropZones` returns N+1 descriptors for N tasks (one before each task, one after last)
- [ ] Empty column returns single descriptor with no before/after
- [ ] Each descriptor carries boardPath and columnId
- [ ] `drop-placement.ts` and its test are deleted

## Tests
- [ ] `kanban-app/ui/src/lib/drop-zones.test.ts` — 3 tasks produces 4 zones with correct before/after IDs
- [ ] `kanban-app/ui/src/lib/drop-zones.test.ts` — empty column produces 1 zone with no placement
- [ ] `kanban-app/ui/src/lib/drop-zones.test.ts` — single task produces 2 zones (before + after)
- [ ] `kanban-app/ui/src/lib/drop-zones.test.ts` — all zones carry boardPath and columnId
- [ ] `pnpm vitest run src/lib/drop-zones.test.ts` passes