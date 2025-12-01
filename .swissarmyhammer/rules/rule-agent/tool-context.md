---
severity: error
tags:
- agent
- tools
- mcp
---

## Rule: ToolContext Must Provide Use Case Agent Resolution

ToolContext must allow tools to get agent configuration for specific use cases with proper fallback.

### Requirements
1. ToolContext must have a method like `get_agent_for_use_case(use_case: AgentUseCase) -> &AgentConfig`
2. Resolution must follow fallback chain: use case → root → default
3. All existing tools must continue to work with root agent (backward compatibility)
4. Agent configs must be stored efficiently (Arc wrapped, not cloned per use case)

### Verification
Check that:
- ToolContext has use case agent resolution method
- Fallback logic works correctly
- Memory efficiency is maintained (Arc references, not full copies)
- Existing tools work without modification
