---
depends_on:
- 01KKP67YRRDJ2134XKEGW7DCX3
position_column: done
position_ordinal: a280
title: Add search_entities Tauri command
---
## What
Expose the search index to the frontend via a Tauri command. The frontend calls `invoke("search_entities", { query, limit })` and gets back search results with entity type, id, display name, and score.

**Files:**
- `kanban-app/src/commands.rs` — add `search_entities` command
- `kanban-app/src/main.rs` — register in `generate_handler!`

**Approach:**
- `search_entities(state, query, limit)` → reads search context from active BoardHandle
- Calls `search_ctx.search(query, limit)` (fuzzy for now, hybrid later when embeddings are available)
- For each SearchResult, resolve the entity's display name using `search_display_field` from entity schema
- Returns `[{ entity_type, entity_id, display_name, score }]`

## Acceptance Criteria
- [ ] `invoke("search_entities", { query: "foo", limit: 20 })` returns matching entities
- [ ] Results include entity_type, entity_id, display_name, score
- [ ] Empty query returns top entities (or empty — TBD)
- [ ] Returns error if no board open

## Tests
- [ ] `cargo nextest run -p kanban-app`
- [ ] Manual: call from devtools console