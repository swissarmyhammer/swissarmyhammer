---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffff8280
title: 'Fix view switching: palette dispatch fails, left nav icons bypass command system'
---
## What

View switching commands appear in the command palette but selecting them does nothing. Three distinct bugs:

### Bug 1: Palette `view.switch:*` commands fail on dispatch
The palette fetches commands from `list_commands_for_scope`, which generates dynamic `view.switch:{view_id}` commands (e.g. `view.switch:board-view`). When the user selects one, the palette calls `invoke("dispatch_command", { cmd: "view.switch:board-view" })` at `command-palette.tsx:228`. But `dispatch_command_internal` at `kanban-app/src/commands.rs:909` does `registry.get(cmd)` which returns `None` — there's no registry entry or `Command` impl for `view.switch:*`. The command silently fails.

**Fix**: In `dispatch_command_internal`, intercept commands matching `view.switch:*` pattern — extract the view ID suffix, and delegate to the `ui.view.set` command impl with `{ view_id }` args. Same pattern for `board.switch:*` → `file.switchBoard`.

### Bug 2: Palette dispatch missing `windowLabel`
The palette's `executeSelectedCommand` at `command-palette.tsx:228` does not pass `windowLabel` to `dispatch_command`. Even if Bug 1 is fixed, `ui.view.set` needs `window_label` to target the correct window (see `ui_commands.rs:190`). Without it, the view change won't be scoped to the invoking window.

**Fix**: Pass `windowLabel: getCurrentWindow().label` in the palette's `dispatch_command` invoke at `command-palette.tsx:228`.

### Bug 3: Left nav icons use local-only commands, not the command system
`LeftNav` at `left-nav.tsx:42` calls `executeCommand("nav.view.{id}")` which resolves through `ViewCommandScope` (App.tsx:716). This scope defines local `execute` handlers that call `invoke("dispatch_command", { cmd: "ui.view.set", args: { view_id }, windowLabel: WINDOW_LABEL })`. This actually *works* because `ui.view.set` has a real impl — but the `nav.view.*` IDs are hardcoded frontend-only commands that don't exist in the backend command system. They should dispatch `view.switch:{id}` through the same path as the palette, once Bug 1 is fixed.

**Fix**: After fixing Bug 1, update `ViewCommandScope` to generate commands that dispatch `view.switch:{id}` through the standard `dispatchCommand` path (no local `execute` handler), or update `LeftNav` to call `dispatchCommand` directly with `view.switch:{id}`.

### Files to modify
- `kanban-app/src/commands.rs` — intercept `view.switch:*` and `board.switch:*` in `dispatch_command_internal`, delegate to real impls
- `kanban-app/ui/src/components/command-palette.tsx` — pass `windowLabel` in `executeSelectedCommand`
- `kanban-app/ui/src/App.tsx` — update `ViewCommandScope` to use `view.switch:*` IDs or remove local execute handlers
- `kanban-app/ui/src/components/left-nav.tsx` — align with updated command IDs

## Acceptance Criteria
- [x] Selecting "Switch to Task Grid" in the palette actually switches the view
- [x] Selecting "Switch to Board View" in the palette switches back to board view
- [x] View switch from palette targets the invoking window (not a random one)
- [x] Left nav icon clicks switch views (continues working)
- [x] In multi-window scenario, view switch in window A doesn't affect window B
- [x] `cargo nextest run -p swissarmyhammer-kanban` passes
- [x] `cargo nextest run -p kanban-app` passes (if applicable)

## Tests
- [x] Integration test in `kanban-app/src/commands.rs`: dispatch `view.switch:tasks-grid` → verify `ui.view.set` is called with correct `view_id`
- [x] Integration test: dispatch `view.switch:tasks-grid` with `window_label` → verify the UIState change is scoped to that window
- [x] Frontend test: palette `executeSelectedCommand` includes `windowLabel` in dispatch invoke
- [x] `cargo nextest run` and verify no regressions