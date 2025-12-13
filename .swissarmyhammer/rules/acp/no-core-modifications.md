---
severity: error
tags:
- acp
- architecture
---

# No Core Agent Modifications

The ACP implementation must not modify existing llama-agent core modules.

## Requirements

- Use existing `AgentServer.generate_stream()` method without changes
- Consume stream in ACP layer only
- Convert StreamChunks to ACP notifications in translation layer
- Leverage existing session management
- Use existing MCP client
- No changes to:
  - agent.rs
  - session.rs
  - mcp.rs
  - model.rs
  - generation/*

## Verification

Git diff should show no changes to core modules, only new files in acp/ directory.