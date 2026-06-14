---
assignees:
- claude-code
position_column: todo
position_ordinal: e780
project: ui-command-cleanup
title: Audit app-side "main" window fallbacks outside the ui_state op path (menu.rs open_and_notify, dormant BoardSwitch/BoardClose handlers, legacy kanban commands module)
---
## What
Follow-up from the per-window hardening card `01KTECWA8D05FVKJ80MA3H0FFY` (which removed the silent `unwrap_or("main")` from the `ui_state` per-window mutation ops). The fallback inventory found three RESIDUAL `"main"` defaults outside that path:

1. **LIVE**: `apps/kanban-app/src/menu.rs` `open_and_notify` — `source_window_label.unwrap_or("main")` when `focused_window_label(app)` returns `None` (File > Open Board with no focused window). It then builds a synthetic scope chain `["window:main"]`. Since windows are created dynamically (`main.rs`: no static "main" window), this can target a nonexistent window. Should resolve a real window (e.g. any open window) or surface an error, not assume "main".
2. **DORMANT**: `apps/kanban-app/src/commands.rs` `handle_board_switch_result` / `handle_board_close_result` — read `window_label` from a `BoardSwitch` / `BoardClose` result key with `.unwrap_or("main")`. The live window-service `switch board` / `close board` ops return `{ok, path}` WITHOUT those keys (the shapes come from the retired legacy Rust `file_commands`), so these handlers appear to never fire on the live TS-plugin path. Verify dead, then delete the handlers (and decide where their side effects — `set_window_board`, forwarder rebind, title update — actually live now, since the shell `switch_board` callback only calls `state.open_board`).
3. **DEAD**: `crates/swissarmyhammer-kanban/src/commands/{ui,app,file,drag,perspective}_commands.rs` — ~20 `ctx.window_label_from_scope().unwrap_or("main")` sites in the legacy Rust `Command` impls retired by the Stage 4 cutover (`state.command_impls` deleted). Only utility fns (e.g. `evaluate_perspective_filter`) are still referenced. Delete the dead command impls or at least their silent fallbacks so they cannot be resurrected with the bug intact.

## Acceptance Criteria
- [ ] `open_and_notify` no longer assumes a "main" window exists when no window is focused.
- [ ] The dormant `BoardSwitch`/`BoardClose` handlers are confirmed dead and removed (or confirmed live and hardened), with their side-effect responsibilities accounted for.
- [ ] No remaining `unwrap_or("main")` window resolution in live code paths. #tech-debt