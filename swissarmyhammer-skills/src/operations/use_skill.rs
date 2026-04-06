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
    async fn test_use_skill_found() {
        let ctx = test_context_with_defaults();
        let op = UseSkill::new("plan");
        let result = op.execute(&ctx).await;

        let value = result
            .into_result()
            .expect("use should succeed for known skill");
        assert_eq!(value["name"].as_str(), Some("plan"));
        assert!(value["description"].is_string());
        assert!(value["instructions"].is_string());
        assert!(value["allowed_tools"].is_array());
        let source = value["source"].as_str().unwrap();
        assert!(
            ["builtin", "local", "user"].contains(&source),
            "source should be a valid SkillSource variant, got: {}",
            source
        );
    }

    #[tokio::test]
    async fn test_use_skill_not_found() {
        let ctx = test_context_with_defaults();
        let op = UseSkill::new("zzz-nonexistent-skill");
        let result = op.execute(&ctx).await;

        match result {
            ExecutionResult::Failed { error, log_entry } => {
                assert!(
                    format!("{}", error).contains("not found"),
                    "error should mention 'not found'"
                );
                assert!(log_entry.is_none(), "NotFound should have no log entry");
            }
            _ => panic!("UseSkill should fail for nonexistent skill"),
        }
    }

    #[tokio::test]
    async fn test_use_skill_empty_library() {
        let ctx = test_context_empty();
        let op = UseSkill::new("plan");
        let result = op.execute(&ctx).await;

        match result {
            ExecutionResult::Failed { error, .. } => {
                assert!(format!("{}", error).contains("not found"));
            }
            _ => panic!("UseSkill should fail when library is empty"),
        }
    }

    #[tokio::test]
    async fn test_use_skill_returns_instructions() {
        let ctx = test_context_with_defaults();
        let op = UseSkill::new("commit");
        let result = op.execute(&ctx).await;

        let value = result
            .into_result()
            .expect("use should succeed for 'commit'");
        let instructions = value["instructions"]
            .as_str()
            .expect("instructions should be a string");
        assert!(
            !instructions.is_empty(),
            "instructions should not be empty for a builtin skill"
        );
    }

    #[tokio::test]
    async fn test_use_skill_returns_unlogged_on_success() {
        let ctx = test_context_with_defaults();
        let op = UseSkill::new("plan");
        let result = op.execute(&ctx).await;

        match result {
            ExecutionResult::Unlogged { .. } => {} // expected
            _ => panic!("UseSkill should return Unlogged on success"),
        }
    }

    #[test]
    fn test_use_skill_affected_resource_ids_empty() {
        let op = UseSkill::new("test");
        assert!(op.affected_resource_ids(&json!({})).is_empty());
    }
}
