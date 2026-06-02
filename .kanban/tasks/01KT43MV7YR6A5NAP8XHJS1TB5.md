---
assignees:
- claude-code
depends_on:
- 01KT43M7TXNKP4C4R69X6F05WZ
position_column: todo
position_ordinal: a780
project: command-service
title: Add scopeId / targetId moniker helpers to the command SDK
---
Moniker parsing over the command `CommandContext` is duplicated across command plugins; it belongs in the command SDK alongside `registerCommands`.

## Problem
Resolving a `"<entity_type>:<id>"` moniker out of `ctx.scope_chain` (leaf-last) or `ctx.target` is reimplemented per plugin:
- `scopeId(ctx, entityType)` in task-commands (`index.ts:128`) and kanban-misc-commands (`index.ts:94`).
- `targetId(ctx, entityType)` in task-commands (`index.ts:148`).

These encode the `from: scope_chain` / `from: target` param resolution that every command callback needs.

## Fix
Add `scopeId(ctx, entityType)` and `targetId(ctx, entityType)` to the command SDK (`commands.ts`), depends on `CommandContext` being exported there (see the CommandContext/Availability card). Export from `@swissarmyhammer/plugin`; delete the local copies in task-commands and kanban-misc-commands.

## Acceptance
- `scopeId` / `targetId` exported from the SDK with unit coverage (leaf-last scan, type-prefix match, target type mismatch → undefined).
- Local copies removed from both plugins.
- Plugin test suite green.

Depends on: [Export CommandContext + Availability card].