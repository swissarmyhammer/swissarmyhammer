use std::collections::{HashMap, HashSet};

use model_embedding::TextEmbedder;
use swissarmyhammer_entity::Entity;

use crate::error::Result;
use crate::fuzzy::fuzzy_search;
use crate::result::SearchResult;
use crate::semantic::{self, EntityEmbedding};

/// In-memory search index over Entity objects.
///
/// Supports fuzzy matching for short queries and semantic (embedding-based)
/// search for longer queries, with automatic fallback.
pub struct EntitySearchIndex {
    entities: HashMap<String, Entity>,
    embeddings: Vec<EntityEmbedding>,
    stale_ids: HashSet<String>,
}

impl EntitySearchIndex {
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
            embeddings: Vec::new(),
            stale_ids: HashSet::new(),
        }
    }

    /// Bulk-load entities into a new index.
    pub fn from_entities(entities: Vec<Entity>) -> Self {
        let mut index = Self::new();
        for entity in entities {
            index.add(entity);
        }
        index
    }

    /// Add an entity to the index. Replaces any existing entity with the same id.
    pub fn add(&mut self, entity: Entity) {
        let id = entity.id.to_string();
        self.embeddings.retain(|e| e.entity_id != id);
        self.stale_ids.insert(id.clone());
        self.entities.insert(id, entity);
    }

    /// Remove an entity from the index.
    pub fn remove(&mut self, id: &str) {
        self.entities.remove(id);
        self.embeddings.retain(|e| e.entity_id != id);
        self.stale_ids.remove(id);
    }

    /// Update an entity (convenience wrapper around add).
    pub fn update(&mut self, entity: Entity) {
        self.add(entity);
    }

    /// Merge fields into an existing entity without replacing it entirely.
    ///
    /// If the entity exists, each field in `fields` is inserted/overwritten
    /// on the existing entity and its embedding is marked stale. If the entity
    /// does not exist, creates a new one from the provided type, id, and fields.
    pub fn merge_fields(
        &mut self,
        entity_type: &str,
        id: &str,
        fields: &std::collections::HashMap<String, serde_json::Value>,
    ) {
        if let Some(existing) = self.entities.get_mut(id) {
            for (k, v) in fields {
                existing.set(k, v.clone());
            }
            self.embeddings.retain(|e| e.entity_id != id);
            self.stale_ids.insert(id.to_string());
        } else {
            let mut entity = Entity::new(entity_type, id);
            for (k, v) in fields {
                entity.set(k, v.clone());
            }
            self.add(entity);
        }
    }

    /// Build or rebuild embeddings for all indexed entities.
    pub async fn build_embeddings(&mut self, embedder: &impl TextEmbedder) -> Result<()> {
        let entity_refs: Vec<(String, &Entity)> = self
            .entities
            .iter()
            .map(|(id, e)| (id.clone(), e))
            .collect();
        self.embeddings = semantic::build_embeddings(&entity_refs, embedder).await?;
        self.stale_ids.clear();
        Ok(())
    }

    /// Re-embed only entities whose embeddings are stale.
    ///
    /// After a successful rebuild, the stale set is cleared.
    pub async fn rebuild_stale_embeddings(&mut self, embedder: &impl TextEmbedder) -> Result<()> {
        if self.stale_ids.is_empty() {
            return Ok(());
        }

        // Collect stale entities
        let stale_refs: Vec<(String, &Entity)> = self
            .stale_ids
            .iter()
            .filter_map(|id| self.entities.get(id).map(|e| (id.clone(), e)))
            .collect();

        if stale_refs.is_empty() {
            self.stale_ids.clear();
            return Ok(());
        }

        // Build new embeddings for stale entities
        let new_embeddings = semantic::build_embeddings(&stale_refs, embedder).await?;

        // Remove old embeddings for stale entities, add new ones
        let stale_set: HashSet<&str> = self.stale_ids.iter().map(|s| s.as_str()).collect();
        self.embeddings
            .retain(|e| !stale_set.contains(e.entity_id.as_str()));
        self.embeddings.extend(new_embeddings);

        self.stale_ids.clear();
        Ok(())
    }

    /// Number of entities with stale embeddings.
    pub fn stale_count(&self) -> usize {
        self.stale_ids.len()
    }

    /// Fuzzy search over entity fields.
    ///
    /// For short queries (<=3 words), this is the primary search method.
    /// Falls back to semantic search if fuzzy returns no results and embeddings are available.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let entity_refs: Vec<(String, &Entity)> = self
            .entities
            .iter()
            .map(|(id, e)| (id.clone(), e))
            .collect();
        fuzzy_search(&entity_refs, query, limit)
    }

    /// Semantic search using embeddings.
    ///
    /// Embeds the query and finds entities with the most similar embeddings.
    /// Returns empty if no embeddings have been built.
    pub async fn search_semantic(
        &self,
        query: &str,
        embedder: &impl TextEmbedder,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        if self.embeddings.is_empty() {
            return Ok(Vec::new());
        }
        let query_result = embedder.embed_text(query).await?;
        let results = semantic::semantic_search(query_result.embedding(), &self.embeddings, limit);
        Ok(results)
    }

    /// Hybrid search: uses strategy based on query length, with fallback.
    ///
    /// - Short query (<=3 words): fuzzy first, fall back to semantic if no results
    /// - Long query (>3 words): semantic first (if embeddings available), fall back to fuzzy
    pub async fn search_hybrid(
        &self,
        query: &str,
        embedder: &impl TextEmbedder,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let word_count = query.split_whitespace().count();
        let has_embeddings = !self.embeddings.is_empty();

        if word_count <= 3 {
            let fuzzy_results = self.search(query, limit);
            if !fuzzy_results.is_empty() || !has_embeddings {
                return Ok(fuzzy_results);
            }
            self.search_semantic(query, embedder, limit).await
        } else {
            if has_embeddings {
                let semantic_results = self.search_semantic(query, embedder, limit).await?;
                if !semantic_results.is_empty() {
                    return Ok(semantic_results);
                }
            }
            Ok(self.search(query, limit))
        }
    }

    /// Number of entities in the index.
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Whether embeddings have been built.
    pub fn has_embeddings(&self) -> bool {
        !self.embeddings.is_empty()
    }

    /// Look up an entity by id.
    pub fn get(&self, id: &str) -> Option<&Entity> {
        self.entities.get(id)
    }
}

