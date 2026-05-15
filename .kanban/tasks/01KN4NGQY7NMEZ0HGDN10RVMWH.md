---
assignees:
- claude-code
depends_on:
- 01KN4NEVT8AVMVWX4WTHJG15FJ
position_column: done
position_ordinal: ffffffffffffffffffffffbe80
title: 7. Perspective filter/sort/group evaluation + view integration
---
## What

Add client-side evaluation of perspective filter/sort/group expressions and wire into both board and grid views.

**Files to create:**
- `kanban-app/ui/src/lib/perspective-eval.ts` — pure evaluation functions

**Files to modify:**
- `kanban-app/ui/src/components/board-view.tsx` — filter tasks through active perspective before rendering
- `kanban-app/ui/src/components/grid-view.tsx` — filter and sort entities through active perspective

**Approach:**

Filter evaluation:
- `evaluateFilter(filter: string | undefined, entities: Entity[]): Entity[]`
- Compiles the JS expression string into a function via `new Function()` (sandboxed arrow fn)
- Caches compiled function by expression string (avoid recompile on every render)
- Returns all entities if filter is undefined/empty
- Catches eval errors gracefully (returns all entities + console.warn)
- Entity fields are passed as a flat object so `entity.Status` works

Sort evaluation:
- `evaluateSort(sort: SortEntry[], entities: Entity[]): Entity[]`
- Multi-level sort: compares by first entry, ties broken by second, etc.
- String comparison (locale-aware), number comparison, date comparison
- Uses field values from entity.fields

Group evaluation (board view):
- Board already groups by column — group expression could override swimlane grouping
- For now, group expression is stored but not evaluated (future card)

**View integration:**
- Board view: `useMemo` to filter tasks through active perspective before grouping into columns
- Grid view: `useMemo` to filter and sort entities before passing to DataTable

## Acceptance Criteria
- [ ] `evaluateFilter` correctly filters entities using JS expressions
- [ ] `evaluateSort` sorts by multiple fields with asc/desc
- [ ] Board view shows only tasks matching the active perspective's filter
- [ ] Grid view shows filtered + sorted entities from active perspective
- [ ] Malformed filter expressions fail gracefully (no crash, all entities shown)

## Tests
- [ ] `kanban-app/ui/src/lib/perspective-eval.test.ts` — filter with various JS expressions, sort with multi-level entries, error handling for bad expressions
- [ ] `pnpm test` from `kanban-app/ui/` passes