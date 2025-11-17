# CLI Tool Registry Not Available for create_todos

## Problem

When running `sah rule check --create-todos` from the CLI, todo creation fails with:
```
Failed to create todo for violation: Tool registry not available in this context
```

## Root Cause

The CLI creates a ToolContext in `mcp_integration.rs` but doesn't populate the `tool_registry` field, so when `rules_check` MCP tool tries to call `context.call_tool("todo_create", ...)`, it fails because `context.tool_registry` is `None`.

**Location**: `swissarmyhammer-cli/src/mcp_integration.rs:47-64`

```rust
let tool_context = ToolContext::new(
    tool_handlers,
    issue_storage,
    git_ops,
    memo_storage,
    agent_config,
);  // tool_registry field is None!

let tool_registry = Arc::new(Self::create_tool_registry());
// tool_registry is created but NOT added to tool_context

Ok(Self {
    tool_registry,
    tool_context,  // Has no registry!
})
```

## Solution

Update `CliToolContext::new_with_dir()` to populate the tool_registry in ToolContext:

```rust
let tool_context = ToolContext::new(...);

let tool_registry = Self::create_tool_registry();
let tool_registry_wrapped = Arc::new(RwLock::new(tool_registry));

// Add registry to context
let tool_context = tool_context.with_tool_registry(tool_registry_wrapped.clone());
```

## Complication

`CliToolContext` stores `Arc<ToolRegistry>` for CliBuilder (which needs unwrapped access), but `ToolContext` needs `Arc<RwLock<ToolRegistry>>` for thread-safe tool-to-tool calls.

**Options:**
1. Change CliBuilder to accept Arc<RwLock<ToolRegistry>> and lock when needed
2. Store both wrapped and unwrapped versions in CliToolContext  
3. Make CliBuilder::new() async so it can lock the registry

## Recommended Approach

Update both CliToolContext and CliBuilder to use Arc<RwLock<ToolRegistry>>:

1. Change CliToolContext.tool_registry type to Arc<RwLock<ToolRegistry>>
2. Change CliBuilder to accept Arc<RwLock<ToolRegistry>> in new()
3. Lock the registry in CliBuilder::new() when precomputing command data
4. Lock the registry in execute_tool() when looking up tools

This makes the types consistent and enables tool-to-tool calls.

## Testing

After fix, verify:
- `sah rule check --create-todos src/**/*.rs` creates todos in .swissarmyhammer/todo/
- No "Tool registry not available" errors
- Todos have proper formatting from rules_check → todo_create MCP call chain

## Related

- Issue: rules-check-bypasses-todo-create-interface.md (broader architectural issue)
- Rule: architecture/tools-use-mcp-interfaces.md (tools must call through MCP)
