---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8c80
project: spatial-nav
title: 'Spatial nav: rip out broadcastNavCommand side-channel, route nav.* through command→Rust dispatch, and fix focus_first_in_layer over-narrow guard'
---
## What

Nav keys (h/j/k/l and their cua/emacs siblings) fire but do NOT produce a focus change in the running app. Click-to-focus works. Enter-to-inspect works. Only spatial navigation is dead. Evidence from the live macOS unified log:

- `[FocusScope] focus → task:<id>` logs when the user clicks a card (setFocus path works)
- `command cmd=nav.up target=None` and `command cmd=nav.down target=None` log when keys pressed (dispatch works)
- **Zero subsequent `[FocusScope] focus → ...` log** after those nav.up/nav.down commands (the round-trip never completes)

Full 1397-test npm suite is green. That is itself a finding: the test harness exercises the broken side-channel, not the real command pipeline, so it can't catch this class of regression.

### Root architectural bug — the "second mechanism"

Nav commands should follow the exact path `ui.inspect` does:

```
keypress → createKeyHandler → dispatch → Rust backend handler → SpatialState mutation → focus-changed event → JS listener updates store → FocusScope pulls new value via useFocusedMoniker()
```

Instead, `nav.up/down/left/right/first/last` run a **parallel in-JS mechanism**:

| Site | Code | Problem |
|---|---|---|
| `kanban-app/ui/src/components/app-shell.tsx:202-251` | `NAV_COMMAND_SPEC` + `buildNavCommands` attach `execute: () => broadcastRef.current(spec.id)` to each nav command | Frontend `execute:` handler short-circuits backend dispatch |
| `kanban-app/ui/src/lib/entity-focus-context.tsx:315-368` | `NAV_DIRECTION_MAP` + `useBroadcastNav` + `broadcastNavCommand` | Second command-id-to-direction mapping that bypasses the dispatch→Rust pipeline |
| `kanban-app/ui/src/components/inspector-focus-bridge.tsx:30-79` | Inspector commands also call `broadcastRef.current("nav.up")` etc. | Yet another consumer of the side-channel |
| `kanban-app/ui/src/components/board-view.tsx:988-1007` | Board view takes `broadcastNavCommand` as a prop | Another consumer |
| `kanban-app/ui/src/components/grid-view.tsx:180-192` | Grid `grid.moveUp` etc. delegate via `navCmd(..., broadcastRef, ...)` | Another consumer |
| `kanban-app/ui/src/test/spatial-fixture-shell.tsx:12-160` | Test harness reimplements the side-channel — "so 'j' → nav.down is identical between fixture and real app" | The harness validates the broken mechanism, not the real dispatch path → all tests pass while nav is dead |

The user's directive: **commands → Rust → navigation state in Rust. Delete the side-channel.**

### Second bug stacked on top — `focus_first_in_layer` is too strict

`swissarmyhammer-spatial-nav/src/spatial_state.rs:551-559` (in the current uncommitted diff) added this guard:

```rust
// Only act on the active (topmost) layer. A call that names a
// non-active layer is an out-of-order RAF from a lower layer...
match inner.layer_stack.active() {
    Some(active) if active.key == layer_key => {}
    _ => return None,
}
```

Problem: the guard also bails on `Some(active)` where `active.key != layer_key` AND on `None`. If the active layer is something the caller didn't expect, this is a silent no-op. Combined with the RAF-deferred call path, this can mean **no layer ever auto-focuses its first entry on mount**, so the app loads with null focus — which the nav commands (even if they worked) would then have to recover via fallback-to-first in `navigate()`.

User's observation: "did you bother to make sure we always have a layer" — yes, the guard assumes "active layer" matches the caller, but doesn't ensure a layer is ever active in the first place. Need to verify: after app boot with the current uncommitted diff, is `layer_stack.active()` ever `Some`? Or is the window-layer push racing against scope registrations?

### What must be fixed

1. **Register `nav.up`, `nav.down`, `nav.left`, `nav.right`, `nav.first`, `nav.last` as backend commands** in `swissarmyhammer-commands/builtin/commands/` (new file `nav.yaml` or extend `ui.yaml`). Each binds keys and declares no client-side execute.

2. **Add Rust handlers** in `swissarmyhammer-kanban/src/commands/` (or the appropriate crate) — each handler:
   - Reads the current focused key from `AppState.spatial_state_for(window.label())`
   - Calls `SpatialState::navigate(focused_key.as_deref(), direction)`
   - Rust's existing `emit_focus_changed` + event path handles the rest

