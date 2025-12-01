---
severity: error
tags:
- agent
- rules
---

## Rule: Rule Checking Must Use Rules Use Case Agent

The rule checking tool must use the agent configured for the Rules use case, not the root agent.

### Requirements
1. RuleCheckTool in `swissarmyhammer-tools/src/mcp/tools/rules/check/mod.rs` must call `context.get_agent_for_use_case(AgentUseCase::Rules)`
2. Agent resolution must happen at rule checking time, not at startup
3. If Rules use case not configured, must fall back to Root agent transparently
4. No behavior change if user hasn't configured Rules use case

### Verification  
Check that:
- RuleCheckTool requests Rules use case agent
- Fallback to Root works correctly
- Rule checking works with both configured and unconfigured Rules use case
