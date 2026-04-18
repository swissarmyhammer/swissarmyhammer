---
assignees:
- claude-code
depends_on:
- 01KPG5YB7GTQ6Q3CEQAMXPJ58F
position_column: todo
position_ordinal: f080
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

- [ ] Audit `CommandError` variants; extend if needed (e.g., `SourceEntityMissing`, `DestinationInvalid`).
- [ ] Each paste handler returns specific errors for its fail modes.
- [ ] Frontend renders a toast with the error message when dispatch returns `Err`.
- [ ] Test the happy path silent + error path visible for at least one handler end-to-end.

## Acceptance Criteria

- [ ] Every paste handler returns a `CommandError` with a user-readable message for each of its fail modes.
- [ ] A dispatched paste that errors shows a toast in the app.
- [ ] The toast message names the specific failure (not a generic "paste failed").
- [ ] A dispatched paste that succeeds shows no toast.

## Tests

- [ ] `paste_task_into_nonexistent_column_returns_destination_invalid_error` (Rust).
- [ ] `paste_tag_onto_deleted_task_returns_source_entity_missing_error` (Rust).
- [ ] `paste_error_renders_toast` in `kanban-app/ui/src/lib/dispatch.test.ts` (frontend) — mock a failing dispatch, assert the toast component appears with the error message.
- [ ] Run commands: `cargo nextest run -p swissarmyhammer-kanban paste_handlers`, `bun test kanban-app/ui/src/lib/dispatch.test.ts` — all green.

## Workflow

- Use `/tdd` — write the frontend toast test first; it should fail until the dispatch error handler wires to the toast component.

#commands

Depends on: 01KPG5YB7GTQ6Q3CEQAMXPJ58F (paste mechanism)