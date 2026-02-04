# Derived MCP Tool Schema

## Status: ✅ Implemented

## Problem

The MCP tool schema is currently sparse and manually maintained:

```rust
fn schema(&self) -> serde_json::Value {
    json!({
        "type": "object",
        "additionalProperties": true,
        "description": "Kanban board operations...",
        "properties": {
            "op": { "type": "string", "description": "Operation to perform..." },
            "id": { "type": "string", "description": "ID of the task or column..." },
            "title": { "type": "string", "description": "Title for new tasks" },
            "name": { "type": "string", "description": "Name for boards or columns" },
            "description": { "type": "string", "description": "Description..." },
            "column": { "type": "string", "description": "Target column ID..." }
        }
    })
}
```

**Problems:**
1. **Incomplete** - Only documents ~6 parameters, missing dozens
2. **Redundant** - Parameters are already defined in operation structs
3. **Manual maintenance** - Easy to get out of sync
4. **Not discoverable** - Doesn't list all 40 operations
5. **Missing aliases** - Doesn't document forgiving input parsing

## Current Operation Metadata

Operations already have metadata via the `#[operation]` macro:

```rust
#[operation(verb = "add", noun = "task", description = "Create a new task")]
pub struct AddTask {
    pub title: String,
    pub description: Option<String>,
    pub position: Option<Position>,
    pub tags: Vec<TagId>,
    pub assignees: Vec<ActorId>,
    pub depends_on: Vec<TaskId>,
}
```

This is used for CLI generation but not for MCP schema.

## Solution: Generate Schema from Operations

### Schema Structure

The schema should document:
1. **Primary parameter: `op`** - The operation to perform
2. **All valid operations** - Enumerated list of 40 operations
3. **Operation-specific parameters** - Derived from operation structs
4. **Forgiving input** - Document aliases and inference rules

### Generated Schema Example

```json
{
  "type": "object",
  "additionalProperties": true,
  "description": "Kanban board operations for task management. Accepts forgiving input with multiple formats.",
  "properties": {
    "op": {
      "type": "string",
      "description": "Operation to perform (verb noun format)",
      "enum": [
        "init board", "get board", "update board",
        "add column", "get column", "update column", "delete column", "list columns",
        "add swimlane", "get swimlane", "update swimlane", "delete swimlane", "list swimlanes",
        "add actor", "get actor", "update actor", "delete actor", "list actors",
        "add task", "get task", "update task", "move task", "delete task", "next task", "complete task", "assign task", "list tasks",
        "tag task", "untag task",
        "add tag", "get tag", "update tag", "delete tag", "list tags",
        "add comment", "get comment", "update comment", "delete comment", "list comments",
        "list activity"
      ],
      "examples": [
        "add task",
        "move task",
        "list tasks",
        "complete task",
        "assign task"
      ]
    }
  },
  "oneOf": [
    {
      "title": "add task",
      "description": "Create a new task on the board",
      "properties": {
        "op": { "const": "add task" },
        "title": { "type": "string", "description": "Task title (required)" },
        "description": { "type": "string", "description": "Detailed description (optional)" },
        "assignees": { "type": "array", "items": { "type": "string" }, "description": "Actor IDs to assign (optional)" },
        "tags": { "type": "array", "items": { "type": "string" }, "description": "Tag IDs to apply (optional)" },
        "depends_on": { "type": "array", "items": { "type": "string" }, "description": "Task IDs this depends on (optional)" }
      },
      "required": ["title"]
    },
    {
      "title": "assign task",
      "description": "Assign an actor to a task",
      "properties": {
        "op": { "const": "assign task" },
        "id": { "type": "string", "description": "Task ID (required)" },
        "assignee": { "type": "string", "description": "Actor ID to assign (required)" }
      },
      "required": ["id", "assignee"]
    },
    // ... all 40 operations
  ],
  "examples": [
    {
      "description": "Initialize a board",
      "value": { "op": "init board", "name": "My Project" }
    },
    {
      "description": "Add a task",
      "value": { "op": "add task", "title": "Fix bug" }
    },
    {
      "description": "Assign task to agent",
      "value": { "op": "assign task", "id": "01ABC...", "assignee": "assistant" }
    },
    {
      "description": "Forgiving input - shorthand",
      "value": { "add": "task", "title": "Fix bug" }
    },
    {
      "description": "Forgiving input - inferred",
      "value": { "title": "Fix bug" }
    }
  ]
}
```

### Implementation: Schema Generator

Create a function that derives schema from operation metadata:

