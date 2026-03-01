//! GetBoard command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::{ColumnId, SwimlaneId};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Get the board with computed task counts
#[operation(
    verb = "get",
    noun = "board",
    description = "Retrieve the board with task counts"
)]
#[derive(Debug, Deserialize)]
pub struct GetBoard {
    /// Whether to include task counts (default: true)
    #[serde(default = "default_include_counts")]
    pub include_counts: bool,
}

impl Default for GetBoard {
    fn default() -> Self {
        Self {
            include_counts: true,
        }
    }
}

fn default_include_counts() -> bool {
    true
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for GetBoard {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let board = ctx.read_board().await?;
            let all_columns = ctx.read_all_columns().await?;
            let all_swimlanes = ctx.read_all_swimlanes().await?;

            // If counts are not requested, return basic board structure
            if !self.include_counts {
                let tags = ctx.read_all_tags().await?;
                let columns_json: Vec<Value> = all_columns
                    .iter()
                    .map(|c| json!({"id": c.id, "name": c.name, "order": c.order}))
                    .collect();
                let swimlanes_json: Vec<Value> = all_swimlanes
                    .iter()
                    .map(|s| json!({"id": s.id, "name": s.name, "order": s.order}))
                    .collect();
                let tags_json: Vec<Value> = tags
                    .iter()
                    .map(|t| json!({"id": t.id, "name": t.name, "description": t.description, "color": t.color}))
                    .collect();
                return Ok(json!({
                    "name": board.name,
                    "description": board.description,
                    "columns": columns_json,
                    "swimlanes": swimlanes_json,
                    "tags": tags_json,
                }));
            }

            // Read all tasks once for efficiency
            let all_tasks = ctx.read_all_tasks().await?;
            let terminal = all_columns.iter().max_by_key(|c| c.order);
            let terminal_id = terminal
                .map(|c| c.id.as_str())
                .unwrap_or("done");

            // Count tasks by column
            let mut column_counts: HashMap<&ColumnId, usize> = HashMap::new();
            let mut column_ready_counts: HashMap<&ColumnId, usize> = HashMap::new();

            for task in &all_tasks {
                *column_counts.entry(&task.position.column).or_insert(0) += 1;

                if task.is_ready(&all_tasks, terminal_id) {
                    *column_ready_counts
                        .entry(&task.position.column)
                        .or_insert(0) += 1;
                }
            }

            // Count tasks by swimlane
            let mut swimlane_counts: HashMap<&SwimlaneId, usize> = HashMap::new();
            for task in &all_tasks {
                if let Some(ref swimlane) = task.position.swimlane {
                    *swimlane_counts.entry(swimlane).or_insert(0) += 1;
                }
            }

            // Count tasks by tag name (computed from description)
            let task_tags: Vec<Vec<String>> = all_tasks.iter().map(|t| t.tags()).collect();
            let mut tag_counts: HashMap<&str, usize> = HashMap::new();
            for tags in &task_tags {
                for tag_name in tags {
                    *tag_counts.entry(tag_name.as_str()).or_insert(0) += 1;
                }
            }

            // Enhance columns with counts
            let columns: Vec<Value> = all_columns
                .iter()
                .map(|col| {
                    let count = column_counts.get(&col.id).copied().unwrap_or(0);
                    let ready = column_ready_counts.get(&col.id).copied().unwrap_or(0);

                    json!({
                        "id": col.id,
                        "name": col.name,
                        "order": col.order,
                        "task_count": count,
                        "ready_count": ready
                    })
                })
                .collect();

            // Enhance swimlanes with counts
            let swimlanes: Vec<Value> = all_swimlanes
                .iter()
                .map(|sl| {
                    let count = swimlane_counts.get(&sl.id).copied().unwrap_or(0);

                    json!({
                        "id": sl.id,
                        "name": sl.name,
                        "order": sl.order,
                        "task_count": count
                    })
                })
                .collect();

            // Read all tags and enhance with counts
            let all_tags = ctx.read_all_tags().await?;
            let tags: Vec<Value> = all_tags
                .iter()
                .map(|tag| {
                    let count = tag_counts.get(tag.name.as_str()).copied().unwrap_or(0);

                    json!({
                        "id": tag.id,
                        "name": tag.name,
                        "description": tag.description,
                        "color": tag.color,
                        "task_count": count
                    })
                })
                .collect();

            // Calculate summary
            let total_tasks = all_tasks.len();
            let ready_tasks = all_tasks
                .iter()
                .filter(|t| t.is_ready(&all_tasks, terminal_id))
                .count();
            let blocked_tasks = total_tasks - ready_tasks;
            let total_actors = ctx.list_actor_ids().await?.len();

            Ok(json!({
                "name": board.name,
                "description": board.description,
                "columns": columns,
                "swimlanes": swimlanes,
                "tags": tags,
                "summary": {
                    "total_tasks": total_tasks,
                    "total_actors": total_actors,
                    "ready_tasks": ready_tasks,
                    "blocked_tasks": blocked_tasks
                }
            }))
        }
        .await
        {
            Ok(value) => ExecutionResult::Unlogged { value },
            Err(error) => ExecutionResult::Failed {
                error,
                log_entry: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::swimlane::AddSwimlane;
    use crate::tag::AddTag;
    use crate::task::{AddTask, MoveTask, TagTask, UpdateTask};
    use crate::types::TaskId;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);

        // Initialize board
        InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        (temp, ctx)
    }

    #[tokio::test]
    async fn test_empty_board() {
        let (_temp, ctx) = setup().await;

        let result = GetBoard::default()
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["name"], "Test");
        assert_eq!(result["summary"]["total_tasks"], 0);
        assert_eq!(result["summary"]["total_actors"], 0);
        assert_eq!(result["summary"]["ready_tasks"], 0);
        assert_eq!(result["summary"]["blocked_tasks"], 0);

        // Check columns have zero counts
        let columns = result["columns"].as_array().unwrap();
        for col in columns {
            assert_eq!(col["task_count"], 0);
            assert_eq!(col["ready_count"], 0);
        }
    }

    #[tokio::test]
    async fn test_board_with_tasks_in_different_columns() {
        let (_temp, ctx) = setup().await;

        // Add tasks to different columns
        AddTask::new("Task 1")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task2_id = AddTask::new("Task 2")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        let task3_id = AddTask::new("Task 3")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();

        // Move tasks to different columns
        MoveTask::to_column(task2_id.clone(), "doing")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        MoveTask::to_column(task3_id, "done")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = GetBoard::default()
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Check summary
        assert_eq!(result["summary"]["total_tasks"], 3);

        // Check column counts
        let columns = result["columns"].as_array().unwrap();
        let todo_col = columns.iter().find(|c| c["id"] == "todo").unwrap();
        let doing_col = columns.iter().find(|c| c["id"] == "doing").unwrap();
        let done_col = columns.iter().find(|c| c["id"] == "done").unwrap();

        assert_eq!(todo_col["task_count"], 1);
        assert_eq!(doing_col["task_count"], 1);
        assert_eq!(done_col["task_count"], 1);
    }

    #[tokio::test]
    async fn test_ready_vs_blocked_counts() {
        let (_temp, ctx) = setup().await;

        // Create tasks with dependencies
        let task1_id = AddTask::new("Task 1")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();

        let task2_id = AddTask::new("Task 2")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();

        let _task3_id = AddTask::new("Task 3")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();

        // Task 2 depends on Task 1 (blocked)
        UpdateTask::new(task2_id.clone())
            .with_depends_on(vec![TaskId::from_string(&task1_id)])
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Task 3 is independent (ready)

        let result = GetBoard::default()
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // 2 ready (task 1 and task 3), 1 blocked (task 2)
        assert_eq!(result["summary"]["ready_tasks"], 2);
        assert_eq!(result["summary"]["blocked_tasks"], 1);

        // All in todo column
        let columns = result["columns"].as_array().unwrap();
        let todo_col = columns.iter().find(|c| c["id"] == "todo").unwrap();
        assert_eq!(todo_col["task_count"], 3);
        assert_eq!(todo_col["ready_count"], 2); // 2 ready tasks in todo

        // Move Task 1 to done - should unblock Task 2
        MoveTask::to_column(task1_id, "done")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = GetBoard::default()
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Now all 3 tasks are ready
        assert_eq!(result["summary"]["ready_tasks"], 3);
        assert_eq!(result["summary"]["blocked_tasks"], 0);

        let columns = result["columns"].as_array().unwrap();
        let todo_col = columns.iter().find(|c| c["id"] == "todo").unwrap();
        assert_eq!(todo_col["ready_count"], 2); // 2 ready tasks in todo
    }

    #[tokio::test]
    async fn test_swimlane_counts() {
        let (_temp, ctx) = setup().await;

        // Add swimlane
        AddSwimlane::new("backend", "Backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Add tasks with swimlanes
        let task1_id = AddTask::new("Task 1")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();

        AddTask::new("Task 2")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Move task 1 to backend swimlane
        MoveTask::to_column_and_swimlane(task1_id, "todo", "backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = GetBoard::default()
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let swimlanes = result["swimlanes"].as_array().unwrap();
        let backend_sl = swimlanes.iter().find(|s| s["id"] == "backend").unwrap();
        assert_eq!(backend_sl["task_count"], 1);
    }

    #[tokio::test]
    async fn test_tag_counts() {
        let (_temp, ctx) = setup().await;

        // Create tags and capture ULIDs
        let bug_result = AddTag::new("bug")
            .with_color("d73a4a")
            .with_description("Something isn't working")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let bug_id = bug_result["id"].as_str().unwrap().to_string();

        let feature_result = AddTag::new("feature")
            .with_color("a2eeef")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let feature_id = feature_result["id"].as_str().unwrap().to_string();

        // Add tasks with tags
        let task1_id = AddTask::new("Task 1")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();

        let task2_id = AddTask::new("Task 2")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();

        // Tag the tasks
        TagTask::new(task1_id, "bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        TagTask::new(task2_id.clone(), "bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        TagTask::new(task2_id, "feature")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = GetBoard::default()
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let tags = result["tags"].as_array().unwrap();
        let bug_tag = tags.iter().find(|t| t["id"].as_str() == Some(&*bug_id)).unwrap();
        let feature_tag = tags.iter().find(|t| t["id"].as_str() == Some(&*feature_id)).unwrap();

        assert_eq!(bug_tag["task_count"], 2);
        assert_eq!(bug_tag["description"], "Something isn't working");
        assert_eq!(bug_tag["color"], "d73a4a");

        assert_eq!(feature_tag["task_count"], 1);
    }

    #[tokio::test]
    async fn test_include_counts_false() {
        let (_temp, ctx) = setup().await;

        // Add a task
        AddTask::new("Task 1")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = GetBoard {
            include_counts: false,
        }
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

        // Should not have summary or counts
        assert!(result["summary"].is_null());

        // Columns should be basic structure without counts
        let columns = result["columns"].as_array().unwrap();
        assert!(columns[0]["task_count"].is_null());
        assert!(columns[0]["ready_count"].is_null());
    }
}
