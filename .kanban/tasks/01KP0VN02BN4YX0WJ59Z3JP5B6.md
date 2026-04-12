---
assignees:
- claude-code
position_column: todo
position_ordinal: bb80
project: kanban-mcp
title: 'sah-cli: add .mcp.json for local dev'
---
## What

Create `swissarmyhammer-cli/.mcp.json` for local development, matching shelltool-cli/.mcp.json and code-context-cli/.mcp.json.

```json
{
  "mcpServers": {
    "sah": {
      "command": "sah",
      "args": ["serve"]
    }
  }
}
```

## Acceptance Criteria
- [ ] `swissarmyhammer-cli/.mcp.json` exists and is valid JSON
