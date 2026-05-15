---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffff9380
title: 'Fix failing tests: EntityInspector - field rendering, sections, markdown, tag pills, focus (8 tests)'
---
File: /Users/wballard/github/swissarmyhammer-kanban/kanban-app/ui/src/components/entity-inspector.test.tsx\n\nFailing tests:\n- EntityInspector > renders fields from schema in section order (header, body)\n- EntityInspector > groups fields into header and body sections\n- EntityInspector > renders markdown fields via Field (click enters edit mode)\n- EntityInspector > allows editing computed tag fields via multi-select\n- EntityInspector > body_field renders #tag as a styled pill when tag entity exists\n- EntityInspector > non-body markdown fields do NOT get tag pills\n- EntityInspector > first visible field has data-focused attribute by default\n- EntityInspector > only one field has data-focused at a time #test-failure