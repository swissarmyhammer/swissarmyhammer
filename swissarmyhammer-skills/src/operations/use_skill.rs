//! UseSkill operation — loads full SKILL.md body for a specific skill (activation)

use crate::context::SkillContext;
use crate::error::SkillError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Load a skill's full instructions by name (progressive disclosure activation)
#[operation(
    verb = "use",
    noun = "skill",
    description = "Activate a skill by loading its full instructions"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct UseSkill {
    /// The skill name to load
    pub name: String,
}

impl UseSkill {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[async_trait]
impl Execute<SkillContext, SkillError> for UseSkill {
    async fn execute(&self, ctx: &SkillContext) -> ExecutionResult<Value, SkillError> {
        let library = ctx.library.read().await;

        match library.get(&self.name) {
            Some(skill) => {
                let result = json!({
                    "name": skill.name.as_str(),
                    "description": skill.description,
                    "instructions": skill.instructions,
                    "allowed_tools": skill.allowed_tools,
                    "source": skill.source.to_string(),
                });

                ExecutionResult::Unlogged { value: result }
            }
            None => ExecutionResult::Failed {
                error: SkillError::NotFound {
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
