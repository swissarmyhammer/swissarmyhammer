---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffc380
title: 'Attachment right-click menu: surface Open, Delete, Cut, Copy, Paste as metadata-driven commands'
---
## What

The right-click context menu on attachment items has lost most of its useful commands. Today an `AttachmentItem` (`kanban-app/ui/src/components/fields/displays/attachment-display.tsx:209`) wraps itself in `Inspectable` + `FocusScope` with the moniker `attachment:<path>`, and the existing `useContextMenu` hook (`kanban-app/ui/src/lib/context-menu.ts`) walks the scope chain on right-click. The backend already registers `attachment.open` and `attachment.reveal` (`swissarmyhammer-kanban/src/commands/mod.rs:95-99`) with `context_menu: true` and there are tests in `swissarmyhammer-kanban/src/scope_commands.rs:2991-3038` proving they appear, but the menu is otherwise empty — Delete, Cut, Copy, and Paste don't show up for attachments even though the cross-cutting `entity.copy`/`entity.cut`/`entity.paste`/`entity.delete` commands already exist (`swissarmyhammer-kanban/src/commands/mod.rs:59,63,67,79`).

The fix is to make the cross-cutting clipboard/delete commands resolve for the `attachment` scope, with attachment-appropriate semantics:

- **Open** (`attachment.open`) — already wired; verify it stays first in the menu (and is the default double-click action).
- **Show in Finder / Reveal** (`attachment.reveal`) — already wired; keep.
- **Delete** — `entity.delete` should resolve for an `attachment:<path>` moniker by removing that one entry from the parent task's `attachments` field (the existing `swissarmyhammer-kanban/src/attachment/delete.rs` already implements the entity layer).
- **Cut** — `entity.cut` should put the attachment on the clipboard *and* remove it from the parent task on successful subsequent paste (mirrors the cut-task semantics in `swissarmyhammer-kanban/src/commands/clipboard_commands.rs`).
- **Copy** — `entity.copy` should put the attachment on the clipboard without mutating the source.
- **Paste** — `entity.paste` should land an attachment from the clipboard onto the focused task or attachment field. The paste handler already exists at `swissarmyhammer-kanban/src/commands/paste_handlers/attachment_onto_task.rs`; this task just has to make sure it dispatches when the focused scope is an attachment field or an attachment item.

### Approach

1. **Backend — make `entity.delete`/`entity.cut`/`entity.copy`/`entity.paste` resolve for the `attachment:` moniker.** Look at how each cross-cutting command decides which scopes it applies to (likely a `applicable_to`/`scope_matches` check in `swissarmyhammer-kanban/src/scope_commands.rs` around `emit_cross_cutting_commands` near line 722). Add the `attachment` scope to whichever predicate currently includes `task` so the four commands light up. For each, route the dispatch to attachment-specific entity-layer code:
   - delete → `swissarmyhammer-kanban/src/attachment/delete.rs`
   - cut → reuse `clipboard_commands` cut path with attachment-shaped payload (file path + parent task id + field name)
   - copy → reuse `clipboard_commands` copy path with the same payload
   - paste → `swissarmyhammer-kanban/src/commands/paste_handlers/attachment_onto_task.rs` (already wired for task drops; verify it accepts an `attachment` source moniker)

2. **Tests in `scope_commands.rs`** — extend the existing `attachment.*` block (around line 2991) so it also asserts the four cross-cutting ids resolve for an `attachment:<path>` scope chain and appear in `context_menu: true` resolution.

3. **UI side — no code change should be required.** `AttachmentItem` already establishes the scope via `FocusScope` + `Inspectable`, and `useContextMenu` already passes the scope chain to the backend. Add a UI test in `kanban-app/ui/src/components/fields/displays/attachment-display.test.tsx` (companion to the existing right-click test on line 228) that verifies the menu items list includes Open, Show in Finder, Delete, Cut, Copy, Paste in the expected order.

4. **Keybindings (out of scope for this task — file follow-up if desired):** the existing CUA bindings (Cmd+X/C/V/Backspace) already map to the cross-cutting entity ids; once the scope resolves, they'll work automatically. No keymap edit needed here.

### Out of scope

- Visual redesign of the attachment row itself.
- Per-OS native menu integration beyond what `show_context_menu` already does.
- Pasting attachments onto entities other than tasks/attachment fields.

## Acceptance Criteria

- [x] Right-clicking an `AttachmentItem` shows a context menu with **Open**, **Show in Finder**, **Delete**, **Cut**, **Copy**, **Paste** (in that group order; the existing group separator between attachment-group and cross-cutting group is preserved).
- [x] Selecting **Delete** removes that one attachment from the parent task's `attachments` field; the file is moved to `.attachments/.trash/` per the existing delete semantics.
- [x] Selecting **Cut** places the attachment on the clipboard and (on subsequent successful **Paste**) removes it from the source task.
- [x] Selecting **Copy** places the attachment on the clipboard without mutating the source.
- [x] Selecting **Paste** when the focus is on a task or attachment field lands the attachment from the clipboard via `attachment_onto_task`.
- [x] `list_commands_for_scope` with an `attachment:<path>` scope chain and `context_menu: true` returns Open, Reveal, plus the four cross-cutting commands resolved with `available: true`.
- [x] No regression to existing `attachment.open`/`attachment.reveal` tests in `swissarmyhammer-kanban/src/scope_commands.rs:2991-3038`.

## Tests

- [x] **Rust unit/integration test** in `swissarmyhammer-kanban/src/scope_commands.rs`: extend the existing attachment-context-menu test block to assert `entity.delete`, `entity.cut`, `entity.copy`, `entity.paste` are present in the resolved list for an `attachment:<path>` scope chain.
- [x] **Rust integration test** for delete: build a task with one attachment, dispatch `entity.delete` against the attachment scope, assert the task no longer references it and the file moved to `.attachments/.trash/`. Mirror the test pattern in `swissarmyhammer-kanban/src/attachment/delete.rs`'s existing tests.
- [x] **Rust integration test** for cut → paste round-trip: cut from task A, paste onto task B; assert the attachment moved (not duplicated). Use the test harness in `swissarmyhammer-kanban/src/commands/paste_handlers/test_support.rs`.
- [x] **Rust integration test** for copy → paste: copy from task A, paste onto task B; assert the attachment is on both.
- [x] **UI test** in `kanban-app/ui/src/components/fields/displays/attachment-display.test.tsx`: companion to the existing test at line 228 — assert the items array sent to `show_context_menu` includes the four new commands in addition to Open + Reveal, with the right group separator.
- [x] `cargo nextest run --workspace` — clean.
- [x] `pnpm -C kanban-app/ui test attachment-display` — clean.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` — clean.

## Workflow

- Use `/tdd` — start with the Rust scope-commands test asserting the four cross-cutting ids resolve for an `attachment:<path>` scope; watch it fail; then make it pass by extending the cross-cutting applicability predicate. Layer the delete/cut/copy/paste integration tests next, each one driving its own dispatch path.