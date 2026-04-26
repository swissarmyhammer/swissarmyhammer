---
assignees:
- claude-code
depends_on:
- 01KNKZYSYT8W352AP4KZFYVH1G
position_column: done
position_ordinal: ffffffffffffffffffffffffffffcc80
title: Add `perspective.goto` command for switching to a perspective by ID
---
## What

Add a Rust-side `perspective.goto` command that switches to a perspective by its ID. The frontend displays perspective names for user selection, then dispatches this command with the chosen ID. No index-based jumping — IDs only.

### Current state

- `ui.perspective.set` (`swissarmyhammer-kanban/src/commands/ui_commands.rs`) sets a perspective by ID via `UIState::set_active_perspective` — but this is a UI-layer command, not a domain command.
- `perspective.next`/`perspective.prev` cycle within a view kind using `KanbanContext` + `UIState`.
- The active perspective and active view are both stored in Rust `UIState`, making them fully testable without the frontend.

### Approach

**Rust side** — add `GotoPerspectiveCmd` to `swissarmyhammer-kanban/src/commands/perspective_commands.rs`:
- Required arg: `id` (perspective ID string)
- Optional arg: `view_kind` (string) — if provided, validates the perspective belongs to that view kind before switching
- Logic: look up perspective by ID via `perspective_context()`. If not found, return error. Otherwise call `UIState::set_active_perspective` with the ID. Return the `UIStateChange`.
- Always available.

**YAML** — add entry to `swissarmyhammer-commands/builtin/commands/perspective.yaml`:
- `perspective.goto` with `id` and optional `view_kind` params. No default keybindings — the frontend provides selection UI.

**Registration** — add to `swissarmyhammer-kanban/src/commands/mod.rs`.

### Files to modify

1. `swissarmyhammer-kanban/src/commands/perspective_commands.rs` — Add `GotoPerspectiveCmd`
2. `swissarmyhammer-kanban/src/commands/mod.rs` — Register `perspective.goto`, update count assertion
3. `swissarmyhammer-commands/builtin/commands/perspective.yaml` — Add YAML entry

## Acceptance Criteria
- [ ] `perspective.goto` with valid `id` switches the active perspective and returns `UIStateChange`
- [ ] `perspective.goto` with nonexistent `id` returns an error
- [ ] `perspective.goto` with `view_kind` that doesn't match the perspective's view returns an error
- [ ] Command is registered in the command registry and YAML definitions
- [ ] Active perspective and view are fully testable from Rust without frontend

## Tests
- [ ] Unit test: goto with valid ID sets active perspective in UIState
- [ ] Unit test: goto with invalid ID returns error
- [ ] Unit test: goto with mismatched view_kind returns error
- [ ] Unit test: goto without view_kind succeeds regardless of perspective's view
- [ ] `cargo test -p swissarmyhammer-kanban perspective_commands` — passes
- [ ] `cargo nextest run -p swissarmyhammer-commands` — YAML parses correctly

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.