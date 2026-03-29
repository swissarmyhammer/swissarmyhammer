# Remember

Capture something to project memory. Works two ways:

## With text: store it directly
```
/remember MCP config hangs in -p mode, use .mcp.json instead
```

## Without text: capture recent activity
```
/remember
```

## Instructions

### With text argument
When the user provides text after `/remember`:
1. Search memory first to avoid duplicates: `memory_search { query: "<key phrases>" }`
2. Store the memory: `memory_store { text: "<user's text>", path: "user/<topic>" }`
3. Promote to user trust since this is a deliberate human request: `memory_promote { chunk_id: "<id from store>", reason: "user-directed remember" }`
4. Confirm what was stored.

### Without text argument (clip that)
When the user runs `/remember` with no additional text:
1. Look at what just happened in the conversation — recent tool uses, errors, discoveries, corrections.
2. Identify what's notable: errors and fixes, non-obvious patterns, design decisions, failed approaches.
3. For each notable item, search memory to check for duplicates, then store it.
4. Summarize what was captured.

### Path conventions
Use descriptive paths that organize by topic:
- `user/preferences` — user preferences and style choices
- `lessons/tauri-mcp-config` — hard-won lessons about specific topics
- `patterns/error-handling` — recurring patterns
- `decisions/auth-approach` — design decisions with rationale
- `gotchas/stdin-pipe-mode` — non-obvious pitfalls
