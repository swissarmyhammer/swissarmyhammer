---
severity: error
tags:
- agent
- workflows
---

## Rule: WorkflowExecutor Must Accept Agent Configuration

WorkflowExecutor must be able to use a configured agent for workflow operations.

### Requirements
1. WorkflowExecutor must have an optional agent field
2. Constructor must accept optional agent parameter: `with_agent(agent: Option<AgentConfig>)`
3. Existing constructors (`new()`, `with_working_dir()`) must continue to work
4. When agent is None, workflows should work as they currently do
5. When agent is Some, workflows should use that agent for operations that need one

### Verification
Check that:
- WorkflowExecutor has agent field
- Constructor with agent parameter exists
- Existing constructors still work
- Backward compatibility maintained
