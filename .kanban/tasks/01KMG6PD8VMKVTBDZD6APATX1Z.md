---
assignees:
- claude-code
position_column: todo
position_ordinal: 7f80
title: Fix markdown checkbox toggling in Field display mode
---
## What

Regression introduced in `d772aaed` ("refactor(field): wire Field into inspector, grid, data-table, entity-card — delete cells/"). Before this commit, entity-inspector's `FieldDispatch` had a special case for markdown fields that rendered `EditableMarkdown` directly — which has working checkbox toggling via `toggleCheckbox()` and `handleCheckboxChange`. After the refactor, all fields route through the generic `Field` component → display registry → `MarkdownDisplayAdapter`, which never passes `onCommit` to `MarkdownDisplay`. The TODO at `registrations/markdown.tsx:33` acknowledges the gap.

**Root cause chain:**
1. `FieldDisplayProps` (`field.tsx:34`) has no `onCommit`
2. `Field` (`field.tsx:153–156`) never passes a commit handler to the display
3. `MarkdownDisplayAdapter` (`registrations/markdown.tsx:33`) has `// TODO: onCommit for checkbox toggling`
4. `MarkdownDisplay.handleCheckboxChange` (`markdown-display.tsx:115`) guards `if (!onCommit) return` — bails silently

**Fix (2 files):**
1. `kanban-app/ui/src/components/fields/field.tsx` — add `onCommit?: (value: unknown) => void` to `FieldDisplayProps`; create a display-only commit handler (calls `updateField` but NOT `onDone` — checkbox toggle must not exit display mode); pass it to `<Display>`
2. `kanban-app/ui/src/components/fields/registrations/markdown.tsx` — forward `onCommit` from `FieldDisplayProps` to `MarkdownDisplay`; remove the TODO comment

## Acceptance Criteria
- [ ] Clicking an unchecked `- [ ]` checkbox in a markdown field's display mode toggles it to `- [x]` and persists via `updateField`
- [ ] Clicking a checked `- [x]` checkbox toggles it to `- [ ]` and persists
- [ ] Clicking a checkbox does NOT enter edit mode (no CM6 editor appears)
- [ ] Clicking non-checkbox markdown content still enters edit mode as before
- [ ] Existing `EditableMarkdown` checkbox tests still pass

## Tests
- [ ] New file `kanban-app/ui/src/components/fields/displays/markdown-display.test.tsx`: render `MarkdownDisplay` in full mode with `- [ ] task\n- [x] done`, click first checkbox, assert `onCommit` called with `- [x] task\n- [x] done`
- [ ] Same file: click second checkbox (checked→unchecked), assert `onCommit` called with `- [ ] task\n- [ ] done`
- [ ] Same file: render with 5 checkboxes, click the 3rd, assert only 3rd toggled
- [ ] Same file: click checkbox, assert parent `onClick` spy is NOT called (stopPropagation works)
- [ ] Run `cd kanban-app/ui && npx vitest run` — all tests pass