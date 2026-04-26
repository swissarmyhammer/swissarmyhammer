---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffca80
title: Add `perspective.next` and `perspective.prev` commands for cycling perspectives within a view
---
## What

Add two new Rust-side commands — `perspective.next` and `perspective.prev` — that cycle through perspectives matching the current view kind. The commands read the perspective list, filter to the active view's kind, find the current active perspective, and switch to the next/previous one (wrapping around).

### Files to modify

1. **`swissarmyhammer-kanban/src/commands/perspective_commands.rs`** — Add `NextPerspectiveCmd` and `PrevPerspectiveCmd`. Both need access to `KanbanContext` (to list perspectives) and `UIState` (to read/write active perspective and active view). The cycling logic:
   - List all perspectives via `perspective_context().await`
   - Filter to those matching the current view kind (derive view kind from `active_view_id` — strip the trailing ID suffix to get the kind, or pass as an arg)
   - Find the index of the current `active_perspective_id`
   - Move to `(index ± 1) % len`, wrapping around
   - Call `ui.set_active_perspective()` with the new ID
   - These commands are always available (like `ui.perspective.set`)

2. **`swissarmyhammer-kanban/src/commands/mod.rs`** — Register `perspective.next` → `NextPerspectiveCmd` and `perspective.prev` → `PrevPerspectiveCmd` in the command registry. Update the command count assertion.

3. **`swissarmyhammer-commands/builtin/commands/perspective.yaml`** — Add YAML entries for `perspective.next` and `perspective.prev` with keyboard shortcuts (e.g., `Mod+]` / `Mod+[` for CUA, `gt` / `gT` for vim).

### Design notes

- The commands need both `KanbanContext` (for perspective list) and `UIState` (for active perspective/view). Follow the pattern in `ui_commands.rs::SetActivePerspectiveCmd` for UIState access and `perspective_commands.rs::SetFilterCmd` for KanbanContext access.
- The `view_kind` arg should be passed from the frontend (the UI knows its view kind). This avoids having to parse view IDs on the Rust side. Add `view_kind` as a required arg.
- If only 0 or 1 perspectives match the view kind, the command is a no-op returning `null`.

## Acceptance Criteria

- [ ] `perspective.next` cycles to the next perspective within the same view kind, wrapping from last to first
- [ ] `perspective.prev` cycles to the previous perspective within the same view kind, wrapping from first to last
- [ ] Both commands accept `view_kind` arg to filter perspectives by view type
- [ ] Both commands update UIState via `set_active_perspective` and return the `UIStateChange`
- [ ] No-op (returns `null`) when fewer than 2 perspectives match the view kind
- [ ] Commands are registered in the command registry and YAML command definitions

## Tests

- [ ] `swissarmyhammer-kanban/src/commands/perspective_commands.rs` — Unit tests: next cycles forward with wrapping, prev cycles backward with wrapping, no-op with 0 or 1 perspectives
- [ ] Run `cargo test -p swissarmyhammer-kanban perspective_commands` — passes

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.