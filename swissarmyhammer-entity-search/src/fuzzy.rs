use std::borrow::Cow;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use swissarmyhammer_entity::Entity;

use crate::result::{SearchResult, SearchStrategy};

/// Run fuzzy matching of `query` against all string field values of the given entities.
///
/// Returns results sorted by score descending, truncated to `limit`.
pub fn fuzzy_search(
    entities: &[(String, &Entity)],
    query: &str,
    limit: usize,
) -> Vec<SearchResult> {
    let matcher = SkimMatcherV2::default();
    let mut results: Vec<SearchResult> = Vec::new();

    for (id, entity) in entities {
        let mut best_score: Option<i64> = None;
        let mut best_field: Option<String> = None;

        // Match against entity id
        if let Some(score) = matcher.fuzzy_match(id, query) {
            if best_score.is_none_or(|s| score > s) {
                best_score = Some(score);
                best_field = Some("id".to_string());
            }
        }

        // Match against all string fields
        for (field_name, value) in &entity.fields {
            let text: Cow<str> = match value.as_str() {
                Some(s) => Cow::Borrowed(s),
                None => Cow::Owned(value.to_string()),
            };

            if let Some(score) = matcher.fuzzy_match(&text, query) {
                if best_score.is_none_or(|s| score > s) {
                    best_score = Some(score);
                    best_field = Some(field_name.clone());
                }
            }
        }

        if let Some(score) = best_score {
            results.push(SearchResult {
                entity_id: id.clone(),
                score: score as f64,
                strategy: SearchStrategy::Fuzzy,
                matched_field: best_field,
            });
        }
    }

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit);
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_entity(id: &str, fields: Vec<(&str, &str)>) -> (String, Entity) {
        let mut e = Entity::new("task", id);
        for (k, v) in fields {
            e.set(k, json!(v));
        }
        (id.to_string(), e)
    }

    #[test]
    fn fuzzy_finds_title_match() {
        let (id, entity) = make_entity("t1", vec![("title", "Fix login bug")]);
        let entities: Vec<(String, &Entity)> = vec![(id, &entity)];
        let results = fuzzy_search(&entities, "login", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entity_id, "t1");
        assert_eq!(results[0].matched_field.as_deref(), Some("title"));
        assert_eq!(results[0].strategy, SearchStrategy::Fuzzy);
    }

    #[test]
    fn fuzzy_returns_empty_on_no_match() {
        let (id, entity) = make_entity("t1", vec![("title", "Fix login bug")]);
        let entities: Vec<(String, &Entity)> = vec![(id, &entity)];
        let results = fuzzy_search(&entities, "zzzznotfound", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn fuzzy_respects_limit() {
        let (id1, e1) = make_entity("t1", vec![("title", "Alpha task")]);
        let (id2, e2) = make_entity("t2", vec![("title", "Alpha work")]);
        let (id3, e3) = make_entity("t3", vec![("title", "Alpha item")]);
        let entities: Vec<(String, &Entity)> = vec![(id1, &e1), (id2, &e2), (id3, &e3)];
        let results = fuzzy_search(&entities, "Alpha", 2);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn fuzzy_matches_id() {
        let (id, entity) = make_entity("my-unique-id", vec![("title", "Something else")]);
        let entities: Vec<(String, &Entity)> = vec![(id, &entity)];
        let results = fuzzy_search(&entities, "unique", 10);
        assert!(!results.is_empty());
    }

    #[test]
    fn fuzzy_picks_best_field() {
        let (id, entity) = make_entity(
            "t1",
            vec![
                ("title", "Deploy the application"),
                ("body", "deploy to production environment"),
            ],
        );
        let entities: Vec<(String, &Entity)> = vec![(id, &entity)];
        let results = fuzzy_search(&entities, "deploy", 10);
        assert_eq!(results.len(), 1);
        assert!(results[0].matched_field.is_some());
    }
}
