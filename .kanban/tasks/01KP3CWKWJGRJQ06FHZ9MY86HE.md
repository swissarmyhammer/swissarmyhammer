---
assignees:
- claude-code
position_column: todo
position_ordinal: c480
project: task-card-fields
title: Make project field editable with autocomplete single-select reference editor
---
## What

The `project` field on tasks is currently not editable in the kanban-app entity inspector and card UI. Clicking it does nothing. Users expect an autocompleteable selector — type to filter, pick one project, commit.

### Root cause

`swissarmyhammer-kanban/builtin/definitions/project.yaml` declares no `editor:` and no `display:`. In `kanban-app/ui/src/components/fields/editors/index.ts` (function `resolveEditor`, ~line 35), a missing `editor` property resolves to `"none"`, and `kanban-app/ui/src/components/fields/field.tsx` (`Field`, around the `editable = resolveEditor(fieldDef) !== "none"` branch) therefore renders the field as display-only with no click handler to enter edit mode.

Even if we declare `editor: select`, the existing `SelectEditor` (`kanban-app/ui/src/components/fields/editors/select-editor.tsx`) only reads hardcoded `field.type.options` — it cannot populate options from the dynamic list of project entities in the entity store. And `MultiSelectEditor` commits `string[]`, which is wrong for a single-select field (`type.multiple: false` per project.yaml).

### Approach

Add a new `select-reference` editor for single-valued reference fields, modeled on `MultiSelectEditor` but constrained to one token:

1. **New editor**: `kanban-app/ui/src/components/fields/editors/select-reference-editor.tsx`
   - CM6-based like `multi-select-editor.tsx`, but the doc holds at most one mention token.
   - Reads `field.type.entity` to discover the target entity type (e.g. `"project"`).
   - Uses `useSchema()` to get `mentionableTypes` (prefix + displayField + slugField).
   - Uses the existing `search_mentions` Tauri command for autocomplete (already registered — see `kanban-app/src/commands.rs` and `kanban-app/src/main.rs`).
   - Reuses `createMentionCompletionSource`, `createMentionAutocomplete`, `createMentionDecorations` from `kanban-app/ui/src/lib/cm-mention-autocomplete.ts` and `kanban-app/ui/src/lib/cm-mention-decorations.ts`.
   - On commit: parses the single token, resolves to entity id, calls `onCommit(id | null)` — commits a **string**, not an array. Empty input commits `null`.
   - Selecting an autocomplete item should replace any existing token (not append).
   - Honors vim/cua keymap conventions via `buildSubmitCancelExtensions` and `useUIState` (match MultiSelectEditor semantics: Enter always commits; Escape → vim saves, cua/emacs cancels; blur commits).

2. **Register it**: extend `kanban-app/ui/src/components/fields/registrations/select.tsx` (or create `kanban-app/ui/src/components/fields/registrations/select-reference.tsx` if keeping registrations one-per-editor matches project convention — check sibling files).
   - `registerEditor("select-reference", SelectReferenceEditorAdapter)`
   - Wire `FieldEditorProps` → `SelectReferenceEditor` props (field, value, onCommit, onCancel, onChange, mode).

3. **Export from index**: add `export { SelectReferenceEditor } from "./select-reference-editor";` in `kanban-app/ui/src/components/fields/editors/index.ts`.

4. **Update field YAML**: `swissarmyhammer-kanban/builtin/definitions/project.yaml` — add:
   ```yaml
   editor: select-reference
   display: badge
   ```
   (`display: badge` uses the existing `BadgeDisplay` which already handles reference fields correctly via `resolveReferenceBadge` at `kanban-app/ui/src/components/fields/displays/badge-display.tsx`.)

### Why not reuse existing editors

- `SelectEditor` (Radix Select) — no autocomplete, options are static.
- `MultiSelectEditor` — commits `string[]`; wrong value shape for single-select. Also, UX lets user accumulate tokens; we want exactly one.

### Why not install shadcn Combobox / cmdk

