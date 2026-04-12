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
1. Add the icon — `icon.png` already exists at `kanban-cli/icon.png`
2. Rewrite the lead paragraph to sell the MCP server angle
3. Add a **Why** section explaining agent persistent task memory
4. Add a **Commands** table including the new lifecycle commands
5. Keep Install section, update Works With

Model the tone and structure on `shelltool-cli/README.md`.

## Acceptance Criteria
- [ ] `kanban-cli/README.md` displays `icon.png` in the header
- [ ] README has a Why section
- [ ] Commands table includes serve, init, deinit, doctor
- [ ] No broken links or missing images
