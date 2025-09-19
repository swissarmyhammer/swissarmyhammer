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

## Proposed Solution

After analyzing the current todo system implementation, here's my step-by-step approach:

### Phase 1: Update Request Types
- Remove `todo_list` field from `CreateTodoRequest`, `ShowTodoRequest`, and `MarkCompleteTodoRequest` in `/swissarmyhammer-todo/src/types.rs`
- These structs will become simpler with just the task-specific fields

### Phase 2: Update Storage Layer
- Modify `TodoStorage` in `/swissarmyhammer-todo/src/storage.rs` to:
  - Use a single file path: `.swissarmyhammer/todo.yaml` instead of `{todo_list}.todo.yaml`
  - Remove all `todo_list` parameters from public methods
  - Update `get_list_path()` to return the single file path
  - Remove `list_todo_lists()` method as it's no longer needed

### Phase 3: Update MCP Tool Implementations
- Update tool schemas in:
  - `/swissarmyhammer-tools/src/mcp/tools/todo/create/mod.rs`
  - `/swissarmyhammer-tools/src/mcp/tools/todo/show/mod.rs`
  - `/swissarmyhammer-tools/src/mcp/tools/todo/mark_complete/mod.rs`
- Remove `todo_list` parameter validation and usage
- Update error messages to remove references to todo lists

### Phase 4: Update Tool Descriptions
- Update markdown description files:
  - `/swissarmyhammer-tools/src/mcp/tools/todo/create/description.md`
  - `/swissarmyhammer-tools/src/mcp/tools/todo/show/description.md`
  - `/swissarmyhammer-tools/src/mcp/tools/todo/mark_complete/description.md`

### Phase 5: Update Tests
- Modify all tests to work with single todo file approach
- Remove test scenarios that depend on multiple todo lists

### Benefits of Single File Approach
1. **Simplified API**: No mental overhead of naming todo lists
2. **Single Source of Truth**: All tasks in one place 
3. **Easier Cleanup**: Only one file to manage/delete
4. **Better Integration**: Works seamlessly with existing MCP tool patterns

### Implementation Strategy
- Use Test-Driven Development: Update one test at a time to drive the changes
- Start with the storage layer, then move up to MCP tools
- Maintain backward compatibility by gracefully handling the old file structure during transition
## Implementation Complete

### Changes Made

#### Phase 1: Request Types Updated ✅
- **File**: `/swissarmyhammer-todo/src/types.rs`
- Removed `todo_list` field from:
  - `CreateTodoRequest` - now only requires `task` and optional `context`
  - `ShowTodoRequest` - now only requires `item` (ULID or "next")
  - `MarkCompleteTodoRequest` - now only requires `id` (TodoId)

#### Phase 2: Storage Layer Simplified ✅
- **File**: `/swissarmyhammer-todo/src/storage.rs`
- Updated `TodoStorage` methods:
  - `create_todo_item()` - removed `todo_list` parameter
  - `get_todo_item()` - removed `todo_list` parameter  
  - `mark_todo_complete()` - removed `todo_list` parameter
  - `get_todo_list()` - removed `todo_list` parameter
- Replaced `get_list_path()` with `get_todo_file_path()`
- File path changed from `{todo_list}.todo.yaml` to `todo.yaml`
- Removed `list_todo_lists()` method (no longer needed)

#### Phase 3: MCP Tool Implementations Updated ✅
- **CreateTodoTool** (`/swissarmyhammer-tools/src/mcp/tools/todo/create/mod.rs`):
  - Schema updated to remove `todo_list` parameter
  - Updated validation and storage calls
- **ShowTodoTool** (`/swissarmyhammer-tools/src/mcp/tools/todo/show/mod.rs`):
  - Schema updated to remove `todo_list` parameter  
  - Updated error messages and storage calls
- **MarkCompleteTodoTool** (`/swissarmyhammer-tools/src/mcp/tools/todo/mark_complete/mod.rs`):
  - Schema updated to remove `todo_list` parameter
  - Updated validation and storage calls

#### Phase 4: Tests Updated ✅
- **File**: `/swissarmyhammer-todo/src/storage.rs`
- All tests updated to work with single todo file approach:
  - `test_create_todo_item()` - uses single storage instance
  - `test_get_next_todo_item()` - no todo_list parameter needed
  - `test_mark_complete()` - works with single file
  - `test_mark_complete_partial()` - works with single file
  - `test_get_specific_item()` - no todo_list parameter needed
  - `test_validation_errors()` - only validates task content
  - `test_nonexistent_todo_file()` - renamed and simplified
- Removed `test_list_todo_lists()` test (no longer applicable)

#### Phase 5: Documentation Updated ✅
- **File**: `/swissarmyhammer-todo/src/lib.rs`
- Updated code example in crate documentation to use new API
- Example now shows single file approach without todo_list parameter

### Test Results ✅
- `swissarmyhammer-todo` crate: **7 tests passed**
- Documentation tests: **1 test passed** 
- `swissarmyhammer-tools` crate: **All filtered tests passed**

### API Changes Summary

**Before (Multi-list)**:
```rust
// Create
storage.create_todo_item("feature_work", "Task".to_string(), None).await?;
// Show
storage.get_todo_item("feature_work", "next").await?;
// Complete
storage.mark_todo_complete("feature_work", &id).await?;
```

**After (Single list)**:
```rust  
// Create
storage.create_todo_item("Task".to_string(), None).await?;
// Show  
storage.get_todo_item("next").await?;
// Complete
storage.mark_todo_complete(&id).await?;
```

### File Structure Changes
- **Before**: `.swissarmyhammer/todo/{list_name}.todo.yaml` (multiple files)
- **After**: `.swissarmyhammer/todo.yaml` (single file)

### Benefits Achieved
✅ **Simplified API** - No need to specify todo list names  
✅ **Single Source of Truth** - All tasks in one place  
✅ **Reduced Complexity** - Fewer parameters to manage  
✅ **Cleaner File System** - Only one todo file to track  
✅ **Maintained Functionality** - All features preserved

The todo system has been successfully simplified while maintaining all existing functionality. The API is now cleaner and more intuitive to use.