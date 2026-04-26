---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffe580
title: Fix Enter key in command palette inserting newline instead of executing command (vim mode)
---
## What

In vim mode, the command palette auto-enters insert mode on open (`command-palette.tsx:192` calls `Vim.handleKey(cm, "i", "mapping")`). When the user presses Enter to execute the selected command, the shared `buildSubmitCancelExtensions` in `cm-submit-cancel.ts:76` checks `if (cm?.state?.vim?.insertMode) return;` and bails out — letting vim insert a newline instead of firing `onSubmitRef`.

This is correct for multiline editors (TextEditor, QuickCapture) but **wrong for the command palette**, which is always single-line and should never allow newlines.

### Approach

Add a new option `alwaysSubmitOnEnter: boolean` (default `false`) to `SubmitCancelOptions` in `cm-submit-cancel.ts`. When `true`, the vim Enter handler skips the insert-mode check and always fires `onSubmitRef`. Then pass `alwaysSubmitOnEnter: true` from `command-palette.tsx`.

### Files to modify

- `kanban-app/ui/src/lib/cm-submit-cancel.ts` — Add `alwaysSubmitOnEnter` option, skip insert-mode guard when set
- `kanban-app/ui/src/components/command-palette.tsx` — Pass `alwaysSubmitOnEnter: true` to `buildSubmitCancelExtensions`
- `kanban-app/ui/src/lib/cm-submit-cancel.test.ts` — Add test for new option
- `kanban-app/ui/src/components/command-palette.test.tsx` — Verify Enter executes selected command in vim mode

## Acceptance Criteria

- [ ] Pressing Enter in the command palette (vim mode) executes the selected command, never inserts a newline
- [ ] Pressing Enter in TextEditor (vim insert mode) still inserts a newline as before
- [ ] FilterEditor and other single-line editors are unaffected (no regression)
- [ ] The `alwaysSubmitOnEnter` option is documented in the `SubmitCancelOptions` interface

## Tests

- [ ] `cm-submit-cancel.test.ts`: Add test "vim insert mode + alwaysSubmitOnEnter calls onSubmitRef" — create EditorView with `alwaysSubmitOnEnter: true`, enter insert mode, dispatch Enter, assert `onSubmitRef` was called
- [ ] `cm-submit-cancel.test.ts`: Add test "vim insert mode WITHOUT alwaysSubmitOnEnter still allows newline" — existing behavior preserved
- [ ] `command-palette.test.tsx`: Add test "Enter executes selected command in vim mode" — open palette, type filter, press Enter, assert command was dispatched
- [ ] Run `npx vitest run` — all existing tests pass