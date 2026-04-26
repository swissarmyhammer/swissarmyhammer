---
assignees:
- claude-code
depends_on:
- 01KPG5YB7GTQ6Q3CEQAMXPJ58F
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffec80
title: 'Commands: paste handler — attachment onto task'
---
## What

Implement `AttachmentOntoTaskHandler` — attaches a file (by path from the clipboard) to a target task. Handler at `swissarmyhammer-kanban/src/commands/paste_handlers/attachment_onto_task.rs`, matches `("attachment", "task")`.

### Action

1. Parse task id from target moniker; read attachment path from `clipboard.entity_id` (attachment monikers use the path as the id — confirm via `attachment.yaml`).
2. Invoke `AddAttachment` op (in `swissarmyhammer-kanban/src/attachment/`) with `(task_id, path, name, mime_type, size)`. Name/mime/size come from the clipboard `fields` payload populated during copy.
3. Ignore `is_cut` — attachments aren't "moved" by paste; cutting an attachment pastes a new association while the original stays in place on the source task. If the product requires "move" semantics later, extend then.
4. Return the new attachment entity id.

### Files

- CREATE `swissarmyhammer-kanban/src/commands/paste_handlers/attachment_onto_task.rs`.
- MODIFY `swissarmyhammer-kanban/src/commands/paste_handlers/mod.rs` — added `pub mod attachment_onto_task;`. Registration line `m.register(AttachmentOntoTaskHandler);` deferred — orchestrator will batch-register (parallel-safety: 2 sibling agents are concurrently creating other paste handlers).

### Subtasks

- [x] Audit `AddAttachment` op signature; confirm what fields are needed.
- [x] Implement `AttachmentOntoTaskHandler`.
- [x] Register — `pub mod` declaration only; `m.register(AttachmentOntoTaskHandler);` deferred to orchestrator (parallel-safety override).
- [x] Colocate tests.

## Acceptance Criteria

- [x] Handler matches `("attachment", "task")`.
- [x] Pasting an attachment onto a task adds it to the task's attachments.
- [x] Source attachment remains on the original task regardless of `is_cut` (product decision; revisit if users ask for move semantics).
- [x] Duplicate attachments on the same task are allowed (the attachment entity id differs even if the path matches).

## Tests

- [x] `paste_attachment_onto_task_adds_attachment` — target task starts with zero attachments; after paste, has one with the expected path.
- [x] `paste_attachment_preserves_original` — source task still has the attachment after paste.
- [x] `paste_attachment_ignores_cut_flag` — cut attachment still exists on source.
- [x] Run command: `cargo nextest run -p swissarmyhammer-kanban paste_handlers::attachment_onto_task` — all green (10 tests passed).

## Workflow

- Use `/tdd`.

#commands

Depends on: 01KPG5YB7GTQ6Q3CEQAMXPJ58F (mechanism)