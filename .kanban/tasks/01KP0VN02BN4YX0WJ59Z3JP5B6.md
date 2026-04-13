---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffbb80
project: kanban-mcp
title: 'sah-cli: add .mcp.json for local dev'
---
## What\n\nCreate `swissarmyhammer-cli/.mcp.json` for local development, matching shelltool-cli/.mcp.json and code-context-cli/.mcp.json.\n\n```json\n{\n  \"mcpServers\": {\n    \"sah\": {\n      \"command\": \"sah\",\n      \"args\": [\"serve\"]\n    }\n  }\n}\n```\n\n## Acceptance Criteria\n- [x] `swissarmyhammer-cli/.mcp.json` exists and is valid JSON\n