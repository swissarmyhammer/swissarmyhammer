---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffff9280
title: 'App.tsx: hardcoded entity type checks and tag_name lookup'
---
**File:** `kanban-app/ui/src/App.tsx`\n\nMultiple issues:\n- Lines 319,354,380-386: Hardcoded `entity_type === \"column\"`, `\"swimlane\"`, `\"board\"` checks to trigger structural refreshes. Should use a schema-declared property (e.g. `structural: true`) on entity definitions.\n- Line 664: `activeView.kind === \"board\"` hardcoded view kind check.\n- Lines 705-707: `entry.entityType === \"tag\"` with `getStr(e, \"tag_name\")` — hardcodes tag lookup by name field. Should use the entity's `search_display_field` or `mention_display_field` from schema.\n- Line 713: `entry.entityType === \"board\"` for special board entity resolution. #field-special-case