# Complete Tool Description

## Status: Complete

## Problem

The MCP tool description file (`swissarmyhammer-tools/src/mcp/tools/kanban/description.md`) only documents:
- Board operations (init, get, update)
- Task operations (add, get, update, move, delete, next, list)
- Column operations (add, get, update, delete, list)

**Missing from documentation:**
- Swimlane operations (5 ops)
- Actor operations (5 ops)
- Tag operations (5 ops)
- Comment operations (5 ops)
- Complete task operation
- Assign/unassign task operations
- Activity operations

This makes the tool harder to use since developers/agents can't discover all available operations.

## Current description.md

Only ~25 lines, covers 1/3 of operations.

## Required Changes

Expand `description.md` to document all 40+ operations with:
- Operation name
- Required parameters
- Optional parameters
- Example usage

## Proposed Structure

```markdown
# kanban

Kanban board operations for task management.

## Overview

The kanban tool provides file-backed task board management. A `.kanban` directory
in a repository root **is** the project - one board per repo.

## Operations

The tool accepts `op` as a "verb noun" string (e.g., "add task", "move task").

### Board Operations

- `init board` - Initialize a new board
  - Required: `name`
  - Optional: `description`

- `get board` - Get board metadata

- `update board` - Update board name or description
  - Optional: `name`, `description`

### Column Operations

- `add column` - Add a workflow stage
  - Required: `id`, `name`
  - Optional: `order`

- `get column` - Get column by ID
  - Required: `id`

- `update column` - Update column name or order
  - Required: `id`
  - Optional: `name`, `order`

- `delete column` - Delete a column (fails if has tasks)
  - Required: `id`

- `list columns` - List all columns

### Swimlane Operations

- `add swimlane` - Add horizontal grouping
  - Required: `id`, `name`
  - Optional: `order`

- `get swimlane` - Get swimlane by ID
  - Required: `id`

- `update swimlane` - Update swimlane name or order
  - Required: `id`
  - Optional: `name`, `order`

- `delete swimlane` - Delete a swimlane
  - Required: `id`

- `list swimlanes` - List all swimlanes

### Actor Operations

- `add actor` - Register a person or agent
  - Required: `id`, `name`, `type` (human|agent)
  - Optional: `ensure` (boolean, default false)
  - When `ensure: true`, returns existing actor instead of error if ID exists

- `get actor` - Get actor by ID
  - Required: `id`

- `update actor` - Update actor name
  - Required: `id`
  - Optional: `name`

- `delete actor` - Delete actor and remove from all task assignments
  - Required: `id`

- `list actors` - List all actors
  - Optional: `type` (filter by human|agent)

### Task Operations

- `add task` - Create a new task
  - Required: `title`
  - Optional: `description`, `assignees`, `tags`, `depends_on`, `position`

- `get task` - Get task by ID
  - Required: `id`

- `update task` - Update task properties
  - Required: `id`
  - Optional: `title`, `description`, `assignees`, `tags`, `depends_on`, `subtasks`, `attachments`

- `move task` - Move task to a different column
  - Required: `id`, `column`
  - Optional: `swimlane`, `ordinal`

- `delete task` - Delete a task (removes from dependencies)
  - Required: `id`

- `complete task` - Move task to the done column
  - Required: `id`

- `assign task` - Assign an actor to a task
  - Required: `id`, `assignee`

- `next task` - Get next actionable task (no incomplete dependencies)

- `list tasks` - List tasks with optional filters
  - Optional: `column`, `ready`, `tag`, `assignee`, `swimlane`

### Tag Operations

- `add tag` - Create a tag for categorizing tasks
  - Required: `id`, `name`, `color` (6-char hex without #)
  - Optional: `description`

- `get tag` - Get tag by ID
  - Required: `id`

- `update tag` - Update tag properties
  - Required: `id`
  - Optional: `name`, `description`, `color`

- `delete tag` - Delete a tag (removes from all tasks)
  - Required: `id`

- `list tags` - List all tags

- `tag task` - Add a tag to a task
  - Required: `id` (task), `tag` (tag ID)

- `untag task` - Remove a tag from a task
  - Required: `id` (task), `tag` (tag ID)

### Comment Operations

- `add comment` - Add a comment to a task
  - Required: `task_id`, `body`, `author`

- `get comment` - Get a specific comment
  - Required: `task_id`, `id`

- `update comment` - Update comment body
  - Required: `task_id`, `id`
  - Optional: `body`

- `delete comment` - Delete a comment
  - Required: `task_id`, `id`

- `list comments` - List all comments on a task
  - Required: `task_id`

### Activity Operations

- `list activity` - List recent operations
  - Optional: `limit` (number of entries)

## Examples Section

Add comprehensive examples showing:
- Complete workflow (init → add tasks → assign → complete)
- Using dependencies
- Using tags
- Using swimlanes
- Agent self-registration with ensure

## Forgiving Input Section

Already exists, but should mention:
- Parameter aliases
- Operation inference
- Batch operations

## File Changes

Update: `swissarmyhammer-tools/src/mcp/tools/kanban/description.md`

Expand from ~25 lines to ~200 lines with full operation reference.
