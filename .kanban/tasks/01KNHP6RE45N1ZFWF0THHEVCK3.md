---
assignees:
- claude-code
depends_on:
- 01KNHP391SXAQ5H2YXEK2MYJD1
position_column: done
position_ordinal: ffffffffffffffffffffffffffffa280
title: 'WARNING: mention-pill.tsx constructs moniker from entityType+entityId instead of entity.moniker'
---
**File:** `kanban-app/ui/src/components/mention-pill.tsx` — scopeMoniker computation\n\n**What:** The FocusScope moniker is computed as:\n```ts\nconst scopeMoniker = focusMoniker ?? moniker(entityType, entityId);\n```\nwhere `entityId = entity?.id ?? slug`. The entity object IS resolved from the store, so `entity.moniker` is available when entity is found.\n\n**Why:** For non-archived entities the result is the same. For archived entities or any future moniker format that diverges from `type:id`, this will produce incorrect monikers.\n\n**Suggestion:** When `entity` is resolved and has a truthy `moniker`, use it:\n```ts\nconst scopeMoniker = focusMoniker ?? entity?.moniker ?? moniker(entityType, entityId);\n```\n\n**Verification:** Right-click a mention pill, confirm commands target the correct moniker. #review-finding