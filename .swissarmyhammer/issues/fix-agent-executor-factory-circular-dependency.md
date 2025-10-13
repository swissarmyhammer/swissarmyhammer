# Fix AgentExecutorFactory Circular Dependency

## Problem

`AgentExecutorFactory` is currently in the `workflow` crate, but should be in `agent-executor` crate. This creates duplicate implementations and prevents proper code reuse.

## Current Circular Dependency

```
workflow → tools (to start MCP server)
tools → workflow (would need AgentExecutorFactory)
= CIRCULAR DEPENDENCY ❌
```

## Evidence

- `swissarmyhammer-workflow/src/actions.rs:264` - Comment: "Start MCP server in workflow layer (breaking circular dependency)"
- `swissarmyhammer-tools/src/mcp/tools/rules/check/mod.rs:21` - Comment: "This cannot use the workflow factory due to circular dependency"
- Two `AgentExecutorFactory` implementations exist:
  - `swissarmyhammer-agent-executor/src/executor.rs:34` (stub, doesn't work)
  - `swissarmyhammer-workflow/src/actions.rs:238` (real implementation with MCP server lifecycle)

## Correct Architecture

```
agent-executor (contains AgentExecutorFactory)
    ↓
workflow (uses factory)
    ↓
tools (uses factory)
```

## Solution

### Option 1: Move MCP Server Startup Out of Factory
- Move MCP server lifecycle to a separate `McpServerManager` in `tools` crate
- `AgentExecutorFactory` takes pre-started MCP handle as parameter
- Factory can move to `agent-executor` crate
- Both `workflow` and `tools` can use the same factory

### Option 2: Extract Common Layer
- Create `swissarmyhammer-agent-factory` crate
- Contains both `AgentExecutorFactory` and MCP server lifecycle
- Both `workflow` and `tools` depend on factory crate
- No circular dependency

### Option 3: Dependency Inversion
- `AgentExecutorFactory` in `agent-executor` crate takes MCP server as trait
- `workflow` and `tools` implement the trait
- Factory doesn't depend on either crate

## Recommended Approach

**Option 1** is cleanest:
1. `tools` crate already has MCP server (`unified_server.rs`)
2. Factory just needs a `McpServerHandle` parameter
3. Minimal changes needed
4. Clear separation of concerns

## Implementation Steps

1. Move `AgentExecutorFactory::create_executor()` from `workflow/actions.rs` to `agent-executor/executor.rs`
2. Change signature to accept `Option<McpServerHandle>` parameter
3. Update `workflow` to call `start_mcp_server()` then pass handle to factory
4. Update `tools/rules/check` to use the centralized factory
5. Remove duplicate factory implementations

## Benefits

- ✅ Single source of truth for executor creation
- ✅ No code duplication between CLI and MCP tool
- ✅ Proper dependency hierarchy
- ✅ Easier to maintain and test
- ✅ Clear separation of concerns

## Related Code

- Current factory: `swissarmyhammer-workflow/src/actions.rs:240-323`
- Stub factory: `swissarmyhammer-agent-executor/src/executor.rs:28-49`
- MCP server: `swissarmyhammer-tools/src/mcp/unified_server.rs`
- CLI usage: `swissarmyhammer-cli/src/commands/rule/check.rs:106-166`
- MCP tool usage: `swissarmyhammer-tools/src/mcp/tools/rules/check/mod.rs:34-61`