```rust
// In swissarmyhammer-kanban/src/schema.rs

use swissarmyhammer_operations::{Operation, ParamMeta};
use serde_json::{json, Value};

pub fn generate_mcp_schema(operations: &[&dyn Operation]) -> Value {
    // Collect all operation strings
    let op_values: Vec<String> = operations
        .iter()
        .map(|op| op.op_string())
        .collect();

    // Generate oneOf schemas for each operation
    let operation_schemas: Vec<Value> = operations
        .iter()
        .map(|op| generate_operation_schema(op))
        .collect();

    json!({
        "type": "object",
        "additionalProperties": true,
        "description": "Kanban board operations. Accepts forgiving input with aliases and inference.",
        "properties": {
            "op": {
                "type": "string",
                "description": "Operation to perform (verb noun format)",
                "enum": op_values,
            }
        },
        "oneOf": operation_schemas,
        "examples": generate_examples()
    })
}

fn generate_operation_schema(op: &dyn Operation) -> Value {
    let params = op.parameters();

    let mut properties = serde_json::Map::new();
    properties.insert(
        "op".to_string(),
        json!({
            "const": op.op_string(),
        })
    );

    let mut required = vec!["op".to_string()];

    for param in params {
        properties.insert(
            param.name.to_string(),
            json!({
                "type": param_type_to_json_type(&param.type_name),
                "description": format!(
                    "{} {}",
                    param.description,
                    if param.required { "(required)" } else { "(optional)" }
                ),
            })
        );

        if param.required {
            required.push(param.name.to_string());
        }
    }

    json!({
        "title": op.op_string(),
        "description": op.description(),
        "properties": properties,
        "required": required,
    })
}

fn param_type_to_json_type(rust_type: &str) -> &str {
    match rust_type {
        "String" | "TaskId" | "ActorId" | "ColumnId" | "TagId" | "SwimlaneId" => "string",
        "Vec<TaskId>" | "Vec<ActorId>" | "Vec<TagId>" => "array",
        "Option<String>" => "string",
        "bool" | "Option<bool>" => "boolean",
        "usize" | "Option<usize>" => "number",
        _ => "object",
    }
}

fn generate_examples() -> Vec<Value> {
    vec![
        json!({
            "description": "Initialize a board",
            "value": { "op": "init board", "name": "My Project" }
        }),
        json!({
            "description": "Add a task (explicit op)",
            "value": { "op": "add task", "title": "Fix bug" }
        }),
        json!({
            "description": "Add a task (shorthand)",
            "value": { "add": "task", "title": "Fix bug" }
        }),
        json!({
            "description": "Add a task (inferred from title)",
            "value": { "title": "Fix bug" }
        }),
        json!({
            "description": "Assign task",
            "value": { "op": "assign task", "id": "01ABC...", "assignee": "assistant" }
        }),
        json!({
            "description": "Move task (explicit)",
            "value": { "op": "move task", "id": "01ABC...", "column": "done" }
        }),
        json!({
            "description": "Move task (inferred)",
            "value": { "id": "01ABC...", "column": "done" }
        }),
        json!({
            "description": "Complete task",
            "value": { "op": "complete task", "id": "01ABC..." }
        }),
        json!({
            "description": "List my assigned tasks",
            "value": { "op": "list tasks", "assignee": "assistant" }
        }),
    ]
}
```

### Update MCP Tool

```rust
impl McpTool for KanbanTool {
    fn name(&self) -> &'static str {
        "kanban"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        // Generate schema from operation metadata
        swissarmyhammer_kanban::schema::generate_mcp_schema(&KANBAN_OPERATIONS)
    }

    // ...
}
```

## Operation Metadata Extraction

The `Operation` trait already provides metadata:

```rust
pub trait Operation {
    fn verb(&self) -> &'static str;
    fn noun(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn parameters(&self) -> &'static [ParamMeta];
    fn examples(&self) -> &'static [(&'static str, &'static str)];
}
```

The `#[operation]` macro populates these via reflection on the struct fields:

```rust
#[operation(verb = "add", noun = "task", description = "Create a new task")]
pub struct AddTask {
    pub title: String,                    // Required
    pub description: Option<String>,      // Optional
    pub assignees: Vec<ActorId>,         // Optional (default)
    pub depends_on: Vec<TaskId>,         // Optional (default)
}

// Macro generates:
impl Operation for AddTask {
    fn parameters(&self) -> &'static [ParamMeta] {
        &[
            ParamMeta {
                name: "title",
                type_name: "String",
                description: "",
                required: true,
            },
            ParamMeta {
                name: "description",
                type_name: "Option<String>",
                description: "",
                required: false,
            },
            ParamMeta {
                name: "assignees",
                type_name: "Vec<ActorId>",
                description: "",
                required: false,
            },
            ParamMeta {
                name: "depends_on",
                type_name: "Vec<TaskId>",
                description: "",
                required: false,
            },
        ]
    }
}
```

