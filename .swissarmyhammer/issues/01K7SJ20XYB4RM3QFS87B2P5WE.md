# Remove notify_create from Tool Registry

## Parent Issue
Eliminate notify_create Tool and Replace with Native MCP Notifications (01K7SM38449JYA2KZP4KNKQ05X)

## Summary
Remove notify_create tool registration from the MCP tool registry.

## Location
`swissarmyhammer-tools/src/mcp/tool_registry.rs`

## Tasks

1. Remove NotifyCreateTool struct and implementation (around line 1797-1810)
   ```rust
   // DELETE THIS:
   #[async_trait::async_trait]
   impl McpTool for NotifyCreateTool {
       fn name(&self) -> &'static str {
           "notify_create"
       }
       // ... rest of implementation
   }
   ```

2. Remove from `register_notify_tools()` function
   - Remove line that registers NotifyCreateTool
   - Keep the function if other notify tools exist, otherwise remove entirely

3. Remove any imports related to NotifyCreateTool

## Dependencies

Must be completed **after**:
- Remove notify_create Tool Implementation

## Verification

- [ ] NotifyCreateTool struct removed
- [ ] Tool not registered in registry
- [ ] `cargo build` succeeds
- [ ] `cargo clippy` shows no warnings about unused code
- [ ] Tool does not appear in `sah serve` tool list
