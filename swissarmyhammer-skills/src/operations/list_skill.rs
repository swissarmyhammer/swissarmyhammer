//! ListSkills operation — returns metadata for all available skills

use crate::context::SkillContext;
use crate::error::SkillError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// List all available skills with name, description, and source
#[operation(verb = "list", noun = "skill", description = "List all available skills with their descriptions")]
#[derive(Debug, Deserialize, Serialize)]
pub struct ListSkills;

impl ListSkills {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ListSkills {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Execute<SkillContext, SkillError> for ListSkills {
    async fn execute(&self, ctx: &SkillContext) -> ExecutionResult<Value, SkillError> {
        let library = ctx.library.read().await;
        let skills = library.list();

        let result: Vec<Value> = skills
            .iter()
            .map(|skill| {
                json!({
                    "name": skill.name.as_str(),
                    "description": skill.description,
                    "source": skill.source.to_string(),
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
