---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffff9580
title: 'entity-commands.ts: hardcoded "entity.inspect" command ID for client-side handling'
---
**File:** `kanban-app/ui/src/lib/entity-commands.ts:77,126`\n\n```ts\nif (cmd.id === \"entity.inspect\") {\n  inspect?.(entityMoniker);\n}\n```\n\nHardcodes that `entity.inspect` is the one command handled client-side instead of dispatched to Rust. Also in `focus-scope.tsx:119` which hardcodes resolving `entity.inspect` for double-click. Commands should declare their execution mode (client vs backend) in the schema rather than being special-cased by ID. #field-special-case