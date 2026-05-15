---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffa080
title: 'CM6 editor: add onSubmit/onCancel semantic callbacks with vim-mode awareness'
---
## What

Add `onSubmit` and `onCancel` semantic callback props to `FieldPlaceholderEditor` that are vim-mode-aware. Currently, Escape/Enter handling is hardcoded inline in the extensions array. This card extracts that logic into clean callbacks that consumers wire up without knowing about vim modes.

**Key behavior:**

| Keymap | State | Key | Result |
|--------|-------|-----|--------|
| vim | insert mode | Escape | → normal mode (internal, no callback) |
| vim | normal mode | Escape | → `onCancel()` |
| vim | normal mode | Enter | → `onSubmit(text)` if non-empty |
| CUA/emacs | — | Escape | → `onCancel()` |
| CUA/emacs | — | Enter | → `onSubmit(text)` if non-empty |

**Affected files:**
- `kanban-app/ui/src/components/fields/field-placeholder.tsx` — `FieldPlaceholderEditor`: add optional `onSubmit`/`onCancel` props, refactor extension building to use them
- `kanban-app/ui/src/components/editable-markdown.tsx` — `EditableMarkdown`: same pattern for its CM6 editor (currently has near-identical inline key handling at lines 293-321)

**Approach:**
- Add optional `onSubmit?: (text: string) => void` and `onCancel?: () => void` props to `FieldPlaceholderEditor`
- Build a reusable CM6 extension factory (e.g. `buildSubmitCancelExtension(mode, onSubmitRef, onCancelRef)`) that encapsulates the vim-mode-aware Escape/Enter routing
- Both `FieldPlaceholderEditor` and `EditableMarkdown` use this shared factory
- When `onSubmit` is provided, Enter in normal mode (vim) or Enter (CUA/emacs) fires it instead of/in addition to `onCommit`
- When `onCancel` is provided, Escape in normal mode (vim) or Escape (CUA/emacs) fires it instead of `commitAndExit`
- Existing behavior (commit-on-escape in vim normal mode, commit-on-enter for single-line) is preserved as defaults when `onSubmit`/`onCancel` are not provided

## Acceptance Criteria
- [ ] `FieldPlaceholderEditor` accepts optional `onSubmit` and `onCancel` props
- [ ] In vim mode, Escape in insert mode transitions to normal mode (no callback fired)
- [ ] In vim mode, Escape in normal mode fires `onCancel` (or falls back to existing `cancelAndExit`)
- [ ] In vim mode, Enter in normal mode fires `onSubmit(text)` when text is non-empty
- [ ] In CUA/emacs mode, Escape fires `onCancel`, Enter fires `onSubmit(text)`
- [ ] `EditableMarkdown` uses the same shared extension factory
- [ ] Existing field-placeholder and editable-markdown behavior is unchanged when `onSubmit`/`onCancel` are not provided
- [ ] The shared extension factory lives in a utility (e.g. `cm-submit-cancel.ts`)

## Tests
- [ ] Unit test: `cm-submit-cancel` extension factory returns correct extensions for each keymap mode
- [ ] Manual test: edit a task field in the board — Escape/Enter behavior unchanged
- [ ] Manual test: edit a grid cell — Escape/Enter behavior unchanged
- [ ] `npm run typecheck` passes in `kanban-app/ui/`