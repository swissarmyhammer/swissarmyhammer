---
assignees:
- claude-code
position_column: todo
position_ordinal: fd80
project: builtin-commands
title: 'Root board context menu: split subject vs paste-target capability metadata; re-caption Paste by clipboard contents'
---
## Problem

The main/root context menu (right-click the board background) resolves to `focused_entity_type = "board"` — `apps/kanban-app/ui/src/components/board-container.tsx:90` mounts `CommandScopeProvider moniker="board:{boardId}"`, and `board` is in `OPERABLE_ENTITY_TYPES`. So the root board shows **Cut/Copy/Paste/Delete/Archive/Unarchive Board**. At the root these make no sense — except the cause is more subtle than "remove board":

- **Cut / Copy / Delete / Archive / Unarchive** act on the focused entity as the **subject**. The board-as-subject is meaningless at the root (you don't copy/delete/archive the root board from itself). → must be removed for `board`.
- **Paste** is the opposite direction: it drops the **clipboard contents INTO** the target. The board is a legitimate paste TARGET — `crates/swissarmyhammer-kanban/src/commands/paste_handlers/task_into_board.rs` and `column_into_board.rs` paste a task/column onto a board (e.g. an empty board background). So paste must STAY on `board`. But its caption `Paste {{entity.type}}` renders the meaningless **"Paste Board"** — paste is about the clipboard item, NOT the target entity.

The root issue: a single `OPERABLE_ENTITY_TYPES` (`builtin/plugins/entity-commands/index.ts`, mirrored by Rust `COPYABLE_ENTITY_TYPES` in `crates/swissarmyhammer-kanban/src/commands/clipboard_commands.rs`) is used as `applies_to` for all six commands, conflating "can be the SUBJECT of an op" with "can RECEIVE a paste." Fix this with metadata, not `if (type === "board")`.

## What

Split the capability metadata into two principled sets and re-caption paste — all data-driven, gated by the existing `applies_to` mechanism (`CommandService::applies_to_focus`), no per-command branching.

- `builtin/plugins/entity-commands/index.ts`:
  - Introduce **SUBJECT_OPERABLE_ENTITY_TYPES** = `OPERABLE_ENTITY_TYPES` minus `board` → `[task, tag, column, actor, project, attachment]`. Use it as `applies_to` for `entity.cut`, `entity.copy`, `entity.delete`, `entity.archive`, `entity.unarchive`.
  - Introduce **PASTE_TARGET_ENTITY_TYPES** = the entity types that have a registered paste handler as a TARGET → `[task, attachment, board, column, project]` (derived from `paste_handlers/`: `*_onto_task`, `attachment_onto_attachment`, `column_into_board`/`task_into_board`, `task_into_column`, `task_into_project`). Use it as `applies_to` for `entity.paste`. `board` stays here.
  - Re-caption `entity.paste` from `Paste {{entity.type}}` to **`Paste`** (clipboard-driven, not target-driven) so "Paste Board" disappears. If a `{{clipboard.type}}` caption token is readily resolvable from the UIState clipboard, `Paste {{clipboard.type}}` (e.g. "Paste Task") is preferred and falls back to plain "Paste" when the clipboard type is empty/unknown — but plain "Paste" is the required minimum.
- `crates/swissarmyhammer-kanban/src/commands/clipboard_commands.rs`: remove `board` from `COPYABLE_ENTITY_TYPES` (the cut/copy SOURCE / dispatch gate) so it matches the new SUBJECT set. Paste dispatch is unchanged (PasteMatrix already board-capable).
- Keep the drift guards honest: the existing guard pins TS subject set == Rust `COPYABLE_ENTITY_TYPES` (both now exclude `board`). Add a guard pinning TS `PASTE_TARGET_ENTITY_TYPES` to the Rust PasteMatrix target-type set, so the paste-target list can't silently drift from the registered handlers.

## Acceptance Criteria
- [ ] Right-click the root board: Cut / Copy / Delete / Archive / Unarchive do NOT appear; Paste DOES appear (when the clipboard holds a paste-compatible item) and is NOT captioned "Paste Board".
- [ ] Paste-into-board still works: pasting a task/column onto a board dispatches the existing `task_into_board` / `column_into_board` handlers.
- [ ] The five subject commands still surface on their real subjects (task/tag/column/actor/project/attachment) — no regression from the set rename.
- [ ] Paste still surfaces on its real targets (task/attachment/board/column/project) and not on non-target types (e.g. tag/actor).
- [ ] No `if (type === "board")` / per-command type branching — the change is entirely in the declared `applies_to` data + caption template.

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/list_applies_to.rs` (+ the production-path `tests/integration/builtin_entity_commands_e2e.rs` that loads the real plugin and reads surfaced `applies_to`): assert with a `board:` focus that cut/copy/delete/archive/unarchive are absent and paste is present; with a `task:`/`column:` focus the subject commands are present.
- [ ] Drift-guard tests (`tests/integration/support.rs` shared helper): SUBJECT set == Rust `COPYABLE_ENTITY_TYPES` (both exclude `board`); PASTE_TARGET set == Rust PasteMatrix target-type set. Both go RED if the lists diverge.
- [ ] A caption test asserting `entity.paste` no longer renders "Paste Board" for a `board:` target (renders "Paste" or "Paste {{clipboard.type}}").
- [ ] `cargo test -p swissarmyhammer-command-service` and `cargo test -p swissarmyhammer-kanban` pass; new assertions red before the change, green after.

## Workflow
- Use `/tdd` — first write the failing list_applies_to assertions (board shows only Paste, not the 5 subject ops) and the "no Paste Board caption" assertion, then split the sets + re-caption to make them pass. #commands #entity-commands #frontend