# Implement Todo Mark Complete MCP Tool

Refer to ./specification/todo_tool.md

## Overview
Implement the `todo_mark_complete` MCP tool for marking todo items as completed while preserving the full file for status tracking.

## Tool Specification
- **Name**: `todo_mark_complete`
- **Purpose**: Mark a todo item as completed
- **Parameters**:
  - `todo_list` (required): Name/path of the todo list file
  - `id` (required): ULID of the completed todo item

## Implementation Tasks
1. Create `MarkCompleteTodoTool` struct in `todo/mark_complete/mod.rs`
2. Implement `McpTool` trait with:
   - `name()`: Return "todo_mark_complete"
   - `description()`: Load from description.md
   - `schema()`: JSON schema for parameters
   - `execute()`: Async implementation

3. Create comprehensive `description.md` file

4. Implement tool logic:
   - Parse and validate arguments
   - Load todo list from file
   - Find item by ULID
   - Update `done: true` for the item
   - Preserve all other items and data
   - Save updated todo list
   - Return success confirmation

## Preservation Strategy
- Keep all completed items in the file
- Maintain original ordering
- Preserve all item fields (task, context)
- Only modify the `done` field
- Full file preservation useful for debugging and status tracking

## Error Handling
- Todo list file not found
- Invalid ULID format
- ULID not found in list
- Item already marked complete
- YAML parsing/serialization errors
- File system permission errors

## Response Format
Return success message with item details:
```
Successfully marked todo item as complete:
ID: 01K1KQM85501ECE8XJGNZKNJQW
Task: "Implement file read tool"
```

## Testing
- Unit tests for tool creation and schema
- Successful completion marking tests
- Item not found scenarios
- Already completed items
- File operation error scenarios
- YAML integrity preservation tests

## Success Criteria
- Tool compiles and registers correctly
- Correctly identifies and updates items by ULID
- Preserves file structure and all other items
- Updates only the `done` field to `true`
- Proper error handling for missing items
- Comprehensive test coverage
- Thread-safe file operations

## Implementation Notes
- Follow patterns from existing update tools
- Use atomic file operations to prevent corruption
- Validate ULID format before searching
- Consider idempotent behavior (already complete items)
- Add proper logging for debugging