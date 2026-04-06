---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffb580
title: 'WARNING: QuickCapture constructs board entity with hardcoded field name ''name'''
---
**File**: kanban-app/ui/src/components/quick-capture.tsx (boardEntity construction)\n\n**What**: The QuickCapture component constructs a synthetic board entity: `{ entity_type: 'board', id: 'board', fields: { name: selected.name } }`. This hardcodes the field name `name` rather than using the schema's `search_display_field`.\n\n**Why**: This violates the metadata-driven-ui principle. If the board entity's display field changes in the YAML schema, this code silently breaks. BoardSelector already resolves `search_display_field` correctly — the problem is in QuickCapture's synthetic entity construction.\n\n**Suggestion**: Use the actual board entity from the entity store instead of constructing a synthetic one, or at minimum look up `search_display_field` from the schema.\n\n**Subtasks**:\n- [ ] Replace synthetic entity with actual board entity from entity store, or use schema lookup\n- [ ] Verify fix by running tests #review-finding