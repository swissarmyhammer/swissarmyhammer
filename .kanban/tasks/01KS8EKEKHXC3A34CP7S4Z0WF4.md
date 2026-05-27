---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffa280
title: Wire ACP client connection into agent wrappers (fix elicitation decline)
---
MCP elicitation always returns "User declined or cancelled" because wrap_claude_into_handle / wrap_llama_into_handle never wire the outbound ConnectionTo<Client> into the agent. Fix: call set_client (claude) and publish_client_connection/clear_client_connection (llama) inside with_spawned. Add observability accessors is_client_connected (ClaudeAgent) and is_elicitation_endpoint_set (AcpServer). TDD regression test in swissarmyhammer-agent must fail before, pass after.