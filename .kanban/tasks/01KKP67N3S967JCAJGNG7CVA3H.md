---
position_column: done
position_ordinal: a480
title: Enrich EntitySearchIndex as the in-memory search context with embedding cache
---
## What
Extend `EntitySearchIndex` (not a new wrapper type) to serve as the in-memory search context. It already holds `entities: HashMap<String, Entity>` and `embeddings: Vec<EntityEmbedding>`. Add embedding staleness tracking so individual entity changes invalidate only that entity's embedding, with lazy rebuild.

**Files:**
- `swissarmyhammer-entity-search/src/index.rs` — extend EntitySearchIndex
- `swissarmyhammer-entity-search/src/lib.rs` — re-exports

**Additions to EntitySearchIndex:**
- `stale_ids: HashSet<String>` — tracks entities whose embeddings are out of date
- `.update()` already exists — enhance to also mark the entity's embedding as stale (add id to `stale_ids`)
- `.remove()` already exists — already clears embeddings, also remove from stale set
- `.rebuild_stale_embeddings(embedder)` — async, re-embeds only stale entities instead of rebuilding all
- `.search_hybrid()` already exists — before searching, optionally rebuild stale embeddings (or just fuzzy-fall-back if stale)
- Bulk init: `::from_entities(entities: Vec<Entity>) -> Self` convenience constructor

The index IS the context — no extra wrapper needed. It's the single object mounted in BoardHandle behind `RwLock<EntitySearchIndex>`.

## Acceptance Criteria
- [ ] `EntitySearchIndex::from_entities()` bulk-loads entities
- [ ] `.update()` marks embedding as stale without full rebuild
- [ ] `.remove()` cleans up both entity and embedding state
- [ ] `.rebuild_stale_embeddings()` only re-embeds changed entities
- [ ] Fuzzy search works immediately regardless of embedding state
- [ ] Thread-safe when wrapped in RwLock (no internal mutexes)

## Tests
- [ ] Unit test: from_entities, search, update entity, search again
- [ ] Unit test: stale tracking — update marks stale, rebuild clears
- [ ] Unit test: remove entity clears from stale set too
- [ ] `cargo nextest run -p swissarmyhammer-entity-search`