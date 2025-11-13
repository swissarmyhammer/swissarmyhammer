# Add todo_list Command to Todo System

## Problem

The todo system currently has three MCP tools:
- `todo_create` - Create a new todo
- `todo_show` - Show a specific todo or the next incomplete todo
- `todo_mark_complete` - Mark a todo as complete

However, there's no way to list all todos or filter todos by status. Users cannot see what todos exist without repeatedly calling `todo_show` with "next".

## Proposed Solution

Add a `todo_list` MCP tool that returns all todos with optional filtering.

## Requirements

### Tool Definition

**Name**: `todo_list`  
**CLI Name**: `list`  
**Category**: `todo`

### Parameters

```rust
pub struct ListTodosRequest {
    /// Optional filter by completion status
    /// - None: Show all todos (default)
    /// - Some(true): Show only completed todos
    /// - Some(false): Show only incomplete todos
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<bool>,
}
```

### Response Format

Return a list of todos with their full details:

```json
{
  "todos": [
    {
      "id": "01K9T6Y3X93JJBB7TWZ2E8B184",
      "task": "Fix bug in validation",
      "context": "Need to check the input sanitization",
      "done": false
    },
    {
      "id": "01K9T7Z4A94KKCC8UXA3F9C295",
      "task": "Add documentation",
      "context": null,
      "done": true
    }
  ],
  "total": 2,
  "completed": 1,
  "pending": 1
}
```

### Sort Order

