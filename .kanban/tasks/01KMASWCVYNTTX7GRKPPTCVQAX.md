---
assignees:
- claude-code
depends_on:
- 01KMASVEQA7K7F1TKE3ACAWDXT
position_column: done
position_ordinal: fffffffffff280
title: Data-driven field editor test harness — all editors × all keymaps × all exit paths
---
## What

Create a parameterized test harness that exercises every field editor across every keymap mode and every exit path. Written FIRST — every test fails. The subsequent cards make them pass.

### Files to create
- `kanban-app/ui/src/components/fields/editors/editor-save.test.tsx` — the matrix test

### Approach
1. Define a test matrix: `editors × keymapModes × exitPaths`
   - Editors: `markdown`, `number`, `select`, `date`, `color-palette`, `multi-select`
   - Keymap modes: `cua`, `vim`, `emacs`
   - Exit paths: `blur`, `Enter`, `Escape`
2. Expected behavior per combination:
   - `blur` → always saves
   - `Enter` → always saves
   - `Escape` → vim saves, CUA/emacs discards
3. Mock `useFieldUpdate` — assert `updateField` called with correct entity/field/value
4. Per-editor adapter: `{ render, setValue, getExitTarget }` — editor-specific setup
5. `describe.each` × `it.each` drives the loop
6. Delete `field-placeholder.test.tsx` when this subsumes it

### This card delivers a fully red test suite. That's the point.

## Acceptance Criteria
- [ ] Single test file covers all editor × keymap × exit combinations
- [ ] No duplicated test logic between editors
- [ ] Failure messages clearly identify editor/keymap/exit
- [ ] All tests fail (editors haven't been migrated yet)

## Tests
- [ ] `cd kanban-app/ui && npx vitest run src/components/fields/editors/editor-save.test.tsx` — runs, all fail
- [ ] Count of failures = editors × keymaps × exit paths (minus discards)