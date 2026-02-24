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
