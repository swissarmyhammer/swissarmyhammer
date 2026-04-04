//! ListSkills operation — returns metadata for all available skills

use crate::context::SkillContext;
use crate::error::SkillError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// List all available skills with name, description, and source
#[operation(
    verb = "list",
    noun = "skill",
    description = "List all available skills with their descriptions"
)]
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
    async fn test_list_skills_returns_all_loaded_skills() {
        let ctx = test_context_with_defaults();
        let op = ListSkills::new();
        let result = op.execute(&ctx).await;

        let value = result.into_result().expect("list should succeed");
        let skills = value.as_array().expect("result should be an array");
        assert!(!skills.is_empty(), "should return at least one skill");

        // Each entry should have name, description, and source fields
        for skill in skills {
            assert!(skill.get("name").is_some(), "skill entry missing 'name'");
            assert!(
                skill.get("description").is_some(),
                "skill entry missing 'description'"
            );
            assert!(
                skill.get("source").is_some(),
                "skill entry missing 'source'"
            );
        }
    }

    #[tokio::test]
    async fn test_list_skills_empty_library() {
        let ctx = test_context_empty();
        let op = ListSkills::new();
        let result = op.execute(&ctx).await;

        let value = result.into_result().expect("list should succeed");
        let skills = value.as_array().expect("result should be an array");
        assert!(skills.is_empty(), "empty library should return no skills");
    }

    #[tokio::test]
    async fn test_list_skills_returns_unlogged() {
        let ctx = test_context_empty();
        let op = ListSkills::new();
        let result = op.execute(&ctx).await;

        // ListSkills is read-only, should always be Unlogged
        match result {
            ExecutionResult::Unlogged { .. } => {} // expected
            _ => panic!("ListSkills should return Unlogged variant"),
        }
    }

    #[tokio::test]
    async fn test_list_skills_contains_known_builtin() {
        let ctx = test_context_with_defaults();
        let op = ListSkills::new();
        let result = op.execute(&ctx).await;

        let value = result.into_result().unwrap();
        let skills = value.as_array().unwrap();

        // The "plan" skill should exist (may be builtin or local override)
        let plan = skills.iter().find(|s| s["name"].as_str() == Some("plan"));
        assert!(plan.is_some(), "should contain the 'plan' skill");
        // Source should be a valid variant
        let source = plan.unwrap()["source"].as_str().unwrap();
        assert!(
            ["builtin", "local", "user"].contains(&source),
            "source should be a valid SkillSource variant, got: {}",
            source
        );
    }

    #[test]
    fn test_list_skills_affected_resource_ids_empty() {
        let op = ListSkills::new();
        assert!(op.affected_resource_ids(&json!([])).is_empty());
    }

    #[test]
    fn test_list_skills_default() {
        // ListSkills implements Default
        let _op = ListSkills;
    }
}
