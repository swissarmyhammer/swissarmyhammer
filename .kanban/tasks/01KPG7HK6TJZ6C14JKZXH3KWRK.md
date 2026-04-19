---
assignees:
- claude-code
depends_on:
- 01KPG5YB7GTQ6Q3CEQAMXPJ58F
position_column: done
position_ordinal: fffffffffffffffffffffff680
title: 'Commands: error propagation from paste handlers to UI toast'
---
## What

If a paste handler's `execute()` returns `Err`, the error must reach the user as a visible toast / error state, not vanish silently. Today `CommandError` propagates back through the dispatch return value, but there's no card asserting the frontend *renders* it or the backend *returns* it with enough context to render meaningfully.

### Fail modes to verify

- Target entity doesn't exist (tag referenced in paste handler was deleted since copy).
- Operation fails at the DB / entity-context level (e.g., duplicate id, invalid field).
- Handler-specific validation fails (paste task with nonexistent column, paste attachment with unreadable file).

### Files to touch

- `swissarmyhammer-commands/src/lib.rs` (or wherever `CommandError` lives) — audit the error variants; ensure they carry enough context for a user-readable message.
- `swissarmyhammer-kanban/src/commands/paste_handlers/*.rs` — emit `CommandError::ExecutionFailed` with specific messages ("Column `<id>` no longer exists", etc.).
- `kanban-app/ui/src/lib/error-handler.ts` (or wherever dispatch errors surface) — confirm toast display; add tests if missing.

### Subtasks

- [x] Audit `CommandError` variants; extend if needed (e.g., `SourceEntityMissing`, `DestinationInvalid`).
- [x] Each paste handler returns specific errors for its fail modes.
- [x] Frontend renders a toast with the error message when dispatch returns `Err`.
- [x] Test the happy path silent + error path visible for at least one handler end-to-end.

## Acceptance Criteria

- [x] Every paste handler returns a `CommandError` with a user-readable message for each of its fail modes.
- [x] A dispatched paste that errors shows a toast in the app.
- [x] The toast message names the specific failure (not a generic "paste failed").
- [x] A dispatched paste that succeeds shows no toast.

## Tests

- [x] `paste_task_into_nonexistent_column_returns_destination_invalid_error` (Rust).
- [x] `paste_tag_onto_deleted_task_returns_source_entity_missing_error` (Rust).
- [x] `paste_error_renders_toast` in `kanban-app/ui/src/lib/dispatch-error.test.ts` (frontend) — the centralised `reportDispatchError` helper, exercised through six cases that mock failing dispatches and assert the toast is called with the specific backend message.
- [x] Run commands: `cargo nextest run -p swissarmyhammer-kanban paste_handlers` (64 passing) and `npx vitest run src/lib/dispatch-error.test.ts` (6 passing).

## Workflow

- Use `/tdd` — write the frontend toast test first; it should fail until the dispatch error handler wires to the toast component.

## Implementation Notes

- New `CommandError` variants: `SourceEntityMissing(String)` and `DestinationInvalid(String)`. Existing `ExecutionFailed` stays for genuinely unstructured failures from downstream operation processors.
- Each paste handler now validates its destination/source up-front and emits the structured variant with a user-readable message ("Column 'doing' no longer exists", "Task '<id>' no longer exists", etc.) before delegating to the underlying operation.
- Variant choice convention: `DestinationInvalid` for container-style destinations (column, board, project) where the new entity would be placed; `SourceEntityMissing` for referenced subjects (target task in tag/actor/attachment-onto-task), missing source files, and missing clipboard source entities.
- Frontend: added `kanban-app/ui/src/lib/dispatch-error.ts` with a single `reportDispatchError(cmdId, err)` helper. Wired it into `app-shell.tsx`'s keybinding `executeCommand` and the `context-menu-command` listener — the two generic dispatch entry points that previously let backend rejections vanish into unhandled promise rejections. Sites with their own contextual error UI (e.g. `useAddTaskHandler`) keep their existing per-call `.catch` since those messages are tailored to the action.
- The `reportDispatchError` helper strips the Tauri "Command failed: " framing so the toast reads cleanly and prefixes with `<cmdId> failed: ` so the user can correlate the toast to the action they took.

#commands

Depends on: 01KPG5YB7GTQ6Q3CEQAMXPJ58F (paste mechanism)