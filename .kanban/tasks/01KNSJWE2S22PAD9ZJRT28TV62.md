---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffeb80
title: 'Verify end-to-end implicit AND filter: #paper #READY must require both tags'
---
## What

`#paper #READY` (two tags side by side without explicit `&&`) should filter to tasks that have BOTH tags, but doesn't work as expected. The backend chumsky parser and evaluator correctly implement implicit AND (tested in `swissarmyhammer-filter-expr/src/parser.rs` test `and_implicit` and integration test `s08_implicit_and`). The issue is either:

1. **Frontend Lezer validator rejects implicit AND** — `validateFilter()` in `filter-editor.tsx` walks the Lezer parse tree for error nodes. The Lezer grammar's `And` rule requires an explicit `AndOp` (`&&` or `and`), so two adjacent atoms produce two separate top-level `expr` nodes, not an `And` node. If Lezer produces error nodes for this, the frontend never dispatches the filter.

2. **Filter not reaching backend** — the filter-wiring card (`01KNS7HSEP1K53R8WMC7NFWMXT`) just landed; confirm the `PerspectiveContainer` `useEffect` correctly passes `activePerspective.filter` to `refreshEntities`.

### Research findings

- **Chumsky (backend)**: Implicit AND works — `parser.rs` line 101-105 uses `.or_not()` to make the operator optional between adjacent atoms.
- **Lezer (frontend)**: Grammar `And { expr !and AndOp expr }` REQUIRES explicit `AndOp`. But `@top FilterExpr { expr+ }` allows multiple exprs. The question is whether this produces error nodes.
- **`validateFilter`** (`filter-editor.tsx`): Rejects expressions with ANY error node. If implicit AND triggers an error node in Lezer, the filter is silently rejected.
- **`EntityFilterAdapter.has_tag`** (`kanban-app/src/commands.rs`): Checks `filter_tags` which is the union of body tags + virtual tags. Virtual tags (READY, BLOCKED, BLOCKING) are added by `enrich_task_entity`.

### Fix

Add a Lezer parser test confirming `#paper #READY` parses without errors. If it DOES produce errors, fix the Lezer grammar to support implicit AND (make `And` accept optional operator, matching chumsky). If it doesn't, the issue is in the wiring — add a test confirming the filter reaches `list_entities`.

### Files to modify

- `kanban-app/ui/src/lang-filter/__tests__/parser.test.ts` — add test: `#paper #READY` parses without error nodes
- `kanban-app/ui/src/lang-filter/filter.grammar` — if needed, fix implicit AND support to match chumsky backend
- `kanban-app/ui/src/components/filter-editor.test.tsx` — add test: `#paper #READY` is accepted by `validateFilter` and dispatches

## Acceptance Criteria
- [ ] `#paper #READY` in the filter bar is treated as implicit AND (both tags required)
- [ ] No error shown in the filter bar for `#paper #READY`
- [ ] Frontend Lezer parser produces no error nodes for `#paper #READY`
- [ ] Backend filter evaluation correctly returns only tasks matching both tags

## Tests
- [ ] `kanban-app/ui/src/lang-filter/__tests__/parser.test.ts` — `#paper #READY` has no error nodes
- [ ] `kanban-app/ui/src/components/filter-editor.test.tsx` — `#paper #READY` dispatches `perspective.filter`
- [ ] Run: `cd kanban-app/ui && npx vitest run src/lang-filter/ src/components/filter-editor.test.tsx`

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.