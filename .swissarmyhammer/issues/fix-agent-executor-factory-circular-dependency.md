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



## Current Status (2025-10-14)

### Partial Progress Made

The `AgentExecutorFactory` implementation was moved to `agent-executor` crate (commit a09fe0f5), but **the circular dependency is NOT resolved** due to duplicate trait definitions.

### Root Cause: Duplicate Traits

There are TWO identical `AgentExecutor` trait definitions:

1. `swissarmyhammer-agent-executor/src/executor.rs:9` - Base trait
2. `swissarmyhammer-workflow/src/actions.rs:213` - Duplicate trait

These are incompatible types in Rust's type system despite having identical signatures:
```rust
// agent-executor crate
Box<dyn agent_executor::AgentExecutor>  

// workflow crate  
Box<dyn workflow::AgentExecutor>

// ❌ These are NOT interchangeable!
```

### What Was Done

✅ Factory implementation exists in `agent-executor/src/executor.rs:36-74`
✅ CLI uses the centralized factory successfully  
❌ Workflow still has duplicate factory (can't use centralized one due to trait mismatch)

### What Remains

To fully fix this:

1. **Remove duplicate trait** - Delete `workflow::AgentExecutor` trait definition
2. **Re-export canonical trait** - Have workflow use `pub use agent_executor::AgentExecutor`
3. **Update all workflow code** - Change references from local trait to re-exported one
4. **Remove duplicate factory** - Delete `workflow::AgentExecutorFactory`, use only the centralized one
5. **Update imports** - Fix all code that imports `workflow::AgentExecutor`

### Blockers

This is a breaking change across multiple crates. Need to:
- Update all trait implementations (ClaudeCodeExecutor, LlamaAgentExecutorWrapper)  
- Fix all call sites in workflow actions
- Ensure tests still pass after trait unification

### Estimated Effort

~2-3 hours of careful refactoring to eliminate duplicate trait while maintaining compatibility.
