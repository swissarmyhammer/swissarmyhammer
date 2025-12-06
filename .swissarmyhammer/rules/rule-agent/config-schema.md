---
severity: error
tags:
- agent
- config
---

## Rule: Config Must Support Use Case Agent Mapping

The configuration schema must support mapping use cases to agent names using an `agents` map.

### Requirements
1. Config YAML must support this format:
   ```yaml
   agents:
     root: "claude-code"
     rules: "qwen-coder-flash"
     workflows: "claude-code"
   ```
2. Config keys must match AgentUseCase enum variants (lowercase)

There is no need to support the old 'agent' field for backward compatibility.

### Verification
Check that:
- Config can be loaded with agents map structure
- Old configs with single `agent` field still work
- AgentManager can resolve agent name for each use case
