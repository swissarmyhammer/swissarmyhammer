---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffff680
title: 'Fix entity-inspector.test.tsx failures (8 tests): schema loading stuck at "Loading schema..."'
---
EntityInspector renders "Loading schema..." and never resolves — tests cannot find field rows like [data-testid="field-row-title"].\n\nRoot cause: SchemaContext mock returns null for entity types causing `TypeError: Cannot read properties of null (reading 'map')` in `loadSchemas` at `src/lib/schema-context.tsx:56`.\n\nFailing tests:\n- renders fields from schema in section order (header, body)\n- groups fields into header and body sections (no [data-testid=\"inspector-header\"] or inspector-body)\n- renders markdown fields via Field (click enters edit mode)\n- allows editing computed tag fields via multi-select\n- body_field renders #tag as a styled pill when tag entity exists\n- non-body markdown fields do NOT get tag pills\n- first visible field has data-focused attribute by default\n- only one field has data-focused at a time\n\nFiles: `/Users/wballard/github/swissarmyhammer-kanban/kanban-app/ui/src/components/entity-inspector.test.tsx`, `/Users/wballard/github/swissarmyhammer-kanban/kanban-app/ui/src/lib/schema-context.tsx`"