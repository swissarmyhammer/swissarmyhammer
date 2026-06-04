---
assignees:
- claude-code
depends_on:
- 01KT57BTE05BAFGYEJHGC7MBR8
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffda80
project: agent-builtins
title: 'tools: validator profile composes read-only file tools (retire is_validator_tool exemption)'
---
The read-only file tools (`ReadFile`/`GlobFiles`/`GrepFiles`) are Agent tools; today they dodge stripping via a per-tool carve-out (the old `is_agent_tool` returned false for the read-only `files` variant, and `is_validator_tool()` filtering). Replace that fuzz with a composed validator profile.

## Change
- With read-only file tools now categorized **Agent** (per the metadata card), the AVP validator no longer gets them "for free" from the shared server.
- Introduce a **validator profile**: a composed registry the validator path serves, containing exactly the read-only file tools (and whatever else the locked-down AVP subset needs — audit current `is_validator_tool()` usage).
- Retire/replace the `is_validator_tool()` per-tool boolean in favor of explicit profile composition, consistent with the Shared/Agent/Replacement model.

## Investigate first
- Enumerate current `is_validator_tool()` tools and the AVP serve path (`avp-common`, validator server entrypoint referenced in `llama-agent` dev-dep comment `start_mcp_server_with_options`).

## Done when
- Validators receive read-only file tools via the composed validator profile, not a boolean exemption.
- The locked-down AVP subset is unchanged in observable behavior (same tools available to validators as before).
- No regression in validator integration tests.