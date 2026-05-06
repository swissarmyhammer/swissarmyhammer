---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffff80
title: 'entity-card.tsx: hardcoded body_field for progress rendering'
---
**File:** `kanban-app/ui/src/components/entity-card.tsx:47,167-169`\n\nReads `schema.entity.body_field` and passes it to SubtaskProgress so it can parse checkboxes from the body. The progress display component should know which field to read from its own configuration (e.g. the compute engine already knows this), not via hardcoded wiring in the card component. #field-special-case
this is my editing  -- editing -- eee - and it is #power
#test