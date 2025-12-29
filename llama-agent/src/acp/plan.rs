//! Agent plan protocol support
//!
//! This module handles conversion between swissarmyhammer's todo system
//! and ACP (Agent Client Protocol) plan format.

use agent_client_protocol::{Plan, PlanEntry as AcpPlanEntry, PlanEntryPriority, PlanEntryStatus};
use swissarmyhammer_todo::{PlanEntry, PlanEntryStatus as TodoStatus, Priority, TodoItem};

/// Convert swissarmyhammer todos to ACP plan format
///
/// Maps TodoItems to ACP PlanEntry format with the following conventions:
/// - TodoItem.task → PlanEntry.content
/// - TodoItem.done → PlanEntry.status (done=true → Completed, done=false → Pending)
/// - All entries get Medium priority by default
/// - TodoItem.id and context are stored in meta field
///
/// # Examples
///
/// ```rust
/// use swissarmyhammer_todo::{TodoItem, TodoItemExt};
/// use llama_agent::acp::plan::todos_to_acp_plan;
///
/// let todos = vec![
///     TodoItem::new_todo("Fix bug in parser".to_string(), Some("Check error handling".to_string())),
///     TodoItem::new_todo("Add tests".to_string(), None),
/// ];
///
/// let plan = todos_to_acp_plan(todos);
/// assert_eq!(plan.entries.len(), 2);
/// ```
pub fn todos_to_acp_plan(todos: Vec<TodoItem>) -> Plan {
    let entries = todos.into_iter().map(todo_to_plan_entry).collect();

    let mut meta = serde_json::Map::new();
    meta.insert(
        "source".to_string(),
        serde_json::json!("swissarmyhammer_todo"),
    );
    meta.insert("generator".to_string(), serde_json::json!("llama-agent"));

    Plan::new(entries).meta(meta)
}

