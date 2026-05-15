---
assignees:
- claude-code
depends_on:
- 01KN79W9P97598SDQG02PSWWXE
position_column: done
position_ordinal: ffffffffffffffffffffffb780
title: 5. Perspective TS types + PerspectiveProvider context
---
## What

Add TypeScript types mirroring the Rust `Perspective` struct and a React context that manages perspective state for the GUI.

**Files to create:**
- `kanban-app/ui/src/lib/perspective-context.tsx` — new PerspectiveProvider

**Files to modify:**
- `kanban-app/ui/src/types/kanban.ts` — add Perspective, PerspectiveFieldEntry, SortEntry, SortDirection types
- `kanban-app/ui/src/App.tsx` — wrap ViewsProvider children with PerspectiveProvider

**Approach:**
- Types mirror Rust types in `swissarmyhammer-perspectives/src/types.rs`
- PerspectiveProvider fetches perspectives via `backendDispatch({ cmd: "perspective.list" })` on mount
- Tracks active perspective per view kind (local React state — UIState persistence is follow-up)
- Provides hooks: `usePerspectives()` returns all perspectives, `useActivePerspective()` returns the current one
- `setActivePerspectiveId(id)` switches active perspective
- Follow the pattern established by `views-context.tsx`

**Refresh strategy:**
The store crate emits change events via `store_context.flush_all()` after every undoable command (including perspective mutations). The PerspectiveProvider listens to perspective change events (emitted by the perspective store's `flush_changes()`) and re-fetches. Same event-driven pattern as entity events driving the entity store UI.

**Key types:**
```typescript
interface PerspectiveFieldEntry {
  field: string;  // ULID
  caption?: string;
  width?: number;
  editor?: string;
  display?: string;
  sort_comparator?: string;
}

interface SortEntry {
  field: string;
  direction: "asc" | "desc";
}

interface Perspective {
  id: string;
  name: string;
  view: string;  // "board" | "grid" | ...
  fields: PerspectiveFieldEntry[];
  filter?: string;  // opaque JS expression
  group?: string;   // opaque JS expression
  sort: SortEntry[];
}
```

## Acceptance Criteria
- [ ] Perspective types exported from `kanban-app/ui/src/types/kanban.ts`
- [ ] PerspectiveProvider fetches perspectives from backend on mount
- [ ] PerspectiveProvider listens to perspective change events and re-fetches
- [ ] `usePerspectives()` returns full list of perspectives
- [ ] `useActivePerspective()` returns the perspective matching active view kind
- [ ] Active perspective switches when tab is clicked (via `setActivePerspectiveId`)
- [ ] PerspectiveProvider nested inside ViewsProvider in App.tsx

## Tests
- [ ] `kanban-app/ui/src/lib/perspective-context.test.tsx` — unit tests for provider: renders without crash, provides default values, switches active perspective
- [ ] `pnpm test` from `kanban-app/ui/` passes