## Enhanced Schema Features

### 1. Document Verb Aliases

```json
{
  "properties": {
    "op": {
      "type": "string",
      "description": "Operation to perform. Supports aliases:\n- add/create/insert/new\n- get/show/read/fetch\n- update/edit/modify/set/patch\n- delete/remove/rm/del\n- list/ls/find/search/query\n- move/mv\n- complete/done/finish/close",
      "examples": [
        "add task",
        "create task",
        "new task",
        "move task",
        "mv task"
      ]
    }
  }
}
```

### 2. Document Shorthand Forms

```json
{
  "examples": [
    {
      "description": "Explicit op parameter",
      "value": { "op": "add task", "title": "Fix bug" }
    },
    {
      "description": "Verb and noun as separate fields",
      "value": { "verb": "add", "noun": "task", "title": "Fix bug" }
    },
    {
      "description": "Shorthand - verb as key",
      "value": { "add": "task", "title": "Fix bug" }
    },
    {
      "description": "Inferred from data - has title but no id",
      "value": { "title": "Fix bug" }
    }
  ]
}
```

### 3. Document Parameter Aliases

Each parameter should list its aliases:

```json
{
  "properties": {
    "id": {
      "type": "string",
      "description": "Resource ID",
      "aliases": ["taskId", "task_id", "taskID"]
    },
    "title": {
      "type": "string",
      "description": "Task title",
      "aliases": ["name", "summary"]
    },
    "description": {
      "type": "string",
      "description": "Detailed description",
      "aliases": ["desc", "body", "content"]
    }
  }
}
```

### 4. Group Operations by Category

```json
{
  "x-operations": {
    "board": ["init board", "get board", "update board"],
    "columns": ["add column", "get column", "update column", "delete column", "list columns"],
    "swimlanes": ["add swimlane", "get swimlane", "update swimlane", "delete swimlane", "list swimlanes"],
    "actors": ["add actor", "get actor", "update actor", "delete actor", "list actors"],
    "tasks": ["add task", "get task", "update task", "move task", "delete task", "next task", "complete task", "assign task", "list tasks", "tag task", "untag task"],
    "tags": ["add tag", "get tag", "update tag", "delete tag", "list tags"],
    "comments": ["add comment", "get comment", "update comment", "delete comment", "list comments"],
    "activity": ["list activity"]
  }
}
```

## Implementation

### File Structure

**File:** `swissarmyhammer-kanban/src/schema.rs` (new)
- `generate_mcp_schema()` - Main entry point
- `generate_operation_schema()` - Schema for one operation
- `generate_examples()` - Usage examples
- `param_type_to_json_type()` - Convert Rust types to JSON schema types
- `group_operations()` - Group by category

**File:** `swissarmyhammer-kanban/src/lib.rs`
```rust
pub mod schema;
```

**File:** `swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs`
```rust
fn schema(&self) -> serde_json::Value {
    swissarmyhammer_kanban::schema::generate_mcp_schema(&KANBAN_OPERATIONS)
}
```

### Schema Generator Implementation

