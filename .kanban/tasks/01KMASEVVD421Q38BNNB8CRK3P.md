---
assignees:
- claude-code
depends_on: []
position_column: done
position_ordinal: ffffffffffffff9480
title: 'data-table.tsx: hardcoded type-kind checks for sort and cell padding'
---
**File:** `kanban-app/ui/src/components/data-table.tsx:302,436-446`\n\nTwo issues:\n1. Line 302: `field.type.kind !== \"color\" && field.type.kind !== \"date\"` — hardcodes cell padding behavior based on type kind during editing\n2. Lines 436-446: Sort comparison hardcodes `kind === \"number\"` for numeric sort, `kind === \"date\"` for lexicographic sort — should use `field.sort` property (the Rust backend already resolves `effective_sort()`). #field-special-case