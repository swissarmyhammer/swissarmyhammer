---
assignees:
- wballard
depends_on:
- 01KNZ3ZX03HSEYVAJFGEFTC2ZE
position_column: done
position_ordinal: ffffffffffffffffffffffbb80
project: pill-via-cm6
title: Replace SelectEditor with searchable combobox for reference fields
---
## What

Replace the Radix `<Select>` dropdown used by `position_column` (and any future scalar reference field) with a shadcn `<Combobox>` — a searchable picker with type-ahead autocomplete that calls `search_mentions` the same way `MultiSelectEditor` does. Commits the resolved entity ID.

**The problem today:** `SelectEditor` reads `field.type.options` which is designed for static YAML-defined enum lists. `position_column` is a reference field (`type.kind: reference, type.entity: column`) with no `options` array, so the dropdown is empty or broken. Even if it worked, there's no search — you scroll and click.

**Files to modify / create:**
- `kanban-app/ui/src/components/fields/editors/reference-select-editor.tsx` (new) — shadcn `Combobox` pattern:
  - `<Popover>` + `<Command>` (shadcn command palette component, which wraps cmdk)
  - `<CommandInput>` for type-ahead search
  - `<CommandList>` with `<CommandItem>` rows showing colored dot + display name
  - On open: call `search_mentions({ entityType, query: "" })` for initial list (first 20)
  - On input change: debounced `search_mentions({ entityType, query })` — same 150ms debounce as multi-select
  - On select: commit the entity ID, close the popover
  - Trigger shows the current value's display name (looked up from entity store) with a colored dot, or `-` for empty
  - Include a clear option (empty value) at the top

- `kanban-app/ui/src/components/fields/registrations/select.tsx` — change the editor registration:
  - When the field is a reference (`field.type.entity` is set), register `ReferenceSelectEditor` instead of `SelectEditor`
  - Keep `SelectEditor` for the enum/options case (dead code today, but the type system supports it)
  - Or: make `SelectEditor` detect reference vs options and delegate internally. Whichever is cleaner.

**Matching behavior:** Same as all other mention search — case-insensitive substring match on display name and ID via `search_mentions`. The user types to filter, sees display names, picks one, ID is committed.

**Keyboard:** Enter commits the highlighted item. Escape: vim → commit current, CUA/emacs → cancel. Same conventions as `MultiSelectEditor` and `TextEditor`.

**Vim/emacs mode:** The combobox input is a regular text input, not CM6. Vim mode only matters for Escape behavior (commit vs cancel). No need for vim keybindings inside the search input.

**Display of current value:** The trigger button shows `%To Do` (prefix + display name) as a MentionView pill, or a plain text label if MentionView isn't ready yet. Consistent with the display-side migration.

## Acceptance Criteria
- [ ] New `ReferenceSelectEditor` component at `components/fields/editors/reference-select-editor.tsx`
- [ ] `position_column` field uses the new editor when editing
- [ ] Typing in the search input filters columns by display name
- [ ] Selecting a column commits its entity ID
- [ ] Current value shown in trigger as display name with color
- [ ] Empty/clear option available
- [ ] Enter commits, Escape follows vim/CUA convention
- [ ] Existing `SelectEditor` still works for enum/options fields (even though none ship today)

## Tests
- [ ] `kanban-app/ui/src/components/fields/editors/reference-select-editor.test.tsx` (new) — render with 3 column entities in the store, assert all 3 appear in the dropdown
- [ ] Type a partial name in the search input, assert the list filters to matching items
- [ ] Select an item, assert `onCommit` is called with the entity ID (not the slug or display name)
- [ ] Render with a current value (entity ID), assert the trigger shows the display name
- [ ] Render with empty value, assert the trigger shows `-` or placeholder
- [ ] Run: `bun test reference-select-editor` — all pass
- [ ] Smoke: `bun run dev`, click the column field on a task card, verify searchable dropdown appears and commits correctly

## Workflow
- Use `/tdd` — write the render + search-filter test first, then the commit-ID test, then implement. Check shadcn docs for the Combobox pattern (Popover + Command) before starting.
