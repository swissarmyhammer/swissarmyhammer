---
position_column: done
position_ordinal: ffff8580
title: 'WARNING: ViewDef interfaces lack readonly modifiers'
---
ui/src/types/kanban.ts:5-26\n\nViewDef, ViewCommand, ViewCommandKeys are response objects with mutable properties.\n\nFix: add readonly to all properties.