```rust
// In swissarmyhammer-kanban/src/schema.rs

use serde_json::{json, Value};
use swissarmyhammer_operations::Operation;

/// Generate MCP tool schema from operation metadata
pub fn generate_mcp_schema(operations: &[&dyn Operation]) -> Value {
    let op_enum: Vec<String> = operations.iter().map(|op| op.op_string()).collect();
    let operation_schemas: Vec<Value> = operations.iter().map(|op| operation_to_schema(op)).collect();

    json!({
        "type": "object",
        "additionalProperties": true,
        "description": "Kanban board operations. Supports forgiving input with aliases and inference.",

        "properties": {
            "op": {
                "type": "string",
                "description": "Operation to perform (verb noun format). Supports aliases: add/create, get/show, update/edit, delete/remove, list/ls, move/mv, complete/done.",
                "enum": op_enum,
            }
        },

        "oneOf": operation_schemas,

        "examples": [
            {
                "description": "Initialize a board",
                "value": { "op": "init board", "name": "My Project" }
            },
            {
                "description": "Add task - explicit op",
                "value": { "op": "add task", "title": "Fix login bug" }
            },
            {
                "description": "Add task - shorthand",
                "value": { "add": "task", "title": "Fix login bug" }
            },
            {
                "description": "Add task - inferred from title",
                "value": { "title": "Fix login bug" }
            },
            {
                "description": "Register as an actor",
                "value": { "op": "add actor", "id": "assistant", "name": "Assistant", "type": "agent", "ensure": true }
            },
            {
                "description": "Assign task to yourself",
                "value": { "op": "assign task", "id": "01ABC...", "assignee": "assistant" }
            },
            {
                "description": "Move task - explicit",
                "value": { "op": "move task", "id": "01ABC...", "column": "doing" }
            },
            {
                "description": "Move task - inferred",
                "value": { "id": "01ABC...", "column": "doing" }
            },
            {
                "description": "Complete task",
                "value": { "op": "complete task", "id": "01ABC..." }
            },
            {
                "description": "List my tasks",
                "value": { "op": "list tasks", "assignee": "assistant", "exclude_done": true }
            },
        ],

        "x-operation-groups": {
            "board": ["init board", "get board", "update board"],
            "columns": ["add column", "get column", "update column", "delete column", "list columns"],
            "swimlanes": ["add swimlane", "get swimlane", "update swimlane", "delete swimlane", "list swimlanes"],
            "actors": ["add actor", "get actor", "update actor", "delete actor", "list actors"],
            "tasks": ["add task", "get task", "update task", "move task", "delete task", "next task", "complete task", "assign task", "list tasks", "tag task", "untag task"],
            "tags": ["add tag", "get tag", "update tag", "delete tag", "list tags"],
            "comments": ["add comment", "get comment", "update comment", "delete comment", "list comments"],
            "activity": ["list activity"]
        },

        "x-forgiving-input": {
            "description": "The tool accepts multiple input formats",
            "formats": [
                "Explicit op: { \"op\": \"add task\", \"title\": \"Fix bug\" }",
                "Shorthand: { \"add\": \"task\", \"title\": \"Fix bug\" }",
                "Verb+noun fields: { \"verb\": \"add\", \"noun\": \"task\", \"title\": \"Fix bug\" }",
                "Inferred: { \"title\": \"Fix bug\" } (infers 'add task')"
            ],
            "aliases": {
                "verbs": {
                    "add": ["create", "insert", "new"],
                    "get": ["show", "read", "fetch"],
                    "update": ["edit", "modify", "set", "patch"],
                    "delete": ["remove", "rm", "del"],
                    "list": ["ls", "find", "search", "query"],
                    "move": ["mv"],
                    "complete": ["done", "finish", "close"]
                },
                "parameters": {
                    "id": ["taskId", "task_id", "taskID"],
                    "description": ["desc", "body", "content"],
                    "assignee": ["assignees", "assign_to", "actor"]
                }
            }
        }
    })
}

fn operation_to_schema(op: &dyn Operation) -> Value {
    let params = op.parameters();

    let mut properties = serde_json::Map::new();
    let mut required = vec!["op"];

    // Op field is always const for this specific operation
    properties.insert(
        "op".to_string(),
        json!({ "const": op.op_string() })
    );

    // Add each parameter
    for param in params {
        let json_type = match param.type_name {
            "String" => "string",
            "Option<String>" => "string",
            "Vec<ActorId>" | "Vec<TaskId>" | "Vec<TagId>" => "array",
            "bool" | "Option<bool>" => "boolean",
            "usize" | "Option<usize>" => "number",
            _ => "string",  // Default to string for complex types
        };

        properties.insert(
            param.name.to_string(),
            json!({
                "type": json_type,
                "description": param.description,
            })
        );

        if param.required {
            required.push(param.name);
        }
    }

    json!({
        "title": op.op_string(),
        "description": op.description(),
        "type": "object",
        "properties": properties,
        "required": required,
    })
}
```

### Update KANBAN_OPERATIONS Static

The static already exists and lists all 40 operations:

```rust
static KANBAN_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| {
    vec![
        &*INIT_BOARD as &dyn Operation,
        &*GET_BOARD as &dyn Operation,
        // ... all 40 operations
    ]
});
```

This is the single source of truth.

### Use in MCP Tool

```rust
impl McpTool for KanbanTool {
    fn schema(&self) -> serde_json::Value {
        swissarmyhammer_kanban::schema::generate_mcp_schema(&KANBAN_OPERATIONS)
    }
}
```

## Benefits

