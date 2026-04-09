//! Kanban-specific MCP schema generation
//!
//! This module provides kanban-specific configuration for MCP schema generation,
//! including examples and verb aliases tailored to kanban board operations.

use serde_json::{json, Map, Value};
use std::sync::LazyLock;
use swissarmyhammer_operations::{generate_mcp_schema, Operation, SchemaConfig};

use crate::actor::{AddActor, DeleteActor, GetActor, ListActors, UpdateActor};
use crate::attachment::{
    AddAttachment, DeleteAttachment, GetAttachment, ListAttachments, UpdateAttachment,
};
use crate::board::{GetBoard, InitBoard, UpdateBoard};
use crate::column::{AddColumn, DeleteColumn, GetColumn, ListColumns, UpdateColumn};
use crate::perspective::{
    AddPerspective, DeletePerspective, GetPerspective, ListPerspectives, UpdatePerspective,
};
use crate::project::{AddProject, DeleteProject, GetProject, ListProjects, UpdateProject};
use crate::tag::{AddTag, DeleteTag, GetTag, ListTags, UpdateTag};
use crate::task::{
    AddTask, ArchiveTask, AssignTask, CompleteTask, DeleteTask, GetTask, ListArchived, ListTasks,
    MoveTask, NextTask, TagTask, UnarchiveTask, UnassignTask, UntagTask, UpdateTask,
};

/// All kanban operations — the canonical list used for schema generation and CLI.
static KANBAN_OPERATIONS: LazyLock<Vec<&'static dyn Operation>> = LazyLock::new(|| {
    vec![
        // Board
        Box::leak(Box::new(InitBoard::new(""))) as &dyn Operation,
        Box::leak(Box::new(GetBoard::default())) as &dyn Operation,
        Box::leak(Box::new(UpdateBoard::new())) as &dyn Operation,
        // Column
        Box::leak(Box::new(AddColumn::new("", ""))) as &dyn Operation,
        Box::leak(Box::new(GetColumn::new(""))) as &dyn Operation,
        Box::leak(Box::new(UpdateColumn::new(""))) as &dyn Operation,
        Box::leak(Box::new(DeleteColumn::new(""))) as &dyn Operation,
        Box::leak(Box::new(ListColumns)) as &dyn Operation,
        // Actor
        Box::leak(Box::new(AddActor::new("", ""))) as &dyn Operation,
        Box::leak(Box::new(GetActor::new(""))) as &dyn Operation,
        Box::leak(Box::new(UpdateActor::new(""))) as &dyn Operation,
        Box::leak(Box::new(DeleteActor::new(""))) as &dyn Operation,
        Box::leak(Box::new(ListActors)) as &dyn Operation,
        // Task
        Box::leak(Box::new(AddTask::new(""))) as &dyn Operation,
        Box::leak(Box::new(GetTask::new(""))) as &dyn Operation,
        Box::leak(Box::new(UpdateTask::new(""))) as &dyn Operation,
        Box::leak(Box::new(DeleteTask::new(""))) as &dyn Operation,
        Box::leak(Box::new(MoveTask::to_column("", ""))) as &dyn Operation,
        Box::leak(Box::new(CompleteTask::new(""))) as &dyn Operation,
        Box::leak(Box::new(AssignTask::new("", ""))) as &dyn Operation,
        Box::leak(Box::new(UnassignTask::new("", ""))) as &dyn Operation,
        Box::leak(Box::new(NextTask::new())) as &dyn Operation,
        Box::leak(Box::new(TagTask::new("", ""))) as &dyn Operation,
        Box::leak(Box::new(UntagTask::new("", ""))) as &dyn Operation,
        Box::leak(Box::new(ListTasks::new())) as &dyn Operation,
        Box::leak(Box::new(ArchiveTask::new(""))) as &dyn Operation,
        Box::leak(Box::new(UnarchiveTask::new(""))) as &dyn Operation,
        Box::leak(Box::new(ListArchived)) as &dyn Operation,
        // Tag
        Box::leak(Box::new(AddTag::new(""))) as &dyn Operation,
        Box::leak(Box::new(GetTag::new(""))) as &dyn Operation,
        Box::leak(Box::new(UpdateTag::new(""))) as &dyn Operation,
        Box::leak(Box::new(DeleteTag::new(""))) as &dyn Operation,
        Box::leak(Box::new(ListTags::default())) as &dyn Operation,
        // Attachment
        Box::leak(Box::new(AddAttachment::new("", "", ""))) as &dyn Operation,
        Box::leak(Box::new(GetAttachment::new("", ""))) as &dyn Operation,
        Box::leak(Box::new(UpdateAttachment::new("", ""))) as &dyn Operation,
        Box::leak(Box::new(DeleteAttachment::new("", ""))) as &dyn Operation,
        Box::leak(Box::new(ListAttachments::new(""))) as &dyn Operation,
        // Project
        Box::leak(Box::new(AddProject::new("", ""))) as &dyn Operation,
        Box::leak(Box::new(GetProject::new(""))) as &dyn Operation,
        Box::leak(Box::new(UpdateProject::new(""))) as &dyn Operation,
        Box::leak(Box::new(DeleteProject::new(""))) as &dyn Operation,
        Box::leak(Box::new(ListProjects)) as &dyn Operation,
        // Perspective
        Box::leak(Box::new(AddPerspective::new("", ""))) as &dyn Operation,
        Box::leak(Box::new(GetPerspective::new(""))) as &dyn Operation,
        Box::leak(Box::new(UpdatePerspective::new(""))) as &dyn Operation,
        Box::leak(Box::new(DeletePerspective::new(""))) as &dyn Operation,
        Box::leak(Box::new(ListPerspectives::new())) as &dyn Operation,
    ]
});

