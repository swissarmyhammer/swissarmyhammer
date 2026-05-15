---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffc980
title: 'Fix: command-palette Escape — replace ad-hoc domEventHandlers with buildSubmitCancelExtensions'
---
## What

In `kanban-app/ui/src/components/command-palette.tsx`, the CM6 extension array (lines 318–371) uses a hand-rolled `EditorView.domEventHandlers` for Escape, registered after the vim extension. The vim extension consumes Escape in normal mode before the dismiss handler fires.

The codebase already has `buildSubmitCancelExtensions` (`kanban-app/ui/src/lib/cm-submit-cancel.ts`) — a shared helper that correctly handles vim Escape via a two-phase capture/bubble DOM listener strategy. `text-editor.tsx` and `filter-editor.tsx` already use it.

**Fix**: Replace the ad-hoc `EditorView.domEventHandlers` Escape handler and the `keymap.of([Enter, ArrowUp, ArrowDown])` block with `buildSubmitCancelExtensions`. The palette's contract:
- **onSubmit** (Enter) → execute selected command/result
- **onCancel** (Escape) → `onClose()` (dismiss palette)
- `singleLine: true`

Arrow key navigation (ArrowUp/ArrowDown) stays as a separate `keymap.of` since that's palette-specific, not submit/cancel.

**Files to modify**:
- `kanban-app/ui/src/components/command-palette.tsx` — replace lines 318–371 with `buildSubmitCancelExtensions` + arrow key keymap
- `kanban-app/ui/src/components/command-palette.test.tsx` — add test for vim normal-mode Escape dismiss

## Acceptance Criteria
- [ ] Pressing Escape in vim normal mode dismisses the command palette
- [ ] Pressing Escape in vim insert mode exits to normal mode (does not dismiss)
- [ ] Pressing Escape in CUA/emacs mode dismisses the command palette
- [ ] Enter still executes the selected command
- [ ] ArrowUp/ArrowDown still navigate the list

## Tests
- [ ] Add test in `command-palette.test.tsx`: "vim normal mode: Escape closes palette"
- [ ] Add test: "vim insert mode: Escape does NOT close palette"
- [ ] Run `pnpm --filter kanban-app-ui test` — all tests pass