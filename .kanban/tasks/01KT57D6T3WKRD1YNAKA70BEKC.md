---
assignees:
- claude-code
depends_on:
- 01KT57C9AFYCHVVK6VKK4V7W8A
position_column: todo
position_ordinal: '8580'
project: agent-builtins
title: 'serve: clientInfo-driven Bash deny for Claude (move off sah init)'
---
Apply the Bash deny at serve time, gated on the actual connecting client being Claude — the honest replacement for the init-time, all-detected-agents deny.

## Change
- In the serve path, after MCP `initialize`, read clientInfo. If the client is **Claude** → `mirdan::install::deny_tool(scope, "Bash")` idempotently. If **llama** → no-op (no native Bash). Reuse the existing, Claude-aware mirdan primitive (`apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:151` is the current call site to mirror).
- This pairs with per-client served-set composition (shell is served to Claude here); together they make shell a true replacement for Bash rather than an addition.
- NOT an `Initializable`. Lives in serve, not init.

## Decide
- **Scope**: serve has a working dir — which `InitScope` does the serve-time deny target (Local vs Project)?
- **clientInfo → mirdan Claude AgentDef** mapping (share the mapping introduced by the served-set composition card).
- **Self-correct?** (open question): should serve re-allow Bash when a non-Claude client connects, or leave removal solely to `deinit`? Default: leave to deinit (paired card).

## Done when
- A Claude client connecting to `sah serve` results in Bash denied in Claude's settings (idempotent).
- A llama client triggers no deny.