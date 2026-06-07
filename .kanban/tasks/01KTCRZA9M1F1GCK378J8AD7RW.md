---
assignees:
- claude-code
position_column: todo
position_ordinal: cc80
title: 'Bug: Read-only/computed fields can be entered and blanked (created-date summary + virtual tags)'
---
## What
MERGED card (2026-06-06): unifies the read-only-date-field blank bug with the virtual/dynamic-tags blank bug (`01KTCRRM1T…`, now archived) — same root cause, one Field-layer fix. A display-only/computed field, when drilled into (Enter), enters edit mode and blanks, and may never restore. A read-only/computed field should be neither focusable-for-edit nor blankable.

## Two concrete cases (both must be fixed + regression-tested)
1. **Read-only date summary** — `apps/kanban-app/ui/src/components/fields/displays/status-date-display.tsx` (created/updated date). Pressing Enter focuses in and blanks it.
2. **Computed virtual tags** — READY/BLOCKED/BLOCKING, rendered via the `virtual-badge-list` adapter (`apps/kanban-app/ui/src/components/fields/registrations/virtual-badge-list.tsx` → `displays/virtual-tag-display.tsx`), computed by the backend `VirtualTagRegistry` and surfaced via `useBoardData().virtualTagMeta`. This display has **no registered editor**, so drill-in flips to a missing/empty editor that overwrites the value and never reverts.

## Mechanism (Field layer)
Editability is governed by `apps/kanban-app/ui/src/components/fields/field.tsx`:
- Intended contract (field.tsx:464–475, 524–535): a field registers the scope-level `field.edit` Enter command **only when in display mode AND it has an `onEdit` callback**; "for non-editable fields the command is also not registered (no `onEdit`), so Enter is a no-op."
- `editCommands` (field.tsx:536) registers the drill-in/edit closure when `onEdit || spatialActions` is present (field.tsx:540) — so a display with spatial children (pills) or a wrongly-supplied `onEdit` still enters edit.

Likely causes (one fix at the Field layer covers both cases):
1. A caller (entity inspector / field row) passes `onEdit` to a field whose metadata is read-only/computed → gate `onEdit` on a read-only/computed metadata flag.
2. The display has spatial children (date pills / tag pills) so drill-in enters edit and a subsequent commit blanks.
3. No read-only flag exists in Field metadata at all → the metadata-driven UI principle requires the field metadata to declare read-only and the Field/editor to honor it (single source of truth).

The unified fix: **read-only/computed fields must not register `field.edit`, must not enter edit mode on drill-in, and must never commit an empty overwrite** — driven by field metadata, not per-call-site guesswork.

## Acceptance Criteria
- [ ] A read-only/computed field cannot enter edit mode via Enter/drill-in (Enter is a no-op or simply marks focus).
- [ ] A read-only/computed field can never be blanked — its displayed value is preserved before, during, and after any focus/Enter interaction (date stays; READY/BLOCKED/BLOCKING pills stay).
- [ ] Read-only-ness is driven by field metadata (single source of truth), not per-call-site guesswork.
- [ ] Root cause identified (caller passing `onEdit` vs. missing read-only metadata vs. spatial-children drill-in path).

## Tests
- [ ] Browser test (case 1): render the created-date / status-date-display field, press Enter → no editor opens, value unchanged (near `field.enter-edit.browser.test.tsx`).
- [ ] Browser test (case 2): drive Enter on a `virtual-badge-list` field → the READY/BLOCKED/BLOCKING pills remain rendered (extend `displays/virtual-tag-display.test.tsx`).
- [ ] Metadata test: a field marked read-only never receives/registers `field.edit`.
- [ ] Regression tests failing before the fix (fields blank), passing after.

## Workflow
- Use `/tdd` — failing test first, then fix. #bug