/// Convert a single TodoItem to ACP PlanEntry
fn todo_to_plan_entry(todo: TodoItem) -> AcpPlanEntry {
    // TodoItem is an alias for PlanEntry from swissarmyhammer_todo
    // Access fields directly from the PlanEntry struct
    let plan_entry: &PlanEntry = &todo;

    // Map the swissarmyhammer PlanEntryStatus to ACP PlanEntryStatus
    let status = match &plan_entry.status {
        TodoStatus::Completed => PlanEntryStatus::Completed,
        TodoStatus::InProgress => PlanEntryStatus::InProgress,
        TodoStatus::Failed | TodoStatus::Cancelled => {
            // ACP doesn't have Failed or Cancelled, map them to Completed
            // since they represent terminal states
            PlanEntryStatus::Completed
        }
        TodoStatus::Pending => PlanEntryStatus::Pending,
    };

    // Map priority
    let priority = match &plan_entry.priority {
        Priority::High => PlanEntryPriority::High,
        Priority::Medium => PlanEntryPriority::Medium,
        Priority::Low => PlanEntryPriority::Low,
    };

    let mut meta = serde_json::Map::new();
    meta.insert("id".to_string(), serde_json::json!(plan_entry.id));
    meta.insert("notes".to_string(), serde_json::json!(plan_entry.notes));
    meta.insert(
        "original_status".to_string(),
        serde_json::json!(plan_entry.status),
    );

    // Timestamps in swissarmyhammer_todo are SystemTime, need to convert
    if let Some(created_at) = plan_entry.created_at {
        if let Ok(duration) = created_at.duration_since(std::time::UNIX_EPOCH) {
            let datetime = chrono::DateTime::<chrono::Utc>::from_timestamp(
                duration.as_secs() as i64,
                duration.subsec_nanos(),
            );
            if let Some(dt) = datetime {
                meta.insert("created_at".to_string(), serde_json::json!(dt.to_rfc3339()));
            }
        }
    }

    if let Some(updated_at) = plan_entry.updated_at {
        if let Ok(duration) = updated_at.duration_since(std::time::UNIX_EPOCH) {
            let datetime = chrono::DateTime::<chrono::Utc>::from_timestamp(
                duration.as_secs() as i64,
                duration.subsec_nanos(),
            );
            if let Some(dt) = datetime {
                meta.insert("updated_at".to_string(), serde_json::json!(dt.to_rfc3339()));
            }
        }
    }

    AcpPlanEntry::new(plan_entry.content.clone(), priority, status).meta(meta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_todo::TodoItemExt;

    #[test]
    fn test_todos_to_acp_plan_empty() {
        let plan = todos_to_acp_plan(vec![]);
        assert_eq!(plan.entries.len(), 0);
        assert!(plan.meta.is_some());
        let meta = plan.meta.unwrap();
        assert_eq!(meta.get("source").unwrap(), "swissarmyhammer_todo");
    }

    #[test]
    fn test_todos_to_acp_plan_single_pending() {
        let mut todo =
            TodoItem::new_todo("Test task".to_string(), Some("Test context".to_string()));
        todo.id = "01HXYZ123".to_string();

        let plan = todos_to_acp_plan(vec![todo]);
        assert_eq!(plan.entries.len(), 1);

        let entry = &plan.entries[0];
        assert_eq!(entry.content, "Test task");

        // Serialize and check the status value
        let status_json = serde_json::to_value(&entry.status).unwrap();
        assert_eq!(status_json, "pending");

        let priority_json = serde_json::to_value(&entry.priority).unwrap();
        assert_eq!(priority_json, "medium");

        let meta = entry.meta.as_ref().unwrap();
        assert_eq!(meta.get("id").unwrap(), "01HXYZ123");
        assert_eq!(
            meta.get("notes").unwrap(),
            &serde_json::json!(Some("Test context"))
        );
    }

    #[test]
    fn test_todos_to_acp_plan_single_completed() {
        let mut todo = TodoItem::new_todo("Completed task".to_string(), None);
        todo.id = "01HXYZ124".to_string();
        todo.mark_complete();

        let plan = todos_to_acp_plan(vec![todo]);
        assert_eq!(plan.entries.len(), 1);

        let entry = &plan.entries[0];
        assert_eq!(entry.content, "Completed task");

        let status_json = serde_json::to_value(&entry.status).unwrap();
        assert_eq!(status_json, "completed");

        let meta = entry.meta.as_ref().unwrap();
        // notes is None, so it serializes as null
        let notes_value = meta.get("notes").unwrap();
        assert!(notes_value.is_null() || notes_value == &serde_json::json!(None::<String>));
    }

    #[test]
    fn test_todos_to_acp_plan_multiple() {
        let mut todo1 = TodoItem::new_todo("First task".to_string(), Some("Context 1".to_string()));
        todo1.id = "01HXYZ125".to_string();

        let mut todo2 =
            TodoItem::new_todo("Second task".to_string(), Some("Context 2".to_string()));
        todo2.id = "01HXYZ126".to_string();
        todo2.mark_complete();

        let mut todo3 = TodoItem::new_todo("Third task".to_string(), None);
        todo3.id = "01HXYZ127".to_string();

        let todos = vec![todo1, todo2, todo3];

        let plan = todos_to_acp_plan(todos);
        assert_eq!(plan.entries.len(), 3);

        // Check first entry
        assert_eq!(plan.entries[0].content, "First task");
        let status_0 = serde_json::to_value(&plan.entries[0].status).unwrap();
        assert_eq!(status_0, "pending");

        // Check second entry
        assert_eq!(plan.entries[1].content, "Second task");
        let status_1 = serde_json::to_value(&plan.entries[1].status).unwrap();
        assert_eq!(status_1, "completed");

        // Check third entry
        assert_eq!(plan.entries[2].content, "Third task");
        let status_2 = serde_json::to_value(&plan.entries[2].status).unwrap();
        assert_eq!(status_2, "pending");
    }

    #[test]
    fn test_todo_to_plan_entry_preserves_all_fields() {
        let mut todo = TodoItem::new_todo(
            "Complete task".to_string(),
            Some("Important context".to_string()),
        );
        todo.id = "01HXYZ128".to_string();

        let entry = todo_to_plan_entry(todo.clone());

        assert_eq!(entry.content, todo.task());

        let meta = entry.meta.as_ref().unwrap();
        assert_eq!(meta.get("id").unwrap(), &serde_json::json!(todo.id));
        assert_eq!(meta.get("notes").unwrap(), &serde_json::json!(todo.notes));
        // Timestamps are present
        assert!(meta.contains_key("created_at"));
        assert!(meta.contains_key("updated_at"));
    }

    #[test]
    fn test_plan_metadata_includes_source() {
        let mut todo = TodoItem::new_todo("Test".to_string(), None);
        todo.id = "01HXYZ129".to_string();
        let todos = vec![todo];

        let plan = todos_to_acp_plan(todos);

        assert!(plan.meta.is_some());
        let meta = plan.meta.unwrap();
        assert_eq!(meta.get("source").unwrap(), "swissarmyhammer_todo");
        assert_eq!(meta.get("generator").unwrap(), "llama-agent");
    }
}
