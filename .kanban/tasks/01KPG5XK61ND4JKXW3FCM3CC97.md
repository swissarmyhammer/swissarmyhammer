---
assignees:
- claude-code
depends_on:
- 01KPEMYJV7BMTJB6GZ8MGTD04J
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffe780
title: 'Commands: generalize copy/cut to work on any entity type (CopyEntityCmd / CutEntityCmd)'
---
## What

`CopyTaskCmd` and `CutTaskCmd` in `swissarmyhammer-kanban/src/commands/clipboard_commands.rs` are task-only in Rust even though their YAML declarations name them `entity.copy` / `entity.cut`. Generalize both to operate on any entity type by reading the target moniker, serializing the entity's fields via `EntityContext`, and putting a structured `(type, id, fields)` payload on the clipboard. After this card, "Copy Tag", "Copy Project", "Cut Column" all work the same way.

### Design

A clipboard entry becomes `{ entity_type: String, entity_id: String, fields: serde_json::Map<String, Value> }`. Cut additionally carries a `delete_on_paste: bool` flag (or cut is implemented as "copy + mark for deletion" — pick the cleaner shape during implementation).

`CopyEntityCmd::available` returns true when `ctx.target` parses to a moniker of any known entity type. `execute` reads the entity via `ectx.get(type, id)`, serializes the fields, writes the clipboard via `UIState::set_clipboard(ClipboardPayload { entity_type, entity_id, fields, is_cut })`. `CutEntityCmd` is the same plus the cut flag.

`UIState::clipboard_entity_type()` (already exists for availability gating of paste) stays working — just becomes `clipboard_payload().map(|p| p.entity_type)`.

### Files to touch

- `swissarmyhammer-commands/src/lib.rs` (or wherever `UIState` lives) — extend clipboard storage from "just the type" to the full `ClipboardPayload`. Preserve `clipboard_entity_type()` as a convenience accessor.
- `swissarmyhammer-kanban/src/commands/clipboard_commands.rs` — rename `CopyTaskCmd` → `CopyEntityCmd`, `CutTaskCmd` → `CutEntityCmd`. Rewrite `available()` / `execute()` to read from `ctx.target` moniker, dispatch via entity type.
- `swissarmyhammer-kanban/src/commands/mod.rs` — registration key is already `entity.copy` / `entity.cut`; just point at the renamed structs.
- `swissarmyhammer-commands/builtin/commands/entity.yaml` — `entity.copy` / `entity.cut` param changes from `{name: task, from: scope_chain, entity_type: task}` to `{name: moniker, from: target}`. No scope pin.

### Subtasks

- [ ] Extend `UIState` clipboard to store structured `ClipboardPayload { entity_type, entity_id, fields, is_cut }`.
- [ ] Rename + generalize `CopyTaskCmd` → `CopyEntityCmd`; reads target moniker, serializes via `ectx.get`.
- [ ] Rename + generalize `CutTaskCmd` → `CutEntityCmd`; same + is_cut flag.
- [ ] Update `entity.yaml` params for `entity.copy` / `entity.cut` to `from: target`.

## Acceptance Criteria

- [ ] `CopyEntityCmd::available` returns true for any known entity moniker (task, tag, project, column, actor, board).
- [ ] After copying a tag, `UIState::clipboard_payload()` contains the tag's type, id, and fields.
- [ ] `entity.copy` / `entity.cut` in `entity.yaml` declare `from: target` with no scope pin.
- [ ] All existing `copy_*` / `cut_*` tests pass with the generalized impls.

## Tests

- [ ] Add `copy_entity_works_on_tag` — create a tag, dispatch `entity.copy` with target `tag:01X`, assert clipboard contains the tag's fields.
- [ ] Add `copy_entity_works_on_project`, `copy_entity_works_on_column` — same pattern.
- [ ] Add `cut_entity_sets_is_cut_flag` — cut marks the payload for deletion on paste.
- [ ] Existing `copy_available_with_task_in_scope` test updated to use target-driven availability.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban clipboard` — all green.

## Workflow

- Use `/tdd` — write `copy_entity_works_on_tag` first; it fails until the generalization lands.

#commands

Depends on: 01KPEMYJV7BMTJB6GZ8MGTD04J (mechanism must exist; target-driven emission relies on the from:target signal which this card adds to copy/cut)