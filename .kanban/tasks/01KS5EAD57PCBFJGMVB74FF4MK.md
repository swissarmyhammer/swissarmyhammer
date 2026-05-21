---
assignees:
- claude-code
depends_on:
- 01KS5F5ZNA0621X8KM2NPERXNV
position_column: todo
position_ordinal: '9880'
project: command-backends
title: 'Extend `kanban` MCP tool: clipboard (cut/copy/paste) + archive/unarchive'
---
## What

Add the operations the entity-commands plugin needs that the existing `kanban` MCP tool does not yet expose: clipboard (cut/copy/paste via `PasteMatrix`) and `archive`/`unarchive`. The `kanban` tool already covers add/update/delete/get for task/column/tag/project/actor/attachment, move/complete/tag/untag/assign task — verified in `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs`. These are the gaps.

Files:
- `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` — add operations:
  - `archive task`, `unarchive task` (entity.archive/unarchive map to these; today archive lives in `EntityContext` but is not exposed as a kanban op — confirm and wire it)
  - `cut`, `copy`, `paste` (clipboard) — wraps the `PasteMatrix` (`Arc<PasteMatrix>`, today in `swissarmyhammer-kanban`, reached via `ctx.kanban.paste_matrix()`). These operate on whatever entity is targeted, mirroring today's `clipboard_commands.rs`.
- Update the kanban tool's operation set + `description.md` so the new ops appear in `tools/list` and `_meta`.

Design note (from the user): clipboard cut/copy/paste and archive are **entity-cross-cutting** — they work on any entity type, same as undo/redo. They belong on the `kanban` tool (which already owns generic entity CRUD) rather than a separate `clipboard` server, keeping the consolidated-server design. Paste must preserve the drag-vs-paste distinction (memory: drag-vs-paste) — external paste creates via PasteMatrix; it is not the internal-drag property mutation.

Archive/unarchive and paste are undoable — they write through the unified changelog so `app.undo` reverts them.

## Acceptance Criteria
- [ ] `kanban` tool exposes `archive task`, `unarchive task`, `cut`, `copy`, `paste`
- [ ] `tools/list` + `_meta` reflect the new operations
- [ ] `cut`/`copy`/`paste` round-trip against the real `PasteMatrix`; paste creates the duplicate entity in the store
- [ ] `archive task`/`unarchive task` move the task in/out of the archive as today's `EntityContext` does
- [ ] New ops are captured by the unified changelog (undoable)

## Tests
- [ ] `crates/swissarmyhammer-tools/tests/integration/kanban_clipboard_archive_e2e.rs` — real kanban store; copy a task → paste → assert duplicate; cut → paste → assert moved; archive → assert in archive; unarchive → assert restored
- [ ] Undo integration: paste a task; `app.undo`; assert the paste reverted
- [ ] `cargo test -p swissarmyhammer-tools` passes

## Workflow
- Use `/tdd`

Prerequisite for: entity-commands plugin. Depends on the operation-struct foundation.