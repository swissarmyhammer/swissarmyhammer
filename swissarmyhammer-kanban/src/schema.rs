//! Kanban-specific MCP schema generation
//!
//! This module provides kanban-specific configuration for MCP schema generation,
//! including examples and verb aliases tailored to kanban board operations.

use serde_json::{json, Map, Value};
use swissarmyhammer_operations::{generate_mcp_schema, Operation, SchemaConfig};

/// Generate MCP schema for kanban operations
///
/// Uses the generic schema generator from swissarmyhammer-operations with
/// kanban-specific examples and verb aliases.
pub fn generate_kanban_mcp_schema(operations: &[&dyn Operation]) -> Value {
    let config = SchemaConfig::new(
        "Kanban board operations for task management. Accepts forgiving input with aliases and inference.",
    )
    .with_examples(generate_kanban_examples())
    .with_verb_aliases(get_kanban_verb_aliases());

    generate_mcp_schema(operations, config)
}

/// Generate kanban-specific usage examples
fn generate_kanban_examples() -> Vec<Value> {
    vec![
        json!({
            "description": "Initialize a board",
            "value": {"op": "init board", "name": "My Project"}
        }),
        json!({
            "description": "Add task - explicit op",
            "value": {"op": "add task", "title": "Fix login bug"}
        }),
        json!({
            "description": "Add task - shorthand",
            "value": {"add": "task", "title": "Fix login bug"}
        }),
        json!({
            "description": "Add task - inferred from title",
            "value": {"title": "Fix login bug"}
        }),
        json!({
            "description": "Register as an actor",
            "value": {"op": "add actor", "id": "assistant", "name": "Assistant", "actor_type": "agent", "ensure": true}
        }),
        json!({
            "description": "Assign task to yourself",
            "value": {"op": "assign task", "id": "01ABC...", "assignee": "assistant"}
        }),
        json!({
            "description": "Move task - explicit",
            "value": {"op": "move task", "id": "01ABC...", "column": "doing"}
        }),
        json!({
            "description": "Move task - inferred",
            "value": {"id": "01ABC...", "column": "doing"}
        }),
        json!({
            "description": "Complete task",
            "value": {"op": "complete task", "id": "01ABC..."}
        }),
        json!({
            "description": "List my assigned tasks",
            "value": {"op": "list tasks", "assignee": "assistant", "exclude_done": true}
        }),
        json!({
            "description": "Add subtask to a task",
            "value": {"op": "add subtask", "task_id": "01ABC...", "title": "Review code"}
        }),
        json!({
            "description": "Add attachment to a task",
            "value": {"op": "add attachment", "task_id": "01ABC...", "name": "screenshot.png", "path": "/path/to/screenshot.png"}
        }),
    ]
}

/// Get kanban verb aliases for documentation
fn get_kanban_verb_aliases() -> Map<String, Value> {
    let mut aliases = Map::new();

    aliases.insert("add".to_string(), json!(["create", "insert", "new"]));
    aliases.insert("get".to_string(), json!(["show", "read", "fetch"]));
    aliases.insert("update".to_string(), json!(["edit", "modify", "set", "patch"]));
    aliases.insert("delete".to_string(), json!(["remove", "rm", "del"]));
    aliases.insert("list".to_string(), json!(["ls", "find", "search", "query"]));
    aliases.insert("move".to_string(), json!(["mv"]));
    aliases.insert("complete".to_string(), json!(["done", "finish", "close"]));

    aliases
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        activity::ListActivity,
        actor::{AddActor, ListActors},
        board::InitBoard,
        task::{AddTask, AssignTask, ListTasks},
    };

    // Helper to create a test operation list with static lifetime
    fn test_operations() -> Vec<&'static dyn Operation> {
        vec![
            Box::leak(Box::new(InitBoard::new(""))) as &dyn Operation,
            Box::leak(Box::new(AddTask::new(""))) as &dyn Operation,
            Box::leak(Box::new(AssignTask::new("", ""))) as &dyn Operation,
            Box::leak(Box::new(ListTasks::new())) as &dyn Operation,
            Box::leak(Box::new(AddActor::human("", ""))) as &dyn Operation,
            Box::leak(Box::new(ListActors::default())) as &dyn Operation,
            Box::leak(Box::new(ListActivity::default())) as &dyn Operation,
        ]
    }

    #[test]
    fn test_generate_kanban_schema_structure() {
        let ops = test_operations();
        let schema = generate_kanban_mcp_schema(&ops);

        // Verify top-level structure
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], true);
        assert!(schema["description"].as_str().unwrap().contains("Kanban"));

        // Verify properties.op exists with enum
        assert!(schema["properties"]["op"].is_object());
        assert_eq!(schema["properties"]["op"]["type"], "string");
        assert!(schema["properties"]["op"]["enum"].is_array());

        // Verify x-operation-schemas exists
        assert!(schema["x-operation-schemas"].is_array());

        // Verify examples exist
        assert!(schema["examples"].is_array());

        // Verify extension fields exist
        assert!(schema["x-operation-groups"].is_object());
        assert!(schema["x-forgiving-input"].is_object());
    }

    #[test]
    fn test_kanban_schema_has_examples() {
        let ops = test_operations();
        let schema = generate_kanban_mcp_schema(&ops);

        let examples = schema["examples"].as_array().unwrap();
        assert!(examples.len() >= 10);

        // Check for kanban-specific examples
        let has_init = examples.iter().any(|ex| {
            ex["description"]
                .as_str()
                .unwrap_or("")
                .contains("Initialize")
        });
        assert!(has_init);

        let has_assign = examples.iter().any(|ex| {
            ex["description"]
                .as_str()
                .unwrap_or("")
                .contains("Assign")
        });
        assert!(has_assign);
    }

    #[test]
    fn test_kanban_schema_has_verb_aliases() {
        let ops = test_operations();
        let schema = generate_kanban_mcp_schema(&ops);

        assert!(schema["x-forgiving-input"]["verb_aliases"].is_object());

        let aliases = schema["x-forgiving-input"]["verb_aliases"].as_object().unwrap();
        assert!(aliases.contains_key("add"));
        assert!(aliases.contains_key("complete"));
        assert!(aliases.contains_key("move"));
    }

    #[test]
    fn test_no_top_level_oneof() {
        // Critical: Claude API doesn't support oneOf/allOf/anyOf at top level
        let ops = test_operations();
        let schema = generate_kanban_mcp_schema(&ops);

        assert!(!schema.as_object().unwrap().contains_key("oneOf"));
        assert!(!schema.as_object().unwrap().contains_key("allOf"));
        assert!(!schema.as_object().unwrap().contains_key("anyOf"));
    }
}
