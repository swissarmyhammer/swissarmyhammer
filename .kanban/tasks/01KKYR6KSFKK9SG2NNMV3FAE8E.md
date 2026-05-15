---
assignees:
- assistant
depends_on:
- 01KKYR690S88SENAN2HVB6X5BJ
position_column: done
position_ordinal: ffffffffffa680
title: Add shutdown-aware window close handler
---
## What
When a user closes a secondary window via X button mid-session, remove its `window_boards` entry so it doesn't resurrect on restart. When the app quits, keep all entries so windows restore on next launch. Use an `AtomicBool` flag to distinguish close-one-window from app-shutdown.

### Files
- `kanban-app/src/state.rs` — Add `shutting_down: AtomicBool` to `AppState`, init false in `new()`
- `kanban-app/src/main.rs` — Change `.run(context).expect(...)` to `.build(context).expect(...).run(callback)` with `RunEvent::ExitRequested` setting shutting_down; add `Destroyed` handler in `on_window_event` that removes `window_boards` entry if not shutting_down

### Subtasks
- [ ] Add `shutting_down: std::sync::atomic::AtomicBool` field to `AppState`
- [ ] Initialize to `false` in `AppState::new()`
- [ ] Convert `Builder.run()` to `Builder.build().run(callback)` pattern
- [ ] In run callback, set `shutting_down = true` on `RunEvent::ExitRequested`
- [ ] Add `WindowEvent::Destroyed` handler for secondary windows
- [ ] In Destroyed handler: skip if shutting_down, else remove from window_boards + save config
- [ ] `cargo nextest run` passes

## Acceptance Criteria
- [ ] Closing a secondary window mid-session removes its `window_boards` entry
- [ ] App quit preserves all `window_boards` entries for next-launch restore
- [ ] `shutting_down` flag prevents cleanup during app shutdown
- [ ] Main and quick-capture windows are excluded from Destroyed handler

## Tests
- [ ] `cargo nextest run` — full suite green
- [ ] Manual: open tear-off, close it via X, quit, restart — window should NOT reappear
- [ ] Manual: open tear-off, quit app, restart — window SHOULD reappear at same position