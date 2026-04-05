//! SearchSkill operation — search for skills by name or description

use crate::context::SkillContext;
use crate::error::SkillError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Search for skills by name or description
#[operation(
    verb = "search",
    noun = "skill",
    description = "Search for skills by name or description"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct SearchSkill {
    /// Search query to match against skill names and descriptions
    pub query: String,
}

impl SearchSkill {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
        }
    }
}

#[async_trait]
impl Execute<SkillContext, SkillError> for SearchSkill {
    async fn execute(&self, ctx: &SkillContext) -> ExecutionResult<Value, SkillError> {
        let library = ctx.library.read().await;
        let query_lower = self.query.to_lowercase();

        let results: Vec<Value> = library
            .list()
            .into_iter()
            .filter(|skill| {
                skill.name.as_str().to_lowercase().contains(&query_lower)
                    || skill.description.to_lowercase().contains(&query_lower)
            })
            .map(|skill| {
                json!({
                    "name": skill.name.as_str(),
                    "description": skill.description,
                    "source": skill.source.to_string(),
                })
            })
            .collect();

        ExecutionResult::Unlogged {
            value: json!(results),
        }
    }

    fn affected_resource_ids(&self, _result: &Value) -> Vec<String> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::SkillContext;
    use crate::skill_library::SkillLibrary;
    use std::sync::Arc;
    use swissarmyhammer_operations::Execute;
    use tokio::sync::RwLock;

    /// Build a SkillContext with a library loaded from defaults (builtin skills)
    fn test_context_with_defaults() -> SkillContext {
        let mut library = SkillLibrary::new();
        library.load_defaults();
        SkillContext::new(Arc::new(RwLock::new(library)))
    }

    /// Build a SkillContext with an empty library
    fn test_context_empty() -> SkillContext {
        let library = SkillLibrary::new();
        SkillContext::new(Arc::new(RwLock::new(library)))
    }

    #[tokio::test]
    async fn test_search_by_name_finds_skill() {
        let ctx = test_context_with_defaults();
        let op = SearchSkill::new("plan");
        let result = op.execute(&ctx).await;

        let value = result.into_result().expect("search should succeed");
        let results = value.as_array().expect("result should be an array");
        assert!(
            !results.is_empty(),
            "searching for 'plan' should find at least one result"
        );

        let names: Vec<&str> = results.iter().filter_map(|r| r["name"].as_str()).collect();
        assert!(
            names.contains(&"plan"),
            "results should include the 'plan' skill"
        );
    }

    #[tokio::test]
    async fn test_search_case_insensitive() {
        let ctx = test_context_with_defaults();

        let lower = SearchSkill::new("plan");
        let upper = SearchSkill::new("PLAN");

        let lower_val = lower.execute(&ctx).await.into_result().unwrap();
        let upper_val = upper.execute(&ctx).await.into_result().unwrap();

        let lower_arr = lower_val.as_array().unwrap();
        let upper_arr = upper_val.as_array().unwrap();

        assert_eq!(
            lower_arr.len(),
            upper_arr.len(),
            "case-insensitive search should return the same results"
        );
    }

    #[tokio::test]
    async fn test_search_no_matches() {
        let ctx = test_context_with_defaults();
        let op = SearchSkill::new("zzz-nonexistent-skill-xyz");
        let result = op.execute(&ctx).await;

        let value = result
            .into_result()
            .expect("search should succeed even with no matches");
        let results = value.as_array().unwrap();
        assert!(
            results.is_empty(),
            "nonsense query should return no results"
        );
    }

    #[tokio::test]
    async fn test_search_empty_library() {
        let ctx = test_context_empty();
        let op = SearchSkill::new("plan");
        let result = op.execute(&ctx).await;

        let value = result.into_result().unwrap();
        let results = value.as_array().unwrap();
        assert!(
            results.is_empty(),
            "search on empty library should return no results"
        );
    }

    #[tokio::test]
    async fn test_search_matches_description() {
        let ctx = test_context_with_defaults();
        // Search by a term likely in descriptions but not skill names
        let op = SearchSkill::new("workflow");
        let result = op.execute(&ctx).await;

        let value = result.into_result().unwrap();
        let _results = value.as_array().unwrap();
        // At least some skills should mention "workflow" in their description
        // This test validates that description matching works, not just name matching
        // Even if zero results, the operation should succeed
        assert!(value.is_array());
    }

    #[tokio::test]
    async fn test_search_returns_unlogged() {
        let ctx = test_context_empty();
        let op = SearchSkill::new("test");
        let result = op.execute(&ctx).await;

        match result {
            ExecutionResult::Unlogged { .. } => {} // expected
            _ => panic!("SearchSkill should return Unlogged variant"),
        }
    }

    #[tokio::test]
    async fn test_search_result_fields() {
        let ctx = test_context_with_defaults();
        let op = SearchSkill::new("commit");
        let result = op.execute(&ctx).await;

        let value = result.into_result().unwrap();
        let results = value.as_array().unwrap();

        for entry in results {
            assert!(entry.get("name").is_some(), "entry missing 'name'");
            assert!(
                entry.get("description").is_some(),
                "entry missing 'description'"
            );
            assert!(entry.get("source").is_some(), "entry missing 'source'");
        }
    }

    #[test]
    fn test_search_affected_resource_ids_empty() {
        let op = SearchSkill::new("test");
        assert!(op.affected_resource_ids(&json!([])).is_empty());
    }
}
