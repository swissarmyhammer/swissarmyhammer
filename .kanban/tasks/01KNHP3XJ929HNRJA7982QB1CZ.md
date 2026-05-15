---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffff9d80
title: 'BLOCKER: rust-engine-container entity-created fast path omits moniker'
---
**File:** `kanban-app/ui/src/components/rust-engine-container.tsx` — entity-created listener, fast path\n\n**What:** When the `entity-created` event carries fields, the handler constructs an Entity literal directly:\n```ts\nconst entity: Entity = { id, entity_type, fields: fields as Record<string, unknown> };\n```\nThis is missing the required `moniker` property. The event payload (`EntityCreatedEvent`) does not carry a moniker either.\n\n**Why:** Entities injected into the store via this path will have `undefined` for `entity.moniker`. Any component that reads `entity.moniker` (the whole point of the migration) will get undefined.\n\n**Suggestion:** Either (a) add `moniker` to the `EntityCreatedEvent` payload and use it, or (b) construct it client-side as fallback: `moniker: moniker(entity_type, id)`. Option (a) is preferred since the backend already computes it.\n\n**Verification:** Create an entity via the UI, confirm the resulting Entity in the store has a truthy `moniker` property matching the backend format. #review-finding