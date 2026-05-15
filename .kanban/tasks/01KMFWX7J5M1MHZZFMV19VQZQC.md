---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffff280
title: 'CM6 vim mode: Escape in insert mode exits editor instead of returning to normal mode'
---
## What

Pressing Escape in a CM6 editor while in vim insert mode exits the editor entirely instead of returning to vim normal mode. The user never gets to use normal mode (navigation, motions, etc.).

**Root cause — two mechanisms both trigger exit on insert→normal transition:**

1. **`kanban-app/ui/src/lib/cm-submit-cancel.ts:84-97`** — The `domEventHandlers` Escape handler fires after vim has already processed the Escape (exiting insert mode). By the time our handler runs, `insertMode` is `false`, so it falls through to `onCancelRef.current?.()` and exits the editor.

2. **`kanban-app/ui/src/components/fields/text-editor.tsx:206-223`** — A `ViewPlugin` watches for insert→normal mode transitions and immediately calls `commitAndExitRef.current()`. Even if the domEventHandlers correctly defers, this plugin detects the transition and exits.

**Fix approach:** The Escape handler in `cm-submit-cancel.ts` needs to track whether vim was in insert mode *before* the current Escape keypress, not check the state after vim has already processed it. One approach: use a capture-phase DOM listener (like the Enter handler already does) that reads `insertMode` before vim's handler fires. If insertMode was true, suppress the cancel — vim will handle the transition to normal mode. The ViewPlugin in `text-editor.tsx` that auto-exits on insert→normal transition should be removed or gated so Escape-driven transitions don't trigger exit.

### Files to modify
- `kanban-app/ui/src/lib/cm-submit-cancel.ts` — Escape handler for vim mode (lines 82-98)
- `kanban-app/ui/src/components/fields/text-editor.tsx` — ViewPlugin insert→normal watcher (lines 206-223)

### Files for reference
- `kanban-app/ui/src/components/editable-markdown.tsx` — uses same `buildSubmitCancelExtensions`
- `kanban-app/ui/src/lib/cm-submit-cancel.test.ts` — existing test suite with vim mode Escape tests

## Acceptance Criteria
- [ ] Pressing Escape in vim insert mode returns to normal mode without exiting the editor
- [ ] Pressing Escape in vim normal mode exits the editor (commits and closes)
- [ ] CUA/emacs Escape behavior unchanged (always exits)
- [ ] Works in both text-editor (inline fields) and editable-markdown editors

## Tests
- [ ] Update `kanban-app/ui/src/lib/cm-submit-cancel.test.ts`: \"Escape in insert mode does NOT call onCancelRef\" must pass (already exists — verify it reflects real behavior)
- [ ] Add test: Escape in insert mode → vim transitions to normal mode → editor remains open
- [ ] Add test: Escape in normal mode → onCancelRef is called → editor exits
- [ ] `npx vitest run` — all 34 suites pass