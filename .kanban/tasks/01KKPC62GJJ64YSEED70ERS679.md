---
position_column: done
position_ordinal: b480
title: Switch frontend search to use backend search_entities IPC
---
## What
Remove client-side fuzzy search from command-palette.tsx. The frontend should call `invoke("search_entities", { query, limit })` and display the results. The backend (EntitySearchIndex) decides fuzzy vs embedding. The frontend is just routing user input and displaying results.

**Files:**
- `kanban-app/ui/src/components/command-palette.tsx` — replace client-side EntityStore search with `invoke("search_entities")`
- `kanban-app/ui/src/components/command-palette.test.tsx` — update tests to mock the IPC call

**Remove from frontend:**
- EntityStore usage for search (useEntityStore)
- fuzzyMatch usage for search
- Schema lookups for display field resolution
- All search logic — the backend handles it

**Frontend search mode should:**
- Debounce user input (150ms)
- Call `invoke("search_entities", { query, limit: 50 })` on the backend
- Receive `[{ entity_type, entity_id, display_name, score }]`
- Display results in FocusScope with entity.inspect
- That's it

## Acceptance Criteria
- [ ] Frontend calls backend search_entities, not client-side fuzzy
- [ ] Backend decides search strategy (fuzzy vs embedding)
- [ ] Frontend only displays results from backend
- [ ] All tests pass