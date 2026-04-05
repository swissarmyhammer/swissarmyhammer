//! SearchAgent operation — search for agents by name or description

use crate::context::AgentContext;
use crate::error::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Search for agents by name or description
#[operation(
    verb = "search",
    noun = "agent",
    description = "Search for agents by name or description"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct SearchAgent {
    /// Search query to match against agent names and descriptions
    pub query: String,
}

impl SearchAgent {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
        }
    }
}

#[async_trait]
impl Execute<AgentContext, AgentError> for SearchAgent {
    async fn execute(&self, ctx: &AgentContext) -> ExecutionResult<Value, AgentError> {
        let library = ctx.library.read().await;
        let query_lower = self.query.to_lowercase();

        let results: Vec<Value> = library
            .list()
            .into_iter()
            .filter(|agent| {
                agent.name.as_str().to_lowercase().contains(&query_lower)
                    || agent.description.to_lowercase().contains(&query_lower)
            })
            .map(|agent| {
                json!({
                    "name": agent.name.as_str(),
                    "description": agent.description,
                    "source": agent.source.to_string(),
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
    use crate::agent_library::AgentLibrary;
    use crate::context::AgentContext;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// Build a context pre-loaded with the default builtin agents.
    fn make_context() -> AgentContext {
        let mut library = AgentLibrary::new();
        library.load_defaults();
        AgentContext::new(Arc::new(RwLock::new(library)))
    }

    #[tokio::test]
    async fn test_search_by_exact_name_returns_match() {
        let ctx = make_context();
        let op = SearchAgent::new("tester");
        let result = op.execute(&ctx).await;

        match result {
            ExecutionResult::Unlogged { value } => {
                let arr = value.as_array().expect("result should be an array");
                assert!(
                    !arr.is_empty(),
                    "searching for 'tester' should return at least one result"
                );
                let names: Vec<&str> = arr
                    .iter()
                    .filter_map(|v| v.get("name").and_then(|n| n.as_str()))
                    .collect();
                assert!(
                    names.contains(&"tester"),
                    "expected 'tester' in results, got: {:?}",
                    names
                );
            }
            other => panic!("expected Unlogged result, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_search_no_match_returns_empty_array() {
        let ctx = make_context();
        let op = SearchAgent::new("xyzzy-this-does-not-exist-42");
        let result = op.execute(&ctx).await;

        match result {
            ExecutionResult::Unlogged { value } => {
                let arr = value.as_array().expect("result should be an array");
                assert!(
                    arr.is_empty(),
                    "non-matching query should return empty array"
                );
            }
            other => panic!("expected Unlogged result, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_search_is_case_insensitive() {
        let ctx = make_context();
        // 'default' agent definitely exists; search with uppercase letters
        let op = SearchAgent::new("DEFAULT");
        let result = op.execute(&ctx).await;

        match result {
            ExecutionResult::Unlogged { value } => {
                let arr = value.as_array().expect("result should be an array");
                assert!(
                    !arr.is_empty(),
                    "case-insensitive search for 'DEFAULT' should match 'default'"
                );
            }
            other => panic!("expected Unlogged result, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_search_partial_name_returns_matches() {
        let ctx = make_context();
        // Search for partial string that matches multiple agents (e.g. "e" is in many names)
        let op = SearchAgent::new("test");
        let result = op.execute(&ctx).await;

        match result {
            ExecutionResult::Unlogged { value } => {
                let arr = value.as_array().expect("result should be an array");
                // "tester" agent should be in results
                let names: Vec<&str> = arr
                    .iter()
                    .filter_map(|v| v.get("name").and_then(|n| n.as_str()))
                    .collect();
                assert!(
                    names.contains(&"tester"),
                    "partial search 'test' should match 'tester', got: {:?}",
                    names
                );
            }
            other => panic!("expected Unlogged result, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_search_result_entries_have_required_fields() {
        let ctx = make_context();
        let op = SearchAgent::new("default");
        let result = op.execute(&ctx).await;

        match result {
            ExecutionResult::Unlogged { value } => {
                let arr = value.as_array().expect("result should be an array");
                for entry in arr {
                    assert!(entry.get("name").is_some(), "each result must have 'name'");
                    assert!(
                        entry.get("description").is_some(),
                        "each result must have 'description'"
                    );
                    assert!(
                        entry.get("source").is_some(),
                        "each result must have 'source'"
                    );
                }
            }
            other => panic!("expected Unlogged result, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_search_affected_resource_ids_is_empty() {
        let op = SearchAgent::new("anything");
        let ids = op.affected_resource_ids(&json!([]));
        assert!(ids.is_empty());
    }
}
