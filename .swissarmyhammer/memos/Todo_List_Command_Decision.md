# Todo List Command Decision

## Context

As of 2025-11-12, we have a clarification todo asking: "Do we need a 'todo list' command to see all current todos?"

## Current State

### Existing CLI Commands
- **`sah todo create`** - Add a new todo item
- **`sah todo show --item next`** - Show the next incomplete todo
- **`sah todo show --item <ULID>`** - Show a specific todo by ID
- **`sah todo complete --id <ULID>`** - Mark a todo as complete

### Missing Command
- **`sah todo list`** - DOES NOT EXIST (returns "unrecognized subcommand 'list'")

### Backend Support
The `TodoStorage` struct (swissarmyhammer-todo/src/storage.rs:109-118) already has a `get_todo_list()` method that returns `Option<TodoList>`, so the backend fully supports listing all todos.

## Use Case Analysis

### When Users Need to See All Todos

1. **After `rules_check` with `create_todo: true`**
   - May create many todos (one per violation)
   - Users need to see the full list to understand scope of work
   - Example: 10+ rule violations found, each becoming a todo

2. **After `plan` workflow**
   - Plan workflow can create multiple todos from a specification
   - Users need to see all planned work items
   - Example: Breaking down a feature into 5-10 steps

3. **Mid-workflow Progress Tracking**
   - Users working through todos want to know:
     - How many todos remain?
     - What's the priority order?
     - Can they skip/reorder items?

4. **Daily Workflow Start**
   - Users returning to work want to see what's pending
   - "What was I working on?" context

### When "Next" is Sufficient

1. **Simple Sequential Work**
   - User creates 2-3 todos manually
   - Works through them one by one
   - FIFO queue approach works fine

2. **Automated Workflows**
   - The `do_todos` workflow processes todos automatically
   - Doesn't need human visibility into the full list

## Decision

**YES, we need a `sah todo list` command.**

### Rationale

1. **Workflow Integration**: The `rules_check` tool's `create_todo` parameter and `plan` workflow both create multiple todos. Users need visibility into what was created.

2. **Progress Tracking**: Users need to answer "how much work is left?" without having to run `todo show next` repeatedly.

3. **Backend Already Supports It**: The `TodoStorage::get_todo_list()` method exists. We just need to wire it up to the CLI.

4. **Consistency with Other Tools**: 
   - `sah issue list` exists
   - `sah memo list` exists
   - `sah todo list` should exist for consistency

5. **Queue Transparency**: Even if todos are processed FIFO, users should be able to see the queue contents.

## Implementation Plan

### 1. Add MCP Tool: `todo_list`

Create `swissarmyhammer-tools/src/mcp/tools/todo/list/` with:
- `mod.rs` - Implement `ListTodoTool`
- `description.md` - Tool documentation

The tool should:
- Call `TodoStorage::get_todo_list()`
- Return all todos with their status (done/not done)
- Include metadata: total count, incomplete count, complete count
- Support format parameter (table, json, yaml)

### 2. Register Tool

Update `swissarmyhammer-tools/src/mcp/tools/todo/mod.rs`:
```rust
pub mod list;
pub use list::ListTodoTool;

pub fn register_todo_tools(registry: &mut ToolRegistry) {
    registry.register(CreateTodoTool);
    registry.register(ShowTodoTool);
    registry.register(MarkCompleteTodoTool);
    registry.register(ListTodoTool);  // Add this
}
```

### 3. CLI Auto-Generation

The dynamic CLI will automatically create `sah todo list` once the MCP tool is registered (no CLI code changes needed).

### 4. Output Format

Default table format:
```
Todo List

ID                           Task                                           Status
01K9WCQZ2X5ZPJMGFJ81TF503K  Fix no-performance-tests violation in Cargo... Pending
01K9WC208GMSGN9H3YGE4W3E6C  Clarify: Do we need a 'todo list' command...  Complete

Summary: 2 total, 1 incomplete, 1 complete
```

JSON format (for automation):
```json
{
  "todos": [
    {
      "id": "01K9WCQZ2X5ZPJMGFJ81TF503K",
      "task": "Fix no-performance-tests violation...",
      "context": "...",
      "done": false
    }
  ],
  "total": 2,
  "incomplete": 1,
  "complete": 1
}
```

### 5. Testing

Add test in `swissarmyhammer-cli/tests/todo_cli_tests.rs`:
- Create multiple todos
- Call `sah todo list`
- Verify output contains all todos
- Verify counts are correct
- Test with no todos (empty list)

## Related Work

- This complements the issue filed at `.swissarmyhammer/issues/add-create-todo-parameter-to-rules-check.md` which implemented `create_todo` parameter
- The `do_todos` workflow will continue to use `todo_show next` internally
- Users can use `todo list` to see progress, then `do_todos` to process items

## Answer to Original Question

**Question**: "Do we need a `sah todos` CLI command to list all todos? Or is 'next' sufficient (FIFO queue approach)?"

**Answer**: We need both:
- **`sah todo show --item next`** for sequential processing (workflows, automation)
- **`sah todo list`** for human visibility and progress tracking (manual work, planning)

The "next" approach is sufficient for automated workflows, but users need full list visibility for planning and progress tracking, especially when multiple todos are created at once by tools like `rules_check` and `plan`.