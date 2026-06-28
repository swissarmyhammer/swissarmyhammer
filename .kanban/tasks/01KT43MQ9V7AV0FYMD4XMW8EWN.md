---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffe180
project: plugin-arch
title: Add a generic CallToolResult unwrap helper to the plugin SDK
---
The SDK marshals callbacks *outbound* but offers nothing to unwrap an MCP `CallToolResult` *inbound*, so every plugin reimplements result parsing. This is a generic "everything is MCP" ergonomics gap, not a per-plugin concern.

## Problem
In-process operation tools answer `tools/call` with a `CallToolResult` whose JSON payload is a string in `content[0].text` (they do NOT populate `structuredContent`). To read e.g. `tasks`/`columns` back, plugins do `JSON.parse(result.content[0].text)`. This is reimplemented as `kanbanPayload(result)` in task-commands (`index.ts:62`) and kanban-misc-commands (`index.ts:67`), and the same pattern is needed anywhere a plugin reads a tool result.

## Fix
Add a public SDK helper (e.g. `unwrapResult(result)` / `resultJson(result)`) in `crates/swissarmyhammer-plugin/src/sdk/` that pulls `content[0].text` and `JSON.parse`s it, returning a typed object (and tolerating absent/non-JSON content the way the current per-plugin helpers do). Export it from `@swissarmyhammer/plugin`.

Consider whether the SDK should unwrap automatically at the dispatch leaf (so `await this.kanban.kanban.tasks.list({})` returns the parsed payload directly) vs. an explicit helper. An explicit helper is the smaller, safer change and keeps flat/operation tools uniform; auto-unwrap is more ergonomic but changes the return contract for every call — evaluate and pick one, documenting the choice.

## Acceptance
- Helper exported from the SDK with a real integration test (plugin reads a kanban list result through it).
- `kanbanPayload` deleted from task-commands and kanban-misc-commands; both use the SDK helper.
- Plugin test suite green.

Related: [shared moniker helpers card], [CommandContext/Availability card].