Todos should be returned in a consistent order:
1. Incomplete todos first, completed todos last
2. Within each group, sort by creation order (oldest first)
   - If timestamps are implemented (see issue #add-timestamps-and-gc-to-todo-system), use `created_at`
   - Otherwise, use the order they appear in the YAML file

### Implementation

**File**: `swissarmyhammer-tools/src/mcp/tools/todo/list/mod.rs` (new file)

```rust
pub struct ListTodosTool;

impl McpTool for ListTodosTool {
    fn name(&self) -> &'static str {
        "todo_list"
    }
    
    fn cli_name(&self) -> &'static str {
        "list"
    }
    
    fn description(&self) -> &'static str {
        include_str!("description.md")
    }
    
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "completed": {
                    "type": ["boolean", "null"],
                    "description": "Filter by completion status (true=completed, false=incomplete, null=all)"
                }
            }
        })
    }
    
    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> Result<CallToolResult, McpError> {
        let request: ListTodosRequest = BaseToolImpl::parse_arguments(arguments)?;
        
        let storage = TodoStorage::new_default()
            .map_err(|e| McpErrorHandler::handle_todo_error(e, "create todo storage"))?;
        
        let all_todos = storage.list_todos().await?;
        
        // Filter by completion status if requested
        let filtered_todos: Vec<_> = match request.completed {
            None => all_todos,
            Some(done) => all_todos.into_iter().filter(|t| t.done == done).collect(),
        };
        
        // Sort: incomplete first, then by creation order
        let mut sorted_todos = filtered_todos;
        sorted_todos.sort_by_key(|t| (t.done, t.id.clone()));
        
        let completed_count = sorted_todos.iter().filter(|t| t.done).count();
        let pending_count = sorted_todos.len() - completed_count;
        
        Ok(BaseToolImpl::create_success_response(
            json!({
                "todos": sorted_todos,
                "total": sorted_todos.len(),
                "completed": completed_count,
                "pending": pending_count
            })
            .to_string(),
        ))
    }
}
```

### TodoStorage Method

Add a `list_todos()` method to `TodoStorage`:

```rust
impl TodoStorage {
    /// List all todos
    pub async fn list_todos(&self) -> Result<Vec<TodoItem>> {
        self.load_all_todos().await
    }
    
    /// Internal method to load all todos from storage
    async fn load_all_todos(&self) -> Result<Vec<TodoItem>> {
        // Read the todo file and deserialize all items
        // Return empty vec if file doesn't exist
    }
}
```

### CLI Usage

Once implemented, users can:

```bash
# List all todos
sah todo list

# List only incomplete todos
sah todo list --completed false

# List only completed todos
sah todo list --completed true
```

### MCP Tool Usage

```json
{
  "tool": "todo_list",
  "arguments": {}
}

{
  "tool": "todo_list",
  "arguments": {
    "completed": false
  }
}
```

## Benefits

1. **Visibility**: Users can see all todos at once
2. **Status Tracking**: Can filter by completion status to focus on pending work
3. **CLI Completion**: Provides the expected "list" command that users expect
4. **Workflow Integration**: Prompts can use this to get an overview of pending work
5. **Consistency**: Matches the pattern of other tools (memo, issue) that have list commands

## Testing

### Unit Tests
- Test listing all todos
- Test filtering by completed status
- Test empty todo list
- Test sort order (incomplete first)

### Integration Tests
Add to `swissarmyhammer-cli/tests/todo_cli_tests.rs`:
- Test `sah todo list` command
- Test with various filter options
- Test output format

### Manual Testing
```bash
# Create some todos
sah todo create --task "Task 1"
sah todo create --task "Task 2"

# List all
sah todo list

# Complete one
sah todo complete --id <id>

# List incomplete only
sah todo list --completed false

# List completed only
sah todo list --completed true
```

## Files to Modify

1. `swissarmyhammer-todo/src/storage.rs` - Add `list_todos()` method
2. `swissarmyhammer-tools/src/mcp/tools/todo/mod.rs` - Register the new list tool
3. `swissarmyhammer-cli/tests/todo_cli_tests.rs` - Add integration tests

## Files to Create

1. `swissarmyhammer-tools/src/mcp/tools/todo/list/mod.rs` - List tool implementation
2. `swissarmyhammer-tools/src/mcp/tools/todo/list/description.md` - Tool description

## Related Issues

- Complements the todo CLI implementation
- Will benefit from timestamps when #add-timestamps-and-gc-to-todo-system is implemented
- Aligns with the review workflow that uses todos for tracking violations

## Future Enhancements

Once timestamps are implemented:
- Could add `--sort-by created|updated` option
- Could add `--limit N` to show only first N todos
- Could add `--older-than` or `--newer-than` filters



## Implementation Plan

### Step 1: Examine existing code structure
- Review `swissarmyhammer-todo/src/storage.rs` to understand current storage implementation
- Review existing todo tools (`todo_create`, `todo_show`, `todo_mark_complete`) for patterns
- Check if there's already a method to load all todos

### Step 2: Add storage method (if needed)
- Add `list_todos()` method to `TodoStorage` if it doesn't exist
- Ensure it returns todos in a consistent order

### Step 3: Create the list tool (TDD)
- Write failing tests first
- Create `swissarmyhammer-tools/src/mcp/tools/todo/list/mod.rs`
- Create `swissarmyhammer-tools/src/mcp/tools/todo/list/description.md`
- Implement the tool to pass the tests

### Step 4: Register the tool
- Add the list tool to the tool registry in `swissarmyhammer-tools/src/mcp/tools/todo/mod.rs`

### Step 5: Add integration tests
- Add CLI integration tests to verify the full workflow

### Step 6: Test the implementation
- Run all tests with `cargo nextest run`
- Verify the tool works correctly

## Implementation Status

### Completed
1. ✅ `ListTodosRequest` type created in `swissarmyhammer-todo/src/types.rs`
2. ✅ `get_todo_list()` method added to `TodoStorage` 
3. ✅ `ListTodoTool` implemented in `swissarmyhammer-tools/src/mcp/tools/todo/list/mod.rs`
4. ✅ Tool registered in `swissarmyhammer-tools/src/mcp/tools/todo/mod.rs`
5. ✅ Comprehensive unit tests added in `swissarmyhammer-tools/tests/todo_list_tests.rs`
6. ✅ CLI integration tests added in `swissarmyhammer-cli/tests/todo_cli_tests.rs`

### Issues Discovered

#### Test Isolation Problem
The unit tests in `todo_list_tests.rs` are failing because they're not properly isolated:

1. **Shared Storage**: Tests use `TodoStorage::new_default()` which points to the system-wide `.swissarmyhammer/todo/todo.yaml` file
2. **Cross-Test Contamination**: Tests interfere with each other because they share the same storage
3. **Cleanup Failures**: The `cleanup_todos()` helper doesn't actually isolate tests

**Symptoms**:
- `test_list_empty_todos` expects 0 todos but finds 4 (leftover from other tests)
- `test_list_filter_incomplete_only` expects 3 todos but finds 5
- Multiple tests fail with "Todo item not found" errors when marking complete (GC removed them)

**Root Cause**:
The tests create a `TempDir` but don't use it for todo storage. They pass it to `create_test_context()` which only uses it for issue storage, not todo storage.

#### Solution Required

The tests need to use isolated storage. Looking at the storage tests in `swissarmyhammer-todo/src/storage.rs`, they correctly use:

```rust
let temp_dir = tempfile::TempDir::new().unwrap();
let storage = TodoStorage::new(temp_dir.path().to_path_buf());
```

However, the MCP tool tests can't easily override the storage path because the tools use `TodoStorage::new_default()` internally.

**Options**:
1. **Environment Variable**: Make todo storage respect an environment variable (like `SWISSARMYHAMMER_TODO_DIR`)
2. **Context Parameter**: Add storage path to `ToolContext` (would require significant refactoring)
3. **Accept Current Behavior**: Document that tests should run in isolation and clean up properly

**Recommendation**: Option 1 - Add environment variable support. This is:
- Minimal invasive
- Consistent with common testing patterns
- Useful for debugging and testing outside the test suite
- Already used by other storage systems in the codebase

### Next Steps

1. Add environment variable support to `swissarmyhammer_todo::utils::get_todo_directory()`
2. Update tests to set the environment variable to point to temp directories
3. Re-run tests to verify isolation
4. Document the environment variable in the crate documentation

### Resolution

**Environment Variable Support Added**: Modified `swissarmyhammer-todo/src/utils.rs` to support the `SWISSARMYHAMMER_TODO_DIR` environment variable for test isolation.

**Tests Fixed**:
1. Updated `create_test_context()` to set environment variable to temp directory
2. Removed `cleanup_todos()` function and all calls (no longer needed)
3. Removed unused imports (`fs`, `PathBuf`)

**Parity Test Fixed**: Added `todo_list` to the static tool list in `tests/mcp_server_parity_tests.rs`

**Test Results**: All 619 tests passing, including 20 new todo_list tests

## Final Summary

The `todo_list` command has been successfully implemented and tested. The implementation includes:

✅ **Core Functionality**:
- `ListTodosRequest` type with optional `completed` filter
- `TodoStorage::get_todo_list()` method
- `ListTodoTool` MCP tool implementation
- Tool registration in the tool registry
- Environment variable support for test isolation

✅ **Testing**:
- 20 comprehensive unit tests covering all scenarios
- CLI integration tests (with git repo requirement noted)
- Parity tests ensuring HTTP/STDIN server consistency

✅ **Features**:
- List all todos or filter by completion status
- Sort order: incomplete first, then by ULID (time-ordered)
- Returns summary counts (total, completed, pending)
- Includes all todo fields (id, task, context, done)

✅ **Documentation**:
- Tool description in `description.md`
- Environment variable documented in code comments
- Comprehensive test coverage demonstrating usage

The implementation is complete, tested, and ready for use.
