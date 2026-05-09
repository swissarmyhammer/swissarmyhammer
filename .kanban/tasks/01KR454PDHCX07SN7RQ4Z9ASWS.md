---
assignees:
- claude-code
position_column: todo
position_ordinal: '7e80'
title: vitest birpc unhandled rejections cause `pnpm test` to exit 1 (multiple browser-mode test files)
---
## Symptom

`cd kanban-app/ui && pnpm test` exits with code 1 even though all 2042 tests pass. The output contains ~123 unhandled rejections of the form:

```
Error: [birpc] function "toJSON" not found
```

## Affected files (as of 2026-05-08)

The noise originates in **four** browser-mode test files, all unrelated to the spatial-nav / focus-layer changes that prompted this discovery:

- `kanban-app/ui/src/components/fields/editors/editor-save.test.tsx`
- `kanban-app/ui/src/components/column-view.scroll-rects.browser.test.tsx`
- `kanban-app/ui/src/components/column-view.virtualized-nav.browser.test.tsx`
- `kanban-app/ui/src/components/data-table.virtualized.test.tsx`

Running each in isolation passes 100% of its assertions; the rejections are Vitest worker-IPC noise.

## Investigation pointers

- All four files use `@vitest/browser` and exercise virtualized lists / field editors.
- Error string is exactly `[birpc] function "toJSON" not found` — a known vitest browser-mode RPC-serialization issue, likely triggered by a circular structure or non-serializable mock value being passed across the worker boundary.
- Hypothesis: a shared helper or mock factory that returns an object with a `toJSON`-like getter that isn't reachable from the worker side. Worth grepping for `toJSON` in the test setup files and any `vi.mock` factories these files share.
- Reproduction: `cd kanban-app/ui && pnpm test src/components/fields/editors/editor-save.test.tsx` (or any of the four) and watch the rejections.

## Acceptance Criteria

- [ ] `pnpm test` exits 0 with all tests passing.
- [ ] No unhandled rejections in test output.
- [ ] All four affected files pass cleanly when run in isolation AND in the full suite.

## Tags

#test-failure