/// Get the canonical list of all kanban operations.
pub fn kanban_operations() -> &'static [&'static dyn Operation] {
    &KANBAN_OPERATIONS
}

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
            "description": "Register an actor",
            "value": {"op": "add actor", "id": "alice", "name": "Alice Smith"}
        }),
        json!({
            "description": "Assign task to an actor",
            "value": {"op": "assign task", "id": "01ABC...", "assignee": "alice"}
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
            "value": {"op": "list tasks", "assignee": "alice", "exclude_done": true}
        }),
        json!({
            "description": "Add attachment to a task",
            "value": {"op": "add attachment", "task_id": "01ABC...", "name": "screenshot.png", "path": "/path/to/screenshot.png"}
        }),
        json!({
            "description": "Add a perspective",
            "value": {"op": "add perspective", "name": "Active Sprint", "view": "board"}
        }),
        json!({
            "description": "List all perspectives",
            "value": {"op": "list perspectives"}
        }),
    ]
}

/// Get kanban verb aliases for documentation
fn get_kanban_verb_aliases() -> Map<String, Value> {
    let mut aliases = Map::new();

    aliases.insert("add".to_string(), json!(["create", "insert", "new"]));
    aliases.insert("get".to_string(), json!(["show", "read", "fetch"]));
    aliases.insert(
        "update".to_string(),
        json!(["edit", "modify", "set", "patch"]),
    );
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
            Box::leak(Box::new(AddActor::new("", ""))) as &dyn Operation,
            Box::leak(Box::new(ListActors)) as &dyn Operation,
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

        let has_assign = examples
            .iter()
            .any(|ex| ex["description"].as_str().unwrap_or("").contains("Assign"));
        assert!(has_assign);
    }

    #[test]
    fn test_kanban_schema_has_verb_aliases() {
        let ops = test_operations();
        let schema = generate_kanban_mcp_schema(&ops);

        assert!(schema["x-forgiving-input"]["verb_aliases"].is_object());

        let aliases = schema["x-forgiving-input"]["verb_aliases"]
            .as_object()
            .unwrap();
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

    #[test]
    fn test_schema_includes_perspective_ops() {
        let ops = kanban_operations();
        let schema = generate_kanban_mcp_schema(ops);

        let op_enum = schema["properties"]["op"]["enum"]
            .as_array()
            .expect("op enum should be an array");
        let op_strings: Vec<&str> = op_enum.iter().filter_map(|v| v.as_str()).collect();

        let expected = [
            "add perspective",
            "get perspective",
            "update perspective",
            "delete perspective",
            "list perspectives",
        ];
        for expected_op in &expected {
            assert!(
                op_strings.contains(expected_op),
                "op enum should contain {:?}, got: {:?}",
                expected_op,
                op_strings
            );
        }
    }

    #[test]
    fn test_schema_has_perspective_examples() {
        let ops = kanban_operations();
        let schema = generate_kanban_mcp_schema(ops);

        let examples = schema["examples"]
            .as_array()
            .expect("examples should be an array");

        let has_perspective_example = examples.iter().any(|ex| {
            let desc = ex["description"].as_str().unwrap_or("");
            let op_val = ex["value"]["op"].as_str().unwrap_or("");
            desc.to_lowercase().contains("perspective") || op_val.contains("perspective")
        });

        assert!(
            has_perspective_example,
            "schema examples should include at least one perspective example"
        );
    }

    #[test]
    fn test_kanban_operations_returns_full_list() {
        let ops = kanban_operations();

        assert!(
            !ops.is_empty(),
            "kanban_operations() should return a non-empty list"
        );

        let op_names: Vec<String> = ops.iter().map(|op| op.op_string()).collect();
        let op_names: Vec<&str> = op_names.iter().map(|s| s.as_str()).collect();

        assert!(op_names.contains(&"init board"), "Missing 'init board'");
        assert!(op_names.contains(&"get board"), "Missing 'get board'");
        assert!(op_names.contains(&"update board"), "Missing 'update board'");
        assert!(op_names.contains(&"add column"), "Missing 'add column'");
        assert!(op_names.contains(&"list columns"), "Missing 'list columns'");
        assert!(op_names.contains(&"add actor"), "Missing 'add actor'");
        assert!(op_names.contains(&"list actors"), "Missing 'list actors'");
        assert!(op_names.contains(&"add task"), "Missing 'add task'");
        assert!(
            op_names.contains(&"complete task"),
            "Missing 'complete task'"
        );
        assert!(op_names.contains(&"move task"), "Missing 'move task'");
        assert!(op_names.contains(&"next task"), "Missing 'next task'");
        assert!(op_names.contains(&"list tasks"), "Missing 'list tasks'");
        assert!(op_names.contains(&"add tag"), "Missing 'add tag'");
        assert!(op_names.contains(&"list tags"), "Missing 'list tags'");
        assert!(op_names.contains(&"add project"), "Missing 'add project'");
        assert!(
            op_names.contains(&"list projects"),
            "Missing 'list projects'"
        );
    }

    #[test]
    fn test_kanban_operations_generates_valid_schema() {
        let ops = kanban_operations();
        let schema = generate_kanban_mcp_schema(ops);

        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], true);
        assert!(schema["description"].as_str().unwrap().contains("Kanban"));

        let op_enum = schema["properties"]["op"]["enum"].as_array().unwrap();
        assert!(!op_enum.is_empty(), "op enum should not be empty");

        let enum_strs: Vec<&str> = op_enum.iter().filter_map(|v| v.as_str()).collect();
        assert!(enum_strs.contains(&"init board"));
        assert!(enum_strs.contains(&"add task"));
        assert!(enum_strs.contains(&"complete task"));

        for op in ops {
            let op_name = op.op_string();
            assert!(
                enum_strs.contains(&op_name.as_str()),
                "Operation '{}' missing from schema enum",
                op_name
            );
        }

        let op_schemas = schema["x-operation-schemas"].as_array().unwrap();
        assert_eq!(
            op_schemas.len(),
            ops.len(),
            "x-operation-schemas count should match number of operations"
        );
    }

    #[test]
    fn test_kanban_operations_is_static() {
        let ops1 = kanban_operations();
        let ops2 = kanban_operations();

        assert_eq!(
            ops1.as_ptr(),
            ops2.as_ptr(),
            "kanban_operations() should return the same static reference"
        );
        assert_eq!(ops1.len(), ops2.len());
    }
}
