---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffb580
title: 'Consolidate: multi-select-editor Escape — replace Prec.highest with buildSubmitCancelExtensions'
---
## What

In `kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx` (lines 230–253), the Escape/Enter handling uses `Prec.highest(keymap.of([...]))` — a third pattern distinct from both the broken `domEventHandlers` approach and the correct `buildSubmitCancelExtensions` approach. It works, but it's a one-off that doesn't check vim insert/normal state: Escape always fires immediately regardless of vim mode.

This means in vim mode, Escape commits/cancels even from insert mode — the user can't exit insert mode without also leaving the editor. `buildSubmitCancelExtensions` handles this correctly with its two-phase capture/bubble strategy.

**Fix**: Replace the `Prec.highest(keymap.of([...]))` block with `buildSubmitCancelExtensions`. The multi-select-editor's contract:
- **onSubmit** (Enter) → `commitRef.current()`
- **onCancel** (Escape) → vim: `commitRef.current()`; CUA/emacs: `cancelRef.current()`
- `singleLine: true`

**Files to modify**:
- `kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx` — replace lines 230–253 with `buildSubmitCancelExtensions`

## Acceptance Criteria
- [ ] In vim mode, Escape in normal mode commits (unchanged behavior)
- [ ] In vim mode, Escape in insert mode now exits to normal mode first (behavior improvement)
- [ ] CUA/emacs Escape cancels (unchanged)
- [ ] Enter commits in all modes (unchanged)

## Tests
- [ ] Add or update multi-select-editor test: vim insert-mode Escape exits to normal without committing
- [ ] Add or update test: vim normal-mode Escape commits
- [ ] Run `pnpm --filter kanban-app-ui test` — all tests pass