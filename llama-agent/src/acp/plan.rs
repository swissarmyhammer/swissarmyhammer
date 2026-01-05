//! Agent plan protocol support
//!
//! This module handles conversion between swissarmyhammer's simple todo system
//! and ACP (Agent Client Protocol) plan format.
//!
//! # Important Design Note
//!
//! The `TodoItem` from `swissarmyhammer-todo` is intentionally a SIMPLE struct
//! designed for ephemeral session-based task tracking:
//! - `id`: ULID identifier
//! - `task`: Brief task description
//! - `context`: Optional implementation notes
//! - `done`: Boolean completion status
//! - `created_at`/`updated_at`: Timestamps
//!
//! The ACP `PlanEntry` is a DIFFERENT, more complex type for agent protocol
//! communication that includes:
//! - `content`: Task description (mapped from `task`)
//! - `status`: Pending/InProgress/Completed enum (mapped from `done`)
//! - `priority`: High/Medium/Low (always Medium for todos)
//! - `meta`: Additional metadata
//!
//! These types are intentionally separate. The todo system should remain
//! simple and focused on ephemeral task tracking. This module provides
//! the mapping layer for ACP protocol integration.

use agent_client_protocol::{Plan, PlanEntry as AcpPlanEntry, PlanEntryPriority, PlanEntryStatus};
use swissarmyhammer_todo::TodoItem;

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
/// use swissarmyhammer_todo::TodoItem;
/// use llama_agent::acp::plan::todos_to_acp_plan;
///
/// let todos = vec![
///     TodoItem::new("Fix bug in parser".to_string(), Some("Check error handling".to_string())),
///     TodoItem::new("Add tests".to_string(), None),
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
///
/// Maps the simple todo structure to the ACP protocol format.
/// Note: TodoItem.id is a TodoId type, timestamps are DateTime<Utc>.
fn todo_to_plan_entry(todo: TodoItem) -> AcpPlanEntry {
    // Map done boolean to ACP status
    let status = if todo.done {
        PlanEntryStatus::Completed
    } else {
        PlanEntryStatus::Pending
    };

    // All todos get Medium priority (simple todo format doesn't have priority)
    let priority = PlanEntryPriority::Medium;

    let mut meta = serde_json::Map::new();
    // TodoItem.id is a TodoId, use as_str() to get the string representation
    meta.insert("id".to_string(), serde_json::json!(todo.id.as_str()));
    meta.insert("context".to_string(), serde_json::json!(todo.context));

    // Timestamps are DateTime<Utc>, not Option
    meta.insert(
        "created_at".to_string(),
        serde_json::json!(todo.created_at.to_rfc3339()),
    );
    meta.insert(
        "updated_at".to_string(),
        serde_json::json!(todo.updated_at.to_rfc3339()),
    );

    AcpPlanEntry::new(todo.task.clone(), priority, status).meta(meta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_todo::TodoId;

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
        let mut todo = TodoItem::new("Test task".to_string(), Some("Test context".to_string()));
        todo.id = TodoId::from_string("01HXYZ123".to_string()).unwrap();

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
            meta.get("context").unwrap(),
            &serde_json::json!(Some("Test context"))
        );
    }

    #[test]
    fn test_todos_to_acp_plan_single_completed() {
        let mut todo = TodoItem::new("Completed task".to_string(), None);
        todo.id = TodoId::from_string("01HXYZ124".to_string()).unwrap();
        todo.mark_complete();

        let plan = todos_to_acp_plan(vec![todo]);
        assert_eq!(plan.entries.len(), 1);

        let entry = &plan.entries[0];
        assert_eq!(entry.content, "Completed task");

        let status_json = serde_json::to_value(&entry.status).unwrap();
        assert_eq!(status_json, "completed");

        let meta = entry.meta.as_ref().unwrap();
        // context is None, so it serializes as null
        let context_value = meta.get("context").unwrap();
        assert!(context_value.is_null() || context_value == &serde_json::json!(None::<String>));
    }

    #[test]
    fn test_todos_to_acp_plan_multiple() {
        let mut todo1 = TodoItem::new("First task".to_string(), Some("Context 1".to_string()));
        todo1.id = TodoId::from_string("01HXYZ125".to_string()).unwrap();

        let mut todo2 = TodoItem::new("Second task".to_string(), Some("Context 2".to_string()));
        todo2.id = TodoId::from_string("01HXYZ126".to_string()).unwrap();
        todo2.mark_complete();

        let mut todo3 = TodoItem::new("Third task".to_string(), None);
        todo3.id = TodoId::from_string("01HXYZ127".to_string()).unwrap();

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
        let todo = TodoItem::new(
            "Complete task".to_string(),
            Some("Important context".to_string()),
        );

        let entry = todo_to_plan_entry(todo.clone());

        assert_eq!(entry.content, todo.task);

        let meta = entry.meta.as_ref().unwrap();
        assert_eq!(
            meta.get("id").unwrap(),
            &serde_json::json!(todo.id.as_str())
        );
        assert_eq!(
            meta.get("context").unwrap(),
            &serde_json::json!(todo.context)
        );
        // Timestamps are present
        assert!(meta.contains_key("created_at"));
        assert!(meta.contains_key("updated_at"));
    }

    #[test]
    fn test_plan_metadata_includes_source() {
        let todo = TodoItem::new("Test".to_string(), None);
        let todos = vec![todo];

        let plan = todos_to_acp_plan(todos);

        assert!(plan.meta.is_some());
        let meta = plan.meta.unwrap();
        assert_eq!(meta.get("source").unwrap(), "swissarmyhammer_todo");
        assert_eq!(meta.get("generator").unwrap(), "llama-agent");
    }
}
