---
assignees:
- claude-code
position_column: todo
position_ordinal: cc80
title: 'Bug: Read-only fields (e.g. created-date summary) can be entered and blanked'
---
## What
Reported by user: pressing Enter ("enter into") a **read-only** field — e.g. the status/date summary showing the **created** date — focuses into it and blanks it out. The user raises the design question directly: a read-only field should be neither focusable-for-edit nor blankable.

The field is rendered by `apps/kanban-app/ui/src/components/fields/displays/status-date-display.tsx` (the created/updated date summary). Editability is governed by `apps/kanban-app/ui/src/components/fields/field.tsx`:
- The doc comment (field.tsx:464–475, 524–535) states the intended contract: a field registers a scope-level `field.edit` Enter command **only when in display mode AND it has an `onEdit` callback**; "for non-editable fields the command is also not registered (no `onEdit`), so Enter is a no-op." And: "For non-editable fields with no spatial children, the kernel returns null and `onEdit` is undefined → Enter is a no-op."
- `editCommands` (field.tsx:536) registers the drill-in/edit closure when `onEdit || spatialActions` is present (field.tsx:540).

So the contract says read-only fields should be inert on Enter — but in practice the created-date field is being entered and blanked. This means one of:
1. The caller (entity inspector / field row) is passing an `onEdit` callback to a field whose metadata is read-only/computed, so `field.edit` registers and opens an editor that commits an empty value. Find where `<Field … onEdit=…>` is wired (entity-inspector / inspector field rows) and gate `onEdit` on a read-only/computed metadata flag.
2. The field has spatial children (date pills) so drill-in enters edit and a subsequent commit blanks the value.
3. There is no read-only flag in the Field metadata at all, so nothing distinguishes display-only fields from editable ones — the metadata-driven UI principle requires the field metadata to declare read-only and the Field/editor to honor it.

This is the same class as the virtual/dynamic-tags blanking bug (`01KTCRRM1TH9ETSDVR1GZ9TGKB`) — both are display-only fields that enter edit mode and blank. Consider a single fix at the Field layer: read-only/computed fields must not register `field.edit`, must not enter edit mode on drill-in, and must never commit an empty overwrite.

Reproduce: open a task inspector, focus the created-date summary field, press Enter → it blanks and (per the related bug) does not restore.

## Acceptance Criteria
- [ ] A read-only/computed field cannot enter edit mode via Enter/drill-in (Enter is a no-op or simply marks focus).
- [ ] A read-only field can never be blanked — its displayed value is preserved before, during, and after any focus/Enter interaction.
- [ ] Read-only-ness is driven by field metadata (single source of truth), not per-call-site guesswork.
- [ ] Root cause identified (caller passing `onEdit` to a read-only field vs. missing read-only metadata vs. spatial-children drill-in path).

## Tests
- [ ] Component/browser test: render a read-only field (created-date / status-date-display) and assert pressing Enter does NOT open an editor and does NOT change the value (extend tests near `field.enter-edit.browser.test.tsx`).
- [ ] Metadata test: a field marked read-only never receives/registers `field.edit`.
- [ ] Regression test failing before the fix (field blanks), passing after.

## Workflow
- Use `/tdd` — failing test first, then fix. #bug