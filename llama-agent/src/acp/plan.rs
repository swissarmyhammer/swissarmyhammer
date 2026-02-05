//! Agent plan protocol support
//!
//! This module handles conversion between swissarmyhammer's kanban task system
//! and ACP (Agent Client Protocol) plan format.
//!
//! # Design Note
//!
//! The kanban `Task` is designed for persistent board-based task tracking:
//! - `id`: TaskId identifier
//! - `title`: Brief task description
//! - `description`: Optional implementation notes
//! - `column`: Current column (todo, doing, done)
//! - `created_at`/`updated_at`: Timestamps
//!
//! The ACP `PlanEntry` is for agent protocol communication:
//! - `content`: Task description (mapped from `title`)
//! - `status`: Pending/InProgress/Completed enum (mapped from column)
//! - `priority`: High/Medium/Low (always Medium for kanban tasks)
//! - `meta`: Additional metadata

use agent_client_protocol::{Plan, PlanEntry as AcpPlanEntry, PlanEntryPriority, PlanEntryStatus};
use serde_json::Value;

/// Convert plan data from kanban MCP tool response to ACP plan format
///
/// The kanban MCP tool includes a `_plan` field in its responses when tasks are modified.
/// This function converts that plan data directly to ACP Plan format.
///
/// # Plan Data Format
///
/// The kanban tool returns plan data with this structure:
/// ```json
/// {
///   "entries": [
///     {
///       "content": "task title",
///       "status": "pending" | "in_progress" | "completed",
///       "priority": "high" | "medium" | "low",
///       "_meta": { "id": "...", "column": "...", "notes": "..." }
///     }
///   ],
///   "_meta": { "source": "...", "trigger": "...", "affected_task_id": "..." }
/// }
/// ```
///
/// # Arguments
///
/// * `plan_data` - JSON object from the `_plan` field of a kanban tool response
///
/// # Returns
///
/// An ACP Plan with entries converted from the plan data
pub fn plan_data_to_acp_plan(plan_data: &Value) -> Plan {
    let entries: Vec<AcpPlanEntry> = plan_data
        .get("entries")
        .and_then(|e| e.as_array())
        .unwrap_or(&Vec::new())
        .iter()
        .map(plan_entry_to_acp_entry)
        .collect();

    // Preserve metadata from the plan data
    let mut meta = serde_json::Map::new();
    if let Some(plan_meta) = plan_data.get("_meta").and_then(|m| m.as_object()) {
        for (key, value) in plan_meta {
            meta.insert(key.clone(), value.clone());
        }
    }
    // Ensure source is always set
    if !meta.contains_key("source") {
        meta.insert(
            "source".to_string(),
            serde_json::json!("swissarmyhammer_kanban"),
        );
    }
    meta.insert("generator".to_string(), serde_json::json!("llama-agent"));

    Plan::new(entries).meta(meta)
}

/// Convert a single plan entry from kanban format to ACP PlanEntry
fn plan_entry_to_acp_entry(entry: &Value) -> AcpPlanEntry {
    let content = entry
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();

    let status = match entry.get("status").and_then(|s| s.as_str()) {
        Some("completed") => PlanEntryStatus::Completed,
        Some("in_progress") => PlanEntryStatus::InProgress,
        _ => PlanEntryStatus::Pending,
    };

    let priority = match entry.get("priority").and_then(|p| p.as_str()) {
        Some("high") => PlanEntryPriority::High,
        Some("low") => PlanEntryPriority::Low,
        _ => PlanEntryPriority::Medium,
    };

    // Preserve metadata from the entry
    let meta = entry.get("_meta").and_then(|m| m.as_object()).cloned();

    let mut acp_entry = AcpPlanEntry::new(content, priority, status);
    if let Some(m) = meta {
        acp_entry = acp_entry.meta(m);
    }
    acp_entry
}

/// Convert kanban tasks (as JSON values) to ACP plan format
///
/// Maps task JSON objects to ACP PlanEntry format with the following conventions:
/// - task.title → PlanEntry.content
/// - task.position.column → PlanEntry.status (done → Completed, doing → InProgress, else → Pending)
/// - All entries get Medium priority by default
/// - task.id and description are stored in meta field
///
/// # Arguments
///
/// * `tasks` - JSON array of task objects from ListTasks result
///
/// # Examples
///
/// ```rust,ignore
/// use llama_agent::acp::plan::tasks_to_acp_plan;
///
/// let tasks_json = serde_json::json!([
///     {"id": "123", "title": "Fix bug", "position": {"column": "todo"}},
///     {"id": "456", "title": "Add tests", "position": {"column": "done"}}
/// ]);
///
/// let plan = tasks_to_acp_plan(&tasks_json);
/// assert_eq!(plan.entries.len(), 2);
/// ```
pub fn tasks_to_acp_plan(tasks: &Value) -> Plan {
    let entries: Vec<AcpPlanEntry> = tasks
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .map(task_json_to_plan_entry)
        .collect();

    let mut meta = serde_json::Map::new();
    meta.insert(
        "source".to_string(),
        serde_json::json!("swissarmyhammer_kanban"),
    );
    meta.insert("generator".to_string(), serde_json::json!("llama-agent"));

    Plan::new(entries).meta(meta)
}

