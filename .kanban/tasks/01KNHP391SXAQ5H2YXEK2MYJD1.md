---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffff9c80
title: 'BLOCKER: entityFromBag() drops backend moniker field'
---
**File:** `kanban-app/ui/src/types/kanban.ts` — `entityFromBag()`\n\n**What:** `entityFromBag` destructures `{ entity_type, id, ...fields }` from the backend bag but does not extract `moniker` as a top-level property. The backend's `Entity::to_json()` emits `\"moniker\"` at the top level (confirmed in `swissarmyhammer-entity/src/entity.rs`), but this function puts it into `fields.moniker` instead of the required `Entity.moniker` property.\n\n**Why:** This is the root cause of the entire migration gap. Every entity created via `entityFromBag` (which is ALL entities from `refreshBoards`, `parseBoardData`, and the inspector fetch path) has no top-level `moniker`. TypeScript currently produces 72 `TS2741` errors related to the missing moniker property.\n\n**Suggestion:** Extract `moniker` alongside `entity_type` and `id`:\n```ts\nconst { entity_type, id, moniker, ...fields } = bag;\nreturn { entity_type, id, moniker: moniker as string, fields };\n```\nAlso update the `EntityBag` type to declare `moniker: string`.\n\n**Verification:** `npx tsc --noEmit` should no longer emit `TS2741` errors for `entityFromBag` call sites. All entities loaded from backend have a truthy `entity.moniker`. #review-finding