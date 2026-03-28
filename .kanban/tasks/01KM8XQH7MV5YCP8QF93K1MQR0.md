---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffea80
title: Route set_focus through dispatch_command
---
## What

`set_focus` is a Tauri command that writes the focus scope chain to UIState directly. Route it through the command layer instead.

### Changes
- Add `ui.setFocus` to YAML command definitions (`ui.yaml`), `undoable: false`
- Add `SetFocusCmd` impl in `swissarmyhammer-kanban/src/commands/ui_commands.rs` — calls `ui.set_scope_chain()`
- Frontend: replace `invoke(\"set_focus\", { scope_chain })` with `invoke(\"dispatch_command\", { cmd: \"ui.setFocus\", args: { scope_chain } })`
- Remove `set_focus` Tauri command from commands.rs and invoke_handler

### Tests (command layer — no GUI)
- [ ] `cargo nextest run -p swissarmyhammer-kanban` — test SetFocusCmd sets scope chain on UIState
- [ ] `cargo nextest run -p kanban-app` passes
- [ ] `pnpm --filter kanban-app test` passes