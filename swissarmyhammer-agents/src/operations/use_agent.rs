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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_library::AgentLibrary;
    use crate::context::AgentContext;
    use crate::error::AgentError;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// Build a context pre-loaded with the default builtin agents.
    fn make_context() -> AgentContext {
        let mut library = AgentLibrary::new();
        library.load_defaults();
        AgentContext::new(Arc::new(RwLock::new(library)))
    }

    #[tokio::test]
    async fn test_use_existing_agent_returns_full_definition() {
        let ctx = make_context();
        let op = UseAgent::new("tester");
        let result = op.execute(&ctx).await;

        match result {
            ExecutionResult::Unlogged { value } => {
                assert_eq!(
                    value.get("name").and_then(|v| v.as_str()),
                    Some("tester"),
                    "returned agent should have name 'tester'"
                );
                assert!(
                    value.get("description").is_some(),
                    "returned agent should have 'description'"
                );
                assert!(
                    value.get("instructions").is_some(),
                    "returned agent should have 'instructions'"
                );
                assert!(
                    value.get("source").is_some(),
                    "returned agent should have 'source'"
                );
            }
            other => panic!("expected Unlogged result, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_use_missing_agent_returns_not_found_error() {
        let ctx = make_context();
        let op = UseAgent::new("this-agent-does-not-exist");
        let result = op.execute(&ctx).await;

        match result {
            ExecutionResult::Failed { error, .. } => match error {
                AgentError::NotFound { name } => {
                    assert_eq!(name, "this-agent-does-not-exist");
                }
                other => panic!("expected NotFound error, got: {:?}", other),
            },
            other => panic!("expected Failed result, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_use_agent_response_has_all_fields() {
        let ctx = make_context();
        let op = UseAgent::new("default");
        let result = op.execute(&ctx).await;

        match result {
            ExecutionResult::Unlogged { value } => {
                let required_fields = [
                    "name",
                    "description",
                    "instructions",
                    "model",
                    "tools",
                    "disallowed_tools",
                    "isolation",
                    "max_turns",
                    "background",
                    "source",
                ];
                for field in &required_fields {
                    assert!(
                        value.get(field).is_some(),
                        "expected field '{}' in use_agent response",
                        field
                    );
                }
            }
            other => panic!("expected Unlogged result, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_use_agent_instructions_are_non_empty() {
        let ctx = make_context();
        let op = UseAgent::new("tester");
        let result = op.execute(&ctx).await;

        match result {
            ExecutionResult::Unlogged { value } => {
                let instructions = value
                    .get("instructions")
                    .and_then(|v| v.as_str())
                    .expect("instructions should be a string");
                assert!(
                    !instructions.is_empty(),
                    "tester agent should have non-empty instructions"
                );
            }
            other => panic!("expected Unlogged result, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_use_agent_affected_resource_ids_is_empty() {
        let op = UseAgent::new("anything");
        let ids = op.affected_resource_ids(&json!({}));
        assert!(ids.is_empty());
    }
}
