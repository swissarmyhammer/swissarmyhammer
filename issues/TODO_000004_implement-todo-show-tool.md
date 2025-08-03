# Implement Todo Show MCP Tool

Refer to ./specification/todo_tool.md

## Overview
Implement the `todo_show` MCP tool for retrieving todo items with support for "next" item functionality.

## Tool Specification
- **Name**: `todo_show`
- **Purpose**: Retrieve todo items or get next item to work on
- **Parameters**:
  - `todo_list` (required): Name/path of the todo list file
  - `item` (required): Either a specific ULID or "next"

## Implementation Tasks
1. Create `ShowTodoTool` struct in `todo/show/mod.rs`
2. Implement `McpTool` trait with:
   - `name()`: Return "todo_show"
   - `description()`: Load from description.md
   - `schema()`: JSON schema for parameters
   - `execute()`: Async implementation

3. Create comprehensive `description.md` file

4. Implement tool logic:
   - Parse and validate arguments
   - Load todo list from file
   - Handle two modes:
     - Specific ULID: Return that specific item
     - "next": Return first incomplete item (FIFO order)
   - Return item as formatted YAML
   - Handle missing files gracefully

## FIFO "Next" Logic
- Iterate through todo items in order
- Find first item where `done: false`
- Return complete item with all fields
- If no incomplete items, return appropriate message

## Error Handling
- Todo list file not found
- Invalid ULID format
- ULID not found in list
- YAML parsing errors
- File system permission errors

## Response Format
Return complete todo item as YAML:
```yaml
id: 01K1KQM85501ECE8XJGNZKNJQW
task: "Implement file read tool"
context: "Use cline's readTool.ts for inspiration"
done: false
```

## Testing
- Unit tests for tool creation and schema
- Tests for specific ULID retrieval
- Tests for "next" item functionality
- Edge cases (empty list, all completed)
- File not found scenarios
- Invalid parameter tests

## Success Criteria
- Tool compiles and registers correctly
- Correctly retrieves specific items by ULID
- Implements FIFO "next" item selection
- Returns well-formatted YAML responses
- Proper error handling for edge cases
- Comprehensive test coverage

## Implementation Notes
- Follow patterns from existing show/get tools
- Use existing YAML formatting utilities
- Single-item focus to avoid context pollution
- Ensure thread-safe file operations