use model_embedding::{cosine_similarity, EmbeddingError, TextEmbedder};
use swissarmyhammer_entity::Entity;

use crate::error::Result;
use crate::result::{SearchResult, SearchStrategy};

/// Stored embedding for an entity's concatenated text.
#[derive(Debug, Clone)]
pub(crate) struct EntityEmbedding {
    pub entity_id: String,
    pub embedding: Vec<f32>,
}

/// Extract all searchable text from an entity by concatenating string field values.
///
/// Fields are sorted by key to ensure deterministic output across runs
/// (HashMap iteration order is non-deterministic).
pub(crate) fn extract_text(entity: &Entity) -> String {
    let mut keys: Vec<&String> = entity.fields.keys().collect();
    keys.sort();
    let mut parts = Vec::new();
    for key in keys {
        if let Some(s) = entity.fields[key].as_str() {
            if !s.is_empty() {
                parts.push(s);
            }
        }
    }
    parts.join(" ")
}

/// Build embeddings for a set of entities.
pub(crate) async fn build_embeddings(
    entities: &[(String, &Entity)],
    embedder: &impl TextEmbedder,
) -> Result<Vec<EntityEmbedding>> {
    let mut embeddings = Vec::new();
    for (id, entity) in entities {
        let text = extract_text(entity);
        if text.is_empty() {
            continue;
        }
        match embedder.embed_text(&text).await {
            Ok(result) => {
                embeddings.push(EntityEmbedding {
                    entity_id: id.clone(),
                    embedding: result.embedding().to_vec(),
                });
            }
            Err(EmbeddingError::ModelNotLoaded) => {
                tracing::warn!("embedder not loaded, skipping entity {id}");
                break;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(embeddings)
}

/// Search embeddings by cosine similarity to the query embedding.
pub(crate) fn semantic_search(
    query_embedding: &[f32],
    embeddings: &[EntityEmbedding],
    limit: usize,
) -> Vec<SearchResult> {
    let mut scored: Vec<SearchResult> = embeddings
        .iter()
        .map(|ee| {
            let sim = cosine_similarity(query_embedding, &ee.embedding);
            SearchResult {
                entity_id: ee.entity_id.clone(),
                score: sim as f64,
                strategy: SearchStrategy::Semantic,
                matched_field: None,
            }
        })
        .collect();

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(limit);
    scored
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_text_concatenates_strings() {
        let mut e = Entity::new("task", "t1");
        e.set("title", json!("Fix bug"));
        e.set("body", json!("The login page crashes"));
        e.set("priority", json!(5)); // non-string, skipped

        let text = extract_text(&e);
        assert!(text.contains("Fix bug"));
        assert!(text.contains("login page crashes"));
        assert!(!text.contains("5"));
    }

    #[test]
    fn extract_text_empty_entity() {
        let e = Entity::new("task", "t1");
        assert!(extract_text(&e).is_empty());
    }

    #[test]
    fn semantic_search_ranks_by_similarity() {
        let embeddings = vec![
            EntityEmbedding {
                entity_id: "close".into(),
                embedding: vec![0.9, 0.1, 0.0],
            },
            EntityEmbedding {
                entity_id: "far".into(),
                embedding: vec![0.0, 0.0, 1.0],
            },
        ];
        let query = vec![1.0, 0.0, 0.0];
        let results = semantic_search(&query, &embeddings, 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].entity_id, "close");
        assert_eq!(results[1].entity_id, "far");
        assert!(results[0].score > results[1].score);
        assert_eq!(results[0].strategy, SearchStrategy::Semantic);
    }
}
