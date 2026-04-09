---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffef80
title: 'BLOCKER: rust-engine-container entity-field-changed upsert omits moniker'
---
**File:** `kanban-app/ui/src/components/rust-engine-container.tsx` — entity-field-changed listener, upsert path\n\n**What:** When a field-changed event arrives for an entity not yet in the store (race recovery), the handler constructs:\n```ts\nreturn [...next, { entity_type, id, fields }];\n```\nThis Entity literal is missing the required `moniker` property.\n\n**Why:** Same impact as the entity-created finding — entities upserted via this race-recovery path lack a moniker.\n\n**Suggestion:** Add `moniker: moniker(entity_type, id)` (importing from `@/lib/moniker`) or propagate moniker from the event payload if the backend is updated to include it.\n\n**Verification:** Trigger a field-changed event before entity-created arrives (simulated in tests), confirm the upserted entity has a truthy `moniker`. #review-finding