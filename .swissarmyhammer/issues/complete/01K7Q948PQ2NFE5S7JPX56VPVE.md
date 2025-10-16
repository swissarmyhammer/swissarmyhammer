# Fix LlamaAgent MCP server initialization error

## Problem
When using a non-Claude model (LlamaAgent), `cargo run -- hello-world` fails with error:
```
MCP server must be provided before initialization. The workflow layer should start the MCP server and pass it to the executor constructor.
```

## Root Cause
In `swissarmyhammer-workflow/src/actions.rs:712`, the code creates `LlamaAgentExecutorWrapper` without an MCP server:
```rust
let mut executor = crate::agents::LlamaAgentExecutorWrapper::new(llama_config.clone());
```

## Why LlamaAgent Needs MCP Server (but ClaudeCode Doesn't)
- **ClaudeCode** (line 687): Uses external Claude API which provides its own tools → No MCP server needed
- **LlamaAgent** (line 712): Uses local model which needs SwissArmyHammer tools provided via MCP → **Requires MCP server**

The LlamaAgent executor requires an MCP server to be started **before** initialization, but:
1. No MCP server is being started in the workflow or CLI layer
2. The `new()` method is being used instead of `new_with_mcp()`
3. The MCP server lifecycle needs to be managed at the CLI/workflow layer

## Context
This likely broke during the AgentExecutorFactory refactor (commit 504e17f4) which centralized executor creation and changed how MCP servers are passed to executors.

## Solution
Need to:
1. Start the MCP server **before** creating the LlamaAgent executor (in CLI or workflow layer)
2. Pass the `McpServerHandle` when creating the wrapper using `new_with_mcp()`
3. Ensure proper MCP server lifecycle management (startup/shutdown)
4. Make this conditional - only start MCP server for LlamaAgent, not for ClaudeCode

## Files to Check
- `swissarmyhammer-workflow/src/actions.rs:704-727` - Where LlamaAgentExecutorWrapper is created
- `swissarmyhammer/src/commands/flow/run.rs` - CLI layer that might need to start MCP server
- `swissarmyhammer-agent-executor/src/executor.rs` - AgentExecutorFactory that might need MCP server parameter
- Previous git history around commit 504e17f4 to understand what changed

## Expected Behavior
- ClaudeCode: Works as-is (no MCP server needed)
- LlamaAgent: MCP server should be started at CLI/workflow layer before initialization, handle passed to `LlamaAgentExecutorWrapper::new_with_mcp()`
- Workflow should execute successfully with both executor types