---
assignees:
- claude-code
depends_on:
- 01KNF8VJ53FZD5YGVW1JS0TKB7
position_column: todo
position_ordinal: '8580'
title: 'Frontend: Project management grid view'
---
## What

Create a new view for managing the project list — adding, editing, and deleting projects. This follows the same pattern as any entity management view in the app.

### Approach:
Projects are a first-class entity with CRUD operations. The management view should:
1. List all projects in a grid (name, description, color, order, task count)
2. Allow creating new projects inline or via a form
3. Allow editing project properties (name, description, color)
4. Allow deleting projects (with confirmation, blocked if tasks reference it)
5. Be accessible from the nav/sidebar

### Files to create/modify:
- **Create** a project management component (follow existing entity management patterns in the codebase)
- **Modify** navigation to include a "Projects" section
- Wire project CRUD commands through `useDispatchCommand`

### Design notes:
- Follow the metadata-driven UI pattern — the grid should read field definitions from the project entity schema
- Use existing grid components if available
- Project color should be shown as a color swatch

## Acceptance Criteria
- [ ] Project list view renders all projects with name, description, color
- [ ] Can create a new project from the view
- [ ] Can edit project name, description, color
- [ ] Can delete a project (shows error if tasks reference it)
- [ ] View is accessible from navigation
- [ ] All tests pass

## Tests
- [ ] Component render test for project list
- [ ] Integration test for project CRUD through the view
- [ ] `npm test` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #swimlane-to-project