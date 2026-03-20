---
position_column: done
position_ordinal: 8f80
title: O(n) linear scan in embeddings.retain on every add/remove
---
swissarmyhammer-entity-search/src/index.rs:31,38\n\n`self.embeddings.retain(|e| e.entity_id != id)` does an O(n) scan over all embeddings on every `add()` and `remove()` call. With tens to hundreds of entities (as stated in the plan), this is fine. But if the index grows, consider a `HashMap<String, Vec<f32>>` for embeddings instead of `Vec<EntityEmbedding>` for O(1) removal.\n\nAcceptable for current scale — noting for future reference.