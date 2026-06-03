---
assignees:
- claude-code
position_column: todo
position_ordinal: af80
project: command-service
title: Dedupe perspective-commands scopeId onto the SDK helper
---
Leftover from 01KT43MV7Y (which scoped scopeId/targetId dedup to task-commands + kanban-misc-commands). A THIRD copy of `scopeId` lives in `builtin/plugins/perspective-commands/commands/context.ts:70`, consumed by its sibling command files via `./context.ts`.

Now that `scopeId`/`targetId` are exported from `@swissarmyhammer/plugin` (commands SDK), perspective-commands should use the SDK helper too.

## Work
- Replace `perspective-commands/commands/context.ts`'s local `scopeId` (and `targetId` if present) with the SDK import; update its sibling command files that import from `./context.ts`.
- If `context.ts` only existed to host that helper (+ maybe a local `CommandContext`), collapse it / re-point imports to `@swissarmyhammer/plugin`.

## Acceptance
- No local `scopeId`/`targetId`/`CommandContext`/`Availability` copies remain in perspective-commands (all from the SDK).
- `builtin_perspective_commands_e2e` + `full_baseline_e2e` green; `tsc --noEmit` clean.