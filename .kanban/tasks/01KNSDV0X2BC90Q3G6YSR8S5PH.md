---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: '[blocker] .mcp.json is malformed JSON'
---
**File**: code-context-cli/.mcp.json\n\n**What**: The file contains two separate JSON objects concatenated together, making it invalid JSON. The first object is `{\"mcpServers\": {}}` and the second is an incomplete fragment containing the actual server entry. This appears to be a broken merge or double-write.\n\n**Why**: Any tool attempting to parse this file as JSON will fail. This will break `code-context init` if it reads the project-level `.mcp.json` or confuse editors and agents that rely on it.\n\n**Note**: The same malformation exists in `shelltool-cli/.mcp.json` -- this is a pre-existing bug in the template, but the new crate copies it.\n\n**Suggestion**: Replace with valid JSON:\n```json\n{\n  \"mcpServers\": {\n    \"code-context\": {\n      \"command\": \"code-context\",\n      \"args\": [\"serve\"]\n    }\n  }\n}\n```\n\n**Verify**: `python3 -c \"import json; json.load(open('code-context-cli/.mcp.json'))\"` should succeed." #review-finding