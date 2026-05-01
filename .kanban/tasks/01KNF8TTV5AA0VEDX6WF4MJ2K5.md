---
assignees:
- claude-code
depends_on:
- 01KNF8SW8CJHRFE6B3ZEQF1FV9
position_column: done
position_ordinal: ffffffffffffffffffffffffffff8c80
title: Update MCP tool description and kanban skill for projects
---
## What

Replace all swimlane documentation with project operations in the MCP tool description and update the kanban skill to describe project-based grouping for plans.

### Files to modify:
- **`swissarmyhammer-tools/src/mcp/tools/kanban/description.md`** — replace swimlane operations section with project operations; update task operations (remove swimlane param from `move task`, `next task`, `list tasks`; add `project` param to `add task`, `update task`); update examples
- **`swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs`** — update operation schemas, parameter definitions, and tests for project operations; remove swimlane schemas
- **`builtin/skills/kanban/SKILL.md`** — add section on using projects to group related plan tasks; describe project CRUD workflow
- **`builtin/_partials/tool-use/kanban.md`** — update any swimlane references

### New MCP operations to document:
```
### Project Operations

- `add project` — Create a project for grouping tasks
  - Required: `id`, `name`
  - Optional: `description`, `color` (6-char hex), `order`

- `get project` — Get project by ID
  - Required: `id`

- `update project` — Update project properties
  - Required: `id`
  - Optional: `name`, `description`, `color`, `order`

- `delete project` — Delete a project (fails if tasks reference it)
  - Required: `id`

- `list projects` — List all projects
```

### Kanban skill addition:
Add guidance on creating a project per plan/initiative and assigning tasks to it, enabling grouping in the UI.

## Acceptance Criteria
- [ ] description.md has no swimlane references
- [ ] description.md documents all 5 project operations with examples
- [ ] Task operations updated: `add task` and `update task` accept `project` param
- [ ] `move task` no longer accepts `swimlane` param
- [ ] kanban SKILL.md describes project-based task grouping
- [ ] MCP tool tests pass

## Tests
- [ ] `cargo test -p swissarmyhammer-tools` passes
- [ ] Grep confirms no swimlane references in description.md

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #swimlane-to-project