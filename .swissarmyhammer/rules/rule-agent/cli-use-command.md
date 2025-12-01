---
severity: error
tags:
- agent
- cli
---

## Rule: CLI Must Support Use Case Agent Assignment

The `sah agent use` command must support setting agents for specific use cases.

### Requirements
1. `sah agent use <agent>` must set Root agent (backward compatible)
2. `sah agent use <use-case> <agent>` must set agent for specific use case
3. Use case names must match AgentUseCase enum (root, rules, workflows)
4. Must validate that agent name exists before setting
5. Must show clear error message if agent not found
6. Must show success message with use case when set

### Verification
Check that:
- `sah agent use claude-code` sets Root agent
- `sah agent use rules qwen-coder` sets Rules agent
- `sah agent use workflows claude-code` sets Workflows agent
- Invalid agent names are rejected with helpful error
- Invalid use case names are rejected with helpful error
