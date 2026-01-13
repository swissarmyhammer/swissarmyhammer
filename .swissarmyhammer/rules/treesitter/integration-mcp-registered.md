---
severity: error
tags:
- integration
- mcp
---

# Tree-sitter: MCP Tool Registration

## Acceptance Criterion
**AC-28**: Tool registered in MCP server and available via protocol

## What to Check
MCP integration must:
- Tool registered in tool registry
- Tool appears in MCP tool list
- Tool can be invoked via MCP protocol
- Tool implements McpTool trait correctly

## Success Criteria
- TreeSitterTool struct implements McpTool trait
- Tool registered in swissarmyhammer-tools/src/mcp/tool_registry.rs
- Tool appears in server tool list response
- Tool executable via MCP JSON-RPC calls

## Reference
See specification/treesitter.md - Integration and Phase 4 sections