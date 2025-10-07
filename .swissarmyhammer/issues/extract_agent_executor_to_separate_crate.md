# Extract AgentExecutor to Separate Crate to Break Circular Dependency

## Problem

The `swissarmyhammer-rules` crate needs to use `AgentExecutor` to run LLM checks, but `AgentExecutor` currently lives in `swissarmyhammer-workflow`. This creates a circular dependency problem:

```
swissarmyhammer-workflow
    ↓ (needs rules)
swissarmyhammer-rules
    ↓ (needs AgentExecutor)
swissarmyhammer-workflow  ← CIRCULAR!
```

This circular dependency is blocking the MCP `rules_check` tool from directly calling `swissarmyhammer-rules`, forcing it to shell out to the CLI instead (which is wrong architecture).

## Root Cause

`AgentExecutor` is in the wrong crate. It's a low-level component that should be available to multiple crates without creating circular dependencies.

## Solution

Extract `AgentExecutor` into its own crate: `swissarmyhammer-agent-executor`

**New dependency structure:**
```
swissarmyhammer-agent-executor (new crate)
    ↑                           ↑
    |                           |
swissarmyhammer-rules    swissarmyhammer-workflow
    ↑                           ↑
    |                           |
swissarmyhammer-tools/mcp  swissarmyhammer-cli
```

This breaks the circular dependency and allows:
- `swissarmyhammer-rules` to use `AgentExecutor` directly
- `swissarmyhammer-workflow` to use `AgentExecutor` 
- MCP tools to call `swissarmyhammer-rules` directly (no CLI shelling)
- CLI to call `swissarmyhammer-rules` directly
- No circular dependencies

## Implementation Steps

1. Create new crate: `swissarmyhammer-agent-executor/`
2. Move `AgentExecutor` and related types from `swissarmyhammer-workflow` to new crate
3. Update `swissarmyhammer-workflow/Cargo.toml` to depend on `swissarmyhammer-agent-executor`
4. Update `swissarmyhammer-rules/Cargo.toml` to depend on `swissarmyhammer-agent-executor`
5. Update imports throughout codebase
6. Update `swissarmyhammer-tools` to call `swissarmyhammer-rules` directly (remove CLI shelling)

## Benefits

- Proper separation of concerns
- No circular dependencies
- MCP tools can call rules directly (fast, type-safe)
- CLI can call rules directly
- Both use same underlying implementation
- Easier to test and maintain
- `AgentExecutor` becomes a reusable low-level component

## Related Issues

- `mcp_rules_check_should_call_crate_not_cli` - blocked by this issue