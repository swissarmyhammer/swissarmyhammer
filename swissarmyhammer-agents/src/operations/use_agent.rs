//! UseAgent operation — loads full AGENT.md body for a specific agent (activation)

use crate::context::AgentContext;
use crate::error::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Load an agent's full definition by name
#[operation(
    verb = "use",
    noun = "agent",
    description = "Activate an agent by loading its full definition and instructions"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct UseAgent {
    /// The agent name to load
    pub name: String,
}

impl UseAgent {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[async_trait]
impl Execute<AgentContext, AgentError> for UseAgent {
    async fn execute(&self, ctx: &AgentContext) -> ExecutionResult<Value, AgentError> {
        let library = ctx.library.read().await;

        match library.get(&self.name) {
            Some(agent) => {
                let result = json!({
                    "name": agent.name.as_str(),
                    "description": agent.description,
                    "instructions": agent.instructions,
                    "model": agent.model,
                    "tools": agent.tools,
                    "disallowed_tools": agent.disallowed_tools,
                    "isolation": agent.isolation,
                    "max_turns": agent.max_turns,
                    "background": agent.background,
                    "source": agent.source.to_string(),
                });

                ExecutionResult::Unlogged { value: result }
            }
            None => ExecutionResult::Failed {
                error: AgentError::NotFound {
                    name: self.name.clone(),
                },
                log_entry: None,
            },
        }
    }

    fn affected_resource_ids(&self, _result: &Value) -> Vec<String> {
        vec![]
    }
}
