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
