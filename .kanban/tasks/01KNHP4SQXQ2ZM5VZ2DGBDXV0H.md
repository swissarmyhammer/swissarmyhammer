---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffff080
title: 'BLOCKER: quick-capture.tsx constructs Entity literal without moniker'
---
**File:** `kanban-app/ui/src/components/quick-capture.tsx` — boardEntity construction\n\n**What:** A synthetic board entity is built inline:\n```ts\n{ entity_type: \"board\", id: \"board\", fields: { name: selected.name } }\n```\nThis is missing the required `moniker` property.\n\n**Why:** This entity is passed to `BoardSelector` as `boardEntity`. If any downstream component accesses `entity.moniker`, it will be undefined.\n\n**Suggestion:** Add `moniker: \"board:board\"` to the literal, or construct via `moniker(\"board\", \"board\")`.\n\n**Verification:** TypeScript compiles without `TS2741` for this file. `boardEntity.moniker` is defined at runtime. #review-finding