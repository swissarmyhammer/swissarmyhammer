---
assignees:
- claude-code
depends_on:
- 01KNJD8P8KPJK4HN8JCDKEH724
- 01KNJDAAZQ4B6PFGR9N1Z7A7YJ
position_column: done
position_ordinal: ffffffffffffffffffffff8b80
position_swimlane: null
title: 'FILTER-6: Remove JS eval path + cleanup'
---
## What

Final cleanup: remove all remnants of the JS `new Function()` filter path and update the Perspective type documentation.

### Files to modify
- `kanban-app/ui/src/lib/perspective-eval.ts` — remove `compileFilter`, `evaluateFilter`, `clearFilterCache`, `permissiveProxy`, `FilterFn` type, `filterCache`. Keep `evaluateSort` and `compareValues` (sort is still client-side).
- `kanban-app/ui/src/lib/perspective-eval.test.ts` — remove filter-related tests, keep sort tests
- `kanban-app/ui/src/components/perspective-container.tsx` — remove `applyFilter` callback that uses `evaluateFilter`, replace with backend-filtered entity consumption
- `kanban-app/ui/src/components/perspective-container.test.tsx` — update tests
- `swissarmyhammer-perspectives/src/types.rs` — update `Perspective.filter` doc comment from \"Opaque filter function string (JS expression)\" to \"Filter DSL expression (e.g. && @will)\"
- `kanban-app/ui/src/types/kanban.ts` — update `PerspectiveDef.filter` doc comment
- Remove `@codemirror/lang-javascript` from package.json if no longer used elsewhere

## Acceptance Criteria
- [ ] No `new Function()` or `with()` usage remains in the codebase for filter evaluation
- [ ] `perspective-eval.ts` only exports sort-related functions
- [ ] All tests pass (`cargo test`, `npm test`)
- [ ] No unused imports or dead code warnings
- [ ] Doc comments on filter fields reference the DSL, not JS expressions

## Tests
- [ ] `kanban-app/ui/src/lib/perspective-eval.test.ts` — sort tests still pass
- [ ] `kanban-app/ui/src/components/perspective-container.test.tsx` — updated tests pass
- [ ] Full test suite green

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.