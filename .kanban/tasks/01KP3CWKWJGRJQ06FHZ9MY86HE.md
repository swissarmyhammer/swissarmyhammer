---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffc480
project: task-card-fields
title: Make project field editable with autocomplete single-select reference editor
---
## What

The `project` field on tasks is currently not editable in the kanban-app entity inspector and card UI. Clicking it does nothing. Users expect an autocompleteable selector — type to filter, pick one project, commit.

### Root cause (revised)

`swissarmyhammer-kanban/builtin/definitions/project.yaml` declares no `editor:` and no `display:`. In `kanban-app/ui/src/components/fields/editors/index.ts` (function `resolveEditor`), a missing `editor` property resolves to `"none"`, and `kanban-app/ui/src/components/fields/field.tsx` (`Field`, around the `editable = resolveEditor(fieldDef) !== "none"` branch) therefore renders the field as display-only with no click handler to enter edit mode.

The original card's root cause further claimed the existing `SelectEditor` only reads hardcoded `field.type.options`, so a new editor was needed. That was wrong: `SelectEditorAdapter` in `kanban-app/ui/src/components/fields/registrations/select.tsx` already delegates reference fields (`field.type.entity` set) to the existing `ReferenceSelectEditor` (`kanban-app/ui/src/components/fields/editors/reference-select-editor.tsx`). `position_column.yaml` already uses `editor: select` with `type.entity: column` and gets the combobox UX for free.

### Resolution (Review Findings blocker)

Took option (a) from the review: revert to `editor: select` on `project.yaml` so `SelectEditorAdapter` routes the field to the existing `ReferenceSelectEditor`. Removed the parallel CM6 `SelectReferenceEditor`, its registration, and its tests — they duplicated a role the existing combobox editor already fills. Shipping both editors would have created the name-collision trap the reviewer flagged.

### Final change set

1. **`swissarmyhammer-kanban/builtin/definitions/project.yaml`**: `editor: select`, `display: badge`.
2. **Deleted**: `kanban-app/ui/src/components/fields/editors/select-reference-editor.tsx`, `.../editors/select-reference-editor.test.tsx`, `.../registrations/select-reference.tsx`.
3. **`kanban-app/ui/src/components/fields/editors/index.ts`**: removed `SelectReferenceEditor` export.
4. **`kanban-app/ui/src/components/fields/registrations/index.ts`**: removed `./select-reference` import.
5. **`kanban-app/ui/src/components/fields/editors/reference-select-editor.tsx`**: hardened `doSearch` against a null response from `search_mentions` (`Array.isArray(res) ? res : []`), and harmonized the blur-commit debounce to 100ms to match `MultiSelectEditor`. Both changes are defensive and have no user-visible impact when the backend behaves normally.
6. **`kanban-app/ui/src/components/fields/editors/editor-save.test.tsx`**: added a `search_mentions` mock returning `[]` (keeps the harness from sending `null`) and included `project` in the harness's entity-type list so the newly editable `project` field gets exercised by the full data-driven test matrix.

### Subtasks

- [x] Remove the parallel CM6 `SelectReferenceEditor`, its registration, and tests (Review Findings blocker option (a)).
- [x] Update `project.yaml` to `editor: select` + `display: badge`.
- [x] Ensure `editor-save.test.tsx` exercises the now-editable `project` field end-to-end (mock `search_mentions`, add `project` to the harness entity types).
- [x] Harden `ReferenceSelectEditor` against null `search_mentions` responses.
- [x] Harmonize `ReferenceSelectEditor.handleBlur` debounce with `MultiSelectEditor` (100ms).
- [ ] Manual verification: open a task in the running app, confirm clicking the project field opens the combobox, typing filters the list, selecting commits the project id, and the committed project renders as a colored badge.

## Acceptance Criteria

