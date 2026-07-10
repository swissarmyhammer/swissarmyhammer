---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffe080
project: command-service
title: Export CommandContext + Availability types from the command SDK (commands.ts)
---
The command-service callback contract is only half-exported by the SDK, forcing every command plugin to hand-redefine the other half.

## Problem
`crates/swissarmyhammer-plugin/src/sdk/commands.ts` exports `CommandRegistration` (the inbound `register command` payload type) but NOT the callback's context/return types. So each plugin redefines them locally, slightly differently:
- `interface CommandContext` is redefined in 5 plugins: task-commands, ui-commands, file-commands, entity-commands, kanban-misc-commands.
- `type Availability` (the `available` return shape) is redefined per plugin; plugins use `{ ok: true } | { ok: false; reason: string }` while command-service.md contracts `boolean | { ok: false; reason: string }`.

This violates the "eliminate duplication" principle and lets the contract drift per plugin.

## Fix
Export `CommandContext` (mirroring `swissarmyhammer_command_service::CommandContext`: `scope_chain?`, `target?`, `args?`) and `Availability` from `commands.ts` (the file that already owns `CommandRegistration`/`registerCommands`). Have all builtin plugins import them and delete their local copies.

## Acceptance
- `CommandContext` and `Availability` exported from `@swissarmyhammer/plugin`.
- All 5 plugins that defined `CommandContext` locally now import it; same for `Availability`.
- Return shape reconciled with the service contract (accept bare `true` or normalize).
- Plugin + frontend tests green.

Related: [shared moniker/result helpers card], [host default-class card].