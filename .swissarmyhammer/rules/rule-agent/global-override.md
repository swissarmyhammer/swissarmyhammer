---
severity: error
tags:
- agent
- cli
---

## Rule: CLI Must Support Global Agent Override

The CLI must support a global `--agent` flag that overrides all use case assignments.

### Requirements
1. Global flag `--agent <name>` must be available on all commands
2. When specified, ALL use case agents must resolve to the override agent
3. Override must not modify config file
4. Override must be runtime only
5. Override must work with all commands that use agents

### Verification
Check that:
- `sah --agent claude-code rules check` uses claude-code for Rules use case
- `sah --agent qwen-coder flow run plan` uses qwen-coder for Workflows use case
- Config file is not modified by override
- Override applies to all use cases simultaneously
