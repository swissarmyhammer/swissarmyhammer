---
assignees:
- claude-code
depends_on:
- 01KM5JYNBZW86RNHCVS9VMZ31D
- 01KM5JYX09RBFTQFD2T715FAEW
position_column: todo
position_ordinal: '8280'
title: Update mention infrastructure for slugified task titles
---
## What
The mention system assumes display field values are already slugs (e.g. tag_name "my-tag"). Task titles have spaces ("Fix Login Bug") so we need to slugify them at every touch point. Import `slugify()` from the previous card and apply it.

### Files to modify:

**1. `kanban-app/ui/src/components/editable-markdown.tsx`** (lines 88-130)
- `buildColorMap()`: key = `slugify(displayField value)` instead of raw value
- `buildMetaMap()`: key = `slugify(displayField value)`
- `mentionData.slugs`: map through `slugify()`
- `buildAsyncSearch()`: set `slug: slugify(r.display_name)`, keep `displayName: r.display_name`

**2. `kanban-app/ui/src/components/mention-pill.tsx`** (line 34-39)
- Entity resolution: also compare `slugify(getStr(e, field))` against the slug prop
- This lets `^fix-login-bug` resolve to the entity with title "Fix Login Bug"

**3. `kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx`** (lines 78-95, 147-170)
- `idToDisplay`: store slugified display name as value (for pill labels)
- `displayToId`: key on slugified name
- Search results: `slug: slugify(r.display_name)`, `displayName: r.display_name`

**4. `kanban-app/ui/src/lib/cm-mention-autocomplete.ts`** (line 49)
- No change needed — `label` already uses `r.slug` and `detail` uses `r.displayName`
- The slugified slug flows through from the search functions

### Important: backward compatibility
- For tags/actors where display fields are already slugs, `slugify("my-tag") === "my-tag"` — no behavior change
- Only task titles (with spaces) produce different slugified values

## Acceptance Criteria
- [ ] Typing `^` in the body editor triggers task autocomplete
- [ ] Selecting a task inserts `^fix-login-bug` (slugified)
- [ ] `^fix-login-bug` is decorated with colored pill in CM6 editor
- [ ] Display mode renders `^fix-login-bug` as a MentionPill with context menu
- [ ] Typing `^` in the depends_on multi-select editor triggers task autocomplete
- [ ] Existing `#tag` and `@actor` mentions still work identically

## Tests
- [ ] Manual: create two tasks, mention one in the other's body with `^`
- [ ] Manual: edit depends_on field, type `^`, verify autocomplete shows tasks
- [ ] Verify `#tag` mentions still decorate and render correctly (no regression)