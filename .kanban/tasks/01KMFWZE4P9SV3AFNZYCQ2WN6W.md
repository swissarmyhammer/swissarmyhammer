---
assignees:
- claude-code
position_column: todo
position_ordinal: c480
title: 'Fix: board card fields start in vim insert mode instead of normal mode'
---
## What

When clicking a field on a board card in vim mode, the CodeMirror editor starts in **insert mode** instead of **normal mode**. The user expects to land in normal mode (standard vim behavior for clicking into an editor).

**Root cause:** `TextEditor.handleCreateEditor` (`kanban-app/ui/src/components/fields/text-editor.tsx:163`) uses `onSubmit` as the signal to auto-enter insert mode. But `MarkdownEditorAdapter` (`kanban-app/ui/src/components/fields/registrations/markdown.tsx:21`) passes `onSubmit` in compact mode just to make Enter commit — it does NOT intend to trigger auto-insert.

The `popup` prop already exists as the correct signal for "auto-enter insert mode" — `quick-capture.tsx` passes it. The fix is to check `popup` instead of `onSubmit` in the `handleCreateEditor` callback.

**File to modify:** `kanban-app/ui/src/components/fields/text-editor.tsx`

**Change:** In `handleCreateEditor` (line ~157-191), replace the `if (onSubmit)` guard (line 163) with `if (popup)`. This makes auto-insert-mode only trigger for popup contexts (quick-capture), while board card fields and grid cells will correctly start in normal mode.

The `else` branch (lines 182-188) that forces normal mode will then cover both board cards and grid cells — which is the desired behavior.

## Acceptance Criteria
- [ ] Clicking a markdown field on a board card in vim mode starts the editor in normal mode
- [ ] Quick-capture popup still auto-enters insert mode (regression check)
- [ ] Grid cell editing still starts in normal mode (no regression)
- [ ] Pressing `i` in the board card editor enters insert mode as expected

## Tests
- [ ] Update `kanban-app/ui/src/lib/cm-submit-cancel.test.ts` — add test: TextEditor with `onSubmit` but no `popup` starts in vim normal mode
- [ ] Add test: TextEditor with `popup={true}` starts in vim insert mode
- [ ] Run `cargo nextest run` and `npx vitest run` — all pass