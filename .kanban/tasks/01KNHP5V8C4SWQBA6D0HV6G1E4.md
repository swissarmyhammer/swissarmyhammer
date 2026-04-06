---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffff980
title: 'WARNING: command-palette.tsx constructs monikers from SearchResult fields'
---
**File:** `kanban-app/ui/src/components/command-palette.tsx` — three sites\n\n**What:** Three call sites construct monikers from `SearchResult` fields:\n1. `moniker(result.entity_type, result.entity_id)` — as React key\n2. `moniker(result.entity_type, result.entity_id)` — as inspect target\n3. `moniker(result.entity_type, result.entity_id)` — as FocusScope moniker\n\n`SearchResult` comes from the backend `search_entities` command and does not carry a `moniker` field — only `entity_type`, `entity_id`, `display_name`, `score`.\n\n**Why:** For non-archived entities this produces the correct moniker. For archived entities it would produce `task:id` instead of `task:id:archive`. This is a backend API gap — `SearchResult` should include the canonical moniker.\n\n**Suggestion:** Add a `moniker` field to the backend `search_entities` response. On the frontend, use `result.moniker` when available, falling back to `moniker(result.entity_type, result.entity_id)`.\n\n**Verification:** Search for an archived entity, inspect it, and confirm the correct moniker is used. #review-finding