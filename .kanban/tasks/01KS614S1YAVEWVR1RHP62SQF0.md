---
assignees:
- claude-code
depends_on:
- 01KS5EAD57PCBFJGMVB74FF4MK
position_column: review
position_ordinal: '80'
project: entity-service
title: '`entity` server: clipboard (cut/copy/paste via PasteMatrix)'
---
## What

Add the clipboard operations to the `entity` MCP server (the CRUD core is the prerequisite task `01KS5EAD57…`): `Cut`, `Copy`, `Paste`. These wrap `PasteMatrix`, which lives in a DIFFERENT crate — `crates/swissarmyhammer-kanban/.../paste_handlers/mod.rs:119` (`PasteMatrix`) — not in `swissarmyhammer-entity`. So either the `entity` server hosts in a crate that can depend on the paste logic, or the paste logic is exposed to it; decide the dependency direction explicitly (entity → kanban is an awkward/new direction — prefer relocating the paste matrix to a shared crate or hosting the clipboard ops where `PasteMatrix` already is, while keeping the `entity` server identity).

Preserve the drag-vs-paste distinction (see the `drag-vs-paste` rule): external drag-in is paste (creates via `PasteMatrix`); internal drag is a property mutation (handled elsewhere). This task is the paste path only.

Files:
- `<entity server crate>/src/operations.rs` — add `#[operation]` structs `Cut`, `Copy`, `Paste` (wrapping `PasteMatrix`)
- wiring so the server can reach `PasteMatrix` without duplicating it

Writes go through the kernel/`StoreContext`, so paste is undoable and emits entity events for free.

## Acceptance Criteria
- [ ] `Cut`/`Copy`/`Paste` reachable on the `entity` server
- [ ] `Paste` uses the existing `PasteMatrix` (no duplicate paste logic); drag-vs-paste distinction preserved
- [ ] Paste is undoable via `store.undo` and emits entity events
- [ ] The crate dependency direction for reaching `PasteMatrix` is chosen and documented (no entity↔kanban cycle)
- [ ] `_meta` tree includes the clipboard ops

## Tests
- [ ] `<entity server crate>/tests/integration/entity_clipboard_e2e.rs` — copy an entity → paste → assert a duplicate is created on disk; cut → paste → assert move semantics; undo a paste → assert reverted
- [ ] `_meta` snapshot
- [ ] `cargo test` for the host crate passes

## Workflow
- Use `/tdd`

Depends on the `entity` CRUD core (`01KS5EAD57…`). Prerequisite for the `entity.cut/copy/paste` commands in the entity-commands plugin.