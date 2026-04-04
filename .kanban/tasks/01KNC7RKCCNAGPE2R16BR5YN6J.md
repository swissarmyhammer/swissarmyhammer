---
assignees:
- claude-code
position_column: todo
position_ordinal: '8780'
position_swimlane: container-refactor
title: Add Rust tests for UI commands (view/perspective switching)
---
## What

Add unit tests for the UI command implementations in Rust that currently have zero test coverage. These commands are the backbone of the container architecture — view switching, perspective switching, inspector management — and must be testable independent of the frontend.

**Files to modify:**
- `swissarmyhammer-kanban/src/commands/ui_commands.rs` — add `#[cfg(test)] mod tests` with tests for all 7 commands

**Commands to test:**
1. `InspectCmd` — pushes entity onto inspector stack
2. `InspectorCloseCmd` — pops top of inspector stack
3. `InspectorCloseAllCmd` — clears inspector stack
4. `PaletteOpenCmd` / `PaletteCloseCmd` — toggle palette state
5. `SetActivePerspectiveCmd` — sets active_perspective_id in UIState
6. `SetActiveViewCmd` — sets active_view_id in UIState
7. `SetFocusCmd` — sets scope_chain in UIState

**Test pattern:** Follow existing test patterns in `commands/mod.rs` (lines 241+) and `commands/perspective_commands.rs` (line 435+). Create a `CommandContext` with mock UIState, execute the command, assert UIState was updated correctly.

**Key assertions:**
- `SetActiveViewCmd` with `view_id: "grid"` → `UIState.windows["main"].active_view_id == "grid"`
- `SetActivePerspectiveCmd` with `perspective_id: "p1"` → `UIState.windows["main"].active_perspective_id == "p1"`
- `InspectCmd` with `target: "task:123"` → `inspector_stack` contains `"task:123"`
- `InspectorCloseCmd` → stack pops
- `InspectorCloseAllCmd` → stack empty

## Acceptance Criteria
- [ ] All 7 UI commands have at least one test each
- [ ] Tests verify UIState mutations, not just that execute() returns Ok
- [ ] Tests pass: `cargo test -p swissarmyhammer-kanban -- ui_commands`

## Tests
- [ ] `cargo test -p swissarmyhammer-kanban -- ui_commands::tests` — all pass
- [ ] `cargo test -p swissarmyhammer-kanban` — full suite still passes #container-refactor