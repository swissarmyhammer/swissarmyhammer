---
depends_on:
- 01KKMKDE3KJ28Y6CJ83FH14E7Z
- 01KKMKDT0BVW106TARYSXP7CAP
position_column: done
position_ordinal: '8980'
title: Implement EntitySearchIndex with hybrid fuzzy→semantic fallback
---
## What
Create `swissarmyhammer-entity-search/src/index.rs` — the `EntitySearchIndex` struct that owns indexed entities + embeddings and provides unified `search()` with automatic strategy selection.

Implementation:
- `EntitySearchIndex` holds: `Vec<Entity>` (or refs), `HashMap<EntityId, Vec<f32>>` embeddings, `SkimMatcherV2`
- `add(entity)`, `remove(id)`, `update(entity)` — manage entity collection, invalidate embedding on update
- `build_embeddings(&mut self, embedder)` — compute embeddings for entities missing them
- `search(query, limit) -> Vec<SearchResult>`:
  - Short query (≤3 words): fuzzy only
  - Long query (>3 words): semantic (if embeddings available)
  - Fallback: if fuzzy returns 0 results and embeddings exist, try semantic
- `search_semantic(query, embedder, limit)` — force semantic path

## Acceptance Criteria
- [ ] `search()` with short query uses fuzzy strategy
- [ ] `search()` with long query uses semantic strategy when embeddings available
- [ ] `search()` falls back to semantic when fuzzy returns no results
- [ ] Gracefully degrades to fuzzy-only when no embeddings built
- [ ] `add/remove/update` maintain index consistency

## Tests
- [ ] `test_search_short_query_uses_fuzzy`
- [ ] `test_search_fallback_to_semantic`
- [ ] `test_search_no_embeddings_fuzzy_only`
- [ ] `test_index_crud`
- [ ] `cargo nextest run -p swissarmyhammer-entity-search index`