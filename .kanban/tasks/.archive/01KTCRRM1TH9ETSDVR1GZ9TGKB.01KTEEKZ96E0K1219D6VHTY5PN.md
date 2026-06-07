---
assignees:
- claude-code
position_column: todo
position_ordinal: c880
title: 'Bug: Entering the virtual/dynamic tags field blanks it permanently'
---
## What
Reported by user: entering (drill-in / focus-to-edit) the tag field that contains **dynamic tags** makes it go blank and it never comes back.

"Dynamic tags" = the computed **virtual tags** (READY, BLOCKED, BLOCKING). They render through the `virtual-badge-list` display adapter (`apps/kanban-app/ui/src/components/fields/registrations/virtual-badge-list.tsx`) → `VirtualTagDisplay` (`apps/kanban-app/ui/src/components/fields/displays/virtual-tag-display.tsx`). The values are computed by the backend `VirtualTagRegistry` and surfaced via `useBoardData().virtualTagMeta` — they are **display-only / not user-editable**.

Hypothesis: the field is computed and has a display adapter but **no registered editor** (see `registerEditor` / edit-mode flow in `apps/kanban-app/ui/src/components/fields/field.tsx:149,416,536`). When the user presses Enter to drill into the field (`nav.drillIn`), the field flips into edit mode and renders an empty/missing editor — or an editor that overwrites the computed value with empty — and never reverts to the display, so it shows blank permanently.

Investigate in `field.tsx`:
- Whether a computed/virtual field should be non-editable and therefore drill-in should NOT enter edit mode (it should be inert or skip).
- What happens to the rendered output when edit mode is entered for a field type that has a display but no editor.
- Whether the empty state persists in UI state (sticky edit mode) so the display never returns even after blur/Escape.

Reproduce: open a board/inspector, focus the virtual_tags field, press Enter. Observe the field blanks and does not restore on Escape/blur.

## Acceptance Criteria
- [ ] Entering the virtual/dynamic tags field does NOT blank it — either drill-in is inert on this computed field, or the display always restores after edit/blur/Escape.
- [ ] The computed virtual tags remain visible (READY/BLOCKED/BLOCKING pills) before, during, and after the interaction.
- [ ] Root cause identified (missing editor for `virtual-badge-list` vs. computed field wrongly treated as editable vs. sticky edit-mode state).

## Tests
- [ ] Component/browser test driving Enter (drill-in) on a `virtual-badge-list` field and asserting the pills remain rendered (extend `apps/kanban-app/ui/src/components/fields/displays/virtual-tag-display.test.tsx` or add a field-edit-flow test alongside `field.enter-edit.browser.test.tsx`).
- [ ] Regression test that fails before the fix (field goes blank) and passes after.

## Workflow
- Use `/tdd` — failing test first, then fix. #bug