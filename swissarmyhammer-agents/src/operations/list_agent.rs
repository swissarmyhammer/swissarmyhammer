//! ListAgents operation — returns metadata for all available agents

use crate::context::AgentContext;
use crate::error::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// List all available agents with name, description, and source
#[operation(
    verb = "list",
    noun = "agent",
    description = "List all available agents with their descriptions"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct ListAgents;

impl ListAgents {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ListAgents {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Execute<AgentContext, AgentError> for ListAgents {
    async fn execute(&self, ctx: &AgentContext) -> ExecutionResult<Value, AgentError> {
        let library = ctx.library.read().await;
        let agents = library.list();

        let result: Vec<Value> = agents
            .iter()
            .map(|agent| {
                json!({
                    "name": agent.name.as_str(),
                    "description": agent.description,
                    "source": agent.source.to_string(),
                })
            })
            .collect();

        ExecutionResult::Unlogged {
            value: json!(result),
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
    async fn test_list_agents_returns_all_agents() {
        let ctx = make_context();
        let op = ListAgents::new();
        let result = op.execute(&ctx).await;

        match result {
            ExecutionResult::Unlogged { value } => {
                let arr = value.as_array().expect("result should be an array");
                assert!(
                    !arr.is_empty(),
                    "should have at least one agent in the library"
                );
            }
            other => panic!("expected Unlogged result, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_list_agents_contains_default_agent() {
        let ctx = make_context();
        let op = ListAgents::new();
        let result = op.execute(&ctx).await;

        match result {
            ExecutionResult::Unlogged { value } => {
                let arr = value.as_array().expect("result should be an array");
                let names: Vec<&str> = arr
                    .iter()
                    .filter_map(|v| v.get("name").and_then(|n| n.as_str()))
                    .collect();
                assert!(
                    names.contains(&"default"),
                    "expected 'default' agent in list, got: {:?}",
                    names
                );
            }
            other => panic!("expected Unlogged result, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_list_agents_each_entry_has_required_fields() {
        let ctx = make_context();
        let op = ListAgents::new();
        let result = op.execute(&ctx).await;

        match result {
            ExecutionResult::Unlogged { value } => {
                let arr = value.as_array().expect("result should be an array");
                for entry in arr {
                    assert!(
                        entry.get("name").is_some(),
                        "each agent must have a 'name' field"
                    );
                    assert!(
                        entry.get("description").is_some(),
                        "each agent must have a 'description' field"
                    );
                    assert!(
                        entry.get("source").is_some(),
                        "each agent must have a 'source' field"
                    );
                }
            }
            other => panic!("expected Unlogged result, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_list_agents_empty_library_returns_empty_array() {
        // Use an empty library to verify no agents are listed
        let library = AgentLibrary::new();
        let ctx = AgentContext::new(Arc::new(RwLock::new(library)));
        let op = ListAgents::new();
        let result = op.execute(&ctx).await;

        match result {
            ExecutionResult::Unlogged { value } => {
                let arr = value.as_array().expect("result should be an array");
                assert!(arr.is_empty(), "empty library should return empty array");
            }
            other => panic!("expected Unlogged result, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_list_agents_affected_resource_ids_is_empty() {
        let op = ListAgents::new();
        let ids = op.affected_resource_ids(&json!([]));
        assert!(ids.is_empty());
    }
}