1. **Single source of truth** - Operations define their own parameters
2. **No manual sync** - Schema auto-updates when operations change
3. **Complete** - All 40 operations documented automatically
4. **Discoverable** - Clients can see all operations and parameters
5. **Examples included** - Shows forgiving input patterns

## Testing

```rust
#[test]
fn test_schema_includes_all_operations() {
    let schema = generate_mcp_schema(&KANBAN_OPERATIONS);

    let op_enum = schema["properties"]["op"]["enum"].as_array().unwrap();
    assert_eq!(op_enum.len(), 40);

    assert!(op_enum.contains(&json!("add task")));
    assert!(op_enum.contains(&json!("assign task")));
    assert!(op_enum.contains(&json!("complete task")));
}

#[test]
fn test_schema_has_operation_schemas() {
    let schema = generate_mcp_schema(&KANBAN_OPERATIONS);

    let one_of = schema["oneOf"].as_array().unwrap();
    assert_eq!(one_of.len(), 40);
}

#[test]
fn test_add_task_schema() {
    let schema = generate_mcp_schema(&KANBAN_OPERATIONS);

    let add_task_schema = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["title"] == "add task")
        .unwrap();

    assert_eq!(add_task_schema["description"], "Create a new task on the board");
    assert!(add_task_schema["required"].as_array().unwrap().contains(&json!("title")));
    assert!(add_task_schema["properties"]["description"].is_object());
}

#[test]
fn test_schema_includes_examples() {
    let schema = generate_mcp_schema(&KANBAN_OPERATIONS);

    let examples = schema["examples"].as_array().unwrap();
    assert!(examples.len() > 5);

    // Check for forgiving input examples
    let has_inferred = examples.iter().any(|ex| {
        ex["description"].as_str().unwrap().contains("inferred")
    });
    assert!(has_inferred);
}
```

## Required Changes

### 1. Create schema module in kanban crate

**File:** `swissarmyhammer-kanban/src/schema.rs` (new)
- Implement `generate_mcp_schema()`
- Implement `operation_to_schema()`
- Implement `generate_examples()`
- Add tests

### 2. Update MCP tool

**File:** `swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs`
```rust
fn schema(&self) -> serde_json::Value {
    swissarmyhammer_kanban::schema::generate_mcp_schema(&KANBAN_OPERATIONS)
}
```

Remove hardcoded schema, use generated one.

### 3. Enhance operation macro (optional)

**File:** `swissarmyhammer-operations-macro/src/lib.rs`

Add support for parameter descriptions:

```rust
#[operation(verb = "add", noun = "task", description = "Create a new task")]
pub struct AddTask {
    /// Task title (required)
    pub title: String,

    /// Detailed description supporting markdown (optional)
    pub description: Option<String>,

    /// Actor IDs to assign to this task (optional)
    pub assignees: Vec<ActorId>,
}
```

Parse doc comments and include in `ParamMeta`.

## Priority

**HIGH** - This significantly improves discoverability and documentation.

Should be implemented after:
1. Activity logging (priority 1)
2. Before other new operations (so schema auto-updates)

## Dependencies

- Requires `KANBAN_OPERATIONS` static (already exists)
- Requires `Operation::parameters()` metadata (already exists)
- No new external dependencies

## File Changes Summary

**Created:**
- `swissarmyhammer-kanban/src/schema.rs`

**Modified:**
- `swissarmyhammer-kanban/src/lib.rs` - Export schema module
- `swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` - Use generated schema

**Optional:**
- `swissarmyhammer-operations-macro/src/lib.rs` - Parse doc comments for param descriptions

## Example Output

When an LLM calls the kanban tool, it sees:

```json
{
  "name": "kanban",
  "description": "Kanban board operations for task management...",
  "inputSchema": {
    "type": "object",
    "properties": {
      "op": {
        "enum": [
          "init board", "get board", "update board",
          "add task", "assign task", "complete task",
          // ... all 40 operations
        ]
      }
    },
    "oneOf": [
      {
        "title": "add task",
        "properties": {
          "op": { "const": "add task" },
          "title": { "type": "string", "description": "Task title (required)" },
          "description": { "type": "string", "description": "Detailed description (optional)" },
          "assignees": { "type": "array", "description": "Actor IDs (optional)" }
        },
        "required": ["op", "title"]
      },
      // ... schemas for all 40 operations
    ],
    "examples": [
      // ... comprehensive examples
    ]
  }
}
```

## Summary

**Current state**: Sparse, manually maintained schema with ~6 parameters

**After implementation**:
- Complete schema with all 40 operations
- All parameters documented
- Examples showing forgiving input
- Automatically derived from operations
- Never gets out of sync
