---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: Replace plain <input> with TextEditor (CM6) for perspective tab rename
---
## What

`kanban-app/ui/src/components/perspective-tab-bar.tsx:233-244` uses a plain `<input>` for inline perspective rename. This breaks vim mode — Escape calls `onRenameCancel()` (discards the edit) instead of committing the value.

The fix: replace the `<input>` with the `TextEditor` CM6 component from `@/components/fields/text-editor`. `TextEditor` already handles vim semantics correctly:
- Escape from insert → normal mode: `saveInPlace` fires `onChange` (preserves text)
- Escape from normal mode: `semanticCancel` → `commitAndExit` → `onCommit` (commits and exits in vim mode)
- Enter: `semanticSubmit` → `onCommit` (commits and exits)
- Blur: fires `onChange` (preserves text)

### Files to modify

- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — Replace `<input>` block (lines 233-244) with `<TextEditor>`. Wire props:
  - `value={renameValue}`
  - `onCommit={(text) => { setRenameValue(text); commitRename(p.id, p.name); }}` — or refactor `commitRename` to accept the new name directly instead of reading `renameValue` state
  - `onCancel={onRenameCancel}`
  - `onChange={onRenameChange}` — keeps local state in sync for debounce
  - `popup={false}` — starts in normal mode for vim
  - Remove `inputRef` (no longer needed — CM6 manages its own focus)
  - Remove `onRenameChange` from `PerspectiveTabProps` if no longer needed as a separate prop

### Design note

`commitRename` currently reads `renameValue` from React state via closure. Since `TextEditor.onCommit` passes the final text directly, refactor `commitRename` to accept the new name as a parameter instead of relying on stale state.

## Acceptance Criteria
- [ ] Perspective tab rename uses CM6 TextEditor, not a plain `<input>`
- [ ] In vim mode: Escape from insert mode preserves text (goes to normal), Escape from normal commits the rename
- [ ] In CUA/emacs mode: Escape cancels (discards), Enter commits
- [ ] Blur commits the rename (same as field editing)
- [ ] Double-click on tab opens inline CM6 editor with the current name

## Tests
- [ ] Update `kanban-app/ui/src/components/perspective-tab-bar.test.tsx` — existing rename tests should pass with the new editor
- [ ] Add test: double-click tab renders CM6 editor (assert `.cm-editor` presence instead of `<input>`)
- [ ] Run `pnpm vitest run perspective-tab-bar` — all tests pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#bug