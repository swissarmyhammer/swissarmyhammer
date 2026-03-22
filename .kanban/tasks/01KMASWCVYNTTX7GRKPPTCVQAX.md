---
assignees:
- claude-code
depends_on:
- 01KMASVEQA7K7F1TKE3ACAWDXT
position_column: todo
position_ordinal: '9980'
title: Data-driven field editor test harness — all editors × all keymaps × all exit paths
---
## What

Create a parameterized test harness that exercises every field editor across every keymap mode and every exit path. One test loop, not duplicated test blocks.

### Files to create
- `kanban-app/ui/src/components/fields/editors/editor-save.test.tsx` — the matrix test

### Approach
1. Define a test matrix: `editors × keymapModes × exitPaths`
   - Editors: `markdown`, `number`, `select`, `date`, `color-palette`, `multi-select`
   - Keymap modes: `cua`, `vim`, `emacs`
   - Exit paths: `blur`, `Enter`, `Escape`
2. For each combination, define expected behavior:
   - `blur` → always saves
   - `Enter` → always saves (or submits)
   - `Escape` → vim saves, CUA/emacs discards
3. Each test renders the editor with mocked `useFieldUpdate`, simulates the exit path, asserts whether `updateField` was called
4. Use `describe.each` or `it.each` to drive the matrix
5. Delete `field-placeholder.test.tsx` — its coverage is subsumed by this

### Key design decisions
- Each editor needs a small adapter: how to set its value (CM6 dispatch vs input.value), how to trigger exit (keyDown vs blur)
- Define these as a `Record<editorName, { render, setValue, getExitTarget }>` map
- The test loop is generic — editor-specific setup is in the adapter

## Acceptance Criteria
- [ ] Single test file covers all editor × keymap × exit combinations
- [ ] No duplicated test logic between editors
- [ ] Test failures clearly identify which editor/keymap/exit failed
- [ ] Existing `field-placeholder.test.tsx` deleted (replaced by this)

## Tests
- [ ] `cd kanban-app/ui && npx vitest run src/components/fields/editors/editor-save.test.tsx` — all pass after editor cards are done
- [ ] Temporarily expect failures for editors not yet migrated — that's the red in TDD