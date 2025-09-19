# Simplify Todo System to Single List

## Problem

The current MCP todo tools require specifying a `todo_list` parameter, creating multiple todo files in `.swissarmyhammer/todo/`. This adds unnecessary complexity when a single todo list would be sufficient for most use cases.

## Solution

### Remove `todo_list` Parameter
- Remove `todo_list` parameter from all MCP todo tools:
  - `todo_create`
  - `todo_show` 
  - `todo_mark_complete`

### Simplify File Structure
- Change from: `.swissarmyhammer/todo/{list_name}.yaml`
- Change to: `.swissarmyhammer/todo.yaml` (single file)

### Updated Tool Signatures

#### todo_create
- Remove: `todo_list` (required parameter)
- Keep: `task` (required), `context` (optional)

#### todo_show
- Remove: `todo_list` (required parameter)
- Keep: `item` (required) - ULID or "next"

#### todo_mark_complete
- Remove: `todo_list` (required parameter)
- Keep: `id` (required) - ULID to mark complete

### Benefits
- Simpler API - no need to think of todo list names
- Single source of truth for all pending tasks
- Reduced file system complexity
- Maintains all existing functionality with cleaner interface

## Implementation Notes

- Ensure backward compatibility during transition
- Update all documentation and examples
- Consider migration path for existing multi-list setups
- Update error messages to remove todo_list references

## Files to Update

- `swissarmyhammer-tools/src/mcp/tools/todo/`
- All todo tool implementations
- Tool descriptions and documentation
- Any existing tests using todo tools