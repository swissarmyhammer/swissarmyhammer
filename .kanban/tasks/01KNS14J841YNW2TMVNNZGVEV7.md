---
assignees:
- claude-code
depends_on:
- 01KNS13JJ850R9NBA0RQCS7E9Z
position_column: todo
position_ordinal: af80
project: kanban-mcp
title: 'kanban-cli: update README with icon and MCP server selling points'
---
## What

Update `kanban-cli/README.md` to match the shelltool-cli README quality and structure.

Changes:
1. Add the icon — `icon.png` already exists at `kanban-cli/icon.png`. Add it to the header div like shelltool does:
   ```html
   <img src="icon.png" alt="kanban" width="256" height="256">
   ```
2. Rewrite the lead paragraph to sell the MCP server angle: agents can plan work, track progress, move cards, assign tasks — all through a single MCP tool, backed by git-friendly files that both humans and agents read.
3. Add a **Why** section (like shelltool's) explaining the value: agents forget work between sessions; kanban gives them persistent, structured memory for the task list — survives context resets, visible in the GUI app, reviewable in git history.
4. Add a **Commands** table including the new lifecycle commands:
   | Command | Description |
   |---------|-------------|
   | `kanban serve` | Run MCP server over stdio |
   | `kanban init [project\|local\|user]` | Install kanban MCP for your agent |
   | `kanban deinit [project\|local\|user]` | Remove kanban MCP |
   | `kanban doctor` | Diagnose setup issues |
   | `kanban task add --title "..."` | Add a task |
   | `kanban open .` | Open the GUI app |
5. Keep Install section, update Works With.

Model the tone and structure on `shelltool-cli/README.md`.

## Acceptance Criteria
- [ ] `kanban-cli/README.md` displays `icon.png` in the header
- [ ] README has a Why section explaining agent persistent task memory
- [ ] Commands table includes all four lifecycle commands
- [ ] No broken links or missing images

## Tests
- [ ] Visual review: render the markdown and confirm icon shows

## Workflow
- Direct edit — no TDD needed for documentation.
