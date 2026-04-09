//! UpdateTask command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::task::shared::auto_create_body_tags;
use crate::task_helpers::task_entity_to_json;
use crate::types::{ActorId, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_entity::Entity;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Update an existing task.
///
/// Tags are derived from `#tag` patterns in the description — edit the
/// description to change tags.
#[operation(verb = "update", noun = "task", description = "Update task properties")]
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateTask {
    /// The task ID to update
    pub id: TaskId,
    /// New title
    pub title: Option<String>,
    /// New description (may contain #tag patterns)
    pub description: Option<String>,
    /// Replace all assignees
    pub assignees: Option<Vec<ActorId>>,
    /// Replace all dependencies
    pub depends_on: Option<Vec<TaskId>>,
    /// Replace all attachment IDs (array of entity ID strings)
    pub attachments: Option<Value>,
    /// Set the project this task belongs to
    pub project: Option<String>,
}

impl UpdateTask {
    /// Create a new UpdateTask command
    pub fn new(id: impl Into<TaskId>) -> Self {
        Self {
            id: id.into(),
            title: None,
            description: None,
            assignees: None,
            depends_on: None,
            attachments: None,
            project: None,
        }
    }

    /// Set the title
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the assignees (replaces all existing assignees)
    pub fn with_assignees(mut self, assignees: Vec<ActorId>) -> Self {
        self.assignees = Some(assignees);
        self
    }

    /// Set the dependencies (replaces all existing dependencies)
    pub fn with_depends_on(mut self, deps: Vec<TaskId>) -> Self {
        self.depends_on = Some(deps);
        self
    }

    /// Set the attachments field value (replaces all existing attachments)
    pub fn with_attachments(mut self, attachments: Value) -> Self {
        self.attachments = Some(attachments);
        self
    }

    /// Set the project this task belongs to
    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project = Some(project.into());
        self
    }

    /// Apply all set fields to the entity.
    fn apply_to(&self, entity: &mut Entity) -> std::result::Result<(), serde_json::Error> {
        if let Some(title) = &self.title {
            entity.set("title", serde_json::json!(title));
        }
        if let Some(desc) = &self.description {
            entity.set("body", serde_json::json!(desc));
        }
        if let Some(assignees) = &self.assignees {
            entity.set("assignees", serde_json::to_value(assignees)?);
        }
        if let Some(deps) = &self.depends_on {
            entity.set("depends_on", serde_json::to_value(deps)?);
        }
        if let Some(attachments) = &self.attachments {
            entity.set("attachments", attachments.clone());
        }
        if let Some(project) = &self.project {
            entity.set("project", serde_json::json!(project));
        }
        Ok(())
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UpdateTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;
            let mut entity = ectx
                .read("task", self.id.as_str())
                .await
                .map_err(KanbanError::from_entity_error)?;

            self.apply_to(&mut entity)?;
            ectx.write(&entity).await?;
            auto_create_body_tags(&ectx, &entity).await?;
            Ok(task_entity_to_json(&entity))
        }
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(value) => ExecutionResult::Logged {
                value: value.clone(),
                log_entry: LogEntry::new(self.op_string(), input, value, None, duration_ms),
            },
            Err(error) => {
                let error_msg = error.to_string();
                ExecutionResult::Failed {
                    error,
                    log_entry: Some(LogEntry::new(
                        self.op_string(),
                        input,
                        serde_json::json!({"error": error_msg}),
                        None,
                        duration_ms,
                    )),
                }
            }
        }
    }

    fn affected_resource_ids(&self, result: &Value) -> Vec<String> {
        result
            .get("id")
            .and_then(|v| v.as_str())
            .map(|id| vec![id.to_string()])
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::task::AddTask;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);

        InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        (temp, ctx)
    }

    #[tokio::test]
    async fn test_update_task_title() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Original")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let result = UpdateTask::new(task_id)
            .with_title("Updated")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["title"], "Updated");
    }

    #[tokio::test]
    async fn test_update_task_description() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let result = UpdateTask::new(task_id)
            .with_description("New description")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["description"], "New description");
    }

    #[tokio::test]
    async fn test_update_task_assignees_replace() {
        let (_temp, ctx) = setup().await;

        use crate::actor::AddActor;

        AddActor::new("alice", "Alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddActor::new("bob", "Bob")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let add_result = AddTask::new("Task")
            .with_assignees(vec![ActorId::from("alice")])
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        // Replace assignees — should only have bob now
        let result = UpdateTask::new(task_id)
            .with_assignees(vec![ActorId::from("bob")])
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let assignees = result["assignees"].as_array().unwrap();
        assert_eq!(assignees.len(), 1);
        assert_eq!(assignees[0], "bob");
    }

    #[tokio::test]
    async fn test_update_task_affected_resource_ids() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let op = UpdateTask::new(task_id).with_title("Updated");
        let exec_result = op.execute(&ctx).await;
        let value = exec_result.into_result().unwrap();

        let ids = op.affected_resource_ids(&value);
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], task_id);
    }

    #[tokio::test]
    async fn test_update_task_set_project() {
        let (_temp, ctx) = setup().await;

        use crate::project::AddProject;
        AddProject::new("backend", "Backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let add_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        // Task starts with no project (null, not empty string)
        assert!(add_result["project"].is_null());

        // Set the project
        let result = UpdateTask::new(task_id)
            .with_project("backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["project"], "backend");
    }

    #[tokio::test]
    async fn test_update_task_change_project() {
        let (_temp, ctx) = setup().await;

        use crate::project::AddProject;
        AddProject::new("backend", "Backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddProject::new("frontend", "Frontend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let add_result = AddTask::new("Task")
            .with_project("backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();
        assert_eq!(add_result["project"], "backend");

        // Change to a different project
        let result = UpdateTask::new(task_id)
            .with_project("frontend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["project"], "frontend");
    }

    #[tokio::test]
    async fn test_update_task_multiple_dependencies() {
        let (_temp, ctx) = setup().await;

        // Create three tasks — the third will depend on the first two.
        let a = AddTask::new("Task A")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let b = AddTask::new("Task B")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let c = AddTask::new("Task C")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let id_a = a["id"].as_str().unwrap();
        let id_b = b["id"].as_str().unwrap();
        let id_c = c["id"].as_str().unwrap();

        // Set two dependencies on task C.
        let result = UpdateTask::new(id_c)
            .with_depends_on(vec![TaskId::from_string(id_a), TaskId::from_string(id_b)])
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let deps = result["depends_on"]
            .as_array()
            .expect("depends_on should be an array");
        assert_eq!(deps.len(), 2, "should have exactly 2 dependencies");
        let dep_strs: Vec<&str> = deps.iter().filter_map(|v| v.as_str()).collect();
        assert!(dep_strs.contains(&id_a), "should contain task A");
        assert!(dep_strs.contains(&id_b), "should contain task B");
    }
}
