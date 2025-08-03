# Implement Todo Create MCP Tool

Refer to ./specification/todo_tool.md

## Overview
Implement the `todo_create` MCP tool for adding new items to todo lists.

## Tool Specification
- **Name**: `todo_create`
- **Purpose**: Add a new item to the todo list
- **Parameters**:
  - `todo_list` (required): Name/path of the todo list file
  - `task` (required): Brief description of the task
  - `context` (optional): Additional context or implementation notes

## Implementation Tasks
1. Create `CreateTodoTool` struct in `todo/create/mod.rs`
2. Implement `McpTool` trait with:
   - `name()`: Return "todo_create"
   - `description()`: Load from description.md
   - `schema()`: JSON schema for parameters
   - `execute()`: Async implementation

3. Create comprehensive `description.md` file

4. Implement tool logic:
   - Parse and validate arguments
   - Load existing todo list or create new one
   - Generate sequential ULID for new item
   - Add item with `done: false` status
   - Save updated todo list
   - Return success response

## Error Handling
- Invalid file paths
- File system permission errors
- YAML parsing/serialization errors
- Missing required parameters
- Invalid parameter types

## Testing
- Unit tests for tool creation and schema
- Integration tests for successful creation
- Error condition tests
- File system isolation tests

## Success Criteria
- Tool compiles and registers correctly
- Creates new todo lists when they don't exist
- Appends to existing todo lists
- Generates sequential ULIDs
- Proper error handling and logging
- Comprehensive test coverage

## Implementation Notes
- Follow patterns from `memoranda/create/mod.rs`
- Use existing file system utilities
- Integrate with SwissArmyHammerError system
- Add proper tracing/logging statements