impl Default for EntitySearchIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn task(id: &str, title: &str) -> Entity {
        let mut e = Entity::new("task", id);
        e.set("title", json!(title));
        e
    }

    #[test]
    fn add_and_search() {
        let mut idx = EntitySearchIndex::new();
        idx.add(task("t1", "Fix the login page"));
        idx.add(task("t2", "Add dashboard widgets"));
        idx.add(task("t3", "Login timeout handling"));

        let results = idx.search("login", 10);
        assert!(!results.is_empty());
        let ids: Vec<&str> = results.iter().map(|r| r.entity_id.as_str()).collect();
        assert!(ids.contains(&"t1"));
        assert!(ids.contains(&"t3"));
    }

    #[test]
    fn remove_entity() {
        let mut idx = EntitySearchIndex::new();
        idx.add(task("t1", "Fix login"));
        assert_eq!(idx.len(), 1);

        idx.remove("t1");
        assert_eq!(idx.len(), 0);
        assert!(idx.search("login", 10).is_empty());
    }

    #[test]
    fn update_entity() {
        let mut idx = EntitySearchIndex::new();
        idx.add(task("t1", "Old title"));
        idx.update(task("t1", "New title entirely different"));

        assert_eq!(idx.len(), 1);
        let new_results = idx.search("New title", 10);
        assert!(!new_results.is_empty());
    }

    #[test]
    fn empty_index_returns_no_results() {
        let idx = EntitySearchIndex::new();
        assert!(idx.is_empty());
        assert!(idx.search("anything", 10).is_empty());
    }

    #[test]
    fn has_embeddings_initially_false() {
        let idx = EntitySearchIndex::new();
        assert!(!idx.has_embeddings());
    }

    #[test]
    fn from_entities_bulk_load() {
        let entities = vec![task("t1", "Fix login"), task("t2", "Dashboard widgets")];
        let idx = EntitySearchIndex::from_entities(entities);
        assert_eq!(idx.len(), 2);
        assert!(!idx.search("login", 10).is_empty());
    }

    #[test]
    fn update_marks_stale() {
        let mut idx = EntitySearchIndex::new();
        idx.add(task("t1", "Old title"));
        assert_eq!(idx.stale_count(), 1);
        // Stale count stays the same for the same id
        idx.update(task("t1", "New title"));
        assert_eq!(idx.stale_count(), 1);
        idx.add(task("t2", "Another"));
        assert_eq!(idx.stale_count(), 2);
    }

    #[test]
    fn remove_clears_stale() {
        let mut idx = EntitySearchIndex::new();
        idx.add(task("t1", "Fix login"));
        assert_eq!(idx.stale_count(), 1);
        idx.remove("t1");
        assert_eq!(idx.stale_count(), 0);
    }

    #[test]
    fn default_creates_empty_index() {
        let idx = EntitySearchIndex::default();
        assert!(idx.is_empty());
        assert!(!idx.has_embeddings());
        assert_eq!(idx.len(), 0);
        assert_eq!(idx.stale_count(), 0);
    }

    #[test]
    fn get_returns_entity() {
        let mut idx = EntitySearchIndex::new();
        idx.add(task("t1", "Fix login"));
        assert!(idx.get("t1").is_some());
        assert_eq!(idx.get("t1").unwrap().id, "t1");
        assert!(idx.get("nonexistent").is_none());
    }

    #[test]
    fn merge_fields_updates_existing_entity() {
        let mut idx = EntitySearchIndex::new();
        idx.add(task("t1", "Old title"));
        // Clear stale so we can verify merge marks it stale again
        assert_eq!(idx.stale_count(), 1);

        let mut fields = std::collections::HashMap::new();
        fields.insert("title".to_string(), json!("New title"));
        fields.insert("priority".to_string(), json!(3));
        idx.merge_fields("task", "t1", &fields);

        assert_eq!(idx.len(), 1);
        let entity = idx.get("t1").unwrap();
        assert_eq!(entity.fields["title"], json!("New title"));
        assert_eq!(entity.fields["priority"], json!(3));
        // Should be stale after merge
        assert_eq!(idx.stale_count(), 1);
    }

    #[test]
    fn merge_fields_creates_new_entity_when_missing() {
        let mut idx = EntitySearchIndex::new();
        assert!(idx.is_empty());

        let mut fields = std::collections::HashMap::new();
        fields.insert("title".to_string(), json!("Brand new"));
        idx.merge_fields("task", "t99", &fields);

        assert_eq!(idx.len(), 1);
        let entity = idx.get("t99").unwrap();
        assert_eq!(entity.fields["title"], json!("Brand new"));
        assert_eq!(idx.stale_count(), 1);
    }

    #[tokio::test]
    async fn build_embeddings_and_search_semantic() {
        use model_embedding::mock::MockEmbedder;

        let mut idx = EntitySearchIndex::new();
        idx.add(task("t1", "Fix login"));
        idx.add(task("t2", "Dashboard widgets"));
        assert!(!idx.has_embeddings());

        let embedder = MockEmbedder::new(4);
        idx.build_embeddings(&embedder).await.unwrap();

        assert!(idx.has_embeddings());
        assert_eq!(idx.stale_count(), 0);

        let results = idx.search_semantic("login", &embedder, 10).await.unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn search_semantic_empty_embeddings_returns_empty() {
        use model_embedding::mock::MockEmbedder;

        let idx = EntitySearchIndex::new();
        let embedder = MockEmbedder::new(4);
        let results = idx
            .search_semantic("anything", &embedder, 10)
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn rebuild_stale_embeddings_noop_when_none_stale() {
        use model_embedding::mock::MockEmbedder;

        let mut idx = EntitySearchIndex::new();
        let embedder = MockEmbedder::new(4);
        // No entities at all — should be a no-op
        idx.rebuild_stale_embeddings(&embedder).await.unwrap();
        assert_eq!(idx.stale_count(), 0);
    }

    #[tokio::test]
    async fn rebuild_stale_embeddings_updates_only_stale() {
        use model_embedding::mock::MockEmbedder;

        let mut idx = EntitySearchIndex::new();
        idx.add(task("t1", "Fix login"));
        idx.add(task("t2", "Dashboard widgets"));

        let embedder = MockEmbedder::new(4);
        idx.build_embeddings(&embedder).await.unwrap();
        assert_eq!(idx.stale_count(), 0);
        assert!(idx.has_embeddings());

        // Update one entity to mark it stale
        idx.update(task("t1", "Fix logout instead"));
        assert_eq!(idx.stale_count(), 1);

        idx.rebuild_stale_embeddings(&embedder).await.unwrap();
        assert_eq!(idx.stale_count(), 0);
        assert!(idx.has_embeddings());
    }

    #[tokio::test]
    async fn rebuild_stale_with_removed_entity_clears_stale() {
        use model_embedding::mock::MockEmbedder;

        let mut idx = EntitySearchIndex::new();
        idx.add(task("t1", "Fix login"));

        let embedder = MockEmbedder::new(4);
        idx.build_embeddings(&embedder).await.unwrap();

        // Manually insert a stale id that has no entity (simulates race)
        idx.stale_ids.insert("ghost".to_string());
        assert_eq!(idx.stale_count(), 1);

        // The stale entity doesn't exist, so stale_refs will be empty
        // after filtering — should still clear
        idx.rebuild_stale_embeddings(&embedder).await.unwrap();
        assert_eq!(idx.stale_count(), 0);
    }

    #[tokio::test]
    async fn search_hybrid_short_query_fuzzy_first() {
        use model_embedding::mock::MockEmbedder;

        let mut idx = EntitySearchIndex::new();
        idx.add(task("t1", "Fix login"));
        idx.add(task("t2", "Dashboard widgets"));

        let embedder = MockEmbedder::new(4);
        // No embeddings built — short query should return fuzzy results
        let results = idx.search_hybrid("login", &embedder, 10).await.unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].entity_id, "t1");
    }

    #[tokio::test]
    async fn search_hybrid_short_query_falls_back_to_semantic() {
        use model_embedding::mock::MockEmbedder;

        let mut idx = EntitySearchIndex::new();
        idx.add(task("t1", "Fix login"));

        let embedder = MockEmbedder::new(4);
        idx.build_embeddings(&embedder).await.unwrap();

        // Short query with no fuzzy match should fall back to semantic
        let results = idx
            .search_hybrid("zzzznotfound", &embedder, 10)
            .await
            .unwrap();
        // Semantic will still return results (mock returns constant embeddings)
        // since embeddings exist
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn search_hybrid_long_query_semantic_first() {
        use model_embedding::mock::MockEmbedder;

        let mut idx = EntitySearchIndex::new();
        idx.add(task("t1", "Fix the login page bug"));

        let embedder = MockEmbedder::new(4);
        idx.build_embeddings(&embedder).await.unwrap();

        // Long query (>3 words) — should use semantic first
        let results = idx
            .search_hybrid("fix the login page bug now", &embedder, 10)
            .await
            .unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn search_hybrid_long_query_no_embeddings_uses_fuzzy() {
        use model_embedding::mock::MockEmbedder;

        let mut idx = EntitySearchIndex::new();
        idx.add(task("t1", "Fix the login page bug"));

        let embedder = MockEmbedder::new(4);
        // No embeddings built — long query (>3 words) falls back to fuzzy
        let results = idx
            .search_hybrid("Fix the login page", &embedder, 10)
            .await
            .unwrap();
        // Even with >3 words, fuzzy fallback should find a match
        // since the query is a substring of the title
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn search_hybrid_long_query_semantic_empty_falls_back_to_fuzzy() {
        use model_embedding::mock::MockEmbedder;

        let mut idx = EntitySearchIndex::new();
        idx.add(task("t1", "Fix the login page bug"));

        let embedder = MockEmbedder::new(4);
        idx.build_embeddings(&embedder).await.unwrap();

        // Remove embeddings manually to simulate empty semantic results
        // while has_embeddings still reflects the cleared state
        idx.embeddings.clear();

        // has_embeddings is false now, so it will go straight to fuzzy
        let results = idx
            .search_hybrid("fix the login page", &embedder, 10)
            .await
            .unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn build_embeddings_error_propagates() {
        use model_embedding::mock::MockEmbedder;

        let mut idx = EntitySearchIndex::new();
        idx.add(task("t1", "Fix login"));

        // Fail on first call
        let embedder = MockEmbedder::with_failures(4, vec![0]);
        let result = idx.build_embeddings(&embedder).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn rebuild_stale_embeddings_error_propagates() {
        use model_embedding::mock::MockEmbedder;

        let mut idx = EntitySearchIndex::new();
        idx.add(task("t1", "Fix login"));

        // Build successfully first
        let good_embedder = MockEmbedder::new(4);
        idx.build_embeddings(&good_embedder).await.unwrap();

        // Mark t1 stale
        idx.update(task("t1", "Updated login"));

        // Rebuild with failing embedder
        let bad_embedder = MockEmbedder::with_failures(4, vec![0]);
        let result = idx.rebuild_stale_embeddings(&bad_embedder).await;
        assert!(result.is_err());
    }
}
