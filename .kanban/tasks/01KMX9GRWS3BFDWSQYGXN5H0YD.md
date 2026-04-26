---
assignees:
- claude-code
depends_on:
- 01KMX9G72EYC2SSXMJYF840SE5
position_column: done
position_ordinal: ffffffffffffffffda80
title: Make clipboard commands polymorphic (tag + task) and update YAML
---
## What

Make entity.copy/cut/paste commands dispatch based on what's in scope (tag vs task) and what's on the clipboard. Update entity YAML for tag and task.

### Files to modify
- `swissarmyhammer-kanban/src/commands/clipboard_commands.rs` — rewrite CopyTaskCmd/CutTaskCmd/PasteTaskCmd to be polymorphic:
  - Copy: if tag in scope → CopyTag; if task in scope → CopyTask
  - Cut: if tag in scope (+ task for untag context) → CutTag; if only task → CutTask
  - Paste: read clipboard entity_type; if \"tag\" + task in scope → PasteTag; if \"task\" + column/board → PasteTask
  - Availability: copy/cut need task OR tag; paste needs has_clipboard AND (task OR column OR board)
- `swissarmyhammer-kanban/builtin/fields/entities/tag.yaml` — add entity.copy, entity.cut commands
- `swissarmyhammer-kanban/builtin/fields/entities/task.yaml` — add entity.paste command

## Acceptance Criteria
- [ ] Copy tag when tag focused, copy task when task focused (no tag)
- [ ] Cut tag untags from source task; cut task deletes task
- [ ] Paste tag onto focused task; paste task into focused column
- [ ] Paste availability: task in scope + tag on clipboard, OR column/board + task on clipboard
- [ ] Tag entity YAML has entity.copy and entity.cut
- [ ] Task entity YAML has entity.paste
- [ ] `cargo nextest run -p swissarmyhammer-kanban` passes

## Tests
- [ ] Integration tests: copy tag → paste onto different task
- [ ] Integration tests: cut tag → paste onto different task
- [ ] Integration tests: copy task still works when no tag focused"
<parameter name="assignees">[]