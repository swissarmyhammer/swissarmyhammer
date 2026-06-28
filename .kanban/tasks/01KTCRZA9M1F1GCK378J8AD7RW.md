---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff9080
title: 'Bug: Read-only/computed fields can be entered and blanked (created-date summary + virtual tags)'
---
## What
MERGED card (2026-06-06): unifies the read-only-date-field blank bug with the virtual/dynamic-tags blank bug (`01KTCRRM1T…`, now archived) — same root cause, one Field-layer fix. A display-only/computed field, when drilled into (Enter), enters edit mode and blanks, and may never restore. A read-only/computed field should be neither focusable-for-edit nor blankable.

## Two concrete cases (both must be fixed + regression-tested)
1. **Read-only date summary** — `apps/kanban-app/ui/src/components/fields/displays/status-date-display.tsx` (created/updated date). Pressing Enter focuses in and blanks it.
2. **Computed virtual tags** — READY/BLOCKED/BLOCKING, rendered via the `virtual-badge-list` adapter (`apps/kanban-app/ui/src/components/fields/registrations/virtual-badge-list.tsx` → `displays/virtual-tag-display.tsx`), computed by the backend `VirtualTagRegistry` and surfaced via `useBoardData().virtualTagMeta`. This display has **no registered editor**, so drill-in flips to a missing/empty editor that overwrites the value and never reverts.

## Root cause (found during implementation)
The metadata existed all along (`editor: none` in `status_date.yaml` / `virtual_tags.yaml`), but the `<Field>` interpreter ignored it:
- `entity-card.tsx`'s `CardFields` passes `onEdit` **unconditionally** to every card field (no editability check, unlike the inspector's `FieldRow`).
- `field.tsx`'s `field.edit` Enter closure fell through to `onEdit?.()`, arming `editing` for an `editor: none` field.
- With `editing=true`, `FieldEditor` resolves `editorRegistry.get("none")` → `undefined` → renders `null` — the value blanks, and with no editor mounted nothing can ever fire `onDone`/`onCancel`, so it never restores.

## Fix (one metadata-driven gate in the interpreter)
- New `isFieldEditable(field)` in `fields/editors/index.ts` (single source of truth: `resolveEditor(field) !== "none"`).
- `field.tsx`'s `Field` gates once at the top: `onEdit` is dropped and `editing` is ignored when the metadata says non-editable — covers Enter, click-to-edit, and any caller that arms `editing` directly (card, grid, inspector).
- `entity-inspector.tsx`'s private duplicate `isEditable` removed; it now consumes the shared `isFieldEditable`.
- No Rust change needed — the YAML schema already declares `editor: none` for computed fields.

## Acceptance Criteria
- [x] A read-only/computed field cannot enter edit mode via Enter/drill-in (Enter is a no-op or simply marks focus).
- [x] A read-only/computed field can never be blanked — its displayed value is preserved before, during, and after any focus/Enter interaction (date stays; READY/BLOCKED/BLOCKING pills stay).
- [x] Read-only-ness is driven by field metadata (single source of truth), not per-call-site guesswork.
- [x] Root cause identified (caller passing `onEdit` vs. missing read-only metadata vs. spatial-children drill-in path) — it was the caller passing `onEdit` + the interpreter honoring armed `editing` despite `editor: none`.

## Tests
- [x] Browser test (case 1): render the status-date field, press Enter → no edit, value unchanged (`fields/field.read-only.browser.test.tsx`, harness mirrors `field.enter-edit.browser.test.tsx`).
- [x] Browser test (case 2): drive Enter on a `virtual-badge-list` field → READY/BLOCKING pills remain rendered (`field.read-only.browser.test.tsx`, with the `useBoardData` stub from `displays/virtual-tag-display.test.tsx`).
- [x] Metadata test: a field marked read-only never arms `field.edit` — `onEdit` is dropped by the metadata gate even when the caller wires it unconditionally (cardfields-shaped harness).
- [x] Regression tests failing before the fix (all 3 RED: onEdit armed + value blanked), passing after (GREEN: 3/3; full field suite 66/66; tsc clean). The 29 failures in inspector/grid suites are pre-existing on this branch — verified identical with the fix reverted to HEAD.

## Workflow
- Use `/tdd` — failing test first, then fix. #bug