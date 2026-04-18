---
assignees:
- claude-code
depends_on:
- 01KPG6G4SGXW1FN92YDXFNEAQ2
- 01KPG6GD34NMPQE1DZD0MHWE0N
- 01KPG6GN9JQSCZKFER5ZJ5JC62
- 01KPG6GYSNGTEJ42XA2QNB3VE0
- 01KPG6H74Z24N48DQR75CT7HP7
- 01KPG6HF1ZHWZ981PS3BEPP1HE
- 01KPG6HQYRRWCP52VH1KNKR35B
position_column: todo
position_ordinal: d880
title: 'Commands: migrate entity.delete/archive/unarchive/copy/cut/paste to auto-emit; purge entity schema opt-ins'
---
## What

With the target-driven emission pass landed (card 01KPEMYJV7BMTJB6GZ8MGTD04J), convert the remaining cross-cutting `entity.*` commands so their params declare `from: target`, audit their Rust `available()` impls, and delete every redundant opt-in entry from the entity schemas. The hygiene test from card 01KPEM811W5XE6WVHDQVRCZ4B0 turns GREEN here.

### Commands to convert

In `swissarmyhammer-commands/builtin/commands/entity.yaml`, confirm each of these declares `from: target` on the param that names the entity. `entity.delete` / `entity.archive` / `entity.unarchive` already do. `entity.copy` / `entity.cut` / `entity.paste` today use `from: scope_chain` with `entity_type: task` — that scopes them to task-in-scope, not target. Change them to `from: target` so the emission pass treats them as cross-cutting:

- `entity.delete` — already `from: target`. Verify.
- `entity.archive` — already `from: target`. Verify.
- `entity.unarchive` — already `from: target`. Verify.
- `entity.copy` — change to `from: target`.
- `entity.cut` — change to `from: target`.
- `entity.paste` — decide: paste's target is the *destination* (column, board, or task for tag-paste). Keep `from: target` but note that the Rust impl inspects the target type against the clipboard type.
- `entity.add` — stays NOT target-driven. Creation is dispatched via the dynamic `entity.add:{type}` rewriting done in `kanban-app/src/commands.rs`; it is not a per-entity context-menu command.
- `entity.update_field` — internal dispatch; stays `visible: false`, not target-driven.

### Rust opt-outs (thoughtful availability)

The scope chain carries the entity type, so `available()` can gate on it directly. Tighten each impl:

- `DeleteEntityCmd` in `swissarmyhammer-kanban/src/commands/entity_commands.rs` — today checks only `target.is_some()`. Make it parse the moniker and return false for unsupported types (attachment).
- `ArchiveEntityCmd` / `UnarchiveEntityCmd` — columns, attachments, and board do not archive; gate by supported types.
- `CopyTaskCmd` / `CutTaskCmd` / `PasteTaskCmd` in `swissarmyhammer-kanban/src/commands/clipboard_commands.rs` — audit. If only task (and maybe tag) are supported, `available()` returns false for other target types. Rename the structs to `CopyEntityCmd` etc. only if scope actually expands; otherwise leave names and let `available()` do the filtering.

### Entity schemas to purge

After this card every cross-cutting `entity.*` entry is gone from these files:

- `swissarmyhammer-kanban/builtin/entities/task.yaml` — keep `task.move`, `task.delete`, `task.untag`, `task.doThisNext`, `attachment.delete`. Delete `entity.copy`, `entity.cut`, `entity.paste`, `entity.archive`, `entity.unarchive`.
- `swissarmyhammer-kanban/builtin/entities/tag.yaml` — keep `tag.update`. Delete `entity.archive`, `entity.copy`, `entity.cut`.
- `swissarmyhammer-kanban/builtin/entities/project.yaml` — delete `entity.archive`. (`project.delete` retires in card 01KPEME1897275TKE61EKN6EVX.)
- `swissarmyhammer-kanban/builtin/entities/column.yaml` — `entity.paste` becomes auto-emit. If column-specific paste UX needs extra behavior, handle it in Rust `available()`/`execute()`, not YAML opt-in.
- `swissarmyhammer-kanban/builtin/entities/board.yaml` — same question as column for `entity.paste`.
- `swissarmyhammer-kanban/builtin/entities/actor.yaml` — `commands:` goes empty; delete the key.
- `swissarmyhammer-kanban/builtin/entities/attachment.yaml` — already empty.

### Files to touch

- `swissarmyhammer-commands/builtin/commands/entity.yaml`
- `swissarmyhammer-kanban/builtin/entities/task.yaml`, `tag.yaml`, `project.yaml`, `column.yaml`, `board.yaml`, `actor.yaml`
- `swissarmyhammer-kanban/src/commands/entity_commands.rs` — tighten `DeleteEntityCmd`, `ArchiveEntityCmd`, `UnarchiveEntityCmd` availability.
- `swissarmyhammer-kanban/src/commands/clipboard_commands.rs` — tighten `Copy/Cut/PasteTaskCmd` availability.

### Subtasks

- [ ] Convert `entity.copy` / `entity.cut` / `entity.paste` params to `from: target`.
- [ ] Tighten `available()` on delete/archive/unarchive/copy/cut/paste — use moniker type to gate.
- [ ] Delete cross-cutting entity.* entries from entity schema YAMLs.
- [ ] Run hygiene test — GREEN.

## Acceptance Criteria

- [ ] `yaml_hygiene_no_cross_cutting_in_entity_schemas` (card 01KPEM811W5XE6WVHDQVRCZ4B0) passes.
- [ ] Right-click on a tag shows **Delete Tag** and it works (was broken — surfaces via auto-emit).
- [ ] Right-click on a task still shows Copy / Cut / Paste / Archive / Unarchive / Delete / Move / Do This Next / Remove Tag with correct keybindings.
- [ ] Right-click on an attachment does NOT show entity.delete or entity.archive (Rust `available()` rejects the moniker type).
- [ ] `grep -E 'id: entity\\.(delete|archive|unarchive|copy|cut|paste)' swissarmyhammer-kanban/builtin/entities/` returns zero matches.

## Tests

- [ ] Add `entity_delete_auto_emits_for_tag_and_project` in `scope_commands.rs` tests — mirrors `ui_inspect_auto_emits_on_every_entity_type`.
- [ ] Add `entity_archive_not_available_on_attachment` — `available()` opt-out filters it out.
- [ ] Existing `delete_entity_deletes_tag`, `archive_entity_archives_task`, etc. still pass.
- [ ] Update `register_commands_returns_expected_count` if totals shift.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban` — all green.

## Workflow

- Use `/tdd` — drive with the hygiene test and `entity_delete_auto_emits_for_tag_and_project`; when both pass, the migration is complete.

#commands

Depends on: 01KPEMYJV7BMTJB6GZ8MGTD04J (emission pass must exist)