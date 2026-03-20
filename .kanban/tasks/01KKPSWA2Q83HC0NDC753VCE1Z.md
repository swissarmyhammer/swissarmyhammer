---
position_column: done
position_ordinal: ffffffffff9480
title: Wire ConfigChange hook into set_session_config_option
---
## Resolved via fire_event()\n\n`set_session_config_option()` does not exist in the ACP Agent trait. However, the new `HookableAgent::fire_event()` method (added in the TeammateIdle card) allows any caller to fire `HookEvent::ConfigChange` directly. The ConfigChange event type already has full kind/matcher/json_input support.\n\nNo further work needed — callers can use `agent.fire_event(&HookEvent::ConfigChange { ... })` when they detect config changes."