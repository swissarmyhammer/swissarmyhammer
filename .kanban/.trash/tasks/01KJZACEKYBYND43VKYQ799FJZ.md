---
position_column: done
position_ordinal: j1
title: Fix 30 llama-agent doctest compilation failures
---
30 doctests in llama-agent fail to compile. They reference crate:: paths that do not resolve in doctest context (E0432 unresolved imports, E0425 not found in scope). Affected files: agent.rs, chat_template.rs, acp/mod.rs, acp/server.rs, lib.rs, generation/mod.rs, storage.rs, types/mcp.rs, validation/agent_validator.rs, validation/mcp_validator.rs, validation/queue_validator.rs. #test-failure