---
assignees:
- claude-code
position_column: todo
position_ordinal: bc80
title: 'TypeScript: unused `registerScopeArgs` in board-view.spatial.test.tsx'
---
## What

`kanban-app/ui/src/components/board-view.spatial.test.tsx` line 364 declares `function registerScopeArgs()` but no longer uses it. `tsc --noEmit` (which runs as the first step of `pnpm test`) errors out with:

```
src/components/board-view.spatial.test.tsx(364,10): error TS6133: 'registerScopeArgs' is declared but its value is never read.
```

This blocks the full `pnpm test` command — vitest never runs.

## Cause

The recent FQM Layer 2b refactor (commit b5c56683f / 95b67e974 / 5ef77b6c4 / 7169b4519) migrated `task:*` cards from `<FocusScope>` (leaf) to `<FocusZone>` (container). All call sites of `registerScopeArgs()` were rewritten to use `registerZoneArgs()`, but the helper itself was not deleted.

## Fix

Delete the `registerScopeArgs` function declaration (and its docstring) from `board-view.spatial.test.tsx`. It has no remaining callers.

## Acceptance Criteria

- [ ] `cd kanban-app/ui && pnpm exec tsc --noEmit` exits 0
- [ ] `cd kanban-app/ui && pnpm test` runs vitest (does not bail at tsc step)

## Context

Discovered while testing the focus-debug-overlay tooltip refactor task (01KQJHE82FPDD1YVN7RW8ZCF3T). This is pre-existing — the file is in `git status` as modified but the unused-function delta predates the focus-debug-overlay change. #test-failure