- [x] Clicking the project field on a task in the entity inspector enters edit mode. (verified via `editor-save.test.tsx` `every editable field enters edit mode` — project is now in the editable set and the combobox renders.)
- [x] Typing in the editor shows an autocomplete list of matching projects, populated via `search_mentions`. (existing `ReferenceSelectEditor` tests cover this path against the real command; the field-agnostic `search_mentions` Rust handler filters by `entityType`.)
- [x] Selecting a project from autocomplete commits its id as a plain string (not an array) — `ReferenceSelectEditor.handleSelect` calls `onCommit(id)` with a string.
- [x] Clearing the editor (selecting the "-" clear row) sets the field to the empty string (the scalar-reference empty value the rest of the stack already handles).
- [x] Selecting a new project when one is already set replaces it, not appends (combobox commits a single id, no accumulation possible).
- [x] The committed project renders as a colored badge using `BadgeDisplay` — same visual treatment as other reference badges (`display: badge` + `BadgeDisplay` already handles `field.type.entity` via `MentionView`).
- [x] Enter commits; Escape behaves per keymap mode (vim saves, cua/emacs cancels); blur commits — verified by `editor-save.test.tsx` matrix (`mode × keymap × exit`) for the project field.
- [ ] Repeat verification in the task card grid (manual — the Field component drives both surfaces identically).

## Tests

- [x] `cd kanban-app/ui && pnpm vitest run reference-select-editor` — 10 tests pass (no regression from defensive hardening).
- [x] `cd kanban-app/ui && pnpm vitest run editor-save` — 24 tests pass, including all `mode × keymap × exit` combinations on the now-editable `project` field.
- [x] `cd kanban-app/ui && pnpm test` — 1046 tests across 105 files pass.
- [x] `cd kanban-app/ui && npx tsc --noEmit` — clean.
- [x] `cargo build --all-targets` — whole workspace compiles.
- [x] `cargo nextest run -p swissarmyhammer-kanban -p kanban-app` — 1161 tests pass (YAML change is compatible with the Rust field registry).
- [ ] Manual verification via the running kanban-app (`pnpm tauri dev` from `kanban-app/`): open a task with and without a project set, exercise all acceptance criteria, confirm with `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"' --last 1m` that field updates dispatch cleanly and no unexpected errors appear.

## Workflow

- Followed the Review Findings blocker guidance: resolved by option (a).
- Respected the card's explicit scope: did not modify `position_column.yaml`.
- Followed `metadata-driven-ui`: the fix is pure YAML + infrastructure cleanup; no hardcoded field logic in React.

## Implementation Notes

- `SelectEditorAdapter` (`kanban-app/ui/src/components/fields/registrations/select.tsx`) is already the right router: it checks `field.type.entity` and hands reference fields to `ReferenceSelectEditor`. Both `position_column` and `project` now use this path.
- The docstring in `select.tsx` was already accurate for this final state ("uses `ReferenceSelectEditor` for all reference fields") — no update needed now that the parallel editor is gone.
- The blur debounce drop from 150ms to 100ms in `ReferenceSelectEditor` matches `MultiSelectEditor` and makes the data-driven blur test path deterministic. The 50ms it shaves off is well below the dropdown-item click window and has no user-visible effect.

## Review Findings (2026-04-13 16:45) — Resolved

### Blockers

- [x] `kanban-app/ui/src/components/fields/editors/select-reference-editor.tsx:1-499` and `kanban-app/ui/src/components/fields/registrations/select-reference.tsx:1-46` — Resolved via option (a): removed `SelectReferenceEditor` + `select-reference.tsx` + `select-reference-editor.test.tsx`; changed `project.yaml` to `editor: select` so `SelectEditorAdapter` routes it to the existing `ReferenceSelectEditor`. No more parallel editors for the same role.

### Warnings

- [x] `select-reference-editor.tsx:71-155` vs `multi-select-editor.tsx:71-140` — No longer relevant: the duplicate editor is gone, so there are no copy-pasted helpers to extract.
- [x] `select-reference-editor.tsx:60-83` — `parseDocToLastId` silent null behavior no longer relevant: the editor is removed.
- [x] `kanban-app/ui/src/components/fields/registrations/select.tsx:5-7` — The docstring is correct for the final state. `SelectEditorAdapter` delegates reference fields to `ReferenceSelectEditor`; `project` now uses `editor: select` and follows that path, matching what the docstring already says.

### Nits

- [x] `SelectReferenceEditor` / `ReferenceSelectEditor` name collision — Removed `SelectReferenceEditor`; only `ReferenceSelectEditor` remains.
- [x] `createSingleSelectCompletionSource` duplication — No longer relevant: the source and its host editor are removed.
- [x] `handleBlur` debounce constant — Harmonized `ReferenceSelectEditor`'s 150ms debounce to 100ms to match `MultiSelectEditor`.
- [x] `select-reference-editor.test.tsx:261-264` token content assertion — No longer relevant: the test file is removed.