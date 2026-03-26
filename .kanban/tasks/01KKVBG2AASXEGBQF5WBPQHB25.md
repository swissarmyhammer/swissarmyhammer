---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffd380
title: Validator server constructed on every create_mcp_router call
---
**swissarmyhammer-tools/src/mcp/unified_server.rs:create_mcp_router()**\n\nThe validator server is created eagerly in `create_mcp_router()` via `server.create_validator_server()`. This runs for every HTTP server start, including stdio mode's companion HTTP server. This is a minor allocation (two tool registrations) but the `create_validator_server()` call also logs at `info` level, which adds noise.\n\n**Why this matters (nit):** Minimal impact — the allocation is cheap. But it means every SAH startup logs about the validator registry even when no validators will be used.\n\n**Fix:** Consider lazy construction or reducing log level to `debug`. Not urgent.\n\n**Verification:** N/A