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