3. **Delete** from `kanban-app/ui/src/lib/entity-focus-context.tsx`:
   - `NAV_DIRECTION_MAP`
   - `useBroadcastNav`
   - `broadcastNavCommand` field from `EntityFocusContextValue`
   - Everything that touches `monikerToKeysRef` only for the nav path (keep it for `syncSpatialFocus` which uses the same map)

4. **Delete** from `kanban-app/ui/src/components/app-shell.tsx`:
   - `NAV_COMMAND_SPEC`
   - `buildNavCommands` (or strip `execute:` from each entry so nav commands fall through to backend dispatch — confirm Rust has registered them first)
   - `broadcastRef` wiring around nav
   - If `buildDynamicGlobalCommands` becomes trivial, collapse it into `STATIC_GLOBAL_COMMANDS`

5. **Fix** `kanban-app/ui/src/components/inspector-focus-bridge.tsx` — inspector's `inspector.moveUp/Down/First/Last` commands should dispatch to the new `nav.*` backend commands (or to new `inspector.nav.*` commands if inspector-specific semantics are needed). Either way: no `broadcastRef`.

6. **Fix** `kanban-app/ui/src/components/board-view.tsx` and `grid-view.tsx` — any remaining `broadcastNavCommand` consumer deletes its prop, calls `dispatch(...)` on the `nav.*` id directly via `useDispatchCommand`.

7. **Fix** `kanban-app/ui/src/test/spatial-fixture-shell.tsx` — rip out the NAV_COMMANDS that wrap `broadcastNavCommand`. The fixture must now exercise the SAME dispatch→Rust path as the real app.

8. **Investigate and fix** `focus_first_in_layer_noop_when_not_active_layer` in `spatial_state.rs:551-559`:
   - Add a test: window layer pushed at boot, no other layer pushed, call `focus_first_in_layer(window_key)` → must NOT bail
   - Determine whether `layer_stack.active()` returns `Some(window)` at that moment (if not, the bug is in layer stack initialization or ordering)
   - Either fix layer-stack init to guarantee the window layer is active immediately after push, OR relax the guard to handle the "no active layer yet" case safely

### The regression-blocking test

One vitest-browser test, in `kanban-app/ui/src/test/spatial-nav-real-dispatch.test.tsx`, that:

