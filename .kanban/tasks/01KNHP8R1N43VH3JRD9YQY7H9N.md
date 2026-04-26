---
assignees:
- claude-code
depends_on:
- 01KNHP391SXAQ5H2YXEK2MYJD1
position_column: done
position_ordinal: ffffffffffffffffffffffffffffa780
title: 'NIT: grid-view.tsx constructs fieldMonikers from entityType+id instead of entity.moniker'
---
**File:** `kanban-app/ui/src/components/grid-view.tsx` — cellMonikerMap and cellMonikers\n\n**What:** Two sites construct field-level monikers:\n```ts\nfieldMoniker(entityType, entities[r].id, columns[c].field.name)\nfieldMoniker(entityType, entity.id, col.field.name)\n```\nThe entity objects are available in the loop.\n\n**Why:** `fieldMoniker` takes (type, id, field) and produces `type:id.field`. With entity.moniker available, this should use the entity's canonical moniker as the base to handle archive monikers correctly.\n\n**Suggestion:** Introduce a helper `fieldMonikerFrom(entity.moniker, fieldName)` that appends `.field` to an existing moniker string, or update `fieldMoniker` to accept a base moniker. Then: `fieldMonikerFrom(entity.moniker, col.field.name)`.\n\n**Verification:** Grid cell focus and editing continues to work. Cell monikers for archived entities include the archive segment. #review-finding