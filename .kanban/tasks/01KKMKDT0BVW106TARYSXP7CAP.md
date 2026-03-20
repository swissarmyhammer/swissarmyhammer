---
depends_on:
- 01KKMKCX7JMCC9MW9FHE593V1J
position_column: done
position_ordinal: '8680'
title: Implement semantic embedding search over Entities
---
## What
Create `swissarmyhammer-entity-search/src/semantic.rs` — embedding-based semantic search over Entity objects using `TextEmbedder` trait from `model-embedding` and `model_embedding::cosine_similarity`.

Implementation:
- Store embeddings as `HashMap<EntityId, Vec<f32>>` (entity id → vector)
- `build_embeddings(entities, embedder)` — for each entity, concatenate all string field values + body, embed the result
- `semantic_search(query_embedding, stored_embeddings, limit) -> Vec<SearchResult>`
- Use `model_embedding::cosine_similarity()` (from card 0)
- Return top-k by descending similarity, with min_similarity threshold (0.3)

## Acceptance Criteria
- [ ] `build_embeddings()` produces one embedding per entity
- [ ] `semantic_search()` returns entities ranked by cosine similarity
- [ ] Results have `strategy: Semantic` and normalized scores
- [ ] Handles case where embedder is not loaded gracefully

## Tests
- [ ] `test_semantic_search_basic` — mock embeddings, verify ranking
- [ ] `test_semantic_search_threshold` — results below threshold excluded
- [ ] `test_semantic_search_empty_index` — returns empty for no entities
- [ ] `cargo nextest run -p swissarmyhammer-entity-search semantic`