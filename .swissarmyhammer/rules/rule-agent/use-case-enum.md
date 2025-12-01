---
severity: error
tags:
- agent
- types
---

## Rule: AgentUseCase Enum Must Define Use Cases

The `AgentUseCase` enum must exist in `swissarmyhammer-config/src/agent.rs` and define exactly three use cases:
- `Root` - Default agent for general operations
- `Rules` - Agent for rule checking operations  
- `Workflows` - Agent for workflow execution

### Requirements
1. Enum must be defined with `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]`
2. Must have `#[serde(rename_all = "lowercase")]` for config file format
3. All three variants (Root, Rules, Workflows) must be present
4. Enum must be public and exported from the agent module

### Verification
Check that:
- `AgentUseCase` enum exists in swissarmyhammer-config/src/agent.rs
- All three variants are defined
- Serde annotations are correct for YAML serialization
