---
position_column: done
position_ordinal: ffffe580
title: CM6 multi-select editor for assignees field
---
Build a CM6-based multi-select editor for reference fields. Adapts behavior based on whether target entity type has a mention prefix.

## Two modes based on target entity type

**With mention prefix** (assignees → actor has `@`):
- CM6 shows `@Display-Name` pills with async autocomplete via search_mentions
- Restricted mode: only valid `@slug` tokens accepted, free text rejected
- On commit: parse `@slug` tokens → resolve to entity IDs

**Without mention prefix** (depends_on → task has no prefix):
- CM6 shows plain search input, autocomplete triggers on any text (no prefix char)
- Selected items render as plain pills (task title)
- On commit: resolve typed text to entity IDs

## How it discovers mention behavior
1. Read `field.type.entity` → e.g. `"actor"`
2. Look up entity type in schema context → `mention_prefix: "@"`, `mention_display_field: "name"`
3. If prefix exists: use prefix-based CM6 editing
4. If no prefix: use plain search-and-select mode

## Storage & resolution
- Stores array of entity IDs: `["01ABC...", "01DEF..."]`
- Bidirectional: ID → display name (render), display name → ID (commit)
- Grid cell: shows AvatarDisplay/BadgeList as popover trigger, popover has CM6 editor

## Files
- New `ui/src/components/fields/editors/multi-select-editor.tsx`
- Modified: `editors/index.ts`, `cell-editor.tsx`, `entity-inspector.tsx`

## Subtasks
- [ ] Create MultiSelectEditor with dual-mode (prefix vs plain search)
- [ ] Read target entity type's mention config from schema context
- [ ] Implement ID ↔ display-name resolution via cached maps
- [ ] Prefix mode: restricted @slug input with decorations
- [ ] Plain mode: search-and-select for entities without prefix
- [ ] Wire into resolveEditor, CellEditor, inspector
- [ ] Grid: display-as-trigger + popover pattern
- [ ] Run `npm test`