/// Convert a single task JSON object to ACP PlanEntry
///
/// Maps the kanban task structure to the ACP protocol format.
fn task_json_to_plan_entry(task: &Value) -> AcpPlanEntry {
    // Get the column from position.column
    let column = task["position"]["column"].as_str().unwrap_or("todo");

    // Map column to ACP status
    let status = match column {
        "done" => PlanEntryStatus::Completed,
        "doing" => PlanEntryStatus::InProgress,
        _ => PlanEntryStatus::Pending,
    };

    // All tasks get Medium priority (kanban doesn't have priority concept)
    let priority = PlanEntryPriority::Medium;

    // Get task title
    let title = task["title"].as_str().unwrap_or("").to_string();

    let mut meta = serde_json::Map::new();
    if let Some(id) = task["id"].as_str() {
        meta.insert("id".to_string(), serde_json::json!(id));
    }
    if let Some(description) = task["description"].as_str() {
        meta.insert("description".to_string(), serde_json::json!(description));
    }
    meta.insert("column".to_string(), serde_json::json!(column));

    if let Some(swimlane) = task["position"]["swimlane"].as_str() {
        meta.insert("swimlane".to_string(), serde_json::json!(swimlane));
    }

    if let Some(created_at) = task["created_at"].as_str() {
        meta.insert("created_at".to_string(), serde_json::json!(created_at));
    }
    if let Some(updated_at) = task["updated_at"].as_str() {
        meta.insert("updated_at".to_string(), serde_json::json!(updated_at));
    }

    AcpPlanEntry::new(title, priority, status).meta(meta)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests for plan_data_to_acp_plan (new kanban tool format)

    #[test]
    fn test_plan_data_to_acp_plan_empty() {
        let plan_data = serde_json::json!({
            "entries": [],
            "_meta": {"source": "swissarmyhammer_kanban", "trigger": "list"}
        });
        let plan = plan_data_to_acp_plan(&plan_data);
        assert_eq!(plan.entries.len(), 0);
        assert!(plan.meta.is_some());
        let meta = plan.meta.unwrap();
        assert_eq!(meta.get("source").unwrap(), "swissarmyhammer_kanban");
    }

    #[test]
    fn test_plan_data_to_acp_plan_with_entries() {
        let plan_data = serde_json::json!({
            "entries": [
                {
                    "content": "Implement feature X",
                    "status": "pending",
                    "priority": "high",
                    "_meta": {"id": "task-1", "column": "todo"}
                },
                {
                    "content": "Write tests",
                    "status": "in_progress",
                    "priority": "medium",
                    "_meta": {"id": "task-2", "column": "doing"}
                },
                {
                    "content": "Review PR",
                    "status": "completed",
                    "priority": "low",
                    "_meta": {"id": "task-3", "column": "done"}
                }
            ],
            "_meta": {"source": "swissarmyhammer_kanban", "trigger": "add task", "affected_task_id": "task-1"}
        });

        let plan = plan_data_to_acp_plan(&plan_data);
        assert_eq!(plan.entries.len(), 3);

        // Check first entry (pending, high priority)
        assert_eq!(plan.entries[0].content, "Implement feature X");
        let status_0 = serde_json::to_value(&plan.entries[0].status).unwrap();
        assert_eq!(status_0, "pending");
        let priority_0 = serde_json::to_value(&plan.entries[0].priority).unwrap();
        assert_eq!(priority_0, "high");

        // Check second entry (in_progress, medium priority)
        assert_eq!(plan.entries[1].content, "Write tests");
        let status_1 = serde_json::to_value(&plan.entries[1].status).unwrap();
        assert_eq!(status_1, "in_progress");

        // Check third entry (completed, low priority)
        assert_eq!(plan.entries[2].content, "Review PR");
        let status_2 = serde_json::to_value(&plan.entries[2].status).unwrap();
        assert_eq!(status_2, "completed");
        let priority_2 = serde_json::to_value(&plan.entries[2].priority).unwrap();
        assert_eq!(priority_2, "low");

        // Check plan metadata
        let meta = plan.meta.unwrap();
        assert_eq!(meta.get("trigger").unwrap(), "add task");
        assert_eq!(meta.get("affected_task_id").unwrap(), "task-1");
    }

    #[test]
    fn test_plan_data_preserves_entry_metadata() {
        let plan_data = serde_json::json!({
            "entries": [
                {
                    "content": "Task with metadata",
                    "status": "pending",
                    "priority": "medium",
                    "_meta": {"id": "task-123", "column": "todo", "notes": "Implementation notes here"}
                }
            ],
            "_meta": {"source": "swissarmyhammer_kanban"}
        });

        let plan = plan_data_to_acp_plan(&plan_data);
        assert_eq!(plan.entries.len(), 1);

        let entry_meta = plan.entries[0].meta.as_ref().unwrap();
        assert_eq!(entry_meta.get("id").unwrap(), "task-123");
        assert_eq!(entry_meta.get("column").unwrap(), "todo");
        assert_eq!(
            entry_meta.get("notes").unwrap(),
            "Implementation notes here"
        );
    }

    // Tests for tasks_to_acp_plan (legacy format)

    #[test]
    fn test_tasks_to_acp_plan_empty() {
        let tasks = serde_json::json!([]);
        let plan = tasks_to_acp_plan(&tasks);
        assert_eq!(plan.entries.len(), 0);
        assert!(plan.meta.is_some());
        let meta = plan.meta.unwrap();
        assert_eq!(meta.get("source").unwrap(), "swissarmyhammer_kanban");
    }

    #[test]
    fn test_tasks_to_acp_plan_single_pending() {
        let tasks = serde_json::json!([
            {"id": "123", "title": "Test task", "position": {"column": "todo"}}
        ]);

        let plan = tasks_to_acp_plan(&tasks);
        assert_eq!(plan.entries.len(), 1);

        let entry = &plan.entries[0];
        assert_eq!(entry.content, "Test task");

        let status_json = serde_json::to_value(&entry.status).unwrap();
        assert_eq!(status_json, "pending");

        let priority_json = serde_json::to_value(&entry.priority).unwrap();
        assert_eq!(priority_json, "medium");
    }

    #[test]
    fn test_tasks_to_acp_plan_single_completed() {
        let tasks = serde_json::json!([
            {"id": "456", "title": "Completed task", "position": {"column": "done"}}
        ]);

        let plan = tasks_to_acp_plan(&tasks);
        assert_eq!(plan.entries.len(), 1);

        let entry = &plan.entries[0];
        assert_eq!(entry.content, "Completed task");

        let status_json = serde_json::to_value(&entry.status).unwrap();
        assert_eq!(status_json, "completed");
    }

    #[test]
    fn test_tasks_to_acp_plan_in_progress() {
        let tasks = serde_json::json!([
            {"id": "789", "title": "In progress task", "position": {"column": "doing"}}
        ]);

        let plan = tasks_to_acp_plan(&tasks);
        assert_eq!(plan.entries.len(), 1);

        let entry = &plan.entries[0];
        let status_json = serde_json::to_value(&entry.status).unwrap();
        assert_eq!(status_json, "in_progress");
    }

    #[test]
    fn test_tasks_to_acp_plan_multiple() {
        let tasks = serde_json::json!([
            {"id": "1", "title": "First task", "position": {"column": "todo"}},
            {"id": "2", "title": "Second task", "position": {"column": "done"}},
            {"id": "3", "title": "Third task", "position": {"column": "doing"}}
        ]);

        let plan = tasks_to_acp_plan(&tasks);
        assert_eq!(plan.entries.len(), 3);

        let status_0 = serde_json::to_value(&plan.entries[0].status).unwrap();
        assert_eq!(status_0, "pending");

        let status_1 = serde_json::to_value(&plan.entries[1].status).unwrap();
        assert_eq!(status_1, "completed");

        let status_2 = serde_json::to_value(&plan.entries[2].status).unwrap();
        assert_eq!(status_2, "in_progress");
    }

    #[test]
    fn test_plan_metadata_includes_source() {
        let tasks = serde_json::json!([
            {"id": "1", "title": "Test", "position": {"column": "todo"}}
        ]);

        let plan = tasks_to_acp_plan(&tasks);

        assert!(plan.meta.is_some());
        let meta = plan.meta.unwrap();
        assert_eq!(meta.get("source").unwrap(), "swissarmyhammer_kanban");
        assert_eq!(meta.get("generator").unwrap(), "llama-agent");
    }
}
