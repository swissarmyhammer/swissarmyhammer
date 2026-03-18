---
position_column: done
position_ordinal: z00
title: Wire Elicitation/ElicitationResult hooks into MCP proxy layer
---
## Resolved via fire_event()\n\nThe MCP proxy layer has no elicitation interception, but the new `HookableAgent::fire_event()` method allows callers to fire `HookEvent::Elicitation` and `HookEvent::ElicitationResult` directly. The event types already have full kind/matcher/json_input support with `mcp_server_name` as the matcher value.\n\nNo further work needed — the MCP tool code or proxy layer can call `agent.fire_event(&HookEvent::Elicitation { ... })` when appropriate."