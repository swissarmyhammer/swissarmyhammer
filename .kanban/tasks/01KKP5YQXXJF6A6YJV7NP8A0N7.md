---
position_column: done
position_ordinal: a380
title: Add search_display_field to EntityDef
---
## What
Add a `search_display_field` property to `EntityDef` (in `swissarmyhammer-fields/src/types.rs`) that controls which field is shown in search results. Analogous to `mention_display_field` but for the search palette. Falls back to `mention_display_field`, then to "name", then to "title".

The search palette will use this field to determine what text to display for each entity in results — NO field captions/labels shown, just the value.

**Files:**
- `swissarmyhammer-fields/src/types.rs` — add `search_display_field: Option<FieldName>` to `EntityDef`
- `swissarmyhammer-kanban/builtin/fields/entities/*.yaml` — set `search_display_field` for each entity type:
  - task.yaml: `search_display_field: title`
  - tag.yaml: `search_display_field: tag_name`
  - actor.yaml: `search_display_field: name`
  - column.yaml: `search_display_field: name`
  - swimlane.yaml: `search_display_field: name`
  - board.yaml: `search_display_field: name`
- `kanban-app/ui/src/types/kanban.ts` — add to EntitySchema type
- `kanban-app/ui/src/components/command-palette.tsx` — use schema's `search_display_field` to resolve display text

**Approach:**
- Add optional field with serde skip_serializing_if, same pattern as `mention_display_field`
- Frontend resolves: `search_display_field ?? mention_display_field ?? "name"` as fallback chain
- Search results show entity type prefix (muted) + display field value (no field caption)

## Acceptance Criteria
- [ ] `search_display_field` is a valid YAML key in entity definitions
- [ ] All builtin entity YAMLs have it configured
- [ ] Search results display the configured field's value, not its caption
- [ ] Fallback chain works when `search_display_field` is not set

## Tests
- [ ] `cargo nextest run` passes (types.rs round-trip tests)
- [ ] Frontend schema context picks up the new field