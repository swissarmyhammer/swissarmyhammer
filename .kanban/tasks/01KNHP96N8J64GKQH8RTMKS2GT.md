---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffa980
title: 'NIT: perspective-tab-bar.tsx constructs perspective moniker by hand'
---
**File:** `kanban-app/ui/src/components/perspective-tab-bar.tsx` — CommandScopeProvider moniker\n\n**What:** Uses `moniker(\"perspective\", p.id)` where `p` is a `PerspectiveDef` (not a full Entity). PerspectiveDef does not have a `moniker` field.\n\n**Why:** Perspectives are not full entities returned by `Entity::to_json()`, so they don't have a backend moniker. The hand-construction here is the only option currently. This is a NIT because it's correct behavior — perspectives don't go through the entity system.\n\n**Suggestion:** No action needed unless perspectives are promoted to full entities. If they are, use `p.moniker`.\n\n**Verification:** Perspective tabs continue to scope commands correctly. #review-finding