The kanban-app deliberately standardizes on CM6 for all prefix autocomplete (actors, tags, dependent tasks, mention pills) — see `kanban-app/ui/src/lib/cm-mention-autocomplete.ts`. Adding cmdk fragments the UX. The CM6 path already has all the infrastructure (debounced search, mention pills, vim/cua keymaps, color resolution).

### Scope boundaries

- Do **not** change `swissarmyhammer-kanban/builtin/definitions/position_column.yaml` in this card. Its `editor: select` works because columns are a static enum — no dynamic reference lookup needed.
- Do **not** touch the Rust side. `search_mentions` already works for `project` (project entity has `mention_prefix: "$"` in `swissarmyhammer-kanban/builtin/entities/project.yaml`).
- Do **not** refactor `MultiSelectEditor` to share code speculatively. If natural shared helpers fall out (e.g. a shared `buildColorMap` or token-resolution helper), extract them; otherwise keep the two editors parallel.

### Subtasks

- [ ] Write failing tests for `SelectReferenceEditor` (see Tests section)
- [ ] Create `kanban-app/ui/src/components/fields/editors/select-reference-editor.tsx`
- [ ] Register `select-reference` editor and export it from editors/index.ts
- [ ] Update `swissarmyhammer-kanban/builtin/definitions/project.yaml` with `editor: select-reference` and `display: badge`
- [ ] Verify in the running app: clicking the project field in the task inspector opens an autocomplete, typing filters projects, picking one commits the project id, reopening the field shows the same project as a colored badge

## Acceptance Criteria

- [ ] Clicking the project field on a task in the entity inspector enters edit mode.
- [ ] Typing in the editor shows an autocomplete list of matching projects, populated via `search_mentions`.
- [ ] Selecting a project from autocomplete commits its id as a plain string (not an array) — verify via entity store / JSONL changelog.
- [ ] Clearing the editor and committing sets the field to `null` (or equivalent empty).
- [ ] Selecting a new project when one is already set replaces it, not appends.
- [ ] The committed project renders as a colored badge using `BadgeDisplay` — same visual treatment as other reference badges.
- [ ] Enter commits; Escape behaves per keymap mode (vim saves, cua/emacs cancels); blur commits — matching `MultiSelectEditor` semantics.
- [ ] Repeat verification in the task card grid (if the project column is shown there), not only the inspector.

## Tests

- [ ] Add `kanban-app/ui/src/components/fields/editors/select-reference-editor.test.tsx` modeled on `multi-select-editor.test.tsx`. Cover:
  - Renders with empty value → doc is empty, placeholder visible.
  - Renders with existing project id → doc shows `${prefix}${slug}` token, mention pill decoration applied.
  - Autocomplete source fires on typing, calls `search_mentions` with `entityType: "project"`.
  - Selecting an autocomplete item replaces any existing token.
  - Commit calls `onCommit(id)` with a **string**, not an array.
  - Empty commit calls `onCommit(null)`.
  - Escape in vim mode commits; Escape in cua mode cancels.
  - Blur commits the current selection.
- [ ] Run: `cd kanban-app/ui && pnpm test select-reference-editor` — all tests pass.
- [ ] Run: `cd kanban-app/ui && pnpm test multi-select-editor` — still green (no regression).
- [ ] Run: `cd kanban-app/ui && pnpm typecheck` — no errors.
- [ ] Manual verification via the running kanban-app (`pnpm tauri dev` from `kanban-app/`): open a task with and without a project set, exercise all acceptance criteria, confirm with `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"' --last 1m` that field updates dispatch cleanly and no unexpected errors appear.

## Workflow

- Use `/tdd` — write the failing `select-reference-editor.test.tsx` first, then implement the editor to make it pass, then the YAML wire-up.
- Follow the `metadata-driven-ui` feedback memory: no hardcoded field logic in React — the editor reads everything it needs from `field.type.entity` and the schema.
- Follow the `frontend-logging` feedback memory: use `console.warn` or `@tauri-apps/plugin-log` for instrumentation; check `log show` yourself after manual verification, do not ask the user.
