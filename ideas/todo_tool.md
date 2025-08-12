# Todo Tool Specification

## Overview

The Todo tool provides ephemeral task management capabilities for tracking work items during development sessions.
Unlike issues, todo lists are temporary and never committed to version control, making them ideal for managing immediate work items and context during active development.

## File Format

Todo lists are stored as `.todo.yaml` files with the following structure:

```yaml
todo:
  - id: 01K1KQM85501ECE8XJGNZKNJQW
    task: "Implement file read tool"
    context: "Use cline's readTool.ts for inspiration"
    done: true
  - id: 01K1KQM85501ECE8XJGNZKNJQX
    task: "Add glob support"
    context: "Refer to qwen-code glob.ts"
    done: false
  - id: 01K1KQM85501ECE8XJGNZKNJQY
    task: "Integrate ripgrep for grep"
    context: "Improve search performance"
    done: false
```


### Fields

- `id`: Sequential ULID identifier for the todo item
- `task`: Brief description of the task to be completed
- `context`: Optional additional context, notes, or implementation details
- `done`: Boolean flag indicating completion status

## Tool Functions

### Create

**Purpose**: Add a new item to the todo list

**Parameters**:
- `todo_list` (required): Name of the todo list file
- `task` (required): Brief description of the task
- `context` (optional): Additional context or implementation notes

**Behavior**:
- Auto-creates the todo list file if it doesn't exist
- Generates a sequential ULID for the new item
- Adds the item with `done: false` status
- Appends to the existing todo list

### Show

**Purpose**: Retrieve the a todo item to work on

**Parameters**:
- `todo_list` (required): Name of the todo list file
- `item` (required): Either a specific ULID or "next", a special keywork to show the next todo that is not complete

**Behavior**:
- If `item` is a ULID: Returns the specific todo item as YAML
- If `item` is "next": Returns the first incomplete todo item (FIFO order)
- Enforces single-item focus to avoid context pollution
- Returns the complete item with all fields

### Mark Complete

**Purpose**: Mark a todo item as completed

**Parameters**:
- `todo_list` (required): Name of the todo list file
- `id` (required): ULID of the completed todo item

**Behavior**:
- Marks the specified item from the todo list as done: true
- Preserves items, leaving the full file is useful for watching status and debugging
- Updates the todo list file
- If all tasks are complete in the todo list file, delete the file

## Usage Patterns

1. **Session Initialization**: Create a todo list at the start of a development session
2. **Task Breakdown**: Break down complex work into manageable todo items
3. **Sequential Processing**: Use "next" to work through items in order
4. **Context Preservation**: Store implementation notes and references in the context field

## File Management

- Todo lists are stored as `.yaml` files in `./swissarmyhammer/todo/`
- `./swissarmyhammer/todo/` should be added to `.gitignore` to prevent accidental commits
- Todo lists are stored in `.swissarmyhammer/todo/` in the current repo root working directory.
- If you are not in a repo, todo lists are stored in `.swissarmyhammer/todo/` in the current working directory.

## Integration Notes

- Todo tools complement but don't replace the issue tracking system, use todos to break down and track the work needed to resolve an issue
- Issues are for long-term work items; todos are for immediate session management
- Todo lists help maintain focus during complex multi-step implementations
- The FIFO "next" pattern encourages completing tasks before starting new ones

## CLI

DO NOT add this to the CLI, the todo system is just for the LLM as a tool.