- Mounts a REAL `AppShell` (not the fixture shell's reimplementation of globals)
- Uses a mock Rust shim that implements `SpatialState` via the `spatial-shim.ts` JS port, but **only receives Tauri invoke calls** — no JS-side `broadcastNavCommand` stand-in
- Registers 3 scopes, clicks the first to focus it
- Presses `j` (vim) / `ArrowDown` (cua) / `Ctrl+n` (emacs)
- Asserts: `invoke("nav.down")` (the backend dispatch) was called; the shim processed it; a `focus-changed` event fired; the focus store's getSnapshot now returns the second moniker; the second scope's DOM element has `data-focused="true"`
- **No mock for `broadcastNavCommand` — it must not exist in the production graph**

If this test passes with `broadcastNavCommand` deleted, the architecture is clean. If it fails, we haven't actually removed the side-channel.

### Files to modify

- `swissarmyhammer-commands/builtin/commands/nav.yaml` (new) or extend `ui.yaml`
- `swissarmyhammer-kanban/src/commands/nav_commands.rs` (new) or extend `ui_commands.rs`
- `swissarmyhammer-kanban/src/commands/mod.rs` — register handlers
- `swissarmyhammer-spatial-nav/src/spatial_state.rs` — fix `focus_first_in_layer` guard
- `kanban-app/ui/src/lib/entity-focus-context.tsx` — delete broadcastNavCommand family
- `kanban-app/ui/src/components/app-shell.tsx` — delete NAV_COMMAND_SPEC + buildNavCommands
- `kanban-app/ui/src/components/inspector-focus-bridge.tsx` — switch to dispatch
- `kanban-app/ui/src/components/board-view.tsx` — switch to dispatch, drop prop
- `kanban-app/ui/src/components/grid-view.tsx` — switch to dispatch
- `kanban-app/ui/src/test/spatial-fixture-shell.tsx` — remove NAV_COMMANDS reimplementation
- `kanban-app/ui/src/test/spatial-nav-real-dispatch.test.tsx` (new) — the regression fence

### What needs to happen to the falsely-closed tasks

The following tasks were closed but their implementation shipped either partially or broken. Each must be reopened with a link back to this task as the corrective work:

- `01KPS1WCQRY8DEWQVA47PZ82ZC` (beam-test pool split) — shipped but obscured by broader nav failure; the `in_beam_dominates` rule itself looks sound but cannot be verified until nav works end-to-end
- `01KPS22R2T4Q5QT9A71E7ZWAAP` (inspector from-grid) — the inspector-focus-bridge still uses broadcastRef, so this task's fix is built on the broken foundation
- `01KPS27H6WE4RPV5V2D42Y5X6F` (toolbar wrap) — added NavBar FocusScopes but Up from them still goes through broadcastNavCommand
- `01KPTFSDB4FKNDJ1X3DBP7ZGNZ` (multi-inspector audit) — added new tests, but they use the fixture shell's reimplementation of the side-channel
- `01KPTFX400WX3Q8DAQXGGC604E` (push-to-pull visuals) — the visual decoration refactor is sound; the broken nav is unrelated to it, but the ride-along `registerClaim → registerSpatialKey` partial refactor may still have gaps per `01KPTHCVBS5E7CAH4JXBAR3EWP`
- `01KPTHCVBS5E7CAH4JXBAR3EWP` (finish registerSpatialKey rename) — verify no callsite still uses the old names after this task lands
- `01KPTJMZCD758KFXRHJN7ZA52H` (6 multi-inspector tests) — tests pass on the fixture's side-channel but may not reflect real-app behavior; re-validate after this task

Those tasks should be moved back to todo with a note pointing at this one, or marked superseded. The user decides.

## Acceptance Criteria

- [ ] `broadcastNavCommand`, `useBroadcastNav`, `NAV_DIRECTION_MAP` no longer exist anywhere in the codebase — grep returns zero hits
- [ ] `nav.up/down/left/right/first/last` are backend commands with Rust handlers that call `SpatialState::navigate`
- [ ] The `NAV_COMMAND_SPEC` entries (if kept for keybindings) have no `execute:` handler; the dispatch path goes straight to Rust
- [ ] `spatial-fixture-shell.tsx` does NOT reimplement nav commands — it relies on the same dispatch path the real app uses
- [ ] Pressing `j`, `ArrowDown`, or `Ctrl+n` in the running app produces a visible focus change (live verification via the macOS log showing `focus-changed` payload AND the next `[FocusScope] focus → ...` entry)
- [ ] `focus_first_in_layer` guard works correctly at boot: window layer pushed, its first entry gets focus; no silent null-focus start
- [ ] Existing `ui.inspect`, Enter-to-activate, click-to-focus, and Escape-to-dismiss still work
- [ ] The new `spatial-nav-real-dispatch.test.tsx` passes AND fails when `broadcastNavCommand` is reintroduced (proving it actually protects against the regression)

## Tests

- [ ] `kanban-app/ui/src/test/spatial-nav-real-dispatch.test.tsx` (new) — regression-blocking, covers real dispatch→Rust path for all 4 cardinal nav keys across all 3 keymaps
- [ ] Rust unit test in `swissarmyhammer-spatial-nav/src/spatial_state.rs` — `focus_first_in_layer` on a single-layer stack with the named layer IS the active layer does NOT bail
- [ ] Rust integration test in `swissarmyhammer-kanban/tests/` — dispatch `nav.down` via the command registry, assert SpatialState mutation + `focus-changed` event
- [ ] `cd kanban-app/ui && npm test` — all tests green (the suite now exercises real dispatch, not the side-channel)
- [ ] `cargo test -p swissarmyhammer-spatial-nav -p swissarmyhammer-kanban` — green
- [ ] Manual: pressing h/j/k/l in the running app visibly moves focus in every view; log shows `focus-changed` events; no orphan focus bars

## Workflow

- Use `/tdd`. Write `spatial-nav-real-dispatch.test.tsx` FIRST with mocks that only intercept `invoke()` — no `broadcastNavCommand` stand-in. The test fails because the command has an `execute:` that short-circuits invoke.
- Then ship Rust handlers, delete frontend side-channel in one pass. Re-run the new test until green.
- Then re-run the full suite. If existing tests break, they relied on the side-channel — update them to use real dispatch.
- Do NOT keep `broadcastNavCommand` "just in case